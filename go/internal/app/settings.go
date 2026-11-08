package app

import (
	"log"
	"os"
	"path/filepath"
	"strings"
	"time"

	domainfiles "github.com/YYHCOPPOLO/wincast/go/internal/domain/filesearch"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/launcher"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

type settingsWrite struct {
	data []byte
}

// loadSettings runs before the UI loop. A missing file is a normal first
// start; a damaged file is remembered and never overwritten with defaults.
func (c *controller) loadSettings() {
	c.settingsPath = filepath.Join(c.profile.settings, "settings.json")
	document, err := storage.LoadDocument(c.settingsPath)
	if err != nil {
		c.settingsErr = err
		c.settingsDocument = storage.NewDocument()
		c.settingsStatus = c.text("设置文件无法读取，未写入", "Settings file is unreadable; nothing was written")
		log.Printf("settings load failed: %v", err)
		return
	}
	c.settingsDocument = document
	c.settingsWritable = true
	c.readSettingsDocument()
	c.clampSettings()
}

// readSettingsDocument copies every modeled value out of the current settings
// document. Import and migration reuse it after merging new keys.
func (c *controller) readSettingsDocument() {
	document := c.settingsDocument
	if document == nil {
		return
	}
	c.language = document.String("language", "")
	if c.language == "" {
		switch raw := document.String("uiLanguage", ""); {
		case strings.HasPrefix(raw, "zh"):
			c.language = "zh"
		case raw == "en":
			c.language = "en"
		default:
			c.language = defaultLanguage()
		}
	}
	c.appearance = document.String("appearance", "system")
	c.openOnCursorScreen = document.Bool("openOnCursorScreen", true)
	c.launchAtLogin = document.Bool("launchAtLogin", false)
	c.showInTray = document.Bool("showInMenuBar", true)
	c.popToRootTimeout = int64(document.Int("popToRootTimeout", 0))
	c.showFavoritesInCompact = document.Bool("showFavoritesInCompactMode", true)
	c.paletteDraggable = document.Bool("paletteDraggable", false)
	c.supportReminders = document.Bool("supportReminders", true)
	c.extensionsEnabled = document.Bool("extensionsEnabled", false)
	c.extensionsShowInLauncher = document.Bool("extensionsShowInLauncher", true)
	c.featureDictionary = document.Bool("dictionaryEnabled", true)
	c.applicationsShowInLauncher = document.Bool("applicationsShowInLauncher", true)
	c.systemSettingsShowInLauncher = document.Bool("systemSettingsShowInLauncher", true)
	c.systemActionsShowInLauncher = document.Bool("systemActionsShowInLauncher", true)
	c.featureNotes = document.Bool("notesEnabled", true)
	c.featureCommands = document.Bool("customCommandsEnabled", false)
	if document.Has("commandsEnabled") {
		c.featureCommands = document.Bool("commandsEnabled", false)
	}
	c.featureQuicklinks = document.Bool("quicklinksEnabled", false)
	c.featureSnippets = document.Bool("snippetsEnabled", false)
	c.featureFileSearch = document.Bool("fileSearchEnabled", false)
	c.fileSearchScopes = document.Strings("fileSearchScopes", domainfiles.DefaultScopes())
	c.fileSearchIgnores = document.Strings("fileSearchIgnorePatterns", nil)
	c.featureWindowManagement = document.Bool("windowManagementEnabled", false)
	c.windowShowInLauncher = document.Bool("windowManagementShowInLauncher", true)
	c.windowGap = document.Int("windowManagementGap", 0)
	c.windowCycle = document.Bool("windowManagementCycleOnRepeat", false)
	c.featureCalendar = document.Bool("calendarEnabled", false)
	c.calendarShowInLauncher = document.Bool("calendarShowInLauncher", true)
	c.autoJoin = document.Bool("autoJoinMeetings", false)
	c.autoJoinConfirms = document.Bool("autoJoinConfirms", true)
	c.joinWindowMinutes = int64(document.Int("joinWindowMinutes", 5))
	c.commandsShowInLauncher = document.Bool("customCommandsShowInLauncher", true)
	if document.Has("commandsShowInLauncher") {
		c.commandsShowInLauncher = document.Bool("commandsShowInLauncher", true)
	}
	c.quicklinksShowInLauncher = document.Bool("quicklinksShowInLauncher", true)
	c.snippetsShowInLauncher = document.Bool("snippetsShowInLauncher", true)
	c.clipboardEnabled.Store(document.Bool("clipboardMonitorEnabled", true))
	c.clipboardRetention = int64(document.Int("clipboardRetentionDays", clipboard.DefaultRetentionDays))
	c.clipboardDisabled = document.Strings("clipboardDisabledApps", clipboard.DefaultDisabledApps)
	c.hotkey = document.String("hotkey", defaultShortcut)
	c.hotkeyDraft = c.hotkey
	c.doubleTap = document.String("doubleTapModifier", "none")
	c.customDirsDraft = strings.Join(c.customDirectories(), "; ")
	c.compactMode = document.Bool("compactMode", false)
	c.loadAISettings()
}

func defaultLanguage() string {
	if windows.ChineseUI() {
		return "zh"
	}
	return "en"
}

func (c *controller) clampSettings() {
	if c.language != "zh" && c.language != "en" {
		c.language = defaultLanguage()
	}
	switch c.appearance {
	case "system", "light", "dark":
	default:
		c.appearance = "system"
	}
	switch c.clipboardRetention {
	case 1, 7, 30, 90, 180, 365, -1:
	default:
		c.clipboardRetention = clipboard.DefaultRetentionDays
	}
	switch c.windowGap {
	case 0, 8, 16, 24:
	default:
		c.windowGap = 0
	}
	switch c.joinWindowMinutes {
	case 1, 2, 5, 10, 15:
	default:
		c.joinWindowMinutes = 5
	}
}

func (c *controller) text(zh, en string) string {
	if c.language == "zh" {
		return zh
	}
	return en
}

func (c *controller) forcedDark() *bool {
	switch c.appearance {
	case "light":
		value := false
		return &value
	case "dark":
		value := true
		return &value
	}
	return nil
}

func (c *controller) applyViewAppearance() {
	if forced := c.forcedDark(); forced != nil {
		c.view.ForcedDark = forced
		return
	}
	c.view.ForcedDark = &c.systemDark
}

func (c *controller) dark() bool {
	if forced := c.forcedDark(); forced != nil {
		return *forced
	}
	return c.systemDark
}

func (c *controller) settingDocument() (*storage.Document, bool) {
	return c.settingsDocument, c.settingsWritable
}

// queueSettingsSave encodes on the UI thread and lets one worker replace the
// file. Only the newest complete snapshot is written; a failure keeps the
// previous file visible in its directory.
func (c *controller) queueSettingsSave() {
	document, writable := c.settingDocument()
	if document == nil || !writable {
		return
	}
	data, err := document.Encode()
	if err != nil {
		c.settingsStatus = c.text("设置序列化失败", "Could not encode settings")
		log.Printf("settings encode failed: %v", err)
		return
	}
	c.settingsStatus = c.text("正在保存…", "Saving…")
	select {
	case <-c.settingsWrites:
	default:
	}
	select {
	case c.settingsWrites <- settingsWrite{data: data}:
	default:
	}
}

func (c *controller) startSettingsWriter() {
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		persist := func(write settingsWrite) {
			err := storage.WriteFileAtomic(c.settingsPath, write.data, 0o600)
			if err != nil {
				log.Printf("settings save failed: %v", err)
			}
			if c.ctx.Err() != nil {
				return
			}
			c.panel.Update(func() {
				if c.closing {
					return
				}
				if err != nil {
					c.settingsStatus = c.text("设置保存失败；原文件已保留", "Could not save settings; the previous file was kept")
				} else {
					c.settingsStatus = c.text("已保存", "Saved")
				}
				if c.settings != nil {
					c.settings.Update(func() {})
				}
			})
		}
		for {
			select {
			case <-c.ctx.Done():
				// Producers live on the stopped UI thread. Drain its newest
				// complete snapshot before releasing the writer identity.
				if write, ok := newestPending(c.settingsWrites); ok {
					persist(write)
				}
				return
			case write := <-c.settingsWrites:
				persist(write)
			}
		}
	}()
}

func newestPending[T any](queue <-chan T) (value T, ok bool) {
	for {
		select {
		case value = <-queue:
			ok = true
		default:
			return value, ok
		}
	}
}

func (c *controller) openSettings() {
	if c.closing {
		return
	}
	c.hide()
	if c.settings == nil {
		c.settings = mygo.NewWindow(mygo.WindowOptions{
			Title:           "Tinycast Settings",
			Width:           int(theme.SettingsWidth),
			Height:          int(theme.SettingsHeight),
			Hidden:          true,
			DisableResize:   false,
			BackgroundColor: "light-dark(#f4f4f6, #1a1a1e)",
			Content:         ui.View(c.settingsView),
		})
		c.settings.OnClosed(func() { c.settings = nil })
	}
	showWindow(c.settings)
	c.updateSettingsTitle()
	c.settings.Center()
}

func (c *controller) updateSettingsTitle() {
	if c.settings == nil {
		return
	}
	prefix := c.text("Tinycast 设置 · ", "Tinycast Settings · ")
	c.settings.SetTitle(prefix + c.settingsTab.Title(c.language == "zh"))
}

func appearanceIndex(value string) int {
	switch value {
	case "light":
		return 1
	case "dark":
		return 2
	}
	return 0
}

func appearanceValue(index int) string {
	switch index {
	case 1:
		return "light"
	case 2:
		return "dark"
	}
	return "system"
}

func doubleTapIndex(value string) int {
	switch value {
	case "ctrl":
		return 1
	case "alt":
		return 2
	case "shift":
		return 3
	}
	return 0
}

func doubleTapValue(index int) string {
	switch index {
	case 1:
		return "ctrl"
	case 2:
		return "alt"
	case 3:
		return "shift"
	}
	return "none"
}

func clipboardRetentionLabel(language string, days int64) string {
	zh := language == "zh"
	switch days {
	case 1:
		if zh {
			return "1 天"
		}
		return "1 Day"
	case 7:
		if zh {
			return "1 周"
		}
		return "1 Week"
	case 30:
		if zh {
			return "1 个月"
		}
		return "1 Month"
	case 90:
		if zh {
			return "3 个月"
		}
		return "3 Months"
	case 180:
		if zh {
			return "6 个月"
		}
		return "6 Months"
	case 365:
		if zh {
			return "1 年"
		}
		return "1 Year"
	default:
		if zh {
			return "永久"
		}
		return "Forever"
	}
}

// clipboardRetentionPresets is the order the retention menu lists.
var clipboardRetentionPresets = []int64{1, 7, 30, 90, 180, 365, -1}

// clipboardExclusionTitle names an excluded executable: the catalog's app
// name when one matches its stem, the stem itself otherwise.
func (c *controller) clipboardExclusionTitle(stem string) string {
	normalized := clipboardExecutableStem(stem)
	for _, app := range c.catalog {
		if strings.EqualFold(app.Name, normalized) {
			return app.Name
		}
		path := strings.TrimSpace(app.Path)
		if strings.HasSuffix(strings.ToLower(path), ".exe") &&
			clipboardExecutableStem(filepath.Base(path)) == normalized {
			return app.Name
		}
	}
	return stem
}

// clipboardExecutableStem is the key an exclusion is stored under.
func clipboardExecutableStem(name string) string {
	return strings.ToLower(strings.TrimSuffix(strings.TrimSpace(name), ".exe"))
}

func (c *controller) addClipboardExcludedApp(stem string) {
	stem = strings.TrimSpace(stem)
	if stem == "" {
		return
	}
	for _, existing := range c.clipboardDisabled {
		if clipboardExecutableStem(existing) == clipboardExecutableStem(stem) {
			return
		}
	}
	c.setClipboardExcludedApps(append(append([]string(nil), c.clipboardDisabled...), stem))
}

func (c *controller) removeClipboardExcludedApp(stem string) {
	apps := make([]string, 0, len(c.clipboardDisabled))
	for _, existing := range c.clipboardDisabled {
		if clipboardExecutableStem(existing) != clipboardExecutableStem(stem) {
			apps = append(apps, existing)
		}
	}
	c.setClipboardExcludedApps(apps)
}

// setClipboardExcludedApps stores the exclusion list and applies it to capture
// immediately; matching stays a case-insensitive executable stem comparison.
func (c *controller) setClipboardExcludedApps(apps []string) {
	c.clipboardDisabled = apps
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("clipboardDisabledApps", apps)
	}
	c.queueSettingsSave()
	if c.clipboard != nil {
		c.clipboard.SetDisabledApps(apps)
	}
	c.settingsStatus = c.text("排除列表已更新", "Excluded applications updated")
	c.refreshSettingsWindow()
}

// pickClipboardExcludedApp asks for the executable to exclude. The original
// lists installed apps by bundle id; Windows identifies a clipboard owner by
// its executable, so the picker saves the chosen .exe's stem.
func (c *controller) pickClipboardExcludedApp() {
	if c.clipboardExcludePicking {
		return
	}
	c.clipboardExcludePicking = true
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		paths, err := mygo.Dialog.Open(mygo.OpenDialogOptions{
			Parent: c.settings,
			Title:  c.text("添加排除的应用", "Add an Excluded Application"),
			Filters: []mygo.FileFilter{{
				Name:       c.text("应用程序", "Applications"),
				Extensions: []string{"exe"},
			}},
		})
		c.updateUI(func() {
			if c.closing {
				return
			}
			c.clipboardExcludePicking = false
			if err != nil {
				log.Printf("excluded application picker failed: %v", err)
				return
			}
			if len(paths) == 0 {
				return
			}
			stem := strings.TrimSuffix(filepath.Base(paths[0]), filepath.Ext(paths[0]))
			c.addClipboardExcludedApp(stem)
		})
	}()
}

func (c *controller) setAppearance(value string) {
	c.appearance = value
	c.applyViewAppearance()
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("appearance", value)
	}
	c.queueSettingsSave()
}

func (c *controller) setLanguage(value string) {
	c.language = value
	c.view.Chinese = value == "zh"
	if c.view != nil {
		c.refreshAIChatView()
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("language", value)
	}
	c.queueSettingsSave()
	c.updateSettingsTitle()
}

func (c *controller) setFeature(key string, enabled bool) {
	if key == "calendarEnabled" {
		c.setCalendarEnabled(enabled)
		return
	}
	switch key {
	case "dictionaryEnabled":
		c.featureDictionary = enabled
		c.view.DictionaryAvailable = enabled && c.dictionaryReady
		if !enabled && c.view.Mode == palette.ModeDictionary {
			c.view.SetModeLauncher()
		}
	case "notesEnabled":
		c.featureNotes = enabled
		if !enabled && c.editor != nil {
			c.hideEditor()
		}
	case "clipboardMonitorEnabled":
		c.clipboardEnabled.Store(enabled)
	case "commandsEnabled":
		c.featureCommands = enabled
	case "quicklinksEnabled":
		c.featureQuicklinks = enabled
	case "snippetsEnabled":
		c.featureSnippets = enabled
		c.applyKeywordListener()
	case "fileSearchEnabled":
		c.featureFileSearch = enabled
		if c.fileSearch != nil {
			c.fileSearch.Cancel()
		}
		if !enabled && c.view.Mode == palette.ModeFileSearch {
			c.view.SetModeLauncher()
		}
		c.refreshExtras()
	case "windowManagementEnabled":
		c.featureWindowManagement = enabled
		c.refreshExtras()
	case "windowManagementShowInLauncher":
		c.windowShowInLauncher = enabled
		c.refreshExtras()
	case "windowManagementCycleOnRepeat":
		c.windowCycle = enabled
	case "calendarShowInLauncher":
		c.calendarShowInLauncher = enabled
		c.refreshExtras()
	case "autoJoinMeetings":
		c.autoJoin = enabled
		if enabled && c.featureCalendar && c.calendar != nil {
			c.calendar.ArmNow(time.Now().Unix())
		}
	case "autoJoinConfirms":
		c.autoJoinConfirms = enabled
	case "commandsShowInLauncher":
		c.commandsShowInLauncher = enabled
	case "quicklinksShowInLauncher":
		c.quicklinksShowInLauncher = enabled
	case "snippetsShowInLauncher":
		c.snippetsShowInLauncher = enabled
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set(key, enabled)
	}
	c.queueSettingsSave()
	if key == "commandsEnabled" || key == "quicklinksEnabled" || key == "snippetsEnabled" ||
		key == "commandsShowInLauncher" || key == "quicklinksShowInLauncher" || key == "snippetsShowInLauncher" {
		c.refreshExtras()
	}
	if key == "commandsEnabled" || key == "quicklinksEnabled" || key == "snippetsEnabled" {
		c.reconcileItemHotkeys()
	}
}

func (c *controller) applyFileSearchScopes() {
	scopes := splitSettingList(c.fileSearchScopesDraft)
	normalized := domainfiles.NormalizeScopes(scopes, userHomeDirectory())
	if len(normalized) == 0 {
		normalized = domainfiles.DefaultScopes()
	}
	c.fileSearchScopes = normalized
	c.fileSearchScopesDraft = strings.Join(normalized, "; ")
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("fileSearchScopes", normalized)
	}
	c.queueSettingsSave()
	if c.fileSearch != nil {
		c.fileSearch.Apply(c.fileSearchScopes, c.fileSearchIgnores)
	}
	c.settingsStatus = c.text("搜索范围已更新", "Search scopes updated")
}

func (c *controller) applyFileSearchIgnores() {
	ignores := splitSettingList(c.fileSearchIgnoresDraft)
	c.fileSearchIgnores = ignores
	c.fileSearchIgnoresDraft = strings.Join(ignores, "; ")
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("fileSearchIgnorePatterns", ignores)
	}
	c.queueSettingsSave()
	if c.fileSearch != nil {
		c.fileSearch.Apply(c.fileSearchScopes, c.fileSearchIgnores)
	}
	c.settingsStatus = c.text("忽略规则已更新", "Ignore rules updated")
}

func splitSettingList(text string) []string {
	var values []string
	for _, part := range strings.FieldsFunc(text, func(r rune) bool { return r == ';' || r == '\n' || r == '\r' }) {
		part = strings.TrimSpace(part)
		if part != "" {
			values = append(values, part)
		}
	}
	return values
}

func userHomeDirectory() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return home
}

// windowGapPresets is the order the window-gap menu lists, in DIP.
var windowGapPresets = []int{0, 8, 16, 24}

func (c *controller) setWindowGap(gap int) {
	c.windowGap = gap
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("windowManagementGap", gap)
	}
	c.queueSettingsSave()
	c.refreshSettingsWindow()
}

func (c *controller) setCompact(enabled bool) {
	c.compactMode = enabled
	c.view.SetExpanded(!enabled)
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("compactMode", enabled)
	}
	c.queueSettingsSave()
}

func (c *controller) setCustomDirectories(text string) {
	var directories []string
	for _, part := range strings.FieldsFunc(text, func(r rune) bool { return r == ';' || r == '\n' || r == '\r' }) {
		part = strings.TrimSpace(part)
		if part != "" {
			directories = append(directories, part)
		}
	}
	if err := launcher.ValidateDirectories(directories); err != nil {
		c.settingsStatus = c.text("目录不存在，未保存", "A directory is missing; nothing was saved")
		log.Printf("custom app directories rejected: %v", err)
		return
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("customAppDirs", directories)
	}
	c.queueSettingsSave()
	c.settingsStatus = c.text("应用目录已更新", "Application directories updated")
	c.scan()
}
