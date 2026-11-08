//go:build windows

package windows

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/registry"
)

// InstalledProgram is one Add/Remove Programs entry. Fields are raw registry
// values; filtering for updates and system components belongs to the caller.
type InstalledProgram struct {
	Key              string
	Hive             string
	DisplayName      string
	DisplayVersion   string
	Publisher        string
	InstallLocation  string
	DisplayIcon      string
	UninstallString  string
	SystemComponent  bool
	WindowsInstaller bool
	ParentKeyName    string
	ReleaseType      string
}

func InstalledPrograms() []InstalledProgram {
	type root struct {
		hive string
		key  registry.Key
		path string
	}
	roots := []root{
		{"HKLM", registry.LOCAL_MACHINE, `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`},
		{"HKLM", registry.LOCAL_MACHINE, `SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`},
		{"HKCU", registry.CURRENT_USER, `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`},
	}
	var programs []InstalledProgram
	for _, entry := range roots {
		key, err := registry.OpenKey(entry.key, entry.path, registry.QUERY_VALUE|registry.ENUMERATE_SUB_KEYS)
		if err != nil {
			continue
		}
		names, err := key.ReadSubKeyNames(-1)
		key.Close()
		if err != nil {
			continue
		}
		for _, name := range names {
			subkey, err := registry.OpenKey(entry.key, entry.path+`\`+name, registry.QUERY_VALUE)
			if err != nil {
				continue
			}
			program := InstalledProgram{Key: name, Hive: entry.hive}
			program.DisplayName = readRegistryString(subkey, "DisplayName")
			if program.DisplayName == "" {
				subkey.Close()
				continue
			}
			program.DisplayVersion = readRegistryString(subkey, "DisplayVersion")
			program.Publisher = readRegistryString(subkey, "Publisher")
			program.InstallLocation = cleanProgramPath(readRegistryString(subkey, "InstallLocation"))
			program.DisplayIcon = cleanProgramPath(readRegistryString(subkey, "DisplayIcon"))
			program.UninstallString = strings.TrimSpace(readRegistryString(subkey, "UninstallString"))
			program.ParentKeyName = readRegistryString(subkey, "ParentKeyName")
			program.ReleaseType = readRegistryString(subkey, "ReleaseType")
			program.SystemComponent = readRegistryDword(subkey, "SystemComponent") == 1
			program.WindowsInstaller = readRegistryDword(subkey, "WindowsInstaller") == 1
			subkey.Close()
			programs = append(programs, program)
		}
	}
	return programs
}

func readRegistryString(key registry.Key, name string) string {
	value, _, err := key.GetStringValue(name)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(value)
}

func readRegistryDword(key registry.Key, name string) uint64 {
	value, _, err := key.GetIntegerValue(name)
	if err != nil {
		return 0
	}
	return value
}

// cleanProgramPath removes quotes and the icon index suffix.
func cleanProgramPath(value string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return ""
	}
	if strings.HasPrefix(value, `"`) {
		if end := strings.Index(value[1:], `"`); end >= 0 {
			return strings.TrimSpace(value[1 : end+1])
		}
	}
	if comma := strings.LastIndex(value, ","); comma > 0 {
		suffix := strings.TrimSpace(value[comma+1:])
		indexOnly := suffix != ""
		for _, r := range suffix {
			if r < '0' || r > '9' {
				indexOnly = false
				break
			}
		}
		if indexOnly {
			value = value[:comma]
		}
	}
	return strings.TrimSpace(strings.Trim(value, `"`))
}

const fileAddFile = 0x0002 // FILE_ADD_FILE, not exported by x/sys/windows.

// recycleSupported rejects drives where SHFileOperation would have to delete
// permanently because they have no Recycle Bin.
func recycleSupported(path string) (bool, string) {
	if recyclePolicyDisabled() {
		return false, "Recycle Bin is disabled by policy"
	}
	volume := filepath.VolumeName(path)
	if volume == "" {
		return false, "no volume"
	}
	if !strings.HasSuffix(volume, `\`) {
		volume += `\`
	}
	pointer, err := syscall.UTF16PtrFromString(volume)
	if err != nil {
		return false, "invalid volume"
	}
	kind, _, _ := getDriveType.Call(uintptr(unsafe.Pointer(pointer)))
	switch uint32(kind) {
	case driveFixed, driveRemovable:
		return true, ""
	}
	return false, "drive has no Recycle Bin"
}

const (
	driveRemovable = 2
	driveFixed     = 3
)

// recyclePolicyDisabled reads the documented Explorer policies that turn the
// Recycle Bin off entirely. With those set, FOF_ALLOWUNDO would delete
// permanently, so those rows are locked instead.
func recyclePolicyDisabled() bool {
	paths := []string{
		`Software\Microsoft\Windows\CurrentVersion\Policies\Explorer`,
		`Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket`,
	}
	for _, hive := range []registry.Key{registry.CURRENT_USER, registry.LOCAL_MACHINE} {
		for _, path := range paths {
			key, err := registry.OpenKey(hive, path, registry.QUERY_VALUE)
			if err != nil {
				continue
			}
			value, _, err := key.GetIntegerValue("NoRecycleFiles")
			key.Close()
			if err == nil && value == 1 {
				return true
			}
		}
	}
	return false
}

// PathLocked reports whether the current user can remove a path without
// escalation. The check opens the object with DELETE and its parent with the
// create-child right, which is the pair the recycle operation needs.
func PathLocked(path string) (bool, string) {
	if supported, reason := recycleSupported(path); !supported {
		return true, reason
	}
	pointer, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return true, "invalid path"
	}
	handle, err := windows.CreateFile(
		pointer, windows.DELETE|windows.FILE_READ_ATTRIBUTES,
		windows.FILE_SHARE_READ|windows.FILE_SHARE_WRITE|windows.FILE_SHARE_DELETE,
		nil, windows.OPEN_EXISTING, windows.FILE_FLAG_BACKUP_SEMANTICS|windows.FILE_FLAG_OPEN_REPARSE_POINT, 0,
	)
	if err != nil {
		return true, accessReason(err)
	}
	windows.CloseHandle(handle)
	parent := filepath.Dir(path)
	if parent == path {
		return true, "no parent directory"
	}
	parentPointer, err := syscall.UTF16PtrFromString(parent)
	if err != nil {
		return true, "invalid parent path"
	}
	parentHandle, err := windows.CreateFile(
		parentPointer, fileAddFile,
		windows.FILE_SHARE_READ|windows.FILE_SHARE_WRITE|windows.FILE_SHARE_DELETE,
		nil, windows.OPEN_EXISTING, windows.FILE_FLAG_BACKUP_SEMANTICS, 0,
	)
	if err != nil {
		return true, "parent " + accessReason(err)
	}
	windows.CloseHandle(parentHandle)
	return false, ""
}

func accessReason(err error) string {
	switch {
	case errors.Is(err, syscall.ERROR_ACCESS_DENIED):
		return "access denied"
	case errors.Is(err, windows.ERROR_SHARING_VIOLATION):
		return "in use"
	case errors.Is(err, syscall.ERROR_FILE_NOT_FOUND), errors.Is(err, syscall.ERROR_PATH_NOT_FOUND):
		return "missing"
	}
	return err.Error()
}

const (
	foDelete           = 3
	fofAllowUndo       = 0x0040
	fofNoConfirmation  = 0x0010
	fofNoConfirmMkDir  = 0x0200
	fofSilent          = 0x0004
	fofNoErrorUI       = 0x0400
	fofWantNukeWarning = 0x4000
)

type shFileOperation struct {
	hwnd                  uintptr
	wFunc                 uint32
	pFrom                 *uint16
	pTo                   *uint16
	fFlags                uint16
	fAnyOperationsAborted int32
	hNameMappings         uintptr
	lpszProgressTitle     *uint16
}

// RecycleResult is the outcome for one path. Err is non-nil when the item
// stayed in place; callers must never fall back to a permanent delete.
type RecycleResult struct {
	Path string
	Err  error
}

// RecyclePaths moves each path to the Recycle Bin. Each item is a separate
// operation so a failure names exactly which item stayed behind.
func RecyclePaths(paths []string) []RecycleResult {
	results := make([]RecycleResult, 0, len(paths))
	for _, path := range paths {
		results = append(results, RecycleResult{Path: path, Err: recyclePath(path)})
	}
	return results
}

func recyclePath(path string) error {
	if strings.TrimSpace(path) == "" {
		return errors.New("empty path")
	}
	from, err := syscall.UTF16FromString(path)
	if err != nil {
		return err
	}
	from = append(from, 0) // SHFileOperation requires a double NUL terminator.
	operation := shFileOperation{
		wFunc:  foDelete,
		pFrom:  &from[0],
		fFlags: fofAllowUndo | fofNoConfirmation | fofNoConfirmMkDir | fofSilent | fofNoErrorUI | fofWantNukeWarning,
	}
	result, _, _ := shFileOperationProc.Call(uintptr(unsafe.Pointer(&operation)))
	if result != 0 {
		return fmt.Errorf("SHFileOperationW: 0x%08x", uint32(result))
	}
	if operation.fAnyOperationsAborted != 0 {
		return errors.New("the operation was aborted")
	}
	if _, err := os.Lstat(path); err == nil {
		return errors.New("the item is still present")
	} else if !errors.Is(err, fs.ErrNotExist) {
		return fmt.Errorf("verify after recycle: %w", err)
	}
	return nil
}

var shFileOperationProc = shell32.NewProc("SHFileOperationW")
var getDriveType = kernel32.NewProc("GetDriveTypeW")
