//go:build windows

package windows

import (
	"fmt"
	"syscall"
	"unsafe"
)

// guid is the Windows GUID layout. Composite literals below follow the
// network order of the Data1/Data2/Data3 fields as the SDK headers spell them.
type guid struct {
	Data1 uint32
	Data2 uint16
	Data3 uint16
	Data4 [8]byte
}

var (
	ole32                = syscall.NewLazyDLL("ole32.dll")
	coInitializeExProc   = ole32.NewProc("CoInitializeEx")
	coUninitializeProc   = ole32.NewProc("CoUninitialize")
	coCreateInstanceProc = ole32.NewProc("CoCreateInstance")
	clsidFromProgIDProc  = ole32.NewProc("CLSIDFromProgID")
)

const (
	clsctxInprocServer  = 0x1
	clsctxAll           = 0x17
	coinitMultithreaded = 0x0
)

// comPointer reinterprets a COM address without a direct uintptr conversion,
// which go vet's unsafeptr check rejects.
func comPointer(value uintptr) unsafe.Pointer {
	return *(*unsafe.Pointer)(unsafe.Pointer(&value))
}

// comCall invokes a COM vtable slot. The COM object pointer points at a table
// of function pointers; slot 0..2 are QueryInterface/AddRef/Release.
func comCall(this uintptr, slot int, args ...uintptr) uintptr {
	if this == 0 {
		return 0x80004003 // E_POINTER
	}
	table := *(*uintptr)(comPointer(this))
	method := *(*uintptr)(comPointer(table + uintptr(slot)*unsafe.Sizeof(uintptr(0))))
	call := make([]uintptr, 0, len(args)+1)
	call = append(call, this)
	call = append(call, args...)
	result, _, _ := syscall.SyscallN(method, call...)
	return result
}

func comRelease(this uintptr) {
	if this != 0 {
		comCall(this, 2)
	}
}

func comQueryInterface(this uintptr, iid *guid) uintptr {
	var out uintptr
	if comCall(this, 0, uintptr(unsafe.Pointer(iid)), uintptr(unsafe.Pointer(&out))) != 0 {
		return 0
	}
	return out
}

func coInitialize() (uintptr, bool) {
	hr, _, _ := coInitializeExProc.Call(0, coinitMultithreaded)
	if int32(hr) == rpcChangedMode {
		return hr, false
	}
	return hr, hr == 0 || hr == 1 // S_OK or S_FALSE both require CoUninitialize.
}

func coCreateInstance(clsid, iid *guid) (uintptr, error) {
	var out uintptr
	hr, _, _ := coCreateInstanceProc.Call(
		uintptr(unsafe.Pointer(clsid)),
		0,
		clsctxInprocServer,
		uintptr(unsafe.Pointer(iid)),
		uintptr(unsafe.Pointer(&out)),
	)
	if int32(hr) != 0 {
		return 0, fmt.Errorf("CoCreateInstance: 0x%08x", uint32(hr))
	}
	return out, nil
}

func clsidFromProgID(name string) (guid, error) {
	var clsid guid
	wide, err := syscall.UTF16PtrFromString(name)
	if err != nil {
		return clsid, err
	}
	hr, _, _ := clsidFromProgIDProc.Call(uintptr(unsafe.Pointer(wide)), uintptr(unsafe.Pointer(&clsid)))
	if int32(hr) != 0 {
		return clsid, fmt.Errorf("CLSIDFromProgID(%s): 0x%08x", name, uint32(hr))
	}
	return clsid, nil
}

func guidValue(data1 uint32, data2, data3 uint16, data4 [8]byte) guid {
	return guid{Data1: data1, Data2: data2, Data3: data3, Data4: data4}
}
