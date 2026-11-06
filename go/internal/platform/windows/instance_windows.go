//go:build windows

package windows

import (
	"errors"
	"fmt"
	"syscall"
	"unsafe"
)

var kernel32 = syscall.NewLazyDLL("kernel32.dll")
var createMutex = kernel32.NewProc("CreateMutexW")

// Instance holds a session-local kernel object. A failure to create the object
// is an error rather than permission to run a second writer.
type Instance struct{ handle syscall.Handle }

func AcquireInstance(identifier string) (*Instance, bool, error) {
	name, err := syscall.UTF16PtrFromString(`Local\` + identifier)
	if err != nil {
		return nil, false, err
	}
	handle, _, callErr := createMutex.Call(0, 0, uintptr(unsafe.Pointer(name)))
	if handle == 0 {
		return nil, false, fmt.Errorf("create preview instance lock: %w", callErr)
	}
	instance := &Instance{handle: syscall.Handle(handle)}
	return instance, !errors.Is(callErr, syscall.ERROR_ALREADY_EXISTS), nil
}

func (i *Instance) Close() {
	if i != nil && i.handle != 0 {
		_ = syscall.CloseHandle(i.handle)
		i.handle = 0
	}
}
