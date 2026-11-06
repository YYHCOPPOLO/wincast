package app

import (
	"context"
	"fmt"
	"log"
	"os"
	"slices"
	"sync"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const previewShortcut = "Alt+Shift+Space"

type controller struct {
	ctx           context.Context
	cancel        context.CancelFunc
	workers       sync.WaitGroup
	panel         *mygo.Window
	tray          *mygo.Tray
	view          *palette.View
	started       time.Time
	showRequested time.Time
	closing       bool
	menuOpen      bool
	scanning      bool
	target        windows.Target
	scale         float64
	icons         map[iconKey]*ui.Bitmap
	pendingIcons  map[iconKey]bool
	iconRequests  chan iconKey
}

func newController(started time.Time) *controller {
	ctx, cancel := context.WithCancel(context.Background())
	c := &controller{
		ctx: ctx, cancel: cancel, started: started, scale: 1,
		icons: make(map[iconKey]*ui.Bitmap), pendingIcons: make(map[iconKey]bool), iconRequests: make(chan iconKey, 32),
	}
	c.view = palette.New(windows.ChineseUI(), c.open, c.hide, c.menu, c.icon, func(elapsed time.Duration, count int) {
		log.Printf("filter elapsed_us=%d results=%d", elapsed.Microseconds(), count)
	})
	return c
}

func (c *controller) ready() {
	c.panel = mygo.NewWindow(mygo.WindowOptions{
		Title: "Tinycast Go Preview", Panel: true, Hidden: true, Frameless: true,
		Width: 680, Height: 420, DisableResize: true, BackgroundColor: "light-dark(#ffffff, #232327)",
		Content: ui.View(c.view.Render),
	})
	c.panel.OnClose(func(e *mygo.CloseEvent) {
		if !c.closing {
			e.PreventDefault()
			c.hide()
		}
	})
	c.panel.OnBlur(func() {
		if !c.closing && !c.menuOpen {
			c.hide()
		}
	})
	c.panel.OnFocus(func() {
		if !c.showRequested.IsZero() {
			log.Printf("focus elapsed_us=%d hwnd=%d", time.Since(c.showRequested).Microseconds(), c.panel.NativeHandle())
		}
	})
	mygo.Screen.OnDisplaysChanged(func() {
		c.logDisplays()
		if c.panel.IsVisible() {
			c.place()
		}
	})
	c.logDisplays()
	if err := mygo.GlobalShortcut.Register(previewShortcut, c.toggle); err != nil {
		log.Printf("shortcut unavailable: %v", err)
		c.view.Status = c.view.Text("热键被占用，请从托盘打开", "Shortcut unavailable; open from the tray")
	}
	if err := c.setupTray(); err != nil {
		log.Printf("tray unavailable: %v", err)
		c.view.Status = c.view.Text("无法创建托盘图标", "Could not create the tray icon")
	}
	c.startIcons()
	c.scan()
	if !slices.Contains(os.Args[1:], "--hidden") {
		c.show()
	}
	log.Printf("ready elapsed_us=%d hwnd=%d", time.Since(c.started).Microseconds(), c.panel.NativeHandle())
}

func (c *controller) logDisplays() {
	for _, display := range mygo.Screen.Displays() {
		log.Printf("display id=%d scale=%.2f bounds=%+v work=%+v primary=%t", display.ID, display.ScaleFactor, display.Bounds, display.WorkArea, display.Primary)
	}
}

func (c *controller) place() {
	display := mygo.Screen.DisplayNearestPoint(mygo.Screen.CursorScreenPoint())
	width, height := c.panel.Size()
	area := display.WorkArea
	x := area.X + max(0, (area.Width-width)/2)
	y := area.Y + max(0, (area.Height-height)/3)
	c.panel.SetPosition(x, y)
	c.scale = display.ScaleFactor
	if c.scale <= 0 {
		c.scale = 1
	}
}

func (c *controller) show() {
	if c.closing || c.panel == nil {
		return
	}
	c.showRequested = time.Now()
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
		c.view.Query = ""
		c.view.Filter()
	}
	c.place()
	c.view.Input.Focus()
	c.panel.Show()
	c.panel.Focus()
	log.Printf("show elapsed_us=%d visible=%t target_available=%t", time.Since(c.showRequested).Microseconds(), c.panel.IsVisible(), c.target.Window != 0)
}

func (c *controller) hide() {
	if c.panel != nil && c.panel.IsVisible() {
		started := time.Now()
		c.panel.Hide()
		log.Printf("hide elapsed_us=%d", time.Since(started).Microseconds())
	}
}

func (c *controller) toggle() {
	if c.closing {
		return
	}
	if c.panel.IsVisible() {
		c.hide()
		return
	}
	c.show()
}

func (c *controller) scan() {
	if c.scanning || c.closing {
		return
	}
	c.scanning, c.view.Loading = true, true
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		started := time.Now()
		apps, err := mygo.Shell.InstalledApps()
		if c.ctx.Err() != nil {
			return
		}
		c.panel.Update(func() {
			if c.closing {
				return
			}
			c.scanning, c.view.Loading = false, false
			if err != nil {
				c.view.Status = c.view.Text("无法读取应用程序目录", "Could not load applications")
				log.Printf("catalog error: %v", err)
				return
			}
			c.view.Apps = apps
			c.view.Filter()
			log.Printf("catalog count=%d elapsed_us=%d", len(apps), time.Since(started).Microseconds())
		})
	}()
}

func (c *controller) open(application mygo.InstalledApp) {
	c.hide()
	if err := mygo.Shell.LaunchApp(application); err != nil {
		log.Printf("launch failed: %v", err)
		c.view.Status = c.view.Text("无法打开应用程序", "Could not open application")
		c.show()
	}
}

func (c *controller) menu() {
	if c.closing || c.menuOpen {
		return
	}
	c.menuOpen = true
	defer func() {
		c.menuOpen = false
		if !c.closing && c.panel.IsVisible() {
			c.view.Input.Focus()
		}
	}()
	menu := mygo.NewMenu([]*mygo.MenuItem{
		{Label: c.view.Text("打开选中应用", "Open selected application"), Disabled: c.view.Chosen < 0, Click: func(*mygo.MenuItem, *mygo.Window) {
			if c.view.Chosen >= 0 && c.view.Chosen < len(c.view.Results) {
				c.open(c.view.Results[c.view.Chosen])
			}
		}},
		{Label: c.view.Text("刷新应用程序", "Refresh applications"), Click: func(*mygo.MenuItem, *mygo.Window) { c.scan() }},
		mygo.Separator(),
		{Label: c.view.Text("退出预览", "Quit Preview"), Click: func(*mygo.MenuItem, *mygo.Window) { mygo.App.Quit() }},
	})
	menu.Popup(c.panel)
}

func (c *controller) setupTray() error {
	tray, err := mygo.NewTray(mygo.TrayOptions{ID: previewID, Icon: trayIcon(), ToolTip: "Tinycast Go Preview"})
	if err != nil {
		return fmt.Errorf("create tray: %w", err)
	}
	c.tray = tray
	tray.OnClick(c.show)
	tray.SetMenu(mygo.NewMenu([]*mygo.MenuItem{
		{Label: c.view.Text("打开 Tinycast Go 预览", "Open Tinycast Go Preview"), Accelerator: previewShortcut, Click: func(*mygo.MenuItem, *mygo.Window) { c.show() }},
		mygo.Separator(),
		{Label: c.view.Text("退出预览", "Quit Preview"), Click: func(*mygo.MenuItem, *mygo.Window) { mygo.App.Quit() }},
	}))
	return nil
}

func (c *controller) stop() {
	if c.closing {
		return
	}
	c.closing = true
	c.cancel()
	mygo.GlobalShortcut.UnregisterAll()
	if c.tray != nil {
		c.tray.Destroy()
	}
	log.Print("shutdown requested")
}

func (c *controller) finish() {
	c.cancel()
	done := make(chan struct{})
	go func() { c.workers.Wait(); close(done) }()
	select {
	case <-done:
	case <-time.After(time.Second):
		log.Print("shutdown worker deadline exceeded")
	}
}
