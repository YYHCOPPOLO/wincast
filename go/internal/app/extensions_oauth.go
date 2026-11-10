package app

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/extensions"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/egoist/mygo"
)

// handleExtensionURL routes a raycast:// or wincast:// activation. OAuth
// redirects answer the waiting authorize call; extension command URLs launch
// the named command; every other Raycast URL only brings the palette back.
func (c *controller) handleExtensionURL(raw string) {
	parsed, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || parsed.Scheme == "" {
		return
	}
	scheme := strings.ToLower(parsed.Scheme)
	if scheme != "raycast" && scheme != "com.raycast" && scheme != "wincast" {
		return
	}
	target := strings.Trim(parsed.Host+parsed.Path, "/")
	if target == "oauth" || strings.HasPrefix(target, "oauth/") {
		values := parsed.Query()
		result := map[string]string{}
		for _, key := range []string{"code", "state", "error", "error_description"} {
			if value := values.Get(key); value != "" {
				result[key] = value
			}
		}
		c.extensionOAuthMu.Lock()
		wait := c.extensionOAuthWait
		c.extensionOAuthMu.Unlock()
		if wait != nil {
			select {
			case wait <- result:
			default:
			}
		}
		return
	}
	if strings.HasPrefix(target, "extensions/") {
		parts := strings.Split(strings.TrimPrefix(target, "extensions/"), "/")
		if len(parts) >= 3 {
			extensionName := parts[len(parts)-2]
			commandName := parts[len(parts)-1]
			entryID := extensions.CommandRef{ExtensionName: extensionName, CommandName: commandName}.EntryID()
			c.startExtension(entryID)
			return
		}
	}
	c.show()
}

func (c *controller) extensionOAuth(method string, arguments []extension.Value) (any, error) {
	if c.extensions == nil {
		return nil, errors.New("Extensions are unavailable")
	}
	switch method {
	case "authorize":
		return c.authorizeExtensionOAuth(arguments)
	case "tokens.get":
		path, err := c.extensionOAuthTokenPath(extensionString(arguments, 0, "providerId"))
		if err != nil {
			return nil, err
		}
		data, err := os.ReadFile(path)
		if errors.Is(err, os.ErrNotExist) {
			return nil, nil
		}
		if err != nil {
			return nil, err
		}
		ciphertext, err := base64.StdEncoding.DecodeString(strings.TrimSpace(string(data)))
		if err != nil {
			return nil, err
		}
		plaintext, err := windows.Unprotect(ciphertext, c.oauthEntropy())
		if err != nil {
			return nil, err
		}
		return string(plaintext), nil
	case "tokens.set":
		path, err := c.extensionOAuthTokenPath(extensionString(arguments, 0, "providerId"))
		if err != nil {
			return nil, err
		}
		object := extensionObject(arguments, 0)
		if object == nil {
			return nil, errors.New("OAuth tokens are missing")
		}
		value, ok := object["tokens"]
		if !ok {
			return nil, errors.New("OAuth tokens are missing")
		}
		var payload []byte
		if text, ok := value.StringValue(); ok {
			payload = []byte(text)
		} else {
			data, marshalErr := json.Marshal(value)
			if marshalErr != nil {
				return nil, marshalErr
			}
			payload = data
		}
		ciphertext, err := windows.Protect(payload, c.oauthEntropy())
		if err != nil {
			return nil, err
		}
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			return nil, err
		}
		return nil, os.WriteFile(path, []byte(base64.StdEncoding.EncodeToString(ciphertext)), 0o600)
	case "tokens.remove":
		path, err := c.extensionOAuthTokenPath(extensionString(arguments, 0, "providerId"))
		if err != nil {
			return nil, err
		}
		if removeErr := os.Remove(path); removeErr != nil && !errors.Is(removeErr, os.ErrNotExist) {
			return nil, removeErr
		}
		return nil, nil
	}
	return nil, fmt.Errorf("Unknown OAuth call %s.", method)
}

func (c *controller) authorizeExtensionOAuth(arguments []extension.Value) (any, error) {
	authURL := extensionString(arguments, 0, "url")
	if authURL == "" {
		return nil, errors.New("OAuth authorize needs a URL")
	}
	state := extensionString(arguments, 0, "state")
	wait := make(chan map[string]string, 1)
	c.extensionOAuthMu.Lock()
	if c.extensionOAuthWait != nil {
		c.extensionOAuthMu.Unlock()
		return nil, errors.New("Another OAuth authorization is already waiting")
	}
	c.extensionOAuthWait = wait
	c.extensionOAuthState = state
	c.extensionOAuthMu.Unlock()
	defer func() {
		c.extensionOAuthMu.Lock()
		c.extensionOAuthWait = nil
		c.extensionOAuthState = ""
		c.extensionOAuthMu.Unlock()
	}()
	if err := mygo.Shell.OpenExternal(authURL); err != nil {
		return nil, err
	}
	select {
	case result := <-wait:
		if message := result["error_description"]; message != "" {
			return nil, errors.New(message)
		}
		if message := result["error"]; message != "" {
			return nil, errors.New(message)
		}
		code := result["code"]
		if code == "" {
			return nil, errors.New("The OAuth provider returned no authorization code")
		}
		payload := map[string]any{"authorizationCode": code}
		if state != "" {
			payload["state"] = state
		}
		if returned := result["state"]; returned != "" && returned != state {
			return nil, errors.New("The OAuth state did not match")
		}
		return payload, nil
	case <-time.After(5 * time.Minute):
		return nil, errors.New("OAuth authorization timed out")
	case <-c.ctx.Done():
		return nil, errors.New("OAuth authorization was cancelled")
	}
}

func (c *controller) extensionOAuthTokenPath(providerID string) (string, error) {
	if providerID == "" {
		return "", errors.New("OAuth provider id is missing")
	}
	if c.extensions == nil {
		return "", errors.New("Extensions are unavailable")
	}
	name := c.extensions.ActiveExtensionName()
	if name == "" {
		return "", errors.New("No extension command is running")
	}
	directory := filepath.Join(c.profile.settings, "extension-oauth", extensions.SafeName(name))
	return filepath.Join(directory, extensions.SafeName(providerID)+".bin"), nil
}

func (c *controller) oauthEntropy() []byte {
	name := ""
	if c.extensions != nil {
		name = c.extensions.ActiveExtensionName()
	}
	return []byte("wincast.extensions.oauth:" + name)
}
