package windows

import (
	"errors"
	"os"
	"strings"

	"golang.org/x/sys/windows/registry"
)

const runKeyPath = `Software\Microsoft\Windows\CurrentVersion\Run`

// ApplyLaunchAtLogin creates or removes the per-user Run entry. The entry
// names the running executable with --hidden, so a login start stays in the
// tray instead of opening the palette.
func ApplyLaunchAtLogin(valueName string, enabled bool) error {
	key, err := registry.OpenKey(registry.CURRENT_USER, runKeyPath, registry.QUERY_VALUE|registry.SET_VALUE)
	if err != nil {
		return err
	}
	defer key.Close()
	if !enabled {
		err := key.DeleteValue(valueName)
		if err == nil || errors.Is(err, registry.ErrNotExist) {
			return nil
		}
		return err
	}
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	command := `"` + executable + `" --hidden`
	return key.SetStringValue(valueName, command)
}

// LaunchAtLoginCommand reports the current Run entry, or "" when absent.
func LaunchAtLoginCommand(valueName string) string {
	key, err := registry.OpenKey(registry.CURRENT_USER, runKeyPath, registry.QUERY_VALUE)
	if err != nil {
		return ""
	}
	defer key.Close()
	value, _, err := key.GetStringValue(valueName)
	if err != nil {
		return ""
	}
	return value
}

// LaunchAtLoginMatches reports whether the entry points at this executable.
func LaunchAtLoginMatches(valueName string) bool {
	command := LaunchAtLoginCommand(valueName)
	executable, err := os.Executable()
	if err != nil || command == "" {
		return false
	}
	return strings.Contains(strings.ToLower(command), strings.ToLower(executable))
}
