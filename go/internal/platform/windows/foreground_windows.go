//go:build windows

package windows

import (
	"time"
	"unsafe"
)

var (
	setForegroundWindow = user32.NewProc("SetForegroundWindow")
	attachThreadInput   = user32.NewProc("AttachThreadInput")
	bringWindowToTop    = user32.NewProc("BringWindowToTop")
	setActiveWindow     = user32.NewProc("SetActiveWindow")
	setFocus            = user32.NewProc("SetFocus")
	getCurrentThreadID  = kernel32.NewProc("GetCurrentThreadId")
)

// ForceForeground brings hwnd in front and gives it the keyboard. A process
// woken by a low-level keyboard hook has no foreground right of its own, so a
// plain SetForegroundWindow can leave the panel inactive until it is clicked.
// AttachThreadInput borrows the current foreground thread's right; a synthetic
// Alt tap is the last resort Windows accepts.
func ForceForeground(hwnd uintptr) bool {
	if hwnd == 0 {
		return false
	}
	if ForegroundWindow() == hwnd {
		return true
	}
	setForegroundWindow.Call(hwnd)
	if ForegroundWindow() == hwnd {
		return true
	}

	if foreground := ForegroundWindow(); foreground != 0 {
		var pid uint32
		foregroundThread, _, _ := getWindowProcess.Call(foreground, uintptr(unsafe.Pointer(&pid)))
		currentThread, _, _ := getCurrentThreadID.Call()
		if foregroundThread != 0 && foregroundThread != currentThread {
			if ok, _, _ := attachThreadInput.Call(foregroundThread, currentThread, 1); ok != 0 {
				setForegroundWindow.Call(hwnd)
				bringWindowToTop.Call(hwnd)
				setActiveWindow.Call(hwnd)
				setFocus.Call(hwnd)
				attachThreadInput.Call(foregroundThread, currentThread, 0)
			}
			if ForegroundWindow() == hwnd {
				return true
			}
		}
	}

	// Releasing and pressing Alt makes Windows consider this process the
	// source of the latest input, which lifts the foreground lock.
	if err := sendInputs([]keyboardInput{keyInput(0x12, false)}); err == nil {
		time.Sleep(5 * time.Millisecond)
		setForegroundWindow.Call(hwnd)
		_ = sendInputs([]keyboardInput{keyInput(0x12, true)})
	}
	return ForegroundWindow() == hwnd
}
