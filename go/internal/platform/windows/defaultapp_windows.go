//go:build windows

package windows

import (
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/windows/registry"
)

// DefaultApplicationForPath resolves the executable Windows would use to open
// path: the user's file-extension choice first, then the class registration.
func DefaultApplicationForPath(path string) (name, executable string, ok bool) {
	cleaned := filepath.Clean(strings.TrimSpace(path))
	if cleaned == "" || cleaned == "." {
		return "", "", false
	}
	if strings.EqualFold(filepath.Ext(cleaned), ".exe") {
		return displayName(cleaned), cleaned, true
	}
	extension := strings.ToLower(filepath.Ext(cleaned))
	if extension == "" {
		if info, err := os.Stat(cleaned); err == nil && info.IsDir() {
			return "File Explorer", `C:\Windows\explorer.exe`, true
		}
		return "", "", false
	}
	progID := extensionProgID(extension)
	if progID == "" {
		return "", "", false
	}
	command := classOpenCommand(progID)
	executable = commandExecutable(command)
	if executable == "" {
		return "", "", false
	}
	return displayName(executable), executable, true
}

func extensionProgID(extension string) string {
	key, err := registry.OpenKey(registry.CURRENT_USER,
		`Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\`+extension+`\UserChoice`,
		registry.QUERY_VALUE)
	if err == nil {
		value, _, valueErr := key.GetStringValue("ProgId")
		_ = key.Close()
		if valueErr == nil && value != "" {
			return value
		}
	}
	key, err = registry.OpenKey(registry.CLASSES_ROOT, extension, registry.QUERY_VALUE)
	if err != nil {
		return ""
	}
	defer key.Close()
	value, _, valueErr := key.GetStringValue("")
	if valueErr != nil {
		return ""
	}
	return value
}

func classOpenCommand(progID string) string {
	key, err := registry.OpenKey(registry.CLASSES_ROOT, progID+`\shell\open\command`, registry.QUERY_VALUE)
	if err != nil {
		return ""
	}
	defer key.Close()
	value, _, valueErr := key.GetStringValue("")
	if valueErr != nil {
		return ""
	}
	return value
}

func commandExecutable(command string) string {
	command = strings.TrimSpace(command)
	if command == "" {
		return ""
	}
	var path string
	if command[0] == '"' {
		if end := strings.IndexByte(command[1:], '"'); end >= 0 {
			path = command[1 : end+1]
		}
	} else {
		path = command
		if space := strings.IndexAny(path, " \t"); space >= 0 {
			path = path[:space]
		}
	}
	path = os.ExpandEnv(path)
	if path == "" {
		return ""
	}
	if info, err := os.Stat(path); err == nil && !info.IsDir() {
		return path
	}
	return ""
}

func displayName(executable string) string {
	base := filepath.Base(executable)
	return strings.TrimSuffix(base, filepath.Ext(base))
}
