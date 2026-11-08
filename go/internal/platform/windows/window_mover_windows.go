//go:build windows

package windows

import (
	"fmt"
	"syscall"
	"time"
	"unsafe"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowcommand"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowlayout"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowmemory"
)

var (
	dwmapi                = syscall.NewLazyDLL("dwmapi.dll")
	dwmGetWindowAttribute = dwmapi.NewProc("DwmGetWindowAttribute")
	getWindowRect         = user32.NewProc("GetWindowRect")
	isZoomed              = user32.NewProc("IsZoomed")
	setWindowPos          = user32.NewProc("SetWindowPos")
)

const (
	dwmwaExtendedFrameBounds = 9
	gwlStyle                 = uintptr(0xFFFFFFF0) // GWL_STYLE = -16
	wsThickFrame             = 0x00040000
	swRestore                = 9
	swMaximize               = 3
	hwndTop                  = 0
	swpNoSize                = 0x0001
	swpNoMove                = 0x0002
	swpNoZOrder              = 0x0004
	swpNoActivate            = 0x0010
)

type WindowFrames struct {
	Window  rect
	Visible rect
	Offset  frameOffset
}

type frameOffset struct {
	Left, Top, Right, Bottom int32
}

func (o frameOffset) apply(r rect) rect {
	return rect{Left: r.Left + o.Left, Top: r.Top + o.Top, Right: r.Right + o.Right, Bottom: r.Bottom + o.Bottom}
}

func WindowValid(hwnd uintptr) bool {
	if hwnd == 0 {
		return false
	}
	ok, _, _ := isWindow.Call(hwnd)
	return ok != 0
}

func readWindowFrames(hwnd uintptr) (WindowFrames, bool) {
	var window rect
	if ok, _, _ := getWindowRect.Call(hwnd, uintptr(unsafe.Pointer(&window))); ok == 0 {
		return WindowFrames{}, false
	}
	visible := window
	var dwm rect
	if hr, _, _ := dwmGetWindowAttribute.Call(hwnd, dwmwaExtendedFrameBounds, uintptr(unsafe.Pointer(&dwm)), unsafe.Sizeof(dwm)); int32(hr) == 0 && dwm.Right > dwm.Left && dwm.Bottom > dwm.Top {
		visible = dwm
	}
	return WindowFrames{
		Window: window, Visible: visible,
		Offset: frameOffset{
			Left: window.Left - visible.Left, Top: window.Top - visible.Top,
			Right: window.Right - visible.Right, Bottom: window.Bottom - visible.Bottom,
		},
	}, true
}

func screensDIP(screens []ScreenPixel) []windowlayout.Screen {
	dpi := PrimaryDPI(screens)
	out := make([]windowlayout.Screen, len(screens))
	for i, screen := range screens {
		x, y, w, h := PixelsToDIP(screen.Frame, dpi)
		wx, wy, ww, wh := PixelsToDIP(screen.Work, dpi)
		out[i] = windowlayout.Screen{
			ID: i, Frame: windowlayout.DipRect{X: x, Y: y, W: w, H: h},
			Visible: windowlayout.DipRect{X: wx, Y: wy, W: ww, H: wh},
		}
	}
	return out
}

func dipRect(px rect, dpi uint32) windowlayout.DipRect {
	x, y, w, h := PixelsToDIP(px, dpi)
	return windowlayout.DipRect{X: x, Y: y, W: w, H: h}
}

func pixelRect(value windowlayout.DipRect, dpi uint32) rect {
	return DIPToPixels(value.X, value.Y, value.W, value.H, dpi)
}

// ApplyWindowCommand moves or resizes hwnd. Space switching has no public
// enumerator on Windows and returns an explanatory message.
func ApplyWindowCommand(hwnd uintptr, id windowcommand.ID, gap float64, cycle bool, memory *windowmemory.Memory) (bool, string) {
	if !WindowValid(hwnd) {
		return false, ""
	}
	if id.Kind() == windowcommand.KindSpace {
		return false, "Switching spaces is not available on Windows."
	}
	if id.Kind() == windowcommand.KindFullscreen {
		return toggleFullscreen(hwnd), ""
	}
	if ok, _, _ := isIconic.Call(hwnd); ok != 0 {
		return false, ""
	}
	if ok, _, _ := isZoomed.Call(hwnd); ok != 0 {
		showWindow.Call(hwnd, swRestore)
	}
	frames, ok := readWindowFrames(hwnd)
	if !ok {
		return false, ""
	}
	screensPx := ScreensPixel()
	if len(screensPx) == 0 {
		return false, ""
	}
	dpi := PrimaryDPI(screensPx)
	screens := screensDIP(screensPx)
	window := dipRect(frames.Visible, dpi)
	host, ok := windowlayout.ScreenContaining(window, screens)
	if !ok {
		return false, ""
	}
	now := time.Now().UnixMilli()
	decision := memory.Decide(hwnd, id, window, host.ID, cycle, now)
	if id == windowcommand.Restore && !decision.CanRestore {
		return false, ""
	}
	restore := decision.Restore
	step := decision.Step
	if id == windowcommand.Restore {
		step = 0
	}
	var lastTile windowcommand.ID
	if decision.LastTile != "" {
		lastTile = decision.LastTile
	}
	placement, ok := windowlayout.Resolve(windowlayout.LayoutQuery{
		Command: id, Window: window, Screens: screens, Gap: gap, Step: step,
		Restore: &restore, LastTile: lastTile,
	})
	if !ok {
		return false, ""
	}
	if !positionSettable(hwnd) {
		return false, ""
	}
	targetVisible := pixelRect(placement.Frame, dpi)
	current, ok := readWindowFrames(hwnd)
	if !ok {
		return false, ""
	}
	canSize := placement.Resizes && sizeSettable(hwnd)
	writeVisible := targetVisible
	if !canSize {
		writeVisible = placePixelSize(current.Visible, targetVisible, placement.Anchor)
	}
	write := current.Offset.apply(writeVisible)
	original := current.Window
	if canSize {
		if !setRect(hwnd, write, true, false) {
			return false, ""
		}
	}
	if !setRect(hwnd, write, false, true) {
		if canSize {
			setRect(hwnd, original, true, false)
		}
		return false, ""
	}
	if canSize {
		setRect(hwnd, write, true, false)
	}
	applied := placement.Frame
	if updated, ok := readWindowFrames(hwnd); ok {
		applied = dipRect(updated.Visible, dpi)
	}
	memory.Commit(hwnd, id, decision, applied, placement.ScreenID, now)
	return true, ""
}

func placePixelSize(current, slot rect, anchor windowlayout.Anchor) rect {
	w := float64(current.width())
	h := float64(current.height())
	slotRect := windowlayout.DipRect{X: float64(slot.Left), Y: float64(slot.Top), W: float64(slot.width()), H: float64(slot.height())}
	placed := anchor.Place(w, h, slotRect)
	return rect{
		Left: int32(placed.X), Top: int32(placed.Y),
		Right: int32(placed.X + placed.W), Bottom: int32(placed.Y + placed.H),
	}
}

func positionSettable(hwnd uintptr) bool {
	visible, _, _ := isWindowVisible.Call(hwnd)
	iconic, _, _ := isIconic.Call(hwnd)
	return visible != 0 && iconic == 0
}

func sizeSettable(hwnd uintptr) bool {
	style, _, _ := getWindowLongW.Call(hwnd, gwlStyle)
	return uint32(style)&wsThickFrame != 0
}

func setRect(hwnd uintptr, value rect, size, position bool) bool {
	flags := uintptr(swpNoZOrder | swpNoActivate)
	if !size {
		flags |= swpNoSize
	}
	if !position {
		flags |= swpNoMove
	}
	ok, _, _ := setWindowPos.Call(hwnd, hwndTop, uintptr(value.Left), uintptr(value.Top),
		uintptr(value.width()), uintptr(value.height()), flags)
	return ok != 0
}

func toggleFullscreen(hwnd uintptr) bool {
	if ok, _, _ := isZoomed.Call(hwnd); ok != 0 {
		showWindow.Call(hwnd, swRestore)
		return true
	}
	showWindow.Call(hwnd, swMaximize)
	return true
}

// DescribeWindowTarget is for logs only; it must not be used as identity.
func DescribeWindowTarget(hwnd uintptr) string {
	return fmt.Sprintf("hwnd=0x%x valid=%t", hwnd, WindowValid(hwnd))
}
