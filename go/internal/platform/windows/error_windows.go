//go:build windows

package windows

import (
	"syscall"
	"unsafe"
)

// ShowStartupError provides a visible error for the GUI-subsystem executable,
// including failures before a native MyGo window or log file can be created.
func ShowStartupError(message string) {
	text, err := syscall.UTF16PtrFromString(message)
	if err != nil {
		return
	}
	title, _ := syscall.UTF16PtrFromString("Wincast")
	user32.NewProc("MessageBoxW").Call(0, uintptr(unsafe.Pointer(text)), uintptr(unsafe.Pointer(title)), 0x10|0x10000)
}
