//go:build windows && amd64

package windows

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"
	"unicode/utf16"
	"unsafe"
)

// syntheticExtra tags every key this application injects, so the keyword
// listener can ignore its own typing even when the low-level flag is lost.
const syntheticExtra uintptr = 0x5443_5354

const (
	keyEventUnicode = 0x0004
	keyEventKeyUp   = 0x0002
)

// sendInputs sends one keyboard batch. A short batch is reported instead of
// retried, so a changed foreground window never receives a replay.
func sendInputs(inputs []keyboardInput) error {
	if len(inputs) == 0 {
		return nil
	}
	count, _, callErr := sendInput.Call(uintptr(len(inputs)), uintptr(unsafe.Pointer(&inputs[0])), unsafe.Sizeof(inputs[0]))
	if count != uintptr(len(inputs)) {
		return fmt.Errorf("send input: %d/%d accepted: %v", count, len(inputs), callErr)
	}
	return nil
}

func taggedKeyInput(key, scan uint16, flags uint32) keyboardInput {
	return keyboardInput{typeID: 1, key: key, scan: scan, flags: flags, extra: syntheticExtra}
}

func sendVirtualKey(key uint16) error {
	return sendInputs([]keyboardInput{
		taggedKeyInput(key, 0, 0),
		taggedKeyInput(key, 0, keyEventKeyUp),
	})
}

// SendUnicodeText types text as Unicode keyboard events. The target must
// already be focused and validated by the caller.
func SendUnicodeText(text string) error {
	units := utf16.Encode([]rune(text))
	for _, unit := range units {
		if err := sendInputs([]keyboardInput{
			taggedKeyInput(0, unit, keyEventUnicode),
			taggedKeyInput(0, unit, keyEventUnicode|keyEventKeyUp),
		}); err != nil {
			return err
		}
	}
	return nil
}

// SendBackspaces deletes count characters in the focused control.
func SendBackspaces(count int) error {
	const vkBack = 0x08
	for range count {
		if err := sendVirtualKey(vkBack); err != nil {
			return err
		}
	}
	return nil
}

// SendLeftArrows moves the caret count grapheme clusters to the left.
func SendLeftArrows(count int) error {
	const vkLeft = 0x25
	for range count {
		if err := sendVirtualKey(vkLeft); err != nil {
			return err
		}
	}
	return nil
}

// CaptureSelection copies the target's selection with Ctrl+C, reads it and
// restores the previous plain-text clipboard when it is still intact. The
// caller hides its own panel first and suppresses clipboard capture.
func CaptureSelection(ctx context.Context, target Target, owner uintptr) (string, error) {
	if err := target.Validate(); err != nil {
		return "", err
	}
	if err := target.checkIntegrity(); err != nil {
		return "", err
	}
	previous, hadPrevious := ReadClipboardText()
	before := ClipboardSequence()
	if err := target.Restore(owner); err != nil {
		return "", err
	}
	time.Sleep(40 * time.Millisecond)
	if err := sendInputs([]keyboardInput{
		taggedKeyInput(0x11, 0, 0),
		taggedKeyInput(0x43, 0, 0),
		taggedKeyInput(0x43, 0, keyEventKeyUp),
		taggedKeyInput(0x11, 0, keyEventKeyUp),
	}); err != nil {
		return "", err
	}
	deadline := time.Now().Add(400 * time.Millisecond)
	copied := before
	for time.Now().Before(deadline) {
		if err := ctx.Err(); err != nil {
			return "", err
		}
		if ClipboardSequence() != before {
			copied = ClipboardSequence()
			break
		}
		time.Sleep(15 * time.Millisecond)
	}
	if copied == before {
		return "", errors.New("nothing was selected")
	}
	text, ok := ReadClipboardText()
	// Restore only while the clipboard still holds exactly what Ctrl+C wrote.
	if ClipboardSequence() == copied {
		if hadPrevious {
			_ = WriteClipboardText(0, previous, false)
		} else if ok {
			_ = WriteClipboardText(0, "", false)
		}
	}
	if !ok {
		return "", errors.New("the selection is not text")
	}
	text = strings.TrimSpace(text)
	if text == "" {
		return "", errors.New("the selection is empty")
	}
	return text, nil
}

// SyntheticTag reports whether an injected event belongs to this application.
func SyntheticTag(extra uintptr) bool { return extra == syntheticExtra }
