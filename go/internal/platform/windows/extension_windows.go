//go:build windows

package windows

import (
	"os"
	"strconv"
	"syscall"
	"unsafe"
)

// ExtensionWindow is one top-level window the extension window-management
// capabilities report. Bounds are physical pixels.
type ExtensionWindow struct {
	ID                  string
	Title               string
	Process             uint32
	X, Y, Width, Height int32
	Active              bool
	Positionable        bool
	Resizable           bool
	FullScreenSettable  bool
}

var (
	getWindowTextW = user32.NewProc("GetWindowTextW")
	getForegroundW = user32.NewProc("GetForegroundWindow")
)

const (
	gwlExStyle     = 0xFFFFFFEC
	wsExToolWindow = 0x00000080
	wsExAppWindow  = 0x00040000
)

func windowTitle(hwnd uintptr) string {
	length, _, _ := getWindowTextLength.Call(hwnd)
	if length == 0 {
		return ""
	}
	buffer := make([]uint16, length+1)
	getWindowTextW.Call(hwnd, uintptr(unsafe.Pointer(&buffer[0])), uintptr(len(buffer)))
	return syscall.UTF16ToString(buffer)
}

func extensionWindowInfo(hwnd uintptr, foreground uintptr) (ExtensionWindow, bool) {
	if hwnd == 0 {
		return ExtensionWindow{}, false
	}
	if visible, _, _ := isWindowVisible.Call(hwnd); visible == 0 {
		return ExtensionWindow{}, false
	}
	if owner, _, _ := getWindow.Call(hwnd, 4); owner != 0 {
		return ExtensionWindow{}, false
	}
	if isShellWindow(hwnd) {
		return ExtensionWindow{}, false
	}
	var cloaked uint32
	if result, _, _ := dwmGetWindowAttribute.Call(hwnd, 14, uintptr(unsafe.Pointer(&cloaked)), 4); result == 0 && cloaked != 0 {
		return ExtensionWindow{}, false
	}
	style, _, _ := getWindowLongW.Call(hwnd, gwlExStyle)
	if uint32(style)&wsExToolWindow != 0 && uint32(style)&wsExAppWindow == 0 {
		return ExtensionWindow{}, false
	}
	frames, ok := readWindowFrames(hwnd)
	if !ok {
		return ExtensionWindow{}, false
	}
	var pid uint32
	getWindowThreadPID.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	if pid == uint32(os.Getpid()) {
		return ExtensionWindow{}, false
	}
	return ExtensionWindow{
		ID:                 strconv.FormatUint(uint64(hwnd), 10),
		Title:              windowTitle(hwnd),
		Process:            pid,
		X:                  frames.Window.Left,
		Y:                  frames.Window.Top,
		Width:              frames.Window.width(),
		Height:             frames.Window.height(),
		Active:             hwnd == foreground,
		Positionable:       true,
		Resizable:          sizeSettable(hwnd),
		FullScreenSettable: true,
	}, true
}

// ExtensionWindows lists visible top-level application windows in z-order.
func ExtensionWindows() []ExtensionWindow {
	result := make([]ExtensionWindow, 0, 32)
	foreground, _, _ := getForegroundW.Call()
	callback := syscall.NewCallback(func(hwnd uintptr, _ uintptr) uintptr {
		if info, ok := extensionWindowInfo(hwnd, foreground); ok {
			result = append(result, info)
		}
		return 1
	})
	enumWindows.Call(callback, 0)
	return result
}

// ExtensionWindowByHandle reports one window when it still exists.
func ExtensionWindowByHandle(hwnd uintptr) (ExtensionWindow, bool) {
	foreground, _, _ := getForegroundW.Call()
	return extensionWindowInfo(hwnd, foreground)
}

// ExtensionActiveWindow reports the foreground window.
func ExtensionActiveWindow() (ExtensionWindow, bool) {
	foreground, _, _ := getForegroundW.Call()
	return extensionWindowInfo(foreground, foreground)
}

// ExtensionSetWindowBounds moves and resizes one window; the presence flags
// say which axes the caller supplied.
func ExtensionSetWindowBounds(id string, x, y, width, height float64, hasX, hasY, hasWidth, hasHeight bool) bool {
	hwnd, valid := extensionWindowHandle(id)
	if !valid {
		return false
	}
	frames, ok := readWindowFrames(hwnd)
	if !ok {
		return false
	}
	left, top := frames.Window.Left, frames.Window.Top
	windowWidth, windowHeight := frames.Window.width(), frames.Window.height()
	if hasX {
		left = int32(x)
	}
	if hasY {
		top = int32(y)
	}
	if hasWidth {
		windowWidth = int32(width)
	}
	if hasHeight {
		windowHeight = int32(height)
	}
	if iconic, _, _ := isIconic.Call(hwnd); iconic != 0 {
		showWindow.Call(hwnd, swRestore)
	}
	positioned, _, _ := setWindowPos.Call(hwnd, hwndTop, uintptr(left), uintptr(top),
		uintptr(windowWidth), uintptr(windowHeight), swpNoZOrder|swpNoActivate)
	return positioned != 0
}

// ExtensionMaximizeWindow makes one window fullscreen-equivalent on Windows:
// it maximizes rather than entering a separate fullscreen space.
func ExtensionMaximizeWindow(id string) bool {
	hwnd, valid := extensionWindowHandle(id)
	if !valid {
		return false
	}
	showWindow.Call(hwnd, swMaximize)
	return true
}

func extensionWindowHandle(id string) (uintptr, bool) {
	value, err := strconv.ParseUint(id, 10, 64)
	if err != nil {
		return 0, false
	}
	hwnd := uintptr(value)
	return hwnd, WindowValid(hwnd)
}

// ExtensionPrimaryScreen reports the primary screen's physical size.
func ExtensionPrimaryScreen() (int32, int32) {
	for _, screen := range ScreensPixel() {
		if screen.Primary {
			return screen.Frame.width(), screen.Frame.height()
		}
	}
	screens := ScreensPixel()
	if len(screens) > 0 {
		return screens[0].Frame.width(), screens[0].Frame.height()
	}
	return 0, 0
}
