//go:build windows

package windows

import (
	"math"
	"sync"
	"syscall"
	"unsafe"
)

var (
	enumDisplayMonitors = user32.NewProc("EnumDisplayMonitors")
	getMonitorInfo      = user32.NewProc("GetMonitorInfoW")
	enumMonitorProc     = syscall.NewCallback(enumMonitorCallback)
	monitorMU           sync.Mutex
	monitorScreens      []ScreenPixel
)

type rect struct {
	Left, Top, Right, Bottom int32
}

func (r rect) width() int32  { return r.Right - r.Left }
func (r rect) height() int32 { return r.Bottom - r.Top }

type monitorInfo struct {
	CbSize  uint32
	Monitor rect
	Work    rect
	Flags   uint32
}

type ScreenPixel struct {
	Frame   rect
	Work    rect
	DPI     uint32
	Primary bool
}

// ScreensPixel enumerates monitors in physical pixels. DPI is the effective
// per-monitor DPI when the process advertises awareness, otherwise 96.
func ScreensPixel() []ScreenPixel {
	monitorMU.Lock()
	monitorScreens = nil
	monitorMU.Unlock()
	enumDisplayMonitors.Call(0, 0, enumMonitorProc, 0)
	monitorMU.Lock()
	out := append([]ScreenPixel(nil), monitorScreens...)
	monitorMU.Unlock()
	return out
}

func enumMonitorCallback(monitor, _ uintptr, _ uintptr, _ uintptr) uintptr {
	info := monitorInfo{}
	info.CbSize = uint32(unsafe.Sizeof(info))
	if ok, _, _ := getMonitorInfo.Call(monitor, uintptr(unsafe.Pointer(&info))); ok == 0 {
		return 1
	}
	dpi := monitorDPI(monitor)
	monitorMU.Lock()
	monitorScreens = append(monitorScreens, ScreenPixel{
		Frame: info.Monitor, Work: info.Work, DPI: dpi, Primary: info.Flags&1 != 0,
	})
	monitorMU.Unlock()
	return 1
}

func monitorDPI(monitor uintptr) uint32 {
	shcore := syscall.NewLazyDLL("shcore.dll")
	getDPI := shcore.NewProc("GetDpiForMonitor")
	var x, y uint32
	const mdtEffectiveDPI = 0
	if hr, _, _ := getDPI.Call(monitor, mdtEffectiveDPI, uintptr(unsafe.Pointer(&x)), uintptr(unsafe.Pointer(&y))); int32(hr) < 0 || x == 0 {
		return 96
	}
	return x
}

// PrimaryDPI is the DPI used for the shared DIP coordinate space.
func PrimaryDPI(screens []ScreenPixel) uint32 {
	for _, screen := range screens {
		if screen.Primary {
			if screen.DPI == 0 {
				return 96
			}
			return screen.DPI
		}
	}
	if len(screens) > 0 && screens[0].DPI != 0 {
		return screens[0].DPI
	}
	return 96
}

func ScaleFromDPI(dpi uint32) float64 {
	if dpi == 0 {
		dpi = 96
	}
	return float64(dpi) / 96
}

func PixelsToDIP(r rect, dpi uint32) (x, y, w, h float64) {
	scale := 1 / ScaleFromDPI(dpi)
	return float64(r.Left) * scale, float64(r.Top) * scale,
		float64(r.width()) * scale, float64(r.height()) * scale
}

func DIPToPixels(x, y, w, h float64, dpi uint32) rect {
	scale := ScaleFromDPI(dpi)
	rounded := func(v float64) int32 { return int32(math.Round(v)) }
	return rect{
		Left: rounded(x * scale), Top: rounded(y * scale),
		Right: rounded((x + w) * scale), Bottom: rounded((y + h) * scale),
	}
}

func DIPScalarToPixels(value float64, dpi uint32) int32 {
	return int32(math.Round(value * ScaleFromDPI(dpi)))
}
