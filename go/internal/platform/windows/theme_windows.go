//go:build windows

package windows

import (
	"golang.org/x/sys/windows/registry"
)

// SystemDark reports the taskbar/apps appearance. A missing value is the
// Windows default: light.
func SystemDark() bool {
	key, err := registry.OpenKey(registry.CURRENT_USER,
		`Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`, registry.QUERY_VALUE)
	if err != nil {
		return false
	}
	defer key.Close()
	value, _, err := key.GetIntegerValue("AppsUseLightTheme")
	if err != nil {
		return false
	}
	return value == 0
}
