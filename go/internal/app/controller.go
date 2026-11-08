package app

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/diagnostics"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	domaincalc "github.com/YYHCOPPOLO/wincast/go/internal/domain/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/emoji"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/systemaction"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/systemsettings"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/windowcommand"
	aifeature "github.com/YYHCOPPOLO/wincast/go/internal/features/ai"
	calcservice "github.com/YYHCOPPOLO/wincast/go/internal/features/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/calendar"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/commands"
	files "github.com/YYHCOPPOLO/wincast/go/internal/features/filesearch"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/launcher"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/migration"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/quickactions"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/quicklinks"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/settings"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/snippets"
	uninstallfeature "github.com/YYHCOPPOLO/wincast/go/internal/features/uninstall"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/windowmanagement"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/editors"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const defaultShortcut = "Alt+Space"

type controller struct {
	ctx                          context.Context
	cancel                       context.CancelFunc
	workers                      sync.WaitGroup
	panel                        *mygo.Window
	tray                         *mygo.Tray
	view                         *palette.View
	started                      time.Time
	showRequested                time.Time
	closing                      bool
	menuOpen                     bool
	panelSizePending             bool
	scanning                     bool
	target                       windows.Target
	scale                        float64
	icons                        map[iconKey]*ui.Bitmap
	pendingIcons                 map[iconKey]bool
	iconRequests                 chan iconKey
	pasteCancel                  context.CancelFunc
	pasteID                      uint64
	profile                      profile
	instance                     *windows.Instance
	diagnostic                   *mygo.Window
	settings                     *mygo.Window
	onboarding                   *mygo.Window
	onboardingStep               int
	supportWindow                *mygo.Window
	supportAskedAt               int64
	supportFirstSeenAt           int64
	supportLoaded                bool
	settingsScroll               ui.ScrollState
	settingsTab                  settings.Tab
	settingsFilter               string
	settingsAliasID              string
	settingsAliasDraft           string
	settingsStatus               string
	settingsConfirmRanking       bool
	settingsPath                 string
	settingsDocument             *storage.Document
	settingsErr                  error
	settingsWritable             bool
	openOnCursorScreen           bool
	launchAtLogin                bool
	showInTray                   bool
	popToRootTimeout             int64
	showFavoritesInCompact       bool
	paletteDraggable             bool
	supportReminders             bool
	extensionsEnabled            bool
	extensionsShowInLauncher     bool
	hiddenAt                     time.Time
	backupStatus                 string
	backupBusy                   bool
	raycastPassphrase            string
	raycastPath                  string
	raycastStatus                string
	backupSettings               bool
	backupClipboard              bool
	backupSnippets               bool
	backupNotes                  bool
	backupLearning               bool
	migrationSource              string
	migrationPreview             *migration.Preview
	migrationStatus              string
	migrationBusy                bool
	migrationSnapshots           []migration.Snapshot
	migrationSnapshotsLoaded     bool
	migrationRestoreConfirm      string
	settingsWrites               chan settingsWrite
	appearance                   string
	language                     string
	systemDark                   bool
	messageHost                  *windows.MessageHost
	clipboard                    *clipboard.Manager
	clipboardReady               bool
	clipboardConfirm             bool
	clipboardError               string
	clipboardFilter              clipboard.Filter
	clipboardRequests            chan struct{}
	clipboardRetention           int64
	clipboardDisabled            []string
	clipboardDisabledDraft       string
	clipboardImages              map[string]*ui.Bitmap
	clipboardImageBytes          int64
	clipboardPending             map[string]bool
	clipboardImageRequests       chan clipboardImageRequest
	clipboardEvents              atomic.Uint64
	editor                       *notesWindow
	notes                        storage.NoteRepository
	notesRoot                    string
	notesDeleteConfirm           bool
	formModel                    *editors.Model
	workflowDeleteConfirm        bool
	snippetConsent               bool
	itemHotkeys                  map[string]bool
	launcher                     *launcher.Service
	launcherPrefs                *launcher.Preferences
	catalog                      []domain.Application
	rankingWrites                chan []byte
	featureDictionary            bool
	applicationsShowInLauncher   bool
	systemSettingsShowInLauncher bool
	systemActionsShowInLauncher  bool
	featureNotes                 bool
	clipboardEnabled             atomic.Bool
	dictionaryReady              bool
	hotkey                       string
	hotkeyDraft                  string
	hotkeyRegistered             bool
	hotkeyOwned                  string
	clipboardHotkeys             bool
	customDirsDraft              string
	compactMode                  bool
	doubleTap                    string
	doubleTapVK                  atomic.Uint32
	doubleTapHeld                atomic.Bool
	lastDoubleTap                atomic.Int64
	lastToggle                   atomic.Int64
	doubleTapStop                func()
	keywordStop                  func()
	expander                     *snippets.Expander
	snippetsEnabled              atomic.Bool
	keywordDelivery              uint64
	snapshot                     diagnostics.Snapshot
	cpuPercent                   float64
	cpuMeasured                  bool
	exeBytes                     int64
	storeStatus                  string
	dictionaryStatus             string
	catalogInfo                  storage.CatalogInfo
	catalogRequests              chan []storage.Application
	lookupRequests               chan lookupRequest
	lookupCancel                 context.CancelFunc
	lookupID                     uint64
	lookupCanceled               uint64
	lookupElapsed                time.Duration
	lookupCount                  uint64
	displaySummary               []string
	calcHistory                  *domaincalc.History
	calcHistoryPath              string
	calcRates                    *calcservice.Rates
	calcRegion                   string
	emojiTone                    string
	commands                     *commands.Store
	quicklinks                   *quicklinks.Store
	snippets                     *snippets.Repository
	snippetSnapshot              snippets.Snapshot
	snippetError                 string
	featureCommands              bool
	featureQuicklinks            bool
	featureSnippets              bool
	featureFileSearch            bool
	fileSearch                   *files.Session
	fileSearchRevision           uint64
	fileSearchScopes             []string
	fileSearchIgnores            []string
	fileSearchScopesDraft        string
	fileSearchIgnoresDraft       string
	calendar                     *calendar.Store
	featureCalendar              bool
	calendarShowInLauncher       bool
	autoJoin                     bool
	autoJoinConfirms             bool
	joinWindowMinutes            int64
	calendarError                string
	calendarFetching             bool
	calendarRevision             uint64
	calendarTickerStarted        bool
	autoJoinBusy                 bool
	uninstall                    *uninstallfeature.Scanner
	uninstallTarget              domain.Application
	uninstallRevision            uint64
	uninstallConfirm             bool
	uninstallBusy                bool
	systemConfirming             systemaction.ID
	volumeLoading                bool
	featureWindowManagement      bool
	windowShowInLauncher         bool
	windowGap                    int
	windowCycle                  bool
	windowManager                *windowmanagement.Manager
	commandsShowInLauncher       bool
	quicklinksShowInLauncher     bool
	snippetsShowInLauncher       bool
	commandConfirm               bool
	pendingArgs                  *argumentSession
	clipboardSuppress            atomic.Bool

	aiEnabled             bool
	aiConnections         []domainai.Connection
	aiDefaultModel        *domainai.ModelSelection
	aiWebSearch           bool
	aiSystemPrompt        string
	aiSystemPromptDraft   string
	aiSystemPromptEnabled bool
	aiRetention           int64
	aiOpensTo             int64
	aiNewChatAfter        int64
	quickActionsEnabled   bool
	quickActionModel      *domainai.ModelSelection
	quickActionLanguage   string
	aiKeys                *aifeature.KeyStore
	aiHistory             *aifeature.History
	aiSession             domainai.Session
	aiDraft               string
	aiStreaming           bool
	aiThinking            bool
	aiNotice              string
	aiConfigError         string
	aiGeneration          uint64
	aiCancel              context.CancelFunc
	aiConnEditing         int
	aiConnDraftProvider   domainai.ProviderKind
	aiConnDraftURL        string
	aiConnDraftModel      string
	aiConnDraftKey        string
	aiConnDraftErr        string
	qa                    *quickactions.Coordinator
	qaWindow              *mygo.Window
	qaScroll              ui.ScrollState
	qaApplied             bool
	aiKeyStatus           map[string]bool
}

func newController(started time.Time, profile profile) *controller {
	ctx, cancel := context.WithCancel(context.Background())
	c := &controller{
		ctx: ctx, cancel: cancel, started: started, scale: 1,
		icons: make(map[iconKey]*ui.Bitmap), pendingIcons: make(map[iconKey]bool), iconRequests: make(chan iconKey, 32),
		profile: profile, catalogRequests: make(chan []storage.Application, 1), lookupRequests: make(chan lookupRequest, 1),
		settingsWrites: make(chan settingsWrite, 1), rankingWrites: make(chan []byte, 1),
		clipboardRequests: make(chan struct{}, 1),
		clipboardImages:   make(map[string]*ui.Bitmap), clipboardPending: make(map[string]bool),
		clipboardImageRequests: make(chan clipboardImageRequest, 16),
		systemDark:             windows.SystemDark(), appearance: "system", language: defaultLanguage(),
		featureDictionary: true, featureNotes: true, itemHotkeys: make(map[string]bool),
		backupSettings: true, backupClipboard: true, backupSnippets: true,
		backupNotes: true, backupLearning: true,
	}
	c.clipboardEnabled.Store(true)
	c.notesRoot = filepath.Join(profile.data, "notes")
	if roaming, err := os.UserConfigDir(); err == nil {
		c.migrationSource = filepath.Join(roaming, storage.DailyID)
	}
	c.notes = storage.NewFileNotes(c.notesRoot)
	c.aiKeys = aifeature.OpenKeyStore(profile.settings)
	c.aiKeyStatus = make(map[string]bool)
	c.loadSettings()
	c.refreshAIKeyStatus()
	c.commands = commands.Open(filepath.Join(profile.settings, "custom-commands.json"))
	c.quicklinks = quicklinks.Load(filepath.Join(profile.settings, "quicklinks.json"))
	c.snippets = snippets.NewRepository(filepath.Join(profile.data, "snippets"))
	c.expander = snippets.NewExpander()
	c.reloadSnippets()
	c.fileSearch = files.New()
	c.fileSearch.Apply(c.fileSearchScopes, c.fileSearchIgnores)
	c.calendar = calendar.New(c.joinWindowMinutes)
	c.uninstall = uninstallfeature.New()
	c.windowManager = windowmanagement.New()
	c.fileSearchScopesDraft = strings.Join(c.fileSearchScopes, "; ")
	c.fileSearchIgnoresDraft = strings.Join(c.fileSearchIgnores, "; ")
	c.clipboardDisabledDraft = strings.Join(c.clipboardDisabled, "; ")
	c.calcHistoryPath = filepath.Join(profile.settings, "calculator-history.json")
	c.calcHistory = domaincalc.LoadHistory(c.calcHistoryPath)
	c.calcRates = calcservice.NewRates(filepath.Join(profile.cache, "currency-rates.json"))
	c.calcRegion = domaincalc.CurrencyForLocale(windows.UserLocale())
	c.emojiTone = c.settingsDocument.String("emojiSkinTone", "none")
	c.clampEmojiTone()
	c.aiHistory = aifeature.NewHistory(profile.settings)
	c.aiSession = domainai.NewSession(c.newAIID(), time.Now().Unix())
	c.qa = quickactions.New()
	preferences, preferencesErr := launcher.LoadPreferences(filepath.Join(profile.settings, "launcher"))
	if preferencesErr != nil {
		log.Printf("launcher preferences load failed: %v", preferencesErr)
	}
	c.launcherPrefs = preferences
	c.launcher = launcher.New(preferences)
	c.launcher.SetApplicationsEnabled(c.applicationsShowInLauncher)
	c.view = palette.New(c.language == "zh", palette.Callbacks{
		Open: c.open, Hide: c.hide, Menu: c.menu, AppMenu: c.appMenu, CopyText: c.copyText, Icon: c.icon,
		Filtered: func(elapsed time.Duration, count int) {
			log.Printf("filter elapsed_us=%d results=%d", elapsed.Microseconds(), count)
		},
		Search:                c.search,
		Favorite:              c.openFavorite,
		FavoriteSlot:          c.launcherPrefs.FavoriteSlot,
		ToggleFavorite:        c.toggleFavorite,
		ToggleExpanded:        c.toggleExpanded,
		ExpandedChanged:       c.schedulePanelSize,
		SaveAlias:             c.saveAlias,
		SetHidden:             c.setHidden,
		ToggleShowHidden:      c.toggleShowHidden,
		ClipboardSearch:       c.clipboardSearch,
		ClipboardPaste:        c.pasteClipboardItem,
		ClipboardCopy:         c.copyClipboardItem,
		ClipboardPin:          c.toggleClipboardPin,
		ClipboardDelete:       c.deleteClipboardItem,
		ClipboardImage:        c.clipboardBitmap,
		ClipboardPinned:       c.pasteClipboardSlot,
		SetClipboardFilter:    c.setClipboardFilter,
		Calculate:             c.calculate,
		CalculatorHistory:     c.calculatorHistory,
		CopyCalculator:        c.copyCalculatorResult,
		CopyCalculatorHistory: c.copyCalculatorHistory,
		OpenCalculatorHistory: c.openCalculatorHistory,
		EmojiSearch:           c.emojiSearch,
		EmojiPaste:            c.pasteEmoji,
		CycleEmojiTone:        c.cycleEmojiTone,
		SubmitArguments:       c.submitArguments,
		FileSearch:            c.startFileSearch,
		OpenFile:              c.openFileHit,
		RevealFile:            c.revealFileHit,
		CopyFilePath:          c.copyFileHitPath,
		JoinMeeting:           c.joinMeeting,
		CopyMeetingLink:       c.copyMeetingLink,
		AISend:                c.sendAIMessage,
		AICancel:              c.cancelAIStream,
		AINew:                 c.newAIChatFromMenu,
		AIHistory:             c.aiHistorySearch,
		AIOpen:                c.openAIConversation,
		AIModelMenu:           c.aiModelMenu,
		AILeave: func(text string) {
			c.aiDraft = text
		},
		ConfirmUninstall:    c.confirmUninstall,
		CopyUninstallPath:   c.copyUninstallPath,
		RevealUninstallPath: c.revealUninstallPath,
		ApplyVolume:         c.applyVolume,
		CloseVolume:         c.closeVolumeDialog,
	})
	c.applyViewAppearance()
	c.view.Expanded = !c.compactMode
	c.view.Shortcut = c.hotkey
	c.view.AIAvailable = c.aiEnabled
	c.view.AIModelLabel = c.aiModelLabel()
	c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
	c.view.SetAIStreaming(c.aiStreaming)
	c.view.EmojiTone = emoji.ToneFromRaw(c.emojiTone)
	c.view.DictionaryAvailable = c.featureDictionary && c.dictionaryReady
	c.view.OnLookup = c.lookup
	c.view.ShowFavoritesInCompact = c.showFavoritesInCompact
	c.view.Draggable = c.paletteDraggable
	c.refreshExtras()
	c.refreshCompactFavorites()
	return c
}

func (c *controller) ready(command windows.Activation, hidden bool) {
	c.panel = mygo.NewWindow(mygo.WindowOptions{
		Title: "Tinycast", Panel: true, Hidden: true, Frameless: true,
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
		if !c.closing && !c.menuOpen && !c.clipboardConfirm && !c.ownsForeground() {
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
	if c.launchAtLogin {
		if err := windows.ApplyLaunchAtLogin(settings.LaunchAtLoginKey, true); err != nil {
			log.Printf("launch at login apply failed: %v", err)
		}
	} else if windows.LaunchAtLoginMatches(settings.LaunchAtLoginKey) {
		// Remove only an entry pointing at this executable; another identity's
		// startup entry (for example the daily copy) must survive a preview run.
		if err := windows.ApplyLaunchAtLogin(settings.LaunchAtLoginKey, false); err != nil {
			log.Printf("launch at login removal failed: %v", err)
		}
	}
	if c.showInTray {
		if err := c.setupTray(); err != nil {
			log.Printf("tray unavailable: %v", err)
			c.view.Status = c.view.Text("无法创建托盘图标", "Could not create the tray icon")
		}
	}
	c.startIcons()
	c.startStorage()
	c.startSettingsWriter()
	c.startRankingWriter()
	c.startClipboard()
	c.startCalendar()
	c.startAI()
	c.startClipboardImages()
	c.startCalculatorRates()
	c.startClipboardHost()
	c.applyDoubleTap()
	c.applyKeywordListener()
	c.reconcileItemHotkeys()
	c.scan()
	c.showOnboardingIfNeeded()
	c.startSupportReminders()
	if command != windows.ActivatePanel || !hidden {
		c.activate(command)
	}
	log.Printf("ready elapsed_us=%d hwnd=%d", time.Since(c.started).Microseconds(), c.panel.NativeHandle())
	// One snapshot per start makes handle/goroutine/working-set growth visible
	// across long lifecycle runs without opening the diagnostics window.
	c.refreshDiagnostics()
}

func (c *controller) logDisplays() {
	c.displaySummary = nil
	for _, display := range mygo.Screen.Displays() {
		log.Printf("display id=%d scale=%.2f bounds=%+v work=%+v primary=%t", display.ID, display.ScaleFactor, display.Bounds, display.WorkArea, display.Primary)
		c.displaySummary = append(c.displaySummary, fmt.Sprintf("%s · %.0f%% · %d×%d DIP · (%d,%d)", display.Label, display.ScaleFactor*100, display.Bounds.Width, display.Bounds.Height, display.Bounds.X, display.Bounds.Y))
	}
}

func (c *controller) place() {
	display := mygo.Screen.PrimaryDisplay()
	if c.openOnCursorScreen {
		display = mygo.Screen.DisplayNearestPoint(mygo.Screen.CursorScreenPoint())
	}
	width, _ := c.panel.Size()
	area := display.WorkArea
	x := area.X + max(0, (area.Width-width)/2)
	y := area.Y + max(0, (area.Height-int(theme.PanelHeight))*18/100)
	c.panel.SetPosition(x, y)
	c.scale = display.ScaleFactor
	if c.scale <= 0 {
		c.scale = 1
	}
}

func (c *controller) show() {
	c.showPanel(true)
}

func (c *controller) showPanel(reset bool) {
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
		if reset && c.popToRootTimeout > 0 && !c.hiddenAt.IsZero() &&
			time.Since(c.hiddenAt) < time.Duration(c.popToRootTimeout)*time.Second {
			reset = false
		}
		if reset {
			c.pendingArgs = nil
			c.view.ResetLauncher(!c.compactMode)
		} else if c.view.Mode != palette.ModeLauncher {
			c.view.SetExpanded(true)
		}
		c.applyPanelSize()
	}
	c.place()
	c.view.Input.Focus()
	showWindow(c.panel)
	log.Printf("show elapsed_us=%d visible=%t target_available=%t", time.Since(c.showRequested).Microseconds(), c.panel.IsVisible(), c.target.Window != 0)
}

func (c *controller) hide() {
	if c.panel != nil && c.panel.IsVisible() {
		started := time.Now()
		c.hiddenAt = started
		c.panel.Hide()
		log.Printf("hide elapsed_us=%d", time.Since(started).Microseconds())
	}
}

func (c *controller) toggle() {
	if c.closing {
		return
	}
	// RegisterHotKey can repeat while the accelerator is held. A held
	// summon key must show once, not flicker on every repeat.
	now := time.Now()
	if previous := c.lastToggle.Swap(now.UnixNano()); previous != 0 &&
		now.Sub(time.Unix(0, previous)) < 250*time.Millisecond {
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
	directories := c.customDirectories() // snapshot UI-owned settings before the worker starts
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		started := time.Now()
		apps, err := mygo.Shell.InstalledApps()
		if c.ctx.Err() != nil {
			return
		}
		custom := c.scanCustomApplications(directories)
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
			if c.view.Mode == palette.ModeLauncher {
				c.view.Filter()
			}
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
	switch application.Kind {
	case domain.KindAI, domain.KindQuickAction:
		c.routeAIEntry(application)
		return
	case domain.KindCommand:
		c.startCommand(application)
		return
	case domain.KindQuicklink:
		c.startQuicklink(application)
		return
	case domain.KindSnippet:
		c.startSnippet(application)
		return
	case domain.KindFileSearch:
		c.openFileSearch()
		return
	case domain.KindCalendar:
		c.openSchedule()
		return
	case domain.KindSystemAction:
		if id, ok := systemaction.FromEntryID(application.ID); ok {
			c.runSystemAction(id)
		}
		return
	case domain.KindSystemSettings:
		if uri, ok := systemsettings.URI(application.ID); ok {
			c.hide()
			if err := quicklinks.Open(uri); err != nil {
				log.Printf("system settings open failed: %v", err)
				c.view.Status = c.view.Text("无法打开系统设置", "Could not open System Settings")
				c.showPanel(false)
			}
		}
		return
	case domain.KindWindowCommand:
		if id, ok := windowcommand.FromEntryID(application.ID); ok {
			c.runWindowCommand(id)
		}
		return
	}
	c.hide()
	if err := mygo.Shell.LaunchApp(mygo.InstalledApp{ID: application.ID, Name: application.Name, Path: application.Path}); err != nil {
		log.Printf("launch failed: %v", err)
		c.view.Status = c.view.Text("无法打开应用程序", "Could not open application")
		c.showPanel(false)
	}
}

func (c *controller) menu() {
	if c.closing || c.menuOpen {
		return
	}
	c.menuOpen = true
	defer c.finishMenu()
	if c.view.Mode == palette.ModeClipboard {
		c.clipboardMenu()
		return
	}
	if c.view.Mode == palette.ModeCalculatorHistory {
		c.calculatorMenu()
		return
	}
	if c.view.Mode == palette.ModeEmoji {
		c.emojiMenu()
		return
	}
	if c.view.Mode == palette.ModeFileSearch {
		c.fileSearchMenu()
		return
	}
	if c.view.Mode == palette.ModeSchedule {
		c.scheduleMenu()
		return
	}
	if c.view.Mode == palette.ModeUninstall {
		c.uninstallMenu()
		return
	}
	if c.view.Mode == palette.ModeAI || c.view.Mode == palette.ModeAIHistory {
		c.aiMenu()
		return
	}
	if !c.view.Expanded || c.view.Mode == palette.ModeDictionary || c.view.Mode == palette.ModeArguments {
		c.popupAppMenu()
		return
	}
	application, ok := c.selected()
	if !ok {
		c.popupAppMenu()
		return
	}
	actions := []components.MenuAction{
		{Label: c.openLabel(application), Accelerator: "↵", Click: func() { c.open(application) }},
	}
	if persistentEntry(application.Kind) {
		actions = append(actions,
			components.MenuAction{Separator: true},
			components.MenuAction{Label: c.favoriteLabel(), Accelerator: "Ctrl+D", Click: c.toggleFavorite},
			components.MenuAction{Label: c.view.Text("设置别名…", "Set alias…"), Click: c.openAliasDialog},
			components.MenuAction{Label: c.hideLabel(), Click: c.toggleSelectedHidden},
		)
	}
	if application.Kind == domain.KindApplication {
		actions = append(actions,
			components.MenuAction{Separator: true},
			components.MenuAction{Label: c.view.Text("卸载应用…", "Uninstall application…"), Click: c.openUninstallSelected},
		)
	}
	components.Menu(actions).Popup(c.panel)
}

// appMenu holds commands that belong to Tinycast itself rather than to the
// highlighted row: refreshing the catalog, feature screens, settings and quit.
func (c *controller) appMenu() {
	if c.closing || c.menuOpen {
		return
	}
	c.menuOpen = true
	defer c.finishMenu()
	c.popupAppMenu()
}

func (c *controller) popupAppMenu() {
	menu := components.Menu([]components.MenuAction{
		{Label: c.view.Text("刷新应用程序", "Refresh applications"), Click: c.scan},
		{Label: c.showHiddenLabel(), Click: c.toggleShowHidden},
		{Label: c.expandLabel(), Accelerator: "Ctrl+E", Click: c.toggleExpanded},
		{Separator: true},
		{Label: c.view.Text("复制文本 / 词条", "Copy text / entry"), Accelerator: "Ctrl+Shift+C", Disabled: c.view.Query == "", Click: func() { c.copyText(c.view.CopyText(), false) }},
		{Label: c.view.Text("粘回原窗口", "Paste to previous window"), Accelerator: "Ctrl+Shift+Enter", Disabled: c.view.Query == "", Click: func() { c.copyText(c.view.CopyText(), true) }},
		{Separator: true},
		{Label: c.view.Text("剪贴板历史…", "Clipboard history…"), Accelerator: "Ctrl+~", Click: c.openClipboard},
		{Label: c.view.Text("计算器历史…", "Calculator history…"), Click: c.openCalculatorHistory},
		{Label: c.view.Text("Emoji 与符号…", "Emoji & Symbols…"), Click: c.openEmoji},
		{Label: c.view.Text("搜索文件…", "Search files…"), Disabled: !c.featureFileSearch, Click: c.openFileSearch},
		{Label: c.view.Text("我的日程…", "My Schedule…"), Disabled: !c.featureCalendar, Click: c.openSchedule},
		{Label: c.view.Text("AI 对话…", "AI Chat…"), Disabled: !c.aiEnabled, Click: c.openAIChat},
		{Label: c.view.Text("快捷操作", "Quick Actions"), Submenu: c.quickActionMenu()},
		{Label: c.view.Text("系统操作", "System actions"), Submenu: c.systemActionMenu()},
		{Label: c.view.Text("窗口布局", "Window layout"), Submenu: c.windowCommandMenu(), Disabled: !c.featureWindowManagement},
		{Label: c.view.Text("词典 / 应用程序", "Dictionary / Applications"), Disabled: !c.view.DictionaryAvailable, Click: c.view.ToggleDictionary},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("笔记…", "Notes…"), Disabled: !c.featureNotes, Click: c.openEditor},
		{Label: c.view.Text("诊断…", "Diagnostics…"), Click: c.openDiagnostics},
		{Separator: true},
		{Label: c.view.Text("退出", "Quit"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}

// finishMenu restores input focus, or hides a panel that lost focus while the
// native menu was tracking the pointer.
func (c *controller) finishMenu() {
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
}

// persistentEntry reports whether a row carries user preferences that the
// row's own menu may edit.
func persistentEntry(kind domain.Kind) bool {
	switch kind {
	case domain.KindApplication, domain.KindCommand, domain.KindQuicklink, domain.KindSnippet,
		domain.KindSystemSettings, domain.KindSystemAction, domain.KindWindowCommand:
		return true
	default:
		return false
	}
}

func (c *controller) openLabel(application domain.Application) string {
	switch application.Kind {
	case domain.KindApplication:
		return c.view.Text("打开应用", "Open Application")
	case domain.KindCommand:
		return c.view.Text("运行命令", "Run Command")
	case domain.KindQuicklink:
		return c.view.Text("打开链接", "Open Quicklink")
	case domain.KindSnippet:
		return c.view.Text("展开片段", "Expand Snippet")
	case domain.KindFileSearch:
		return c.view.Text("搜索文件…", "Search Files…")
	case domain.KindCalendar:
		return c.view.Text("打开日程", "Open Schedule")
	case domain.KindAI:
		return c.view.Text("打开 AI 对话", "Open AI Chat")
	case domain.KindQuickAction:
		return c.view.Text("运行快捷操作", "Run Quick Action")
	case domain.KindSystemAction:
		return c.view.Text("运行操作", "Run Action")
	case domain.KindSystemSettings:
		return c.view.Text("打开系统设置", "Open System Settings")
	case domain.KindWindowCommand:
		return c.view.Text("执行窗口命令", "Run Window Command")
	}
	return c.view.Text("打开", "Open")
}

func (c *controller) emojiMenu() {
	selected := c.view.EmojiChosen >= 0 && c.view.EmojiChosen < len(c.view.EmojiItems)
	menu := components.Menu([]components.MenuAction{
		{Label: c.view.Text("粘回选中 Emoji", "Paste selected emoji"), Disabled: !selected, Click: func() {
			if selected {
				c.pasteEmoji(c.view.EmojiItems[c.view.EmojiChosen].Glyph)
			}
		}},
		{Label: c.view.Text("切换肤色", "Change skin tone") + " · " + c.view.EmojiTone.Label(), Click: func() {
			c.cycleEmojiTone()
			c.view.Filter()
		}},
		{Separator: true},
		{Label: c.view.Text("返回启动器", "Back to launcher"), Accelerator: "Esc", Click: c.view.SetModeLauncher},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("退出", "Quit"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}

func (c *controller) calculatorMenu() {
	var selected *domaincalc.HistoryEntry
	if c.view.Chosen >= 0 && c.view.Chosen < len(c.view.CalcHistory) {
		entry := c.view.CalcHistory[c.view.Chosen]
		selected = &entry
	}
	canCopyResult := c.view.Chosen < 0 && c.view.CalcResult != nil && !c.view.CalcResult.IsError()
	menu := components.Menu([]components.MenuAction{
		{Label: c.view.Text("复制当前答案", "Copy current answer"), Disabled: !canCopyResult, Click: func() {
			if canCopyResult {
				c.copyCalculatorResult(*c.view.CalcResult)
			}
		}},
		{Label: c.view.Text("复制选中历史", "Copy selected history"), Disabled: selected == nil, Click: func() {
			if selected != nil {
				c.copyCalculatorHistory(*selected)
			}
		}},
		{Separator: true},
		{Label: c.view.Text("返回启动器", "Back to launcher"), Accelerator: "Esc", Click: c.view.SetModeLauncher},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("退出", "Quit"), Click: mygo.App.Quit},
	})
	menu.Popup(c.panel)
}

func (c *controller) clipboardMenu() {
	var selected *clipboard.Item
	if c.view.ClipboardChosen >= 0 && c.view.ClipboardChosen < len(c.view.ClipboardItems) {
		item := c.view.ClipboardItems[c.view.ClipboardChosen]
		selected = &item
	}
	pinLabel := c.view.Text("置顶", "Pin")
	if selected != nil && selected.Pinned() {
		pinLabel = c.view.Text("取消置顶", "Unpin")
	}
	menu := components.Menu([]components.MenuAction{
		{Label: c.view.Text("粘回原窗口", "Paste to previous window"), Disabled: selected == nil, Click: func() {
			if selected != nil {
				c.pasteClipboardItem(*selected)
			}
		}},
		{Label: c.view.Text("复制", "Copy"), Disabled: selected == nil, Click: func() {
			if selected != nil {
				c.copyClipboardItem(*selected)
			}
		}},
		{Label: pinLabel, Disabled: selected == nil, Click: func() {
			if selected != nil {
				c.toggleClipboardPin(*selected)
			}
		}},
		{Label: c.view.Text("删除", "Delete"), Disabled: selected == nil, Click: func() {
			if selected != nil {
				c.deleteClipboardItem(*selected)
			}
		}},
		{Separator: true},
		{Label: c.view.Text("清空历史…", "Clear history…"), Click: c.clearClipboardHistory},
		{Label: c.view.Text("返回启动器", "Back to launcher"), Accelerator: "Esc", Click: c.view.SetModeLauncher},
		{Separator: true},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("退出", "Quit"), Click: mygo.App.Quit},
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
	tray, err := mygo.NewTray(mygo.TrayOptions{ID: previewID, Icon: trayIcon(), ToolTip: "Tinycast"})
	if err != nil {
		return fmt.Errorf("create tray: %w", err)
	}
	c.tray = tray
	tray.OnClick(c.show)
	tray.SetMenu(components.Menu([]components.MenuAction{
		{Label: c.view.Text("打开 Tinycast", "Open Tinycast"), Accelerator: c.hotkey, Click: c.show},
		{Separator: true},
		{Label: c.view.Text("设置…", "Settings…"), Click: c.openSettings},
		{Label: c.view.Text("笔记…", "Notes…"), Click: c.openEditor},
		{Label: c.view.Text("诊断…", "Diagnostics…"), Click: c.openDiagnostics},
		{Separator: true},
		{Label: c.view.Text("退出", "Quit"), Click: mygo.App.Quit},
	}))
	return nil
}

func (c *controller) stop() {
	if c.closing {
		return
	}
	c.closing = true
	c.cancel()
	if c.aiCancel != nil {
		c.aiCancel()
		c.aiCancel = nil
	}
	if c.qa != nil {
		c.qa.Cancel()
	}
	c.instance.StopServing()
	mygo.GlobalShortcut.UnregisterAll()
	if c.messageHost != nil {
		c.messageHost.Close()
		c.messageHost = nil
		c.doubleTapStop = nil
		c.keywordStop = nil
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
		c.closeAIHistory()
		aifeature.CloseHTTPIdleConnections()
	case <-time.After(time.Second):
		c.instance.HoldIdentityUntilExit()
		log.Print("shutdown worker deadline exceeded; pending persistence may be incomplete")
	}
}
