//go:build windows

package windows

import (
	"syscall"
	"unsafe"
)

var getUserDefaultLocaleName = kernel32.NewProc("GetUserDefaultLocaleName")

// UserLocale returns the Windows user locale, e.g. "en-US" or "zh-CN".
func UserLocale() string {
	buffer := make([]uint16, 85)
	length, _, _ := getUserDefaultLocaleName.Call(uintptr(unsafe.Pointer(&buffer[0])), uintptr(len(buffer)))
	if length <= 1 {
		return ""
	}
	return syscall.UTF16ToString(buffer[:length-1])
}
