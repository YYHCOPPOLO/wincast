package app

import (
	"fmt"
	"log"
	"path/filepath"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
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
	c.language = document.String("language", defaultLanguage())
	c.appearance = document.String("appearance", "system")
	c.clampSettings()
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
		for {
			select {
			case <-c.ctx.Done():
				return
			case write := <-c.settingsWrites:
				err := storage.WriteFileAtomic(c.settingsPath, write.data, 0o600)
				c.panel.Update(func() {
					if c.closing {
						return
					}
					if err != nil {
						c.settingsStatus = c.text("设置保存失败；原文件已保留", "Could not save settings; the previous file was kept")
						log.Printf("settings save failed: %v", err)
					} else {
						c.settingsStatus = c.text("已保存", "Saved")
					}
					if c.settings != nil {
						c.settings.Update(func() {})
					}
				})
			}
		}
	}()
}

func (c *controller) openSettings() {
	if c.closing {
		return
	}
	c.hide()
	if c.settings == nil {
		c.settings = mygo.NewWindow(mygo.WindowOptions{
			Title:           "Tinycast Go Preview Settings",
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
	c.settings.Center()
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

func (c *controller) settingsView(ctx *ui.Context) {
	tokens := theme.For(c.dark())
	ctx.SetTheme(tokens.Theme())
	ctx.Root().Background(tokens.Page)
	ui.Column(ctx).Fill().Padding(24).Gap(18).Children(func() {
		ui.Text(ctx, c.text("设置", "Settings")).FontSize(24).Bold().Role(ui.RoleHeading)
		ui.Text(ctx, c.text("这些设置只属于 Go 预览，不影响现有日用版本。", "These settings belong to the Go preview and never change the daily build.")).
			FontSize(theme.SectionFont).TextColor(tokens.TextTertiary)

		components.Card(ctx, tokens, func() {
			components.FormRow(ctx, tokens, c.text("外观", "Appearance"), func() {
				selected := appearanceIndex(c.appearance)
				ui.Segmented(ctx, &selected,
					c.text("跟随系统", "System"), c.text("浅色", "Light"), c.text("深色", "Dark")).Grow(1)
				if value := appearanceValue(selected); value != c.appearance {
					c.setAppearance(value)
				}
			})
			components.FormRow(ctx, tokens, c.text("语言", "Language"), func() {
				selected := 0
				if c.language == "en" {
					selected = 1
				}
				ui.Segmented(ctx, &selected, "中文", "English").Grow(1)
				language := "zh"
				if selected == 1 {
					language = "en"
				}
				if language != c.language {
					c.setLanguage(language)
				}
			})
		})

		components.Card(ctx, tokens, func() {
			ui.Text(ctx, c.text("数据与诊断", "Data and diagnostics")).FontSize(theme.SectionFont).
				TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
			components.FormRow(ctx, tokens, c.text("设置目录", "Settings"), func() {
				ui.Text(ctx, c.profile.settings).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
			})
			components.FormRow(ctx, tokens, c.text("缓存目录", "Cache"), func() {
				ui.Text(ctx, c.profile.cache).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
			})
			components.FormRow(ctx, tokens, c.text("词典库", "Dictionary"), func() {
				ui.Text(ctx, c.profile.dictionary).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
			})
			components.FormRow(ctx, tokens, c.text("索引条目", "Catalog"), func() {
				count := c.text("等待索引", "Waiting for the index")
				if c.catalogInfo.Count > 0 {
					count = fmt.Sprintf("%d", c.catalogInfo.Count)
				}
				ui.Text(ctx, count).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			})
			components.FormRow(ctx, tokens, c.text("剪贴板事件", "Clipboard events"), func() {
				ui.Text(ctx, c.clipboardSummary()).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			})
		})

		status := c.settingsStatus
		if c.settingsErr != nil {
			status = c.text("设置文件无法读取；已禁用写入以保留原文件", "Settings are unreadable; writes are disabled to preserve the file")
		}
		if status != "" {
			ui.Text(ctx, status).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Role(ui.RoleStatus)
		}
	})
}

func (c *controller) setAppearance(value string) {
	c.appearance = value
	c.view.ForcedDark = c.forcedDark()
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("appearance", value)
	}
	c.queueSettingsSave()
}

func (c *controller) setLanguage(value string) {
	c.language = value
	c.view.Chinese = value == "zh"
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("language", value)
	}
	c.queueSettingsSave()
}
