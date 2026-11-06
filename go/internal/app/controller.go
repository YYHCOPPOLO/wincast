package app

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"slices"
	"sync"
	"sync/atomic"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/diagnostics"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/launcher"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const previewShortcut = "Alt+Shift+Space"

type controller struct {
	ctx               context.Context
	cancel            context.CancelFunc
	workers           sync.WaitGroup
	panel             *mygo.Window
	tray              *mygo.Tray
	view              *palette.View
	started           time.Time
	showRequested     time.Time
	closing           bool
	menuOpen          bool
	scanning          bool
	target            windows.Target
	scale             float64
	icons             map[iconKey]*ui.Bitmap
	pendingIcons      map[iconKey]bool
	iconRequests      chan iconKey
	pasteCancel       context.CancelFunc
	pasteID           uint64
	profile           profile
	instance          *windows.Instance
	diagnostic        *mygo.Window
	settings          *mygo.Window
	settingsPath      string
	settingsDocument  *storage.Document
	settingsErr       error
	settingsWritable  bool
	settingsStatus    string
	settingsWrites    chan settingsWrite
	appearance        string
	language          string
	systemDark        bool
	messageHost       *windows.MessageHost
	clipboardEvents   atomic.Uint64
	editor            *editorWindow
	notes             storage.NoteRepository
	launcher          *launcher.Service
	launcherPrefs     *launcher.Preferences
	catalog           []domain.Application
	rankingWrites     chan []byte
	featureDictionary bool
	featureNotes      bool
	clipboardEnabled  atomic.Bool
	dictionaryReady   bool
	hotkey            string
	hotkeyDraft       string
	hotkeyRegistered  bool
	hotkeyOwned       string
	customDirsDraft   string
	compactMode       bool
	doubleTap         string
	doubleTapVK       atomic.Uint32
	doubleTapHeld     atomic.Bool
	lastDoubleTap     atomic.Int64
	keyboardInstalled bool
	snapshot          diagnostics.Snapshot
	cpuPercent        float64
	cpuMeasured       bool
	exeBytes          int64
	storeStatus       string
	dictionaryStatus  string
	catalogInfo       storage.CatalogInfo
	catalogRequests   chan []storage.Application
	lookupRequests    chan lookupRequest
	lookupCancel      context.CancelFunc
	lookupID          uint64
	lookupCanceled    uint64
	lookupElapsed     time.Duration
	lookupCount       uint64
	displaySummary    []string
}

func newController(started time.Time, profile profile) *controller {
	ctx, cancel := context.WithCancel(context.Background())
	c := &controller{
		ctx: ctx, cancel: cancel, started: started, scale: 1,
		icons: make(map[iconKey]*ui.Bitmap), pendingIcons: make(map[iconKey]bool), iconRequests: make(chan iconKey, 32),
		profile: profile, catalogRequests: make(chan []storage.Application, 1), lookupRequests: make(chan lookupRequest, 1),
		settingsWrites: make(chan settingsWrite, 1), rankingWrites: make(chan []byte, 1),
		systemDark: windows.SystemDark(), appearance: "system", language: defaultLanguage(),
		featureDictionary: true, featureNotes: true,
	}
	c.clipboardEnabled.Store(true)
	c.notes = storage.NewFileNotes(filepath.Join(profile.data, "notes"))
	c.loadSettings()
	preferences, preferencesErr := launcher.LoadPreferences(filepath.Join(profile.settings, "launcher"))
	if preferencesErr != nil {
		log.Printf("launcher preferences load failed: %v", preferencesErr)
	}
	c.launcherPrefs = preferences
	c.launcher = launcher.New(preferences)
	c.view = palette.New(c.language == "zh", palette.Callbacks{
		Open: c.open, Hide: c.hide, Menu: c.menu, CopyText: c.copyText, Icon: c.icon,
		Filtered: func(elapsed time.Duration, count int) {
			log.Printf("filter elapsed_us=%d results=%d", elapsed.Microseconds(), count)
		},
		Search:           c.search,
		Favorite:         c.openFavorite,
		FavoriteSlot:     c.launcherPrefs.FavoriteSlot,
		ToggleFavorite:   c.toggleFavorite,
		ToggleExpanded:   c.toggleExpanded,
		SaveAlias:        c.saveAlias,
		SetHidden:        c.setHidden,
		ToggleShowHidden: c.toggleShowHidden,
	})
	c.applyViewAppearance()
	c.view.Expanded = !c.compactMode
	c.view.DictionaryAvailable = c.featureDictionary && c.dictionaryReady
	c.view.OnLookup = c.lookup
	return c
}

func (c *controller) ready() {
	c.panel = mygo.NewWindow(mygo.WindowOptions{
		Title: "Tinycast Go Preview", Panel: true, Hidden: true, Frameless: true,
		Width: int(theme.PanelWidth), Height: c.panelHeight(), DisableResize: true,
		BackgroundColor: "light-dark(#ffffff, #232327)",
		Content:         ui.View(c.view.Render),
	})
	c.panel.OnClose(func(e *mygo.CloseEvent) {
		if !c.closing {
			e.PreventDefault()
			c.hide()
		}
	})
	c.panel.OnBlur(func() {
		if !c.closing && !c.menuOpen && !c.ownsForeground() {
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
	c.applyHotkey()
	if err := c.setupTray(); err != nil {
		log.Printf("tray unavailable: %v", err)
		c.view.Status = c.view.Text("无法创建托盘图标", "Could not create the tray icon")
	}
	c.startIcons()
	c.startStorage()
	c.startSettingsWriter()
	c.startRankingWriter()
	c.startClipboardHost()
	c.applyDoubleTap()
	c.scan()
	if slices.Contains(os.Args[1:], "--diagnostics") {
		c.openDiagnostics()
	} else if !slices.Contains(os.Args[1:], "--hidden") {
		c.show()
	}
	log.Printf("ready elapsed_us=%d hwnd=%d", time.Since(c.started).Microseconds(), c.panel.NativeHandle())
}

func (c *controller) logDisplays() {
	c.displaySummary = nil
	for _, display := range mygo.Screen.Displays() {
		log.Printf("display id=%d scale=%.2f bounds=%+v work=%+v primary=%t", display.ID, display.ScaleFactor, display.Bounds, display.WorkArea, display.Primary)
		c.displaySummary = append(c.displaySummary, fmt.Sprintf("%s · %.0f%% · %d×%d DIP · (%d,%d)", display.Label, display.ScaleFactor*100, display.Bounds.Width, display.Bounds.Height, display.Bounds.X, display.Bounds.Y))
	}
}

func (c *controller) place() {
	display := mygo.Screen.DisplayNearestPoint(mygo.Screen.CursorScreenPoint())
	width, height := c.panel.Size()
	area := display.WorkArea
	x := area.X + max(0, (area.Width-width)/2)
	y := area.Y + max(0, (area.Height-height)*18/100)
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
	if c.pasteCancel != nil {
		c.pasteCancel()
		c.pasteCancel = nil
		c.pasteID++
	}
	c.showRequested = time.Now()
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
		c.view.SetQuery("")
	}
	c.place()
	c.view.Input.Focus()
	showWindow(c.panel)
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
		custom := c.scanCustomApplications()
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
			c.launcher.SetApps(append(domainApplications(apps), custom...))
			c.catalog = c.launcher.Applications()
			c.view.Apps = c.catalog
			c.persistCatalog(c.catalog)
			c.view.Filter()
			log.Printf("catalog count=%d elapsed_us=%d", len(c.catalog), time.Since(started).Microseconds())
		})
	}()
}

func domainApplications(apps []mygo.InstalledApp) []domain.Application {
	converted := make([]domain.Application, len(apps))
	for i, app := range apps {
		converted[i] = domain.Application{ID: app.ID, Name: app.Name, Path: app.Path}
	}
	return converted
}

func (c *controller) open(application domain.Application) {
	if c.launcher != nil && c.view.Query != "" && application.ID != "" {
		c.launcher.Record(c.view.Query, application.ID, time.Now())
		c.queueRankingSave()
	}
	c.hide()
	if err := mygo.Shell.LaunchApp(mygo.InstalledApp{ID: application.ID, Name: application.Name, Path: application.Path}); err != nil {
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
			if c.panel.IsFocused() {
				c.view.Input.Focus()
			} else {
				// Blur during TrackPopupMenu is intentionally deferred, not
				// forgotten when the user switches to another window.
				c.hide()
			}
		}
	}()
	menu := components.Menu([]components.MenuAction{
		{Label: c.view.Text("打开选中应用", "Open selected application"), Disabled: c.view.Dictionary || c.view.Chosen < 0, Click: func() {
			if c.view.Chosen >= 0 && c.view.Chosen < len(c.view.Results) {
				c.open(c.view.Results[c.view.Chosen])
			}
		}},
		{Label: c.view.Text("刷新应用程序", "Refresh applications"), Click: c.scan},
		{Separator: true},
		{Label: c.favoriteLabel(), Accelerator: "Ctrl+D", Disabled: c.view.Dictionary || c.view.Chosen < 0, Click: c.toggleFavorite},
		{Label: c.view.Text("设置别名…", "Set alias…"), Disabled: c.view.Dictionary || c.view.Chosen < 0, Click: c.openAliasDialog},
		{Label: c.hideLabel(), Disabled: c.view.Dictionary || c.view.Chosen < 0, Click: c.toggleSelectedHidden},
		{Label: c.showHiddenLabel(), Click: c.toggleShowHidden},
		{Label: c.expandLabel(), Accelerator: "Ctrl+E", Click: c.toggleExpanded},
		{Separator: true},
		{Label: c.view.Text("复制文本 / 词条", "Copy text / entry"), Accelerator: "Ctrl+Shift+C", Disabled: c.view.Query == "", Click: func() { c.copyText(c.view.CopyText(), false) }},
		{Label: c.view.Text("粘回原窗口", "Paste to previous window"), Accelerator: "Ctrl+Shift+Enter", Disabled: c.view.Query == "", Click: func() { c.copyText(c.view.CopyText(), true) }},
		{Separator: true},
		{Label: c.view.Text("词典 / 应用程序", "Dictionary / Applications"), Disabled: !c.view.DictionaryAvailable, Click: c.view.ToggleDictionary},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("笔记…", "Notes…"), Disabled: !c.featureNotes, Click: c.openEditor},
		{Label: c.view.Text("诊断…", "Diagnostics…"), Click: c.openDiagnostics},
		{Separator: true},
		{Label: c.view.Text("退出预览", "Quit Preview"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}

func (c *controller) favoriteLabel() string {
	if application, ok := c.selected(); ok && c.launcherPrefs.IsFavorite(application.ID) {
		return c.view.Text("取消收藏", "Remove from favorites")
	}
	return c.view.Text("加入收藏", "Add to favorites")
}

func (c *controller) hideLabel() string {
	if application, ok := c.selected(); ok && c.launcherPrefs.IsHidden(application.ID) {
		return c.view.Text("显示此应用", "Show this application")
	}
	return c.view.Text("隐藏此应用", "Hide this application")
}

func (c *controller) toggleSelectedHidden() {
	application, ok := c.selected()
	if !ok {
		return
	}
	c.setHidden(application, !c.launcherPrefs.IsHidden(application.ID))
}

func (c *controller) showHiddenLabel() string {
	if c.view.ShowHidden {
		return c.view.Text("只显示可见应用", "Only show visible applications")
	}
	return c.view.Text("显示隐藏应用", "Show hidden applications")
}

func (c *controller) expandLabel() string {
	if c.view.Expanded {
		return c.view.Text("精简面板", "Compact panel")
	}
	return c.view.Text("展开面板", "Expand panel")
}

func (c *controller) ownsForeground() bool {
	foreground := windows.ForegroundWindow()
	if foreground == 0 {
		return false
	}
	if c.settings != nil && foreground == c.settings.NativeHandle() {
		return true
	}
	if c.editor != nil && c.editor.window != nil && foreground == c.editor.window.NativeHandle() {
		return true
	}
	if c.diagnostic != nil && foreground == c.diagnostic.NativeHandle() {
		return true
	}
	return false
}

func (c *controller) setupTray() error {
	tray, err := mygo.NewTray(mygo.TrayOptions{ID: previewID, Icon: trayIcon(), ToolTip: "Tinycast Go Preview"})
	if err != nil {
		return fmt.Errorf("create tray: %w", err)
	}
	c.tray = tray
	tray.OnClick(c.show)
	tray.SetMenu(components.Menu([]components.MenuAction{
		{Label: c.view.Text("打开 Tinycast Go 预览", "Open Tinycast Go Preview"), Accelerator: previewShortcut, Click: c.show},
		{Separator: true},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("笔记…", "Notes…"), Click: c.openEditor},
		{Label: c.view.Text("诊断…", "Diagnostics…"), Click: c.openDiagnostics},
		{Separator: true},
		{Label: c.view.Text("退出预览", "Quit Preview"), Click: mygo.App.Quit},
	}))
	return nil
}

func (c *controller) stop() {
	if c.closing {
		return
	}
	c.closing = true
	c.cancel()
	c.instance.StopServing()
	mygo.GlobalShortcut.UnregisterAll()
	if c.messageHost != nil {
		c.messageHost.Close()
		c.messageHost = nil
		c.keyboardInstalled = false
	}
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
		c.instance.HoldIdentityUntilExit()
		log.Print("shutdown worker deadline exceeded")
	}
}
