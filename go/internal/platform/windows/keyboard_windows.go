//go:build windows

package windows

import (
	"syscall"
	"unicode/utf16"
	"unsafe"
)

var (
	imm32                   = syscall.NewLazyDLL("imm32.dll")
	getKeyStateSync         = user32.NewProc("GetKeyState")
	toUnicode               = user32.NewProc("ToUnicode")
	immGetContext           = imm32.NewProc("ImmGetContext")
	immReleaseContext       = imm32.NewProc("ImmReleaseContext")
	immGetCompositionString = imm32.NewProc("ImmGetCompositionStringW")
)

const (
	gcsCompStr             = 0x0008
	toUnicodeNoStateChange = 0x0004
)

// KeyText converts a key-down transition to the character the target will
// receive. It never mutates keyboard state, so dead keys and IME state remain
// with the target application.
func KeyText(event KeyEvent) (rune, bool) {
	if !event.Down {
		return 0, false
	}
	state := keyboardState()
	var buffer [8]uint16
	count, _, _ := toUnicode.Call(
		uintptr(event.VirtualKey), uintptr(event.ScanCode),
		uintptr(unsafe.Pointer(&state[0])), uintptr(unsafe.Pointer(&buffer[0])),
		8, toUnicodeNoStateChange,
	)
	if int32(count) <= 0 {
		return 0, false
	}
	decoded := utf16.Decode(buffer[:count])
	if len(decoded) == 0 {
		return 0, false
	}
	ch := decoded[0]
	if ch < 0x20 || ch == 0x7f {
		return 0, false
	}
	return ch, true
}

// keyboardState builds the 256-byte key state ToUnicode expects. The hook
// thread is not the target's UI thread, so modifier state comes from the
// global asynchronous key state and toggle bits from GetKeyState. Only the
// keys ToUnicode consults are filled, keeping the hook fast.
func keyboardState() [256]byte {
	var state [256]byte
	for _, key := range [...]uintptr{
		0x08, 0x09, 0x0D, 0x10, 0x11, 0x12, 0x14, 0x15, 0x1B, 0x20,
		0x5B, 0x5C, 0x90, 0x91, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5,
	} {
		async, _, _ := getKeyState.Call(key)
		var value byte
		if async&0x8000 != 0 {
			value |= 0x80
		}
		sync, _, _ := getKeyStateSync.Call(key)
		if sync&1 != 0 {
			value |= 0x01
		}
		state[key] = value
	}
	return state
}

// IMEComposing reports whether the foreground window has a live input-method
// composition. Composed text must never be treated as typed keyword input.
func IMEComposing() bool {
	foreground, _, _ := getForeground.Call()
	if foreground == 0 {
		return false
	}
	context, _, _ := immGetContext.Call(foreground)
	if context == 0 {
		return false
	}
	defer immReleaseContext.Call(foreground, context)
	length, _, _ := immGetCompositionString.Call(context, gcsCompStr, 0, 0)
	return int32(length) > 0
}

// CommandingDown reports Ctrl, Alt or Win, the modifiers that end a keyword
// buffer. Shift alone still types characters.
func CommandingDown() bool {
	for _, key := range []uintptr{0x11, 0x12, 0x5b, 0x5c} {
		state, _, _ := getKeyState.Call(key)
		if uint16(state)&0x8000 != 0 {
			return true
		}
	}
	return false
}

// KeyDown reports the current global state of one virtual key. The message
// host keyboard hook uses it so a fallback global shortcut sees modifiers it
// did not observe itself.
func KeyDown(virtualKey uint32) bool {
	state, _, _ := getKeyState.Call(uintptr(virtualKey))
	return uint16(state)&0x8000 != 0
}

// ResetVirtualKey reports keys that end the keyword buffer without typing.
func ResetVirtualKey(key uint32) bool {
	switch key {
	case 0x09, 0x0D, 0x1B, 0x25, 0x26, 0x27, 0x28, 0x24, 0x23, 0x2E:
		return true
	}
	return false
}

// BackspaceVirtualKey is VK_BACK.
const BackspaceVirtualKey = 0x08
