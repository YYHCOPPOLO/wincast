package app

import (
	"log"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/egoist/mygo"
)

func showWindow(window *mygo.Window) {
	window.Show()
	// STARTUPINFO may request SW_HIDE, overriding the first ShowWindow even
	// when the app explicitly requested Show. A hidden-created panel has not
	// consumed that override yet. Retry once based on actual native visibility.
	if !window.IsVisible() {
		window.Show()
		log.Printf("window show retried visible=%t", window.IsVisible())
	}
	window.Focus()
	if !windows.ForceForeground(window.NativeHandle()) {
		log.Printf("foreground activation failed hwnd=%d", window.NativeHandle())
	}
}

// newMaterialWindow reserves a composited surface even when glass is disabled.
// Windows cannot add that surface later, so create it with a material and turn
// the material off before showing the window when the saved preference is off.
func (c *controller) newMaterialWindow(options mygo.WindowOptions) *mygo.Window {
	options.Transparent = true
	options.Vibrancy = mygo.VibrancyAcrylic
	window := mygo.NewWindow(options)
	if !c.glassBackground {
		window.SetVibrancy(mygo.VibrancyNone)
	}
	return window
}

func (c *controller) applyWindowMaterials() {
	material := mygo.VibrancyNone
	if c.glassBackground {
		material = mygo.VibrancyAcrylic
	}
	windows := []*mygo.Window{c.panel, c.settings, c.qaWindow, c.onboarding, c.supportWindow, c.diagnostic}
	if c.editor != nil {
		windows = append(windows, c.editor.window)
	}
	for _, window := range windows {
		if window != nil {
			window.SetVibrancy(material)
		}
	}
}
