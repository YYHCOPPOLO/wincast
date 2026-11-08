package app

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/quicklink"
	domainsnippet "github.com/YYHCOPPOLO/wincast/go/internal/domain/snippet"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/commands"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/launcher"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/migration"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/quicklinks"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

func (c *controller) migrationTarget() migration.Target {
	return migration.Target{SettingsDir: c.profile.settings, DataDir: c.profile.data}
}

// legacyClipboardSource is the directory the legacy application used for its
// clipboard database and images.
func legacyClipboardSource() string {
	local, err := os.UserCacheDir()
	if err != nil {
		return ""
	}
	return filepath.Join(local, storage.DailyID)
}

func (c *controller) renderMigrationCard(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	if !c.migrationSnapshotsLoaded {
		if snapshots, err := migration.Snapshots(c.profile.settings); err == nil {
			c.migrationSnapshots = snapshots
		}
		c.migrationSnapshotsLoaded = true
	}
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("旧版数据", "Legacy data")).FontSize(theme.SectionFont).
			TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		ui.Text(ctx, c.text("从旧版 Windows 应用导入设置、命令、快捷链接、片段、笔记、启动器学习和 AI 历史；迁移前会先快照当前配置。",
			"Import settings, commands, quicklinks, snippets, notes, launcher learning and AI history from the legacy Windows app; the current profile is snapshotted first.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		components.FormRow(ctx, tokens, c.text("来源目录", "Source folder"), func() {
			ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.TextInput(ctx, &c.migrationSource).Grow(1).Label(c.text("旧版设置目录", "Legacy settings folder"))
				previewButton := ui.Button(ctx, c.text("检测", "Inspect"))
				migrateButton := ui.Button(ctx, c.text("迁移…", "Migrate…"))
				if c.migrationBusy {
					previewButton.Disabled(true)
					migrateButton.Disabled(true)
				}
				if previewButton.Clicked() {
					c.inspectLegacy()
				}
				if migrateButton.Clicked() {
					c.migrateLegacy()
				}
			})
		})
		if preview := c.migrationPreview; preview != nil {
			line := fmt.Sprintf(c.text("设置 %d 项 · 命令 %d · 快捷链接 %d · 片段 %d · 笔记 %d · 剪贴板 %d 条 · 收藏 %d · 隐藏 %d · 学习 %d 条",
				"settings %d · commands %d · quicklinks %d · snippets %d · notes %d · clips %d · favorites %d · hidden %d · learning %d"),
				preview.SettingsFields, preview.Commands, preview.Quicklinks, preview.Snippets, preview.Notes,
				preview.ClipboardItems, preview.Favorites, preview.Hidden, preview.Ranking)
			ui.Text(ctx, line).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			ai := c.text("AI 历史：无", "AI history: none")
			if preview.AIHistory {
				ai = c.text("AI 历史：有", "AI history: present")
			}
			if preview.AIKeys > 0 {
				ai += c.text(fmt.Sprintf(" · 密钥 %d", preview.AIKeys), fmt.Sprintf(" · keys %d", preview.AIKeys))
			}
			ui.Text(ctx, ai).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
			for _, warning := range preview.Warnings {
				ui.Text(ctx, warning).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
			}
		}
		if c.migrationStatus != "" {
			ui.Text(ctx, c.migrationStatus).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
		}
	})
	if len(c.migrationSnapshots) > 0 {
		components.Card(ctx, tokens, func() {
			ui.Text(ctx, c.text("快照", "Snapshots")).FontSize(theme.SectionFont).
				TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
			ui.Text(ctx, c.text("恢复只覆盖快照内的文件；迁移后新增的数据不会被删除，需先导出。",
				"Restoring only overwrites files in the snapshot; data created after migration is kept, so export it first.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
			limit := min(len(c.migrationSnapshots), 5)
			for index := 0; index < limit; index++ {
				snapshot := c.migrationSnapshots[index]
				components.FormRow(ctx, tokens, snapshot.At.Format("2006-01-02 15:04:05"), func() {
					button := ui.Button(ctx, c.text("恢复…", "Restore…")).AlignSelf(ui.Start)
					if c.migrationBusy {
						button.Disabled(true)
					}
					if button.Clicked() {
						c.confirmRestoreSnapshot(snapshot.Path)
					}
				})
			}
		})
	}
}

func (c *controller) inspectLegacy() {
	if c.migrationBusy {
		return
	}
	source := strings.TrimSpace(c.migrationSource)
	c.migrationBusy = true
	c.migrationStatus = c.text("正在检测…", "Inspecting…")
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		preview, err := migration.PreviewOf(source, c.profile.settings, legacyClipboardSource())
		c.updateUI(func() {
			if c.closing {
				return
			}
			c.migrationBusy = false
			if err != nil {
				c.migrationPreview = nil
				c.migrationStatus = c.text("检测失败：", "Inspection failed: ") + err.Error()
				return
			}
			c.migrationPreview = &preview
			c.migrationStatus = c.text("检测完成", "Inspection complete")
		})
	}()
}

func (c *controller) migrateLegacy() {
	if c.migrationBusy {
		return
	}
	source := strings.TrimSpace(c.migrationSource)
	if source == "" {
		c.migrationStatus = c.text("请先填写来源目录", "Choose a source folder first")
		return
	}
	c.migrationBusy = true
	c.migrationStatus = c.text("正在迁移…", "Migrating…")
	clipboardSource := legacyClipboardSource()
	skipAI := c.aiEnabled
	target := c.migrationTarget()
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := migration.Apply(migration.Options{
			Source: source, Target: target,
			Resolve: c.resolveLegacyID, SkipAIHistory: skipAI,
		})
		var clipboardErr error
		clipboardImported := 0
		if err == nil && c.clipboard != nil {
			clipboardImported, clipboardErr = c.importLegacyClipboard(clipboardSource)
			if clipboardErr != nil {
				result.Warnings = append(result.Warnings, c.text("剪贴板导入失败：", "clipboard import failed: ")+clipboardErr.Error())
			}
		}
		c.updateUI(func() {
			if c.closing {
				return
			}
			c.migrationBusy = false
			if err != nil {
				c.migrationStatus = c.text("迁移失败：", "Migration failed: ") + err.Error()
				log.Printf("legacy migration failed: %v", err)
				return
			}
			c.reloadAfterMigration()
			c.migrationSnapshotsLoaded = false
			c.migrationPreview = nil
			c.migrationStatus = fmt.Sprintf(c.text("迁移完成：设置 %d、片段 %d、笔记 %d、收藏 %d、学习 %d、剪贴板 %d；快照 %s",
				"Migrated: settings %d, snippets %d, notes %d, favorites %d, learning %d, clips %d; snapshot %s"),
				result.Settings, result.Snippets, result.Notes, result.Favorites, result.Ranking, clipboardImported, result.Snapshot)
			if len(result.Warnings) > 0 {
				c.migrationStatus += c.text("；提示：", "; notes: ") + strings.Join(result.Warnings, "；")
			}
		})
	}()
}

func (c *controller) importLegacyClipboard(source string) (int, error) {
	items, err := clipboard.ReadLegacy(c.ctx, source)
	if err != nil {
		return 0, err
	}
	store := c.clipboard.Store()
	imported := 0
	for _, item := range items {
		if item.Kind == clipboard.KindImage {
			data, err := os.ReadFile(item.ImagePath)
			if err != nil {
				continue
			}
			if store.ImportImage(data, item.CreatedAt, item.PinnedAt) {
				imported++
			}
			continue
		}
		if store.ImportItem(item) {
			imported++
		}
	}
	return imported, nil
}

// reloadAfterMigration reloads every store whose files a migration replaced.
func (c *controller) reloadAfterMigration() {
	document, err := storage.LoadDocument(c.settingsPath)
	if err == nil {
		c.settingsDocument = document
		c.settingsWritable = true
		c.readSettingsDocument()
		c.clampSettings()
		c.applySettingsEffects()
	}
	c.commands = commands.Open(filepath.Join(c.profile.settings, "custom-commands.json"))
	c.quicklinks = quicklinks.Load(filepath.Join(c.profile.settings, "quicklinks.json"))
	c.reloadSnippets()
	c.notes = storage.NewFileNotes(c.notesRoot)
	if preferences, err := launcher.LoadPreferences(filepath.Join(c.profile.settings, "launcher")); err == nil {
		c.launcherPrefs = preferences
	}
	c.launcher = launcher.New(c.launcherPrefs)
	c.queueSettingsSave()
	c.scan()
	c.refreshExtras()
	c.view.Filter()
	c.refreshClipboardView()
}

func (c *controller) resolveLegacyID(old string) (string, bool) {
	if old == "" {
		return "", false
	}
	if application, ok := c.launcher.Application(old); ok {
		return application.ID, true
	}
	prefix, rest, hasPrefix := strings.Cut(old, ":")
	if !hasPrefix {
		return "", false
	}
	rest = strings.TrimSpace(rest)
	equal := func(left, right string) bool { return strings.EqualFold(left, right) }
	baseName := func(path string) string {
		name := filepath.Base(strings.ReplaceAll(path, "/", "\\"))
		return strings.TrimSuffix(name, filepath.Ext(name))
	}
	switch strings.ToLower(prefix) {
	case "app":
		for _, application := range c.catalog {
			if equal(application.ID, rest) || equal(application.Name, rest) || equal(baseName(application.Path), rest) {
				return application.ID, true
			}
		}
	case "command":
		if c.commands != nil {
			for _, command := range c.commands.Commands() {
				if equal(command.ID, rest) || equal(command.Name, rest) {
					return commands.EntryID(command.ID), true
				}
			}
		}
	case "quicklink":
		if c.quicklinks != nil {
			for _, link := range c.quicklinks.Links() {
				if equal(link.ID, rest) || equal(link.Name, rest) {
					return quicklink.EntryID(link.ID), true
				}
			}
		}
	case "snippet":
		for _, record := range c.snippetSnapshot.Records {
			if equal(record.ID, rest) || equal(record.Name, rest) || equal(baseName(record.ID), rest) {
				return domainsnippet.EntryID(record.ID), true
			}
		}
	}
	return "", false
}

func (c *controller) confirmRestoreSnapshot(path string) {
	if c.migrationBusy || c.migrationRestoreConfirm != "" {
		return
	}
	c.migrationRestoreConfirm = path
	options := mygo.MessageOptions{
		Parent: c.settings, Type: mygo.MessageWarning,
		Title:   c.text("恢复快照？", "Restore snapshot?"),
		Message: c.text("将用快照覆盖当前设置与数据文件；迁移后新增的文件会保留。", "The snapshot will overwrite current settings and data files; files created later are kept."),
		Buttons: []string{c.text("取消", "Cancel"), c.text("恢复", "Restore")}, CancelButton: 0, DefaultButton: 0,
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := mygo.Dialog.Message(options)
		if c.ctx.Err() != nil {
			return
		}
		c.updateUI(func() {
			snapshot := c.migrationRestoreConfirm
			c.migrationRestoreConfirm = ""
			if c.closing || err != nil || result.Button != 1 || snapshot == "" {
				return
			}
			c.restoreSnapshot(snapshot)
		})
	}()
}

func (c *controller) restoreSnapshot(path string) {
	c.migrationBusy = true
	c.migrationStatus = c.text("正在恢复快照…", "Restoring snapshot…")
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		err := migration.Restore(c.profile.settings, c.profile.data, path)
		c.updateUI(func() {
			if c.closing {
				return
			}
			c.migrationBusy = false
			if err != nil {
				c.migrationStatus = c.text("恢复失败：", "Restore failed: ") + err.Error()
				log.Printf("snapshot restore failed: %v", err)
				return
			}
			c.reloadAfterMigration()
			c.migrationStatus = c.text("快照已恢复；迁移后新增的数据仍保留", "Snapshot restored; data created after migration is kept")
		})
	}()
}
