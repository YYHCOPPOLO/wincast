package app

import (
	"fmt"
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
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
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
	c.language = document.String("language", defaultLanguage())
	c.appearance = document.String("appearance", "system")
	c.featureDictionary = document.Bool("dictionaryEnabled", true)
	c.featureNotes = document.Bool("notesEnabled", true)
	c.featureCommands = document.Bool("commandsEnabled", false)
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
	c.commandsShowInLauncher = document.Bool("commandsShowInLauncher", true)
	c.quicklinksShowInLauncher = document.Bool("quicklinksShowInLauncher", true)
	c.snippetsShowInLauncher = document.Bool("snippetsShowInLauncher", true)
	c.clipboardEnabled.Store(document.Bool("clipboardMonitorEnabled", true))
	c.clipboardRetention = int64(document.Int("clipboardRetentionDays", clipboard.DefaultRetentionDays))
	c.clipboardDisabled = document.Strings("clipboardDisabledApps", clipboard.DefaultDisabledApps)
	c.hotkey = document.String("hotkey", previewShortcut)
	c.hotkeyDraft = c.hotkey
	c.doubleTap = document.String("doubleTapModifier", "none")
	c.customDirsDraft = strings.Join(c.customDirectories(), "; ")
	c.compactMode = document.Bool("compactMode", false)
	c.loadAISettings()
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

func cycleClipboardRetention(days int64) int64 {
	presets := []int64{1, 7, 30, 90, 180, 365, -1}
	for index, preset := range presets {
		if preset == days {
			return presets[(index+1)%len(presets)]
		}
	}
	return 90
}

func (c *controller) setClipboardDisabledApps(text string) {
	var apps []string
	seen := make(map[string]bool)
	for _, part := range strings.FieldsFunc(text, func(r rune) bool {
		return r == ';' || r == '\n' || r == '\r'
	}) {
		part = strings.TrimSpace(part)
		if part == "" || seen[strings.ToLower(part)] {
			continue
		}
		seen[strings.ToLower(part)] = true
		apps = append(apps, part)
	}
	c.clipboardDisabled = apps
	c.clipboardDisabledDraft = strings.Join(apps, "; ")
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("clipboardDisabledApps", apps)
	}
	c.queueSettingsSave()
	if c.clipboard != nil {
		c.clipboard.SetDisabledApps(apps)
	}
	c.settingsStatus = c.text("排除列表已更新", "Excluded apps updated")
}

func (c *controller) settingsView(ctx *ui.Context) {
	tokens := theme.For(c.dark())
	ctx.SetTheme(tokens.Theme())
	ctx.Root().Background(tokens.Page)
	ui.Column(ctx).Fill().Padding(24).Gap(18).Children(func() {
		ui.Text(ctx, c.text("设置", "Settings")).FontSize(24).Bold().Role(ui.RoleHeading)
		ui.Text(ctx, c.text("更改会应用于当前 Tinycast。", "Changes apply to this Tinycast application.")).
			FontSize(theme.SectionFont).TextColor(tokens.TextTertiary)

		ui.Scroll(ctx).Grow(1).TrackScroll(&c.settingsScroll).Children(func() {
			ui.Column(ctx).FillWidth().Gap(18).Children(func() {
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
					ui.Text(ctx, c.text("功能", "Features")).FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
					components.FormRow(ctx, tokens, c.text("词典", "Dictionary"), func() {
						enabled := c.featureDictionary
						ui.Switch(ctx, &enabled).Label(c.text("Tab 查询词典", "Look up words with Tab"))
						if enabled != c.featureDictionary {
							c.setFeature("dictionaryEnabled", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("笔记", "Notes"), func() {
						enabled := c.featureNotes
						ui.Switch(ctx, &enabled).Label(c.text("独立笔记窗口", "Separate notes window"))
						if enabled != c.featureNotes {
							c.setFeature("notesEnabled", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("剪贴板监听", "Clipboard monitor"), func() {
						enabled := c.clipboardEnabled.Load()
						ui.Switch(ctx, &enabled).Label(c.text("保存文本和图片历史", "Keep text and image history"))
						if enabled != c.clipboardEnabled.Load() {
							c.setFeature("clipboardMonitorEnabled", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("文件搜索", "File search"), func() {
						enabled := c.featureFileSearch
						ui.Switch(ctx, &enabled).Label(c.text("搜索文件和文件夹", "Search files and folders"))
						if enabled != c.featureFileSearch {
							c.setFeature("fileSearchEnabled", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("搜索范围", "Search scopes"), func() {
						ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.TextInput(ctx, &c.fileSearchScopesDraft).Grow(1).
								Label(c.text("目录或 ~，用分号分隔", "Directories or ~, separated by semicolons"))
							if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
								c.applyFileSearchScopes()
							}
						})
					})
					components.FormRow(ctx, tokens, c.text("忽略规则", "Ignore rules"), func() {
						ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.TextInput(ctx, &c.fileSearchIgnoresDraft).Grow(1).
								Label(c.text("名称或通配符，用分号分隔", "Names or globs, separated by semicolons"))
							if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
								c.applyFileSearchIgnores()
							}
						})
					})
					components.FormRow(ctx, tokens, c.text("窗口管理", "Window management"), func() {
						enabled := c.featureWindowManagement
						ui.Switch(ctx, &enabled).Label(c.text("移动和调整前台窗口", "Move and resize the frontmost window"))
						if enabled != c.featureWindowManagement {
							c.setFeature("windowManagementEnabled", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("窗口间距", "Window gap"), func() {
						if ui.Button(ctx, c.text(fmt.Sprintf("%d DIP", c.windowGap), fmt.Sprintf("%d DIP", c.windowGap))).Clicked() {
							c.cycleWindowGap()
						}
					})
					components.FormRow(ctx, tokens, c.text("循环半屏", "Cycle halves"), func() {
						enabled := c.windowCycle
						ui.Switch(ctx, &enabled).Label(c.text("重复同一命令时轮换半屏比例", "Cycle half-screen fractions on repeat"))
						if enabled != c.windowCycle {
							c.setFeature("windowManagementCycleOnRepeat", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("显示在启动器", "Show in launcher"), func() {
						enabled := c.windowShowInLauncher
						ui.Switch(ctx, &enabled).Label(c.text("允许搜索窗口命令", "Make window commands searchable"))
						if enabled != c.windowShowInLauncher {
							c.setFeature("windowManagementShowInLauncher", enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("日历", "Calendar"), func() {
						enabled := c.featureCalendar
						ui.Switch(ctx, &enabled).Label(c.text("读取 Windows 日历和会议链接", "Read Windows calendar and meeting links"))
						if enabled != c.featureCalendar {
							c.setFeature("calendarEnabled", enabled)
						}
					})
					if c.featureCalendar {
						components.FormRow(ctx, tokens, c.text("自动入会", "Auto join"), func() {
							enabled := c.autoJoin
							ui.Switch(ctx, &enabled).Label(c.text("会议开始时自动加入", "Join meetings when they start"))
							if enabled != c.autoJoin {
								c.setFeature("autoJoinMeetings", enabled)
							}
						})
						components.FormRow(ctx, tokens, c.text("入会前确认", "Confirm join"), func() {
							enabled := c.autoJoinConfirms
							ui.Switch(ctx, &enabled).Label(c.text("自动入会前询问", "Ask before joining automatically"))
							if enabled != c.autoJoinConfirms {
								c.setFeature("autoJoinConfirms", enabled)
							}
						})
						components.FormRow(ctx, tokens, c.text("提前量", "Join window"), func() {
							if ui.Button(ctx, c.text(fmt.Sprintf("%d 分钟", c.joinWindowMinutes), fmt.Sprintf("%d min", c.joinWindowMinutes))).Clicked() {
								c.cycleJoinWindow()
							}
						})
						components.FormRow(ctx, tokens, c.text("显示在启动器", "Show in launcher"), func() {
							enabled := c.calendarShowInLauncher
							ui.Switch(ctx, &enabled).Label(c.text("允许搜索我的日程", "Make My Schedule searchable"))
							if enabled != c.calendarShowInLauncher {
								c.setFeature("calendarShowInLauncher", enabled)
							}
						})
						if c.calendarError != "" {
							ui.Text(ctx, c.calendarError).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
						}
					}
					components.FormRow(ctx, tokens, c.text("保留时间", "Retention"), func() {
						if ui.Button(ctx, clipboardRetentionLabel(c.language, c.clipboardRetention)).Clicked() {
							c.setClipboardRetention(cycleClipboardRetention(c.clipboardRetention))
						}
					})
					components.FormRow(ctx, tokens, c.text("排除应用", "Excluded apps"), func() {
						ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.TextInput(ctx, &c.clipboardDisabledDraft).Grow(1).
								Label(c.text("按可执行文件名排除，用分号分隔", "Excluded executable names, separated by semicolons"))
							if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
								c.setClipboardDisabledApps(c.clipboardDisabledDraft)
							}
						})
					})
					components.FormRow(ctx, tokens, c.text("剪贴板历史", "Clipboard history"), func() {
						if ui.Button(ctx, c.text("清空历史", "Clear history")).Clicked() {
							c.clearClipboardHistory()
						}
						if c.clipboardError != "" {
							ui.Text(ctx, c.clipboardError).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
						}
					})
					components.FormRow(ctx, tokens, c.text("Emoji 肤色", "Emoji skin tone"), func() {
						if ui.Button(ctx, c.view.EmojiTone.Label()).Clicked() {
							c.cycleEmojiTone()
						}
					})
				})

				c.workflowSettings(ctx, tokens)

				components.Card(ctx, tokens, func() {
					ui.Text(ctx, "AI").FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
					components.FormRow(ctx, tokens, c.text("AI", "AI"), func() {
						enabled := c.aiEnabled
						ui.Switch(ctx, &enabled).Label(c.text("启用 AI 对话与快捷操作", "Enable AI chat and Quick Actions"))
						if enabled != c.aiEnabled {
							c.setAIEnabled(enabled)
						}
					})
					if c.aiConfigError != "" {
						ui.Text(ctx, c.aiConfigError).FontSize(theme.TrailingFont).
							TextColor(tokens.Destructive).SingleLine()
					}
					if !c.aiEnabled {
						ui.Text(ctx, c.text("默认关闭；打开后才会连接 AI 服务并保存对话历史。",
							"Off by default. Enabling starts provider requests and chat history.")).
							FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
						return
					}
					components.FormRow(ctx, tokens, c.text("默认模型", "Default model"), func() {
						if ui.Button(ctx, c.aiModelLabel()).Clicked() {
							c.cycleAIDefaultModel()
						}
					})
					components.FormRow(ctx, tokens, c.text("联网搜索", "Web search"), func() {
						enabled := c.aiWebSearch
						ui.Switch(ctx, &enabled).Label(c.text("OpenRouter 使用 web 插件", "Use the web plugin on OpenRouter"))
						if enabled != c.aiWebSearch {
							c.setAIWebSearch(enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("系统提示", "System prompt"), func() {
						enabled := c.aiSystemPromptEnabled
						ui.Switch(ctx, &enabled).Label(c.text("发送内置 Tinycast 提示", "Send the built-in Tinycast prompt"))
						if enabled != c.aiSystemPromptEnabled {
							c.setAISystemPromptEnabled(enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("提示文本", "Prompt text"), func() {
						ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.TextInput(ctx, &c.aiSystemPromptDraft).Grow(1).
								Label(c.text("附加到内置提示之后", "Appended after the built-in prompt"))
							if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
								c.applyAISystemPrompt(c.aiSystemPromptDraft)
							}
						})
					})
					components.FormRow(ctx, tokens, c.text("历史保留", "History retention"), func() {
						if ui.Button(ctx, c.aiRetentionLabel()).Clicked() {
							c.cycleAIRetention()
						}
					})
					components.FormRow(ctx, tokens, c.text("打开方式", "Opening"), func() {
						if ui.Button(ctx, c.aiOpensToLabel()).Clicked() {
							c.cycleAIOpensTo()
						}
					})
					for index, connection := range c.aiConnections {
						index, connection := index, connection
						components.FormRow(ctx, tokens, connection.Title(), func() {
							ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
								ui.Column(ctx).Grow(1).Gap(1).Children(func() {
									ui.Text(ctx, connection.BaseURL).FontSize(theme.TrailingFont).
										TextColor(tokens.TextSecondary).SingleLine()
									keyState := c.text("未设置密钥", "No key")
									if c.aiConnectionKeySaved(index) {
										keyState = c.text("密钥已保存在本机", "Key saved on this PC")
									}
									ui.Text(ctx, c.aiConnectionModel(connection)+" · "+keyState).
										FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine()
								})
								if ui.Button(ctx, c.text("编辑", "Edit")).Clicked() {
									c.editAIConnection(index)
								}
								if ui.Button(ctx, c.text("移除", "Remove")).Clicked() {
									c.removeAIConnection(index)
								}
							})
						})
					}
					if c.aiConnEditing >= 0 {
						components.FormRow(ctx, tokens, c.text("服务商", "Provider"), func() {
							if ui.Button(ctx, c.aiConnDraftProvider.Title()).Clicked() {
								c.cycleAIConnectionProvider()
							}
						})
						components.FormRow(ctx, tokens, c.text("服务地址", "Base URL"), func() {
							ui.TextInput(ctx, &c.aiConnDraftURL).Grow(1).
								Label(c.text("HTTPS；本机回环可用 HTTP", "HTTPS; loopback may use HTTP"))
						})
						components.FormRow(ctx, tokens, c.text("模型", "Model"), func() {
							ui.TextInput(ctx, &c.aiConnDraftModel).Grow(1).
								Label(c.text("例如 gpt-4.1-mini", "For example gpt-4.1-mini"))
						})
						components.FormRow(ctx, tokens, c.text("API 密钥", "API key"), func() {
							ui.TextInput(ctx, &c.aiConnDraftKey).Grow(1).Password().
								Label(c.text("留空以保留已保存的密钥", "Leave empty to keep the saved key"))
						})
						if c.aiConnDraftErr != "" {
							ui.Text(ctx, c.aiConnDraftErr).FontSize(theme.TrailingFont).
								TextColor(tokens.Destructive).SingleLine()
						}
						ui.Row(ctx).Gap(8).Children(func() {
							if ui.Button(ctx, c.text("保存连接", "Save connection")).Clicked() {
								c.saveAIConnection()
							}
							if ui.Button(ctx, c.text("取消", "Cancel")).Clicked() {
								c.cancelAIConnectionEdit()
							}
						})
					} else {
						if ui.Button(ctx, c.text("添加连接…", "Add connection…")).Clicked() {
							c.startNewAIConnection()
						}
					}
					components.FormRow(ctx, tokens, "ChatGPT", func() {
						ui.Text(ctx, c.text("订阅版暂不可用；请使用 API 连接。",
							"ChatGPT subscription is not available yet. Use an API connection.")).
							FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
					})
				})

				components.Card(ctx, tokens, func() {
					ui.Text(ctx, c.text("快捷操作", "Quick Actions")).FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
					components.FormRow(ctx, tokens, c.text("启用", "Enable"), func() {
						enabled := c.quickActionsEnabled
						ui.Switch(ctx, &enabled).Label(c.text("对选中文本运行 AI 操作", "Run AI actions on selected text"))
						if enabled != c.quickActionsEnabled {
							c.setQuickActionsEnabled(enabled)
						}
					})
					if !c.quickActionsEnabled {
						return
					}
					components.FormRow(ctx, tokens, c.text("翻译语言", "Translate to"), func() {
						if ui.Button(ctx, c.quickActionLanguageLabel()).Clicked() {
							c.cycleQuickActionLanguage()
						}
					})
					components.FormRow(ctx, tokens, c.text("模型", "Model"), func() {
						if ui.Button(ctx, c.quickActionModelLabel()).Clicked() {
							c.cycleQuickActionModel()
						}
					})
				})

				components.Card(ctx, tokens, func() {
					ui.Text(ctx, c.text("输入", "Input")).FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
					components.FormRow(ctx, tokens, c.text("全局热键", "Shortcut"), func() {
						ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.TextInput(ctx, &c.hotkeyDraft).Grow(1).Label(c.text("全局热键", "Global shortcut"))
							if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
								c.setHotkey(c.hotkeyDraft)
							}
						})
					})
					components.FormRow(ctx, tokens, c.text("应用目录", "App directories"), func() {
						ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.TextInput(ctx, &c.customDirsDraft).Grow(1).
								Label(c.text("自定义应用目录，用分号分隔", "Custom application directories, separated by semicolons"))
							if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
								c.setCustomDirectories(c.customDirsDraft)
							}
						})
					})
					components.FormRow(ctx, tokens, c.text("精简模式", "Compact mode"), func() {
						enabled := c.compactMode
						ui.Switch(ctx, &enabled).Label(c.text("只显示搜索栏，Ctrl+E 切换", "Show only the search bar; Ctrl+E toggles"))
						if enabled != c.compactMode {
							c.setCompact(enabled)
						}
					})
					components.FormRow(ctx, tokens, c.text("双击修饰键", "Double-tap modifier"), func() {
						selected := doubleTapIndex(c.doubleTap)
						ui.Segmented(ctx, &selected, c.text("无", "None"), "Ctrl", "Alt", "Shift").Grow(1)
						if value := doubleTapValue(selected); value != c.doubleTap {
							c.setDoubleTap(value)
						}
					})
				})

				components.Card(ctx, tokens, func() {
					ui.Text(ctx, c.text("数据与诊断", "Data and diagnostics")).FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
					components.FormRow(ctx, tokens, c.text("运行状态", "Runtime"), func() {
						ui.Text(ctx, c.text("运行中", "Running")+" · "+c.started.Format("15:04:05")).
							FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
					})
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

func (c *controller) cycleWindowGap() {
	switch c.windowGap {
	case 0:
		c.windowGap = 8
	case 8:
		c.windowGap = 16
	case 16:
		c.windowGap = 24
	default:
		c.windowGap = 0
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("windowManagementGap", c.windowGap)
	}
	c.queueSettingsSave()
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
