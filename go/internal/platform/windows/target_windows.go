//go:build windows

package windows

import (
	"errors"
	"os"
	"syscall"
	"unsafe"
)

var (
	user32           = syscall.NewLazyDLL("user32.dll")
	getForeground    = user32.NewProc("GetForegroundWindow")
	getWindowProcess = user32.NewProc("GetWindowThreadProcessId")
	isWindow         = user32.NewProc("IsWindow")
	getProcessTimes  = kernel32.NewProc("GetProcessTimes")
	getUILanguage    = kernel32.NewProc("GetUserDefaultUILanguage")
	getClassName     = user32.NewProc("GetClassNameW")
)

func ForegroundWindow() uintptr {
	hwnd, _, _ := getForeground.Call()
	return hwnd
}

// Target remembers both the window and the lifetime of its process. HWND/PID
// values may be reused, so neither value alone is sufficient for paste-back.
type Target struct {
	Window  uintptr
	Process uint32
	created syscall.Filetime
}

func ForegroundTarget() Target {
	hwnd, _, _ := getForeground.Call()
	var pid uint32
	getWindowProcess.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	if hwnd == 0 || pid == 0 || pid == uint32(os.Getpid()) {
		return Target{}
	}
	var class [128]uint16
	getClassName.Call(hwnd, uintptr(unsafe.Pointer(&class[0])), uintptr(len(class)))
	switch syscall.UTF16ToString(class[:]) {
	case "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "Progman", "WorkerW":
		return Target{}
	}
	created, err := processCreation(pid)
	if err != nil {
		return Target{}
	}
	return Target{Window: hwnd, Process: pid, created: created}
}

func (t Target) Validate() error {
	valid, _, _ := isWindow.Call(t.Window)
	if t.Window == 0 || valid == 0 {
		return errors.New("target window is no longer available")
	}
	var pid uint32
	getWindowProcess.Call(t.Window, uintptr(unsafe.Pointer(&pid)))
	created, err := processCreation(pid)
	if err != nil || pid != t.Process || created != t.created {
		return errors.New("target process has changed")
	}
	return nil
}

// CheckEditable validates the remembered window and refuses a target with a
// higher integrity level, matching the paste path.
func (t Target) CheckEditable() error {
	if err := t.Validate(); err != nil {
		return err
	}
	return t.checkIntegrity()
}

// ProcessImagePath returns the executable path of a process, or "".
func ProcessImagePath(pid uint32) string {
	handle, err := syscall.OpenProcess(0x1000, false, pid) // QUERY_LIMITED_INFORMATION
	if err != nil {
		return ""
	}
	defer syscall.CloseHandle(handle)
	buffer := make([]uint16, 1024)
	size := uint32(len(buffer))
	queryFullProcessImageName := kernel32.NewProc("QueryFullProcessImageNameW")
	ok, _, _ := queryFullProcessImageName.Call(uintptr(handle), 0,
		uintptr(unsafe.Pointer(&buffer[0])), uintptr(unsafe.Pointer(&size)))
	if ok == 0 || size == 0 {
		return ""
	}
	return syscall.UTF16ToString(buffer[:size])
}

func processCreation(pid uint32) (syscall.Filetime, error) {
	handle, err := syscall.OpenProcess(0x1000, false, pid) // QUERY_LIMITED_INFORMATION
	if err != nil {
		return syscall.Filetime{}, err
	}
	defer syscall.CloseHandle(handle)
	var created, exited, kernel, user syscall.Filetime
	ok, _, callErr := getProcessTimes.Call(uintptr(handle), uintptr(unsafe.Pointer(&created)), uintptr(unsafe.Pointer(&exited)), uintptr(unsafe.Pointer(&kernel)), uintptr(unsafe.Pointer(&user)))
	if ok == 0 {
		return syscall.Filetime{}, callErr
	}
	return created, nil
}

func ChineseUI() bool {
	language, _, _ := getUILanguage.Call()
	return language&0x3ff == 0x04
}
