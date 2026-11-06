//go:build windows && amd64

package windows

import (
	"context"
	"errors"
	"fmt"
	"os"
	"runtime"
	"strconv"
	"strings"
	"syscall"
	"time"
	"unsafe"
)

var (
	setForeground = user32.NewProc("SetForegroundWindow")
	showWindow    = user32.NewProc("ShowWindow")
	isIconic      = user32.NewProc("IsIconic")
	getKeyState   = user32.NewProc("GetAsyncKeyState")
	sendInput     = user32.NewProc("SendInput")
	clipboardSeq  = user32.NewProc("GetClipboardSequenceNumber")
)

var (
	ErrForeground = errors.New("target is not the foreground window")
	ErrPermission = errors.New("target has a higher integrity level")
	ErrModifiers  = errors.New("modifier keys are still held")
	ErrClipboard  = errors.New("clipboard ownership has changed")
)

func ClipboardSequence() uint32 {
	sequence, _, _ := clipboardSeq.Call()
	return uint32(sequence)
}

// Restore restores only the remembered identity. It never grants blanket
// foreground rights or attaches to another thread's input queue.
func (t Target) Restore(owner uintptr) error {
	if err := t.Validate(); err != nil {
		return err
	}
	if err := t.checkIntegrity(); err != nil {
		return err
	}
	foreground, _, _ := getForeground.Call()
	if foreground != 0 && foreground != owner && foreground != t.Window {
		return ErrForeground
	}
	iconic, _, _ := isIconic.Call(t.Window)
	if iconic != 0 {
		showWindow.Call(t.Window, 9) // SW_RESTORE
	}
	setForeground.Call(t.Window)
	foreground, _, _ = getForeground.Call()
	if foreground != t.Window {
		return ErrForeground
	}
	return nil
}

// Paste waits on actual modifier state, checking foreground and clipboard
// ownership throughout. It sends one bounded batch only after all guards pass.
func (t Target) Paste(ctx context.Context, sequence uint32) error {
	ticker := time.NewTicker(10 * time.Millisecond)
	defer ticker.Stop()
	for {
		if err := ctx.Err(); err != nil {
			return err
		}
		if err := t.Validate(); err != nil {
			return err
		}
		foreground, _, _ := getForeground.Call()
		if foreground != t.Window {
			return ErrForeground
		}
		if ClipboardSequence() != sequence {
			return ErrClipboard
		}
		if !modifiersHeld() {
			break
		}
		select {
		case <-ctx.Done():
			return ErrModifiers
		case <-ticker.C:
		}
	}
	if err := t.checkIntegrity(); err != nil {
		return err
	}
	// Recheck immediately before injection: neither a stale HWND nor an app
	// activated while the shortcut was being released may receive the paste.
	if err := t.Validate(); err != nil {
		return err
	}
	foreground, _, _ := getForeground.Call()
	if foreground != t.Window || modifiersHeld() {
		return ErrForeground
	}
	if ClipboardSequence() != sequence {
		return ErrClipboard
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	inputs := [4]keyboardInput{keyInput(0x11, false), keyInput(0x56, false), keyInput(0x56, true), keyInput(0x11, true)}
	count, _, callErr := sendInput.Call(uintptr(len(inputs)), uintptr(unsafe.Pointer(&inputs[0])), unsafe.Sizeof(inputs[0]))
	if count != uintptr(len(inputs)) {
		// A short write may have pressed Ctrl. A key-up cannot type text; do
		// not retry the text-producing keys into a possibly changed target.
		release := keyInput(0x11, true)
		sendInput.Call(1, uintptr(unsafe.Pointer(&release)), unsafe.Sizeof(release))
		return fmt.Errorf("send paste shortcut (%d/4): %w", count, callErr)
	}
	return nil
}

func modifiersHeld() bool {
	for _, key := range []uintptr{0x10, 0x11, 0x12, 0x5b, 0x5c} {
		state, _, _ := getKeyState.Call(key)
		if state&0x8000 != 0 {
			return true
		}
	}
	return false
}

// INPUT is 40 bytes on Windows/amd64: 8-byte tag/padding and a 32-byte union.
// The KEYBDINPUT member occupies the first 24 bytes of that union.
type keyboardInput struct {
	typeID uint32
	_      uint32
	key    uint16
	scan   uint16
	flags  uint32
	time   uint32
	_      uint32
	extra  uintptr
	_      [8]byte
}

func keyInput(key uint16, up bool) keyboardInput {
	input := keyboardInput{typeID: 1, key: key}
	if up {
		input.flags = 2
	}
	return input
}

func (t Target) checkIntegrity() error {
	target, err := integrityLevel(t.Process)
	if err != nil {
		return fmt.Errorf("read target integrity: %w", err)
	}
	current, err := integrityLevel(uint32(os.Getpid()))
	if err != nil {
		return fmt.Errorf("read preview integrity: %w", err)
	}
	if target > current {
		return ErrPermission
	}
	return nil
}

func integrityLevel(pid uint32) (uint64, error) {
	process, err := syscall.OpenProcess(0x1000, false, pid)
	if err != nil {
		return 0, err
	}
	defer syscall.CloseHandle(process)
	var token syscall.Token
	if err := syscall.OpenProcessToken(process, syscall.TOKEN_QUERY, &token); err != nil {
		return 0, err
	}
	defer token.Close()
	var size uint32
	err = syscall.GetTokenInformation(token, syscall.TokenIntegrityLevel, nil, 0, &size)
	if !errors.Is(err, syscall.ERROR_INSUFFICIENT_BUFFER) || size < uint32(unsafe.Sizeof(uintptr(0))) {
		return 0, errors.New("could not size integrity token")
	}
	buffer := make([]byte, size)
	if err := syscall.GetTokenInformation(token, syscall.TokenIntegrityLevel, &buffer[0], size, &size); err != nil {
		return 0, err
	}
	sid := *(**syscall.SID)(unsafe.Pointer(&buffer[0]))
	value, err := sid.String()
	runtime.KeepAlive(buffer)
	if err != nil {
		return 0, err
	}
	last := strings.LastIndex(value, "-")
	if last < 0 {
		return 0, errors.New("invalid integrity SID")
	}
	return strconv.ParseUint(value[last+1:], 10, 32)
}
