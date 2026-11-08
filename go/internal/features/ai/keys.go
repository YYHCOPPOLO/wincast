// Package ai owns the AI service side: DPAPI key files, HTTP streaming and
// the local chat history database. UI state stays in the controller.
package ai

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

// KeyStore keeps one DPAPI-sealed file per connection UUID. The UUID text is
// the entropy, exactly as the existing application wrote it.
type KeyStore struct {
	dir string
}

func OpenKeyStore(settingsDir string) *KeyStore {
	return &KeyStore{dir: filepath.Join(settingsDir, "ai-keys")}
}

func (k *KeyStore) path(id string) (string, error) {
	if !safeKeyID(id) {
		return "", errors.New("invalid connection identity")
	}
	return filepath.Join(k.dir, id+".dpapi"), nil
}

// safeKeyID keeps a saved connection's entropy text while refusing names that
// could escape the key directory.
func safeKeyID(id string) bool {
	if id == "" || len(id) > 128 || id == "." || id == ".." {
		return false
	}
	if strings.ContainsAny(id, `/\:*?"<>|`) {
		return false
	}
	for _, r := range id {
		if r < 0x20 {
			return false
		}
	}
	return true
}

func (k *KeyStore) Get(id string) (string, bool) {
	path, err := k.path(id)
	if err != nil {
		return "", false
	}
	sealed, err := os.ReadFile(path)
	if err != nil {
		return "", false
	}
	if len(sealed) == 0 || len(sealed) > 64<<10 {
		return "", false
	}
	plain, err := windows.Unprotect(sealed, []byte(id))
	if err != nil || !utf8.Valid(plain) {
		return "", false
	}
	key := string(plain)
	if strings.TrimSpace(key) == "" {
		return "", false
	}
	return key, true
}

func (k *KeyStore) Has(id string) bool {
	_, ok := k.Get(id)
	return ok
}

func (k *KeyStore) Set(id, key string) error {
	if strings.TrimSpace(key) == "" {
		k.Remove(id)
		return nil
	}
	path, err := k.path(id)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(k.dir, 0o700); err != nil {
		return err
	}
	sealed, err := windows.Protect([]byte(key), []byte(id))
	if err != nil {
		return errors.New("could not store API key")
	}
	if err := storage.WriteFileAtomic(path, sealed, 0o600); err != nil {
		return errors.New("could not store API key")
	}
	return nil
}

func (k *KeyStore) Remove(id string) {
	path, err := k.path(id)
	if err != nil {
		return
	}
	_ = os.Remove(path)
}

// ProviderFor validates a saved connection and reads its key at the last
// moment. Remote endpoints need a key; loopback endpoints do not.
func ProviderFor(conn domainai.Connection, model string, keys *KeyStore) (*Provider, error) {
	if !conn.Provider.Valid() {
		return nil, errors.New("Unsupported AI provider.")
	}
	parsed, err := domainai.ValidateURL(conn.BaseURL)
	if err != nil {
		return nil, err
	}
	key := ""
	if keys != nil {
		key, _ = keys.Get(conn.ID)
	}
	if key == "" && !domainai.URLIsLoopback(parsed.Raw) {
		return nil, domainai.ErrNoKey
	}
	return &Provider{
		Kind:     conn.Provider,
		Endpoint: domainai.CompletionEndpoint(conn.Provider, parsed.Raw),
		Model:    model,
		Key:      key,
	}, nil
}
