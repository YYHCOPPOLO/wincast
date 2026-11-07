//go:build windows && amd64

package windows

import (
	"bytes"
	"errors"
	"fmt"
	"image"
	"image/png"
	"runtime"
	"sync"
	"syscall"
	"time"
	"unsafe"
)

// The clipboard is a shared, transient resource. Every read is bounded and
// retried briefly because another process may own it while Explorer is
// updating the format list. Writes stamp a private marker so the capture path
// can distinguish a copy the user made here from one made in another app.
const (
	cfUnicodeText = 13
	cfDIB         = 8
	cfDIBV5       = 17

	gmemMoveable = 0x0002

	clipboardOpenAttempts = 8
	clipboardOpenSleep    = 8 * time.Millisecond
	maxClipboardTextUnits = 64 * 1024
	maxClipboardImageSize = 64 * 1024 * 1024
	maxDIBDimension       = 8192
)

var (
	user32Clipboard = syscall.NewLazyDLL("user32.dll")
	kernelClipboard = syscall.NewLazyDLL("kernel32.dll")

	openClipboard           = user32Clipboard.NewProc("OpenClipboard")
	closeClipboard          = user32Clipboard.NewProc("CloseClipboard")
	emptyClipboard          = user32Clipboard.NewProc("EmptyClipboard")
	getClipboardData        = user32Clipboard.NewProc("GetClipboardData")
	setClipboardData        = user32Clipboard.NewProc("SetClipboardData")
	isClipboardFormatAvail  = user32Clipboard.NewProc("IsClipboardFormatAvailable")
	registerClipboardFormat = user32Clipboard.NewProc("RegisterClipboardFormatW")
	globalAlloc             = kernelClipboard.NewProc("GlobalAlloc")
	globalLock              = kernelClipboard.NewProc("GlobalLock")
	globalUnlock            = kernelClipboard.NewProc("GlobalUnlock")
	globalSize              = kernelClipboard.NewProc("GlobalSize")
	globalFree              = kernelClipboard.NewProc("GlobalFree")

	clipboardFormatOnce sync.Once
	clipboardAccess     sync.Mutex
	clipboardInternalF  uint32
	clipboardPNGF       uint32
	clipboardPNGMimeF   uint32
)

// ClipboardImage is one clipboard image payload. PNG data is already encoded;
// DIB data needs conversion and is kept as the original bytes so the worker
// does not hold a clipboard handle after the block is closed.
type ClipboardImage struct {
	PNG []byte
	DIB []byte
}

func clipboardFormats() (internal, pngFormat, mimeFormat uint32) {
	clipboardFormatOnce.Do(func() {
		clipboardInternalF = registerFormat("com.tinycast.win.internal")
		clipboardPNGF = registerFormat("PNG")
		clipboardPNGMimeF = registerFormat("image/png")
	})
	return clipboardInternalF, clipboardPNGF, clipboardPNGMimeF
}

func registerFormat(name string) uint32 {
	ptr, err := syscall.UTF16PtrFromString(name)
	if err != nil {
		return 0
	}
	value, _, _ := registerClipboardFormat.Call(uintptr(unsafe.Pointer(ptr)))
	return uint32(value)
}

func openClipboardRetry(owner uintptr) error {
	var lastErr error
	for attempt := 0; attempt < clipboardOpenAttempts; attempt++ {
		ok, _, callErr := openClipboard.Call(owner)
		if ok != 0 {
			return nil
		}
		lastErr = callErr
		if attempt+1 < clipboardOpenAttempts {
			time.Sleep(clipboardOpenSleep)
		}
	}
	if lastErr == nil {
		lastErr = errors.New("clipboard is busy")
	}
	return lastErr
}

func closeClipboardQuietly() {
	closeClipboard.Call()
}

// globalPointer reinterprets the address GlobalLock returned. The value is
// used immediately while the block stays locked, so the uintptr cannot become
// stale; the intermediate pointer avoids vet's raw uintptr conversion warning.
func globalPointer(value uintptr) unsafe.Pointer {
	return *(*unsafe.Pointer)(unsafe.Pointer(&value))
}

// ClipboardHasInternalMarker reports whether the current clipboard contents
// were written by this application. The capture path must ignore those.
func ClipboardHasInternalMarker() bool {
	internal, _, _ := clipboardFormats()
	if internal == 0 {
		return false
	}
	ok, _, _ := isClipboardFormatAvail.Call(uintptr(internal))
	return ok != 0
}

// ReadClipboardText returns bounded UTF-16 clipboard text. It never leaves the
// clipboard open when a handle or allocation is malformed.
func ReadClipboardText() (string, bool) {
	clipboardAccess.Lock()
	defer clipboardAccess.Unlock()
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if openClipboardRetry(0) != nil {
		return "", false
	}
	defer closeClipboardQuietly()
	ok, _, _ := isClipboardFormatAvail.Call(cfUnicodeText)
	if ok == 0 {
		return "", false
	}
	handle, _, _ := getClipboardData.Call(cfUnicodeText)
	if handle == 0 {
		return "", false
	}
	return readGlobalText(handle)
}

func readGlobalText(handle uintptr) (string, bool) {
	size, _, _ := globalSize.Call(handle)
	if size < 2 || size%2 != 0 {
		return "", false
	}
	if size > uintptr((maxClipboardTextUnits+1)*2) {
		size = uintptr((maxClipboardTextUnits + 1) * 2)
	}
	ptr, _, _ := globalLock.Call(handle)
	if ptr == 0 {
		return "", false
	}
	defer globalUnlock.Call(handle)
	raw := unsafe.Slice((*uint16)(globalPointer(ptr)), int(size/2))
	end := -1
	for i, unit := range raw {
		if unit == 0 {
			end = i
			break
		}
	}
	if end < 0 {
		return "", false
	}
	return syscall.UTF16ToString(raw[:end]), true
}

// ReadClipboardImage returns PNG data when the clipboard already carries it,
// otherwise the raw DIB/DIBV5 block for conversion off the UI thread.
func ReadClipboardImage() (ClipboardImage, bool) {
	clipboardAccess.Lock()
	defer clipboardAccess.Unlock()
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if openClipboardRetry(0) != nil {
		return ClipboardImage{}, false
	}
	defer closeClipboardQuietly()
	_, pngFormat, mimeFormat := clipboardFormats()
	for _, format := range []uint32{pngFormat, mimeFormat} {
		if format == 0 {
			continue
		}
		ok, _, _ := isClipboardFormatAvail.Call(uintptr(format))
		if ok == 0 {
			continue
		}
		if data, ok := readGlobalBytes(format); ok && len(data) >= 8 &&
			bytes.HasPrefix(data, []byte{0x89, 'P', 'N', 'G'}) {
			return ClipboardImage{PNG: data}, true
		}
	}
	for _, format := range []uint32{cfDIBV5, cfDIB} {
		ok, _, _ := isClipboardFormatAvail.Call(uintptr(format))
		if ok == 0 {
			continue
		}
		if data, ok := readGlobalBytes(format); ok {
			return ClipboardImage{DIB: data}, true
		}
	}
	return ClipboardImage{}, false
}

func readGlobalBytes(format uint32) ([]byte, bool) {
	handle, _, _ := getClipboardData.Call(uintptr(format))
	if handle == 0 {
		return nil, false
	}
	size, _, _ := globalSize.Call(handle)
	if size == 0 || size > maxClipboardImageSize {
		return nil, false
	}
	ptr, _, _ := globalLock.Call(handle)
	if ptr == 0 {
		return nil, false
	}
	defer globalUnlock.Call(handle)
	data := make([]byte, int(size))
	copy(data, unsafe.Slice((*byte)(globalPointer(ptr)), int(size)))
	return data, true
}

// EncodeClipboardImage converts a DIB to PNG. PNG payloads pass through.
func EncodeClipboardImage(imageBytes ClipboardImage) ([]byte, bool) {
	if len(imageBytes.PNG) > 0 {
		return imageBytes.PNG, true
	}
	if len(imageBytes.DIB) == 0 {
		return nil, false
	}
	return dibToPNG(imageBytes.DIB)
}

func dibToPNG(data []byte) ([]byte, bool) {
	width, height, rgba, ok := dibToRGBA(data)
	if !ok {
		return nil, false
	}
	img := &image.NRGBA{Pix: rgba, Stride: width * 4, Rect: image.Rect(0, 0, width, height)}
	var buffer bytes.Buffer
	if err := png.Encode(&buffer, img); err != nil {
		return nil, false
	}
	return buffer.Bytes(), true
}

func dibToRGBA(data []byte) (int, int, []byte, bool) {
	if len(data) < 40 || len(data) > maxClipboardImageSize {
		return 0, 0, nil, false
	}
	readU32 := func(offset int) (uint32, bool) {
		if offset < 0 || offset+4 > len(data) {
			return 0, false
		}
		return uint32(data[offset]) | uint32(data[offset+1])<<8 |
			uint32(data[offset+2])<<16 | uint32(data[offset+3])<<24, true
	}
	readI32 := func(offset int) (int32, bool) {
		value, ok := readU32(offset)
		return int32(value), ok
	}
	headerSize, ok := readU32(0)
	if !ok || (headerSize != 40 && headerSize != 52 && headerSize != 56 &&
		headerSize != 108 && headerSize != 124) || uint64(headerSize) > uint64(len(data)) {
		return 0, 0, nil, false
	}
	width, ok := readI32(4)
	if !ok {
		return 0, 0, nil, false
	}
	heightSigned, ok := readI32(8)
	if !ok || width <= 0 || heightSigned == 0 {
		return 0, 0, nil, false
	}
	planes, ok := readU32(12)
	if !ok {
		return 0, 0, nil, false
	}
	bpp, ok := readU32(14)
	if !ok {
		return 0, 0, nil, false
	}
	compression, ok := readU32(16)
	if !ok {
		return 0, 0, nil, false
	}
	if planes != 1 {
		return 0, 0, nil, false
	}
	if !((compression == 0 && (bpp == 24 || bpp == 32)) ||
		(compression == 3 && (bpp == 16 || bpp == 32))) {
		return 0, 0, nil, false
	}
	if headerSize == 124 {
		profileData, _ := readU32(112)
		profileSize, _ := readU32(116)
		signature, _ := readU32(56)
		if profileData != 0 || profileSize != 0 || signature == 0x4c494e4b || signature == 0x4d424544 {
			return 0, 0, nil, false
		}
	}
	height := int(heightSigned)
	topDown := height < 0
	if height < 0 {
		height = -height
	}
	if width > maxDIBDimension || height > maxDIBDimension {
		return 0, 0, nil, false
	}
	widthInt := int(width)
	rgbaLen := widthInt * height * 4
	if rgbaLen <= 0 || rgbaLen > maxClipboardImageSize {
		return 0, 0, nil, false
	}
	offset := int(headerSize)
	masks := [4]uint32{0x00ff0000, 0x0000ff00, 0x000000ff, 0}
	if compression == 3 {
		red, ok1 := readU32(40)
		green, ok2 := readU32(44)
		blue, ok3 := readU32(48)
		if !ok1 || !ok2 || !ok3 {
			return 0, 0, nil, false
		}
		masks[0], masks[1], masks[2] = red, green, blue
		if headerSize == 40 {
			offset += 12
		} else if headerSize >= 56 {
			alpha, ok := readU32(52)
			if !ok {
				return 0, 0, nil, false
			}
			masks[3] = alpha
		}
	}
	var used uint32
	for _, mask := range masks {
		if mask&used != 0 {
			return 0, 0, nil, false
		}
		used |= mask
	}
	redChannel, ok := newDIBChannel(masks[0], int(bpp))
	if !ok {
		return 0, 0, nil, false
	}
	greenChannel, ok := newDIBChannel(masks[1], int(bpp))
	if !ok {
		return 0, 0, nil, false
	}
	blueChannel, ok := newDIBChannel(masks[2], int(bpp))
	if !ok {
		return 0, 0, nil, false
	}
	var alphaChannel *dibChannel
	if masks[3] != 0 {
		channel, ok := newDIBChannel(masks[3], int(bpp))
		if !ok {
			return 0, 0, nil, false
		}
		alphaChannel = &channel
	}
	colorsUsed, ok := readU32(32)
	if !ok || uint64(colorsUsed) > uint64(1)<<bpp {
		return 0, 0, nil, false
	}
	offset += int(colorsUsed) * 4
	rowBytes := (widthInt*int(bpp) + 31) / 32 * 4
	if rowBytes <= 0 {
		return 0, 0, nil, false
	}
	pixelBytes := rowBytes * height
	if pixelBytes <= 0 {
		return 0, 0, nil, false
	}
	declared, ok := readU32(20)
	if !ok {
		return 0, 0, nil, false
	}
	declaredBytes := int(declared)
	if declaredBytes != 0 && declaredBytes < pixelBytes {
		return 0, 0, nil, false
	}
	needed := offset + pixelBytes
	if declaredBytes > pixelBytes {
		needed = offset + declaredBytes
	}
	if needed > len(data) {
		return 0, 0, nil, false
	}
	bytesPerPixel := int(bpp) / 8
	rgba := make([]byte, rgbaLen)
	for y := 0; y < height; y++ {
		sourceY := y
		if !topDown {
			sourceY = height - 1 - y
		}
		start := offset + sourceY*rowBytes
		row := data[start : start+rowBytes]
		for x := 0; x < widthInt; x++ {
			base := x * bytesPerPixel
			var pixel uint32
			for i := 0; i < bytesPerPixel; i++ {
				pixel |= uint32(row[base+i]) << (8 * i)
			}
			target := (y*widthInt + x) * 4
			rgba[target] = redChannel.extract(pixel)
			rgba[target+1] = greenChannel.extract(pixel)
			rgba[target+2] = blueChannel.extract(pixel)
			if alphaChannel != nil {
				rgba[target+3] = alphaChannel.extract(pixel)
			} else {
				rgba[target+3] = 255
			}
		}
	}
	return widthInt, height, rgba, true
}

type dibChannel struct {
	mask  uint32
	shift uint
	max   uint32
}

func newDIBChannel(mask uint32, bpp int) (dibChannel, bool) {
	if mask == 0 || (bpp < 32 && mask>>uint(bpp) != 0) {
		return dibChannel{}, false
	}
	shift := uint(0)
	for mask&(1<<shift) == 0 {
		shift++
	}
	maximum := mask >> shift
	if maximum&(maximum+1) != 0 {
		return dibChannel{}, false
	}
	return dibChannel{mask: mask, shift: shift, max: maximum}, true
}

func (c dibChannel) extract(pixel uint32) byte {
	if c.max == 0 {
		return 0
	}
	value := uint64((pixel & c.mask) >> c.shift)
	return byte((value*255 + uint64(c.max)/2) / uint64(c.max))
}

// WriteClipboardText replaces the clipboard with text. When marked, the
// private marker makes this app's own capture path ignore the update.
func WriteClipboardText(owner uintptr, text string, marked bool) error {
	units, err := syscall.UTF16FromString(text)
	if err != nil {
		return fmt.Errorf("encode clipboard text: %w", err)
	}
	return writeClipboardBytes(owner, cfUnicodeText, uintptr(len(units))*2, func(ptr unsafe.Pointer) {
		target := unsafe.Slice((*uint16)(ptr), len(units))
		copy(target, units)
	}, marked)
}

// WriteClipboardPNG replaces the clipboard with PNG bytes.
func WriteClipboardPNG(owner uintptr, data []byte, marked bool) error {
	if len(data) == 0 {
		return errors.New("empty png")
	}
	_, format, _ := clipboardFormats()
	if format == 0 {
		return errors.New("PNG clipboard format is unavailable")
	}
	return writeClipboardBytes(owner, format, uintptr(len(data)), func(ptr unsafe.Pointer) {
		copy(unsafe.Slice((*byte)(ptr), len(data)), data)
	}, marked)
}

func writeClipboardBytes(owner uintptr, format uint32, size uintptr, fill func(unsafe.Pointer), marked bool) error {
	if owner == 0 {
		return errors.New("clipboard write requires an application window")
	}
	var pid uint32
	getWindowProcess.Call(owner, uintptr(unsafe.Pointer(&pid)))
	if pid != uint32(syscall.Getpid()) {
		return errors.New("clipboard owner is not an application window")
	}
	clipboardAccess.Lock()
	defer clipboardAccess.Unlock()
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	// Prepare the payload before clearing the user's current clipboard.
	handle, _, callErr := globalAlloc.Call(gmemMoveable, size)
	if handle == 0 {
		return fmt.Errorf("allocate clipboard block: %w", callErr)
	}
	ptr, _, _ := globalLock.Call(handle)
	if ptr == 0 {
		globalFree.Call(handle)
		return errors.New("lock clipboard block")
	}
	fill(globalPointer(ptr))
	globalUnlock.Call(handle)
	if err := openClipboardRetry(owner); err != nil {
		globalFree.Call(handle)
		return fmt.Errorf("open clipboard for writing: %w", err)
	}
	defer closeClipboardQuietly()
	if ok, _, err := emptyClipboard.Call(); ok == 0 {
		globalFree.Call(handle)
		return fmt.Errorf("empty clipboard: %w", err)
	}
	ok, _, callErr := setClipboardData.Call(uintptr(format), handle)
	if ok == 0 {
		globalFree.Call(handle)
		return fmt.Errorf("set clipboard data: %w", callErr)
	}
	// The clipboard owns the handle now; GlobalFree would be a double free.
	if marked {
		stampClipboardMarker()
	}
	return nil
}

func stampClipboardMarker() {
	internal, _, _ := clipboardFormats()
	if internal == 0 {
		return
	}
	handle, _, _ := globalAlloc.Call(gmemMoveable, 4)
	if handle == 0 {
		return
	}
	ptr, _, _ := globalLock.Call(handle)
	if ptr == 0 {
		globalFree.Call(handle)
		return
	}
	*(*uint32)(globalPointer(ptr)) = 1
	globalUnlock.Call(handle)
	ok, _, _ := setClipboardData.Call(uintptr(internal), handle)
	if ok == 0 {
		globalFree.Call(handle)
	}
}

// ForegroundProcessStem returns the executable stem of the foreground window,
// used for clipboard source exclusion. Explorer and this process return "".
func ForegroundProcessStem() string {
	hwnd := ForegroundWindow()
	if hwnd == 0 {
		return ""
	}
	var pid uint32
	getWindowProcess.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	if pid == 0 || pid == uint32(syscall.Getpid()) {
		return ""
	}
	handle, err := syscall.OpenProcess(0x1000, false, pid)
	if err != nil {
		return ""
	}
	defer syscall.CloseHandle(handle)
	buffer := make([]uint16, 1024)
	size := uint32(len(buffer))
	queryFullProcessImageName := kernel32.NewProc("QueryFullProcessImageNameW")
	ok, _, _ := queryFullProcessImageName.Call(uintptr(handle), 0,
		uintptr(unsafe.Pointer(&buffer[0])), uintptr(unsafe.Pointer(&size)))
	if ok == 0 || size == 0 {
		return ""
	}
	path := syscall.UTF16ToString(buffer[:size])
	base := path
	for i := len(path) - 1; i >= 0; i-- {
		if path[i] == '\\' || path[i] == '/' {
			base = path[i+1:]
			break
		}
	}
	for i := len(base) - 1; i >= 0; i-- {
		if base[i] == '.' {
			return base[:i]
		}
	}
	return base
}
