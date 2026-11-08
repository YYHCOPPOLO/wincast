package app

import (
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime/debug"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	domaincalc "github.com/YYHCOPPOLO/wincast/go/internal/domain/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/emoji"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/backup"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/settings"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

func appVersion() string {
	if info, ok := debug.ReadBuildInfo(); ok && info.Main.Version != "" && info.Main.Version != "(devel)" {
		return info.Main.Version
	}
	return "0.1.0"
}

func (c *controller) renderBackup(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("备份", "Backup")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		ui.Text(ctx, c.text("一个 .tinycast 文件可包含以下类别；API 密钥、AI 对话与缓存永不进入备份。",
			"A single .tinycast file can carry the categories below; API keys, AI chats and caches never travel.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		c.backupCategoryRow(ctx, tokens, c.text("设置与快捷键", "Settings & shortcuts"), &c.backupSettings)
		c.backupCategoryRow(ctx, tokens, c.text("剪贴板历史", "Clipboard history"), &c.backupClipboard)
		c.backupCategoryRow(ctx, tokens, c.text("片段", "Snippets"), &c.backupSnippets)
		c.backupCategoryRow(ctx, tokens, c.text("笔记", "Notes"), &c.backupNotes)
		c.backupCategoryRow(ctx, tokens, c.text("启动器学习", "Launcher learning"), &c.backupLearning)
		ui.Row(ctx).Gap(8).Children(func() {
			export := ui.Button(ctx, c.text("导出…", "Export…"))
			importButton := ui.Button(ctx, c.text("导入…", "Import…"))
			if c.backupBusy {
				export.Disabled(true)
				importButton.Disabled(true)
			}
			if export.Clicked() {
				c.exportBackup()
			}
			if importButton.Clicked() {
				c.importBackup()
			}
		})
		if c.backupStatus != "" {
			ui.Text(ctx, c.backupStatus).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
		}
	})

	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("从 Raycast 导入", "Import from Raycast")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("密码", "Passphrase"), func() {
			ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.TextInput(ctx, &c.raycastPassphrase).Grow(1).Password().
					Label(c.text("Raycast 导出的密码", "Raycast export passphrase"))
				button := ui.Button(ctx, c.text("选择文件并导入", "Choose file and import"))
				if c.backupBusy {
					button.Disabled(true)
				}
				if button.Clicked() {
					c.importRaycast()
				}
			})
		})
		if c.raycastStatus != "" {
			ui.Text(ctx, c.raycastStatus).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
		}
	})

	c.renderMigrationCard(ctx, tokens, zh)
}

func (c *controller) backupCategoryRow(ctx *ui.Context, tokens theme.Tokens, label string, value *bool) {
	components.FormRow(ctx, tokens, label, func() {
		ui.Switch(ctx, value).Label(label)
	})
}

// backupInput snapshots the UI-owned settings and learning values. File-heavy
// categories are collected by fillBackupFiles off the UI thread.
func (c *controller) backupInput(selection backup.Selection) backup.Input {
	input := backup.Input{Version: appVersion(), Selection: selection,
		Settings: make(map[string]json.RawMessage), Learning: make(map[string]json.RawMessage)}
	if c.settingsDocument != nil {
		for _, key := range c.settingsDocument.Keys() {
			if raw, ok := c.settingsDocument.Raw(key); ok {
				input.Settings[key] = raw
			}
		}
	}
	if c.launcherPrefs != nil {
		if data, err := c.launcherPrefs.RankingBytes(); err == nil {
			input.Learning["ranking"] = data
		}
	}
	if data, err := json.Marshal(c.emojiTone); err == nil {
		input.Learning["emoji"] = data
	}
	return input
}

func (c *controller) fillBackupFiles(input *backup.Input) error {
	if input.Selection.Clipboard && c.clipboard != nil {
		store := c.clipboard.Store()
		items := store.AllItems()
		input.Images = make(map[string][]byte)
		for _, item := range items {
			record := backup.ClipboardRecord{CreatedAt: item.CreatedAt, PinnedAt: item.PinnedAt}
			if item.Kind == 0 {
				record.Kind = "text"
				record.Text = item.Text
			} else {
				record.Kind = "image"
				name := filepath.Base(item.ImagePath)
				if name == "" || name == "." || name == string(filepath.Separator) {
					continue
				}
				record.Image = name
				if _, ok := input.Images[name]; !ok {
					data, err := os.ReadFile(item.ImagePath)
					if err != nil {
						return fmt.Errorf("read clipboard image: %w", err)
					}
					input.Images[name] = data
				}
			}
			input.Clipboard = append(input.Clipboard, record)
		}
	}
	if input.Selection.Snippets && c.snippets != nil {
		input.Snippets = make(map[string][]byte)
		entries, err := os.ReadDir(c.snippets.Directory())
		if err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("read snippets: %w", err)
		}
		for _, entry := range entries {
			if entry.IsDir() || !strings.HasSuffix(strings.ToLower(entry.Name()), ".md") {
				continue
			}
			data, err := os.ReadFile(filepath.Join(c.snippets.Directory(), entry.Name()))
			if err != nil {
				return fmt.Errorf("read snippet %s: %w", entry.Name(), err)
			}
			input.Snippets[entry.Name()] = data
		}
	}
	if input.Selection.Notes {
		input.Notes = make(map[string][]byte)
		entries, err := os.ReadDir(c.notesRoot)
		if err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("read notes: %w", err)
		}
		for _, entry := range entries {
			if entry.IsDir() || !strings.HasSuffix(strings.ToLower(entry.Name()), ".md") {
				continue
			}
			data, err := os.ReadFile(filepath.Join(c.notesRoot, entry.Name()))
			if err != nil {
				return fmt.Errorf("read note %s: %w", entry.Name(), err)
			}
			input.Notes[entry.Name()] = data
		}
	}
	if input.Selection.Learning {
		if data, err := os.ReadFile(c.calcHistoryPath); err == nil && len(data) > 0 {
			input.Learning["calculator"] = data
		}
	}
	return nil
}

func (c *controller) exportBackup() {
	if c.backupBusy {
		return
	}
	selection := backup.Selection{
		Settings: c.backupSettings, Clipboard: c.backupClipboard, Snippets: c.backupSnippets,
		Notes: c.backupNotes, Learning: c.backupLearning,
	}
	if len(selection.Categories()) == 0 {
		c.backupStatus = c.text("至少选择一个类别", "Select at least one category")
		return
	}
	input := c.backupInput(selection)
	c.backupBusy = true
	c.backupStatus = c.text("正在准备导出…", "Preparing export…")
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		path, err := mygo.Dialog.Save(mygo.SaveDialogOptions{
			Parent: c.settings, Title: c.text("导出 Tinycast 备份", "Export Tinycast Backup"),
			DefaultPath: "Tinycast-" + time.Now().Format("20060102-150405") + ".tinycast",
			Filters:     []mygo.FileFilter{{Name: "Tinycast Backup", Extensions: []string{"tinycast"}}},
		})
		if err == nil && path != "" {
			err = c.fillBackupFiles(&input)
			if err == nil {
				var data []byte
				data, err = backup.Export(input)
				if err == nil {
					err = storage.WriteFileAtomic(path, data, 0o600)
				}
			}
		}
		c.updateUI(func() {
			c.backupBusy = false
			switch {
			case err != nil:
				c.backupStatus = c.text("导出失败：", "Export failed: ") + err.Error()
				log.Printf("backup export failed: %v", err)
			case path == "":
				c.backupStatus = c.text("已取消导出", "Export cancelled")
			default:
				c.backupStatus = c.text("已导出到 ", "Exported to ") + path
			}
		})
	}()
}

func (c *controller) importBackup() {
	if c.backupBusy {
		return
	}
	c.backupBusy = true
	c.backupStatus = c.text("正在选择备份…", "Choosing a backup…")
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		paths, err := mygo.Dialog.Open(mygo.OpenDialogOptions{
			Parent: c.settings, Title: c.text("导入 Tinycast 备份", "Import Tinycast Backup"),
			Filters: []mygo.FileFilter{{Name: "Tinycast Backup", Extensions: []string{"tinycast", "zip"}}},
		})
		var bundle *backup.Bundle
		if err == nil && len(paths) > 0 {
			var data []byte
			data, err = os.ReadFile(paths[0])
			if err == nil {
				bundle, err = backup.Read(data)
			}
		}
		clipboardApplied := 0
		if err == nil && bundle != nil && c.clipboard != nil {
			clipboardApplied = c.importBackupClipboard(bundle)
		}
		c.updateUI(func() {
			if c.closing {
				return
			}
			c.backupBusy = false
			if err != nil {
				c.backupStatus = c.text("导入失败：", "Import failed: ") + err.Error()
				log.Printf("backup import failed: %v", err)
				return
			}
			if bundle == nil {
				c.backupStatus = c.text("已取消导入", "Import cancelled")
				return
			}
			c.applyBackup(bundle, clipboardApplied)
		})
	}()
}

// importBackupClipboard restores clips on the worker; the store serializes its
// own access, and a long history never blocks the settings window.
func (c *controller) importBackupClipboard(bundle *backup.Bundle) int {
	store := c.clipboard.Store()
	applied := 0
	for _, item := range bundle.Clipboard {
		if item.Kind == "image" {
			png, ok := bundle.Images[item.Image]
			if ok && store.ImportImage(png, item.CreatedAt, item.PinnedAt) {
				applied++
			}
			continue
		}
		if store.ImportItem(clipboard.Item{Kind: clipboard.KindText, Text: item.Text, CreatedAt: item.CreatedAt, PinnedAt: item.PinnedAt}) {
			applied++
		}
	}
	return applied
}

func (c *controller) importRaycast() {
	if c.backupBusy {
		return
	}
	passphrase := c.raycastPassphrase
	c.backupBusy = true
	c.raycastStatus = c.text("正在选择 Raycast 导出…", "Choosing a Raycast export…")
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		paths, err := mygo.Dialog.Open(mygo.OpenDialogOptions{
			Parent: c.settings, Title: c.text("选择 Raycast 导出", "Choose a Raycast export"),
			Filters: []mygo.FileFilter{{Name: "Raycast", Extensions: []string{"rayconfig"}}, {Name: "All files", Extensions: []string{"*"}}},
		})
		var payload *backup.RaycastPayload
		if err == nil && len(paths) > 0 {
			var data []byte
			data, err = os.ReadFile(paths[0])
			if err == nil {
				payload, err = backup.ReadRaycast(data, passphrase)
			}
		}
		c.updateUI(func() {
			if c.closing {
				return
			}
			c.backupBusy = false
			switch {
			case errors.Is(err, backup.ErrRaycastPass):
				c.raycastStatus = c.text("密码错误", "Wrong passphrase")
			case errors.Is(err, backup.ErrRaycastFormat):
				c.raycastStatus = c.text("不是 Raycast 导出文件", "Not a Raycast export")
			case err != nil:
				c.raycastStatus = c.text("Raycast 导入失败：", "Raycast import failed: ") + err.Error()
			case payload == nil:
				c.raycastStatus = c.text("已取消导入", "Import cancelled")
			default:
				c.applyRaycast(payload)
			}
		})
	}()
}

func (c *controller) applyRaycast(payload *backup.RaycastPayload) {
	settingsApplied := 0
	if c.settingsDocument != nil {
		for key, value := range payload.Settings {
			c.settingsDocument.SetRaw(key, value)
			settingsApplied++
		}
		if settingsApplied > 0 {
			c.readSettingsDocument()
			c.clampSettings()
			c.applySettingsEffects()
			c.queueSettingsSave()
		}
	}
	snippetsApplied := 0
	for _, snippet := range payload.Snippets {
		if _, err := c.snippets.Create(snippet.Name, snippet.Keyword, true, false, snippet.Text); err != nil {
			log.Printf("raycast snippet import failed name=%q error=%v", snippet.Name, err)
			continue
		}
		snippetsApplied++
	}
	if snippetsApplied > 0 {
		c.reloadSnippets()
		c.refreshExtras()
	}
	status := fmt.Sprintf(c.text("已导入 %d 项设置、%d 个片段", "Imported %d settings, %d snippets"), settingsApplied, snippetsApplied)
	if len(payload.Unsupported) > 0 {
		status += c.text("；未导入：", "; not imported: ") + strings.Join(payload.Unsupported, ", ")
	}
	c.raycastStatus = status
	c.refreshSettingsWindow()
}

func (c *controller) applyBackup(bundle *backup.Bundle, clipboardApplied int) {
	summary := make([]string, 0, 5)
	settingsApplied := 0
	if applied := backup.ApplyableSettings(bundle.Settings); len(applied) > 0 && c.settingsDocument != nil {
		for key, value := range applied {
			c.settingsDocument.SetRaw(key, value)
			settingsApplied++
		}
		c.readSettingsDocument()
		c.clampSettings()
		c.applySettingsEffects()
		c.queueSettingsSave()
	}
	summary = append(summary, fmt.Sprintf(c.text("%d 项设置", "%d settings"), settingsApplied))

	if clipboardApplied > 0 {
		c.refreshClipboardView()
	}
	summary = append(summary, fmt.Sprintf(c.text("%d 条剪贴板", "%d clips"), clipboardApplied))

	snippetsApplied := 0
	for name, data := range bundle.Snippets {
		if _, err := c.snippets.ImportFile(name, data); err != nil {
			log.Printf("backup snippet import failed name=%q error=%v", name, err)
			continue
		}
		snippetsApplied++
	}
	if snippetsApplied > 0 {
		c.reloadSnippets()
		c.refreshExtras()
	}
	summary = append(summary, fmt.Sprintf(c.text("%d 个片段", "%d snippets"), snippetsApplied))

	notesApplied := 0
	for name, data := range bundle.Notes {
		if err := importNoteFile(c.notesRoot, name, data); err != nil {
			log.Printf("backup note import failed name=%q error=%v", name, err)
			continue
		}
		notesApplied++
	}
	if notesApplied > 0 {
		c.notes = storage.NewFileNotes(c.notesRoot)
	}
	summary = append(summary, fmt.Sprintf(c.text("%d 条笔记", "%d notes"), notesApplied))

	learningApplied := 0
	if data, ok := bundle.Learning["ranking"]; ok && c.launcherPrefs != nil {
		var records []domain.RankingRecord
		if err := json.Unmarshal(data, &records); err == nil {
			if err := c.launcherPrefs.ReplaceRanking(records); err == nil {
				learningApplied++
			}
		}
	}
	if data, ok := bundle.Learning["emoji"]; ok {
		var tone string
		if json.Unmarshal(data, &tone) == nil {
			c.emojiTone = tone
			c.clampEmojiTone()
			c.view.EmojiTone = emoji.ToneFromRaw(c.emojiTone)
			learningApplied++
		}
	}
	if data, ok := bundle.Learning["calculator"]; ok && len(data) > 0 {
		if err := storage.WriteFileAtomic(c.calcHistoryPath, data, 0o600); err == nil {
			c.calcHistory = domaincalc.LoadHistory(c.calcHistoryPath)
			learningApplied++
		}
	}
	summary = append(summary, fmt.Sprintf(c.text("%d 项学习数据", "%d learning entries"), learningApplied))

	c.backupStatus = c.text("已导入：", "Imported: ") + strings.Join(summary, " · ")
	if len(bundle.Skipped) > 0 {
		c.backupStatus += c.text("；不支持：", "; unsupported: ") + strings.Join(bundle.Skipped, ", ")
	}
	c.view.Filter()
	c.refreshSettingsWindow()
}

func importNoteFile(root, name string, data []byte) error {
	base := filepath.Base(strings.ReplaceAll(name, "\\", "/"))
	base = strings.TrimSpace(base)
	if base == "" || base == "." || base == ".." {
		return fmt.Errorf("invalid note name")
	}
	if !strings.HasSuffix(strings.ToLower(base), ".md") {
		base += ".md"
	}
	stem := strings.TrimSuffix(base, filepath.Ext(base))
	if err := os.MkdirAll(root, 0o700); err != nil {
		return err
	}
	for suffix := 1; suffix <= 10000; suffix++ {
		candidate := base
		if suffix > 1 {
			candidate = fmt.Sprintf("%s-%d.md", stem, suffix)
		}
		path := filepath.Join(root, candidate)
		handle, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			if os.IsExist(err) {
				continue
			}
			return err
		}
		_, writeErr := handle.Write(data)
		if writeErr == nil {
			writeErr = handle.Sync()
		}
		closeErr := handle.Close()
		if writeErr != nil || closeErr != nil {
			os.Remove(path)
			return writeErr
		}
		return nil
	}
	return fmt.Errorf("could not claim a note file name")
}

// applySettingsEffects reprojects imported settings into the live services.
func (c *controller) applySettingsEffects() {
	c.applyViewAppearance()
	c.view.Chinese = c.language == "zh"
	c.view.Shortcut = c.hotkey
	c.view.Expanded = !c.compactMode
	c.view.ShowFavoritesInCompact = c.showFavoritesInCompact
	c.view.Draggable = c.paletteDraggable
	if c.clipboard != nil {
		c.clipboard.SetDisabledApps(c.clipboardDisabled)
		c.clipboard.ApplyRetention(c.clipboardRetention)
	}
	c.applyDoubleTap()
	c.applyHotkey()
	c.applyKeywordListener()
	if c.fileSearch != nil {
		c.fileSearch.Apply(c.fileSearchScopes, c.fileSearchIgnores)
	}
	c.refreshExtras()
	if c.launcher != nil {
		c.launcher.SetApplicationsEnabled(c.applicationsShowInLauncher)
	}
	c.reconcileItemHotkeys()
	c.refreshAIKeyStatus()
	c.view.AIAvailable = c.aiEnabled
	if c.aiEnabled {
		c.openAIHistory()
	} else {
		c.closeAIHistory()
	}
	c.refreshAIChatView()
	c.refreshCompactFavorites()
	c.refreshSettingsWindow()
	if c.showInTray && c.tray == nil {
		if err := c.setupTray(); err != nil {
			log.Printf("tray restore failed: %v", err)
		}
	} else if !c.showInTray && c.tray != nil {
		c.tray.Destroy()
		c.tray = nil
	}
	if c.launchAtLogin {
		if err := windows.ApplyLaunchAtLogin(settings.LaunchAtLoginKey, true); err != nil {
			log.Printf("launch at login apply failed: %v", err)
		}
	} else if windows.LaunchAtLoginMatches(settings.LaunchAtLoginKey) {
		if err := windows.ApplyLaunchAtLogin(settings.LaunchAtLoginKey, false); err != nil {
			log.Printf("launch at login removal failed: %v", err)
		}
	}
}
