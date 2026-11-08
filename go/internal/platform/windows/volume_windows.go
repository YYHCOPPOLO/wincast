//go:build windows

package windows

import (
	"errors"
	"fmt"
	"math"
	"runtime"
	"sync"
	"syscall"
	"unsafe"
)

var (
	clsidMMDeviceEnumerator = guidValue(0xBCDE0395, 0xE52F, 0x467C, [8]byte{0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E})
	iidIMMDeviceEnumerator  = guidValue(0xA95664D2, 0x9614, 0x4F35, [8]byte{0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6})
	iidIMMDevice            = guidValue(0xD666063F, 0x1587, 0x4E43, [8]byte{0x81, 0xF1, 0xB9, 0x48, 0xE8, 0x07, 0x36, 0x3F})
	iidIAudioEndpointVolume = guidValue(0x5CDF2C82, 0x841E, 0x4546, [8]byte{0x97, 0x22, 0x0C, 0xF7, 0x40, 0x78, 0x22, 0x9A})
	virtualAllocProc        = kernel32.NewProc("VirtualAlloc")
	virtualFreeProc         = kernel32.NewProc("VirtualFree")
	floatThunkOnce          sync.Once
	setVolumeThunk          uintptr
	getVolumeThunk          uintptr
	thunkErr                error
)

type volumeEndpoint struct {
	enumerator uintptr
	device     uintptr
	volume     uintptr
	uninit     bool
}

func (v *volumeEndpoint) close() {
	comRelease(v.volume)
	comRelease(v.device)
	comRelease(v.enumerator)
}

// openVolumeEndpoint binds the default multimedia render endpoint on the
// calling OS thread.
func openVolumeEndpoint() (*volumeEndpoint, error) {
	runtime.LockOSThread()
	hr, uninit := coInitialize()
	if int32(hr) != 0 && int32(hr) != 1 && int32(hr) != rpcChangedMode {
		runtime.UnlockOSThread()
		return nil, fmt.Errorf("CoInitializeEx: 0x%08x", uint32(hr))
	}
	enumerator, err := coCreateInstanceCtx(&clsidMMDeviceEnumerator, &iidIMMDeviceEnumerator, clsctxAll)
	if err != nil {
		if uninit {
			coUninitializeProc.Call()
		}
		runtime.UnlockOSThread()
		return nil, err
	}
	var device uintptr
	if hr := comCall(enumerator, 4, 0, 1, uintptr(unsafe.Pointer(&device))); int32(hr) < 0 || device == 0 {
		comRelease(enumerator)
		if uninit {
			coUninitializeProc.Call()
		}
		runtime.UnlockOSThread()
		return nil, fmt.Errorf("GetDefaultAudioEndpoint: 0x%08x", uint32(hr))
	}
	var volume uintptr
	if hr := comCall(device, 3, uintptr(unsafe.Pointer(&iidIAudioEndpointVolume)), clsctxAll, 0, uintptr(unsafe.Pointer(&volume))); int32(hr) < 0 || volume == 0 {
		comRelease(device)
		comRelease(enumerator)
		if uninit {
			coUninitializeProc.Call()
		}
		runtime.UnlockOSThread()
		return nil, fmt.Errorf("Activate IAudioEndpointVolume: 0x%08x", uint32(hr))
	}
	return &volumeEndpoint{enumerator: enumerator, device: device, volume: volume, uninit: uninit}, nil
}

func (v *volumeEndpoint) release() {
	v.close()
	if v.uninit {
		coUninitializeProc.Call()
	}
	runtime.UnlockOSThread()
}

func installFloatThunks() error {
	floatThunkOnce.Do(func() {
		// SetMasterVolumeLevelScalar(float, GUID*): this is RCX, the float is
		// in R9 so the thunk can set both candidate XMM slots, and the null
		// event-context pointer stays in RDX/R8.
		setCode := []byte{
			0x66, 0x41, 0x0F, 0x6E, 0xC1, // movd xmm0, r9d
			0x66, 0x41, 0x0F, 0x6E, 0xC9, // movd xmm1, r9d
			0x48, 0x8B, 0xC1, // mov rax, rcx
			0x48, 0x8B, 0x00, // mov rax, [rax]
			0x48, 0x83, 0xEC, 0x28, // sub rsp, 0x28
			0xFF, 0x50, 0x38, // call [rax+0x38]
			0x48, 0x83, 0xC4, 0x28, // add rsp, 0x28
			0xC3, // ret
		}
		// GetMasterVolumeLevelScalar(float*): save the out pointer, call slot
		// 9, and store XMM0 through it.
		getCode := []byte{
			0x56,             // push rsi
			0x48, 0x89, 0xD6, // mov rsi, rdx
			0x48, 0x8B, 0xC1, // mov rax, rcx
			0x48, 0x8B, 0x00, // mov rax, [rax]
			0x48, 0x83, 0xEC, 0x20, // sub rsp, 0x20
			0xFF, 0x50, 0x48, // call [rax+0x48]
			0xF3, 0x0F, 0x11, 0x06, // movss [rsi], xmm0
			0x48, 0x83, 0xC4, 0x20, // add rsp, 0x20
			0x5E, // pop rsi
			0xC3, // ret
		}
		setVolumeThunk, thunkErr = allocateThunk(setCode)
		if thunkErr != nil {
			return
		}
		getVolumeThunk, thunkErr = allocateThunk(getCode)
	})
	return thunkErr
}

func allocateThunk(code []byte) (uintptr, error) {
	const (
		memCommit  = 0x1000
		memReserve = 0x2000
		pageRWX    = 0x40
	)
	address, _, err := virtualAllocProc.Call(0, uintptr(len(code)), memCommit|memReserve, pageRWX)
	if address == 0 {
		return 0, fmt.Errorf("VirtualAlloc: %v", err)
	}
	moveMemory.Call(address, uintptr(unsafe.Pointer(&code[0])), uintptr(len(code)))
	return address, nil
}

// CurrentVolume returns the default output's scalar level and mute state.
func CurrentVolume() (float64, bool, error) {
	endpoint, err := openVolumeEndpoint()
	if err != nil {
		return 0, false, err
	}
	defer endpoint.release()
	if err := installFloatThunks(); err != nil {
		return 0, false, err
	}
	var level float32
	hr, _, _ := syscall.SyscallN(getVolumeThunk, endpoint.volume, uintptr(unsafe.Pointer(&level)))
	if int32(hr) < 0 {
		return 0, false, fmt.Errorf("GetMasterVolumeLevelScalar: 0x%08x", uint32(hr))
	}
	var muted int32
	hr = comCall(endpoint.volume, 15, uintptr(unsafe.Pointer(&muted)))
	if int32(hr) < 0 {
		return float64(level), false, fmt.Errorf("GetMute: 0x%08x", uint32(hr))
	}
	return float64(level), muted != 0, nil
}

// SetVolume sets the default output's scalar level. A positive level also
// clears mute, matching the Windows volume slider.
func SetVolume(level float64) error {
	if level < 0 {
		level = 0
	}
	if level > 1 {
		level = 1
	}
	endpoint, err := openVolumeEndpoint()
	if err != nil {
		return err
	}
	defer endpoint.release()
	if err := installFloatThunks(); err != nil {
		return err
	}
	bits := math.Float32bits(float32(level))
	hr, _, _ := syscall.SyscallN(setVolumeThunk, endpoint.volume, 0, 0, uintptr(bits))
	if int32(hr) < 0 {
		return fmt.Errorf("SetMasterVolumeLevelScalar: 0x%08x", uint32(hr))
	}
	if level > 0 {
		if hr := comCall(endpoint.volume, 14, 0, 0); int32(hr) < 0 {
			return fmt.Errorf("SetMute: 0x%08x", uint32(hr))
		}
	}
	return nil
}

// ToggleMute flips the default output's mute flag.
func ToggleMute() error {
	endpoint, err := openVolumeEndpoint()
	if err != nil {
		return err
	}
	defer endpoint.release()
	var muted int32
	if hr := comCall(endpoint.volume, 15, uintptr(unsafe.Pointer(&muted))); int32(hr) < 0 {
		return fmt.Errorf("GetMute: 0x%08x", uint32(hr))
	}
	next := uintptr(0)
	if muted == 0 {
		next = 1
	}
	if hr := comCall(endpoint.volume, 14, next, 0); int32(hr) < 0 {
		return fmt.Errorf("SetMute: 0x%08x", uint32(hr))
	}
	return nil
}

// StepVolume moves the endpoint by one system volume step.
func StepVolume(up bool) error {
	endpoint, err := openVolumeEndpoint()
	if err != nil {
		return err
	}
	defer endpoint.release()
	slot := 18 // VolumeStepDown
	if up {
		slot = 17
	}
	if hr := comCall(endpoint.volume, slot); int32(hr) < 0 {
		return errors.New("Windows could not change the output volume")
	}
	return nil
}

// VolumeAvailable reports whether the default endpoint exposes software volume.
func VolumeAvailable() bool {
	endpoint, err := openVolumeEndpoint()
	if err != nil {
		return false
	}
	endpoint.release()
	return true
}
