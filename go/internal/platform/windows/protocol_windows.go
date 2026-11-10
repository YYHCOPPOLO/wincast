//go:build windows

package windows

import (
	"fmt"
	"os"
	"strings"

	"golang.org/x/sys/windows/registry"
)

// RegisterURLProtocols claims the given schemes for this executable under the
// current user. Extensions open `raycast://` URLs for OAuth redirects and to
// return focus after an external flow; claiming the schemes keeps those links
// inside Wincast instead of launching Raycast.
func RegisterURLProtocols(schemes ...string) error {
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	command := `"` + executable + `" "%1"`
	for _, scheme := range schemes {
		base := `Software\Classes\` + scheme
		key, _, err := registry.CreateKey(registry.CURRENT_USER, base, registry.WRITE)
		if err != nil {
			return fmt.Errorf("register %s URL scheme: %w", scheme, err)
		}
		_ = key.SetStringValue("", "URL:"+scheme+" protocol")
		_ = key.SetStringValue("URL Protocol", "")
		key.Close()
		commandKey, _, err := registry.CreateKey(registry.CURRENT_USER, base+`\shell\open\command`, registry.WRITE)
		if err != nil {
			return fmt.Errorf("register %s URL command: %w", scheme, err)
		}
		if err := commandKey.SetStringValue("", command); err != nil {
			commandKey.Close()
			return err
		}
		commandKey.Close()
	}
	return nil
}

// URLFromArgs returns the first argument that looks like an activation URL.
func URLFromArgs(args []string) string {
	for _, argument := range args {
		lower := strings.ToLower(argument)
		if strings.HasPrefix(lower, "raycast:") || strings.HasPrefix(lower, "wincast:") ||
			strings.HasPrefix(lower, "com.raycast:") {
			return argument
		}
	}
	return ""
}
