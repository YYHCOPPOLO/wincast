//go:build windows

package windows

import (
	"errors"
	"fmt"
	"runtime"
	"syscall"
	"time"
	"unsafe"
)

// Windows Runtime activation and async completion. This is intentionally a
// small ABI bridge: no generated bindings, no second event loop.
var (
	combase                   = syscall.NewLazyDLL("combase.dll")
	roInitializeProc          = combase.NewProc("RoInitialize")
	roUninitializeProc        = combase.NewProc("RoUninitialize")
	roGetActivationFactory    = combase.NewProc("RoGetActivationFactory")
	roActivateInstanceProc    = combase.NewProc("RoActivateInstance")
	windowsCreateString       = combase.NewProc("WindowsCreateString")
	windowsDeleteString       = combase.NewProc("WindowsDeleteString")
	windowsGetStringRawBuffer = combase.NewProc("WindowsGetStringRawBuffer")

	iidIRadioStatics = guidValue(0x5FB6A12E, 0x67CB, 0x46AE, [8]byte{0xAA, 0xE9, 0x65, 0x91, 0x9F, 0x86, 0xEF, 0xF4})
)

const (
	roInitMultithreaded = 1
	asyncStarted        = 0
	asyncCompleted      = 1
	asyncCanceled       = 2
	asyncError          = 3
	radioAccessAllowed  = 1
	radioKindBluetooth  = 3
	radioStateOn        = 1
	radioStateOff       = 2
)

type hstring uintptr

func newHString(value string) (hstring, error) {
	units, err := syscall.UTF16FromString(value)
	if err != nil {
		return 0, err
	}
	var out uintptr
	hr, _, _ := windowsCreateString.Call(uintptr(unsafe.Pointer(&units[0])), uintptr(len(units)-1), uintptr(unsafe.Pointer(&out)))
	if int32(hr) < 0 {
		return 0, fmt.Errorf("WindowsCreateString: 0x%08x", uint32(hr))
	}
	return hstring(out), nil
}

func (h hstring) String() string {
	if h == 0 {
		return ""
	}
	var length uint32
	buffer, _, _ := windowsGetStringRawBuffer.Call(uintptr(h), uintptr(unsafe.Pointer(&length)))
	if buffer == 0 || length == 0 {
		return ""
	}
	return syscall.UTF16ToString(unsafe.Slice((*uint16)(comPointer(buffer)), length))
}

func (h hstring) Close() {
	if h != 0 {
		windowsDeleteString.Call(uintptr(h))
	}
}

func roInitialize() (bool, error) {
	hr, _, _ := roInitializeProc.Call(roInitMultithreaded)
	if int32(hr) < 0 && int32(hr) != rpcChangedMode {
		return false, fmt.Errorf("RoInitialize: 0x%08x", uint32(hr))
	}
	return int32(hr) >= 0, nil
}

func activationFactory(class string, iid *guid) (uintptr, error) {
	name, err := newHString(class)
	if err != nil {
		return 0, err
	}
	defer name.Close()
	var factory uintptr
	hr, _, _ := roGetActivationFactory.Call(uintptr(name), uintptr(unsafe.Pointer(iid)), uintptr(unsafe.Pointer(&factory)))
	if int32(hr) < 0 || factory == 0 {
		return 0, fmt.Errorf("RoGetActivationFactory(%s): 0x%08x", class, uint32(hr))
	}
	return factory, nil
}

func activateInstance(class string) (uintptr, error) {
	name, err := newHString(class)
	if err != nil {
		return 0, err
	}
	defer name.Close()
	var instance uintptr
	hr, _, _ := roActivateInstanceProc.Call(uintptr(name), uintptr(unsafe.Pointer(&instance)))
	if int32(hr) < 0 || instance == 0 {
		return 0, fmt.Errorf("RoActivateInstance(%s): 0x%08x", class, uint32(hr))
	}
	return instance, nil
}

// awaitOperation polls IAsyncOperation/IAsyncInfo until completion. The
// request may be canceled by the caller through the operation's Cancel slot.
func awaitOperation(operation uintptr, timeout time.Duration) (uintptr, error) {
	if operation == 0 {
		return 0, errors.New("empty WinRT operation")
	}
	deadline := time.Now().Add(timeout)
	for {
		var status int32
		hr := comCall(operation, 7, uintptr(unsafe.Pointer(&status)))
		if int32(hr) < 0 {
			return 0, fmt.Errorf("IAsyncInfo.Status: 0x%08x", uint32(hr))
		}
		switch status {
		case asyncCompleted:
			var result uintptr
			hr := comCall(operation, 13, uintptr(unsafe.Pointer(&result)))
			if int32(hr) < 0 {
				return 0, fmt.Errorf("IAsyncOperation.GetResults: 0x%08x", uint32(hr))
			}
			return result, nil
		case asyncCanceled:
			return 0, errors.New("WinRT operation canceled")
		case asyncError:
			var code uint32
			comCall(operation, 8, uintptr(unsafe.Pointer(&code)))
			return 0, fmt.Errorf("WinRT operation failed: 0x%08x", code)
		}
		if time.Now().After(deadline) {
			// Cancelling a WinRT operation whose provider is unavailable has
			// been observed to fault inside the proxy. Releasing the reference
			// at the caller is safe; the completed callback owns its own
			// lifetime.
			return 0, errors.New("WinRT operation timed out")
		}
		time.Sleep(10 * time.Millisecond)
	}
}

func awaitEnum(operation uintptr, timeout time.Duration) (int32, error) {
	result, err := awaitOperation(operation, timeout)
	if err != nil {
		return 0, err
	}
	return int32(result), nil
}

func withWinRT(fn func() error) error {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	initialized, err := roInitialize()
	if err != nil {
		return err
	}
	if initialized {
		defer roUninitializeProc.Call()
	}
	return fn()
}

func radioFactoryAndAccess() (uintptr, error) {
	var factory uintptr
	err := withWinRT(func() error {
		var err error
		factory, err = activationFactory("Windows.Devices.Radios.Radio", &iidIRadioStatics)
		if err != nil {
			return err
		}
		var operation uintptr
		if hr := comCall(factory, 9, uintptr(unsafe.Pointer(&operation))); int32(hr) < 0 || operation == 0 {
			return fmt.Errorf("Radio.RequestAccessAsync: 0x%08x", uint32(hr))
		}
		access, err := awaitEnum(operation, 15*time.Second)
		comRelease(operation)
		if err != nil {
			return err
		}
		if access != radioAccessAllowed {
			return errors.New("Bluetooth permission is required")
		}
		return nil
	})
	if err != nil {
		return 0, err
	}
	return factory, nil
}

func requestBluetoothAccess() error {
	factory, err := radioFactoryAndAccess()
	if factory != 0 {
		comRelease(factory)
	}
	return err
}

func toggleBluetoothState() (bool, error) {
	factory, err := radioFactoryAndAccess()
	if err != nil {
		return false, err
	}
	defer comRelease(factory)
	var enabled bool
	err = withWinRT(func() error {
		var operation uintptr
		if hr := comCall(factory, 6, uintptr(unsafe.Pointer(&operation))); int32(hr) < 0 || operation == 0 {
			return fmt.Errorf("Radio.GetRadiosAsync: 0x%08x", uint32(hr))
		}
		view, err := awaitOperation(operation, 15*time.Second)
		comRelease(operation)
		if err != nil {
			return err
		}
		defer comRelease(view)
		var size uint32
		if hr := comCall(view, 7, uintptr(unsafe.Pointer(&size))); int32(hr) < 0 {
			return fmt.Errorf("IVectorView.Size: 0x%08x", uint32(hr))
		}
		for index := uint32(0); index < size; index++ {
			var radio uintptr
			if hr := comCall(view, 6, uintptr(index), uintptr(unsafe.Pointer(&radio))); int32(hr) < 0 || radio == 0 {
				continue
			}
			var kind int32
			if hr := comCall(radio, 11, uintptr(unsafe.Pointer(&kind))); int32(hr) < 0 || kind != radioKindBluetooth {
				comRelease(radio)
				continue
			}
			var state int32
			if hr := comCall(radio, 9, uintptr(unsafe.Pointer(&state))); int32(hr) < 0 {
				comRelease(radio)
				continue
			}
			next := int32(radioStateOn)
			if state == radioStateOn {
				next = radioStateOff
			}
			var change uintptr
			if hr := comCall(radio, 6, uintptr(next), uintptr(unsafe.Pointer(&change))); int32(hr) < 0 || change == 0 {
				comRelease(radio)
				return fmt.Errorf("Radio.SetStateAsync: 0x%08x", uint32(hr))
			}
			access, err := awaitEnum(change, 15*time.Second)
			comRelease(change)
			comRelease(radio)
			if err != nil {
				return err
			}
			if access != radioAccessAllowed {
				return errors.New("Bluetooth permission is required")
			}
			enabled = next == radioStateOn
			return nil
		}
		return errors.New("No Bluetooth radio is available")
	})
	return enabled, err
}
