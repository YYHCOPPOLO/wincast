package windows

import (
	"errors"
	"os"
	"strings"

	"golang.org/x/sys/windows/registry"
)

const runKeyPath = `Software\Microsoft\Windows\CurrentVersion\Run`
const approvedKeyPath = `Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`

// approvedEnabled is the 12-byte Task Manager startup flag that marks a Run
// entry enabled; without it Windows can keep a previously disabled entry off.
var approvedEnabled = []byte{2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}

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
			clearStartupApproved(valueName)
			return nil
		}
		return err
	}
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	command := `"` + executable + `" --hidden`
	if err := key.SetStringValue(valueName, command); err != nil {
		return err
	}
	markStartupApproved(valueName)
	return nil
}

// markStartupApproved makes the Run entry visible as enabled in Task Manager.
// A failure here does not undo the Run entry, matching Windows' own behavior.
func markStartupApproved(valueName string) {
	key, err := registry.OpenKey(registry.CURRENT_USER, approvedKeyPath, registry.SET_VALUE)
	if err != nil {
		return
	}
	defer key.Close()
	_ = key.SetBinaryValue(valueName, approvedEnabled)
}

func clearStartupApproved(valueName string) {
	key, err := registry.OpenKey(registry.CURRENT_USER, approvedKeyPath, registry.SET_VALUE)
	if err != nil {
		return
	}
	defer key.Close()
	_ = key.DeleteValue(valueName)
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
