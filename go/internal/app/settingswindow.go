package app

import (
	"fmt"
	"runtime"
	"runtime/debug"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/emoji"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/systemaction"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/systemsettings"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/settings"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/editors"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const supportCheckoutURL = "https://buy.polar.sh/polar_cl_NDVFC20DKQpLcNawsh97QzbARBXD3WNn8v35R0mbJmT"
const projectURL = "https://github.com/abue-ammar/tinycast"

// settingsView is the settings window workspace: a sidebar with the original
// four sections and nineteen pages, and one scrollable page at a time.
func (c *controller) settingsView(ctx *ui.Context) {
	tokens := theme.For(c.dark())
	ctx.SetTheme(tokens.Theme())
	ctx.Root().Background(tokens.Page)
	zh := c.language == "zh"
	ui.Row(ctx).Fill().AlignItems(ui.Stretch).Children(func() {
		selected := c.settingsTab.ID()
		sidebar := ui.Sidebar(ctx, &selected, func() {
			for _, section := range settings.AllSections() {
				ui.SidebarSection(ctx, section.Title(zh), nil, func() {
					for _, tab := range section.Tabs() {
						ui.SidebarItem(ctx, tab.ID(), nil, tab.Title(zh))
					}
				})
			}
		}).Width(210).FillHeight()
		if sidebar.Changed() {
			if tab, ok := settings.TabFromID(selected); ok {
				c.settingsTab = tab
				c.settingsScroll = ui.ScrollState{}
				c.settingsStatus = ""
				c.updateSettingsTitle()
			}
		}
		ui.Scroll(ctx).Grow(1).TrackScroll(&c.settingsScroll).Children(func() {
			ui.Column(ctx).FillWidth().Padding(24).Gap(16).Children(func() {
				ui.Text(ctx, c.settingsTab.Title(zh)).FontSize(24).Bold().Role(ui.RoleHeading)
				c.settingsPage(ctx, tokens, zh)
				if c.settingsStatus != "" {
					ui.Text(ctx, c.settingsStatus).FontSize(theme.SectionFont).
						TextColor(tokens.TextTertiary).Role(ui.RoleStatus)
				}
			})
		})
	})
}

func (c *controller) settingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	switch c.settingsTab {
	case settings.General:
		c.generalSettingsPage(ctx, tokens, zh)
	case settings.Applications:
		c.applicationsPage(ctx, tokens, zh)
	case settings.SystemSettings:
		c.systemSettingsPage(ctx, tokens, zh)
	case settings.SystemActions:
		c.systemActionsPage(ctx, tokens, zh)
	case settings.Commands:
		c.commandsPage(ctx, tokens, zh)
	case settings.Quicklinks:
		c.quicklinksPage(ctx, tokens, zh)
	case settings.AI:
		c.aiSettingsPage(ctx, tokens, zh)
	case settings.QuickActions:
		c.quickActionsSettingsPage(ctx, tokens, zh)
	case settings.FileSearch:
		c.fileSearchSettingsPage(ctx, tokens, zh)
	case settings.Notes:
		c.notesSettingsPage(ctx, tokens, zh)
	case settings.Snippets:
		c.snippetsPage(ctx, tokens, zh)
	case settings.WindowManagement:
		c.windowSettingsPage(ctx, tokens, zh)
	case settings.Clipboard:
		c.clipboardSettingsPage(ctx, tokens, zh)
	case settings.Emoji:
		c.emojiSettingsPage(ctx, tokens, zh)
	case settings.Calendar:
		c.calendarSettingsPage(ctx, tokens, zh)
	case settings.Extensions:
		c.extensionsSettingsPage(ctx, tokens, zh)
	case settings.Permissions:
		c.permissionsSettingsPage(ctx, tokens, zh)
	case settings.Backup:
		c.renderBackup(ctx, tokens, zh)
	case settings.About:
		c.aboutSettingsPage(ctx, tokens, zh)
	}
}

func (c *controller) generalSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("全局快捷键", "Global shortcuts")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("应用启动器", "App launcher"), func() {
			ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.TextInput(ctx, &c.hotkeyDraft).Grow(1).Label(c.text("全局热键", "Global shortcut"))
				if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
					c.setHotkey(c.hotkeyDraft)
				}
			})
		})
	})

	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("搜索", "Search")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("学习排序", "Learned ranking"), func() {
			label := c.text("重置…", "Reset…")
			button := ui.Button(ctx, label).AlignSelf(ui.Start)
			if c.launcherPrefs != nil && c.launcherPrefs.RankingEmpty() {
				button.Disabled(true)
			}
			if button.Clicked() {
				c.confirmResetRanking()
			}
		})
		components.FormRow(ctx, tokens, c.text("应用目录", "App directories"), func() {
			ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.TextInput(ctx, &c.customDirsDraft).Grow(1).
					Label(c.text("自定义应用目录，用分号分隔", "Additional app directories, separated by semicolons"))
				if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
					c.setCustomDirectories(c.customDirsDraft)
				}
			})
		})
	})

	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("外观", "Appearance")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("主题", "Theme"), func() {
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
		components.FormRow(ctx, tokens, c.text("紧凑模式", "Compact mode"), func() {
			enabled := c.compactMode
			ui.Switch(ctx, &enabled).Label(c.text("只显示搜索栏，Ctrl+E 切换", "Show only the search bar; Ctrl+E toggles"))
			if enabled != c.compactMode {
				c.setCompact(enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("紧凑模式显示收藏", "Show favorites in compact mode"), func() {
			enabled := c.showFavoritesInCompact
			sw := ui.Switch(ctx, &enabled).Label(c.text("在紧凑栏固定收藏应用", "Pin favorite app slots under the compact bar"))
			if !c.compactMode {
				sw.Disabled(true)
			}
			if enabled != c.showFavoritesInCompact {
				c.setShowFavoritesInCompact(enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("跟随指针", "Follow the pointer"), func() {
			enabled := c.openOnCursorScreen
			ui.Switch(ctx, &enabled).Label(c.text("在指针所在显示器打开面板", "Open the panel on the pointer's display"))
			if enabled != c.openOnCursorScreen {
				c.setOpenOnCursorScreen(enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("拖动以重新放置", "Drag to reposition"), func() {
			enabled := c.paletteDraggable
			ui.Switch(ctx, &enabled).Label(c.text("拖动搜索框上方的细条移动面板", "Drag the thin strip above search to move the panel"))
			if enabled != c.paletteDraggable {
				c.setPaletteDraggable(enabled)
			}
		})
	})

	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("通用", "General")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("登录时启动", "Launch at login"), func() {
			enabled := c.launchAtLogin
			ui.Switch(ctx, &enabled).Label(c.text("登录 Windows 后自动启动", "Start Tinycast automatically when you log in"))
			if enabled != c.launchAtLogin {
				c.setLaunchAtLogin(enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("在托盘显示图标", "Show in tray"), func() {
			enabled := c.showInTray
			ui.Switch(ctx, &enabled).Label(c.text("隐藏后快捷键仍可用", "Shortcuts still work when the icon is hidden"))
			if enabled != c.showInTray {
				c.setShowInTray(enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("返回根搜索", "Pop to Root Search"), func() {
			ui.MenuButton(ctx, c.popToRootLabel(), func(m *ui.Menu) {
				for _, preset := range popToRootPresets {
					if m.Item(c.popToRootChoiceLabel(preset)).Checked(c.popToRootTimeout == preset).Chosen() {
						c.setPopToRoot(preset)
					}
				}
			}).AlignSelf(ui.Start)
		})
		components.FormRow(ctx, tokens, c.text("双击修饰键", "Double-tap modifier"), func() {
			selected := doubleTapIndex(c.doubleTap)
			ui.Segmented(ctx, &selected, c.text("无", "None"), "Ctrl", "Alt", "Shift").Grow(1)
			if value := doubleTapValue(selected); value != c.doubleTap {
				c.setDoubleTap(value)
			}
		})
	})
}

func (c *controller) confirmResetRanking() {
	if c.settingsConfirmRanking {
		return
	}
	c.settingsConfirmRanking = true
	options := mygo.MessageOptions{
		Parent: c.settings, Type: mygo.MessageQuestion,
		Title:        c.text("重置学习排序？", "Reset learned launcher ranking?"),
		Message:      c.text("Tinycast 会重新学习你偏好的结果。", "Tinycast will relearn your preferred results."),
		Buttons:      []string{c.text("取消", "Cancel"), c.text("重置排序", "Reset Ranking")},
		CancelButton: 0, DefaultButton: 0,
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		result, err := mygo.Dialog.Message(options)
		if c.ctx.Err() != nil {
			return
		}
		c.updateUI(func() {
			c.settingsConfirmRanking = false
			if c.closing {
				return
			}
			if err == nil && result.Button == 1 {
				c.resetLearnedRanking()
			}
		})
	}()
}

func (c *controller) settingsApplications() []domain.Application {
	apps := c.catalog
	if query := strings.TrimSpace(c.settingsFilter); query != "" {
		apps = domain.Filter(apps, query)
	}
	if len(apps) > 80 {
		apps = apps[:80]
	}
	return apps
}

func (c *controller) applicationsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("所有应用", "All applications")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
			enabled := c.applicationsShowInLauncher
			ui.Switch(ctx, &enabled).Label(c.text("在启动器搜索结果中显示应用", "Show applications in launcher search"))
			if enabled != c.applicationsShowInLauncher {
				c.setApplicationsVisible(enabled)
			}
		})
		ui.TextInput(ctx, &c.settingsFilter).Grow(1).Label(c.text("筛选应用", "Filter applications")).
			Placeholder(c.text("输入名称筛选", "Filter by name"))
		apps := c.settingsApplications()
		if len(apps) == 0 {
			ui.Text(ctx, c.text("没有匹配的应用", "No matching applications")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		}
		for _, application := range apps {
			c.settingsEntryRow(ctx, tokens, application, zh)
		}
	})
}

func (c *controller) systemSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("Windows 设置", "Windows Settings")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
			enabled := c.systemSettingsShowInLauncher
			ui.Switch(ctx, &enabled).Label(c.text("允许搜索系统设置", "Make System Settings searchable"))
			if enabled != c.systemSettingsShowInLauncher {
				c.setSystemSettingsVisible(enabled)
			}
		})
		for _, pane := range systemsettings.All {
			application := domain.Application{ID: pane.ID, Name: pane.Name, Path: pane.URI, Kind: domain.KindSystemSettings}
			c.settingsEntryRow(ctx, tokens, application, zh)
		}
	})
}

func (c *controller) systemActionsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("系统操作", "System actions")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
			enabled := c.systemActionsShowInLauncher
			ui.Switch(ctx, &enabled).Label(c.text("允许搜索系统操作", "Make system actions searchable"))
			if enabled != c.systemActionsShowInLauncher {
				c.setSystemActionsVisible(enabled)
			}
		})
		for _, id := range systemaction.All {
			application := domain.Application{ID: id.EntryID(), Name: id.Name(zh), Kind: domain.KindSystemAction}
			c.settingsEntryRow(ctx, tokens, application, zh)
		}
	})
}

// settingsEntryRow is the one list row applications, system settings and
// system actions share: name, favorite, visibility and an inline alias editor.
func (c *controller) settingsEntryRow(ctx *ui.Context, tokens theme.Tokens, application domain.Application, zh bool) {
	ui.Row(ctx).FillWidth().Padding(4, 8).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Column(ctx).Grow(1).Gap(1).Children(func() {
			ui.Text(ctx, application.Name).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
			subtitle := application.Path
			if application.Kind == domain.KindSystemSettings || application.Kind == domain.KindSystemAction {
				subtitle = strings.TrimSpace(c.launcherPrefs.Alias(application.ID))
				if subtitle == "" {
					subtitle = c.text("系统条目", "System entry")
				}
			}
			if subtitle == "" {
				subtitle = application.ID
			}
			ui.Text(ctx, subtitle).FontSize(theme.TrailingFont).
				TextColor(tokens.TextTertiary).SingleLine()
		})
		favorite := "☆"
		if c.launcherPrefs.IsFavorite(application.ID) {
			favorite = "★"
		}
		if ui.Button(ctx, favorite).Clicked() {
			c.toggleEntryFavorite(application)
		}
		hidden := c.launcherPrefs.IsHidden(application.ID)
		label := c.text("隐藏", "Hide")
		if hidden {
			label = c.text("显示", "Show")
		}
		if ui.Button(ctx, label).Clicked() {
			c.toggleEntryHidden(application)
		}
		if ui.Button(ctx, c.text("别名", "Alias")).Clicked() {
			c.settingsAliasID = application.ID
			c.settingsAliasDraft = c.launcherPrefs.Alias(application.ID)
			c.refreshSettingsWindow()
		}
	})
	if c.settingsAliasID == application.ID {
		ui.Row(ctx).FillWidth().Padding(4, 32).Gap(8).AlignItems(ui.Center).Children(func() {
			ui.TextInput(ctx, &c.settingsAliasDraft).Grow(1).Label(c.text("别名", "Alias"))
			if ui.Button(ctx, c.text("保存", "Save")).Clicked() {
				c.saveSettingsAlias(application.ID, c.settingsAliasDraft)
			}
			if ui.Button(ctx, c.text("取消", "Cancel")).Clicked() {
				c.settingsAliasID = ""
				c.settingsAliasDraft = ""
				c.refreshSettingsWindow()
			}
		})
	}
}

func (c *controller) commandsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("命令", "Commands")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		c.workflowFeature(ctx, tokens, c.text("启用命令", "Enable commands"), c.featureCommands, func(enabled bool) {
			c.setFeature("commandsEnabled", enabled)
		})
		if !c.featureCommands {
			return
		}
		c.launcherVisibility(ctx, tokens, "commandsShowInLauncher", c.commandsShowInLauncher)
		if c.commands == nil || !c.commands.Available() {
			ui.Text(ctx, c.text("命令库无法读取，原文件已保留", "The command library is unreadable; the file was kept")).
				FontSize(theme.TrailingFont).TextColor(tokens.Destructive)
		} else {
			for _, command := range c.commands.Commands() {
				c.workflowRow(ctx, tokens, command.Name, command.Text, command.Hotkey, "", nil, func() {
					c.editCommand(command)
				}, func() {
					c.confirmWorkflowDelete(editors.Command, command.ID, command.Name)
				})
			}
		}
		if ui.Button(ctx, c.text("新建命令", "New command")).AlignSelf(ui.Start).Clicked() {
			c.newCommand()
		}
	})
	c.renderWorkflowEditor(ctx, tokens)
}

func (c *controller) quicklinksPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("快捷链接", "Quicklinks")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		c.workflowFeature(ctx, tokens, c.text("启用快捷链接", "Enable quicklinks"), c.featureQuicklinks, func(enabled bool) {
			c.setFeature("quicklinksEnabled", enabled)
		})
		if !c.featureQuicklinks {
			return
		}
		c.launcherVisibility(ctx, tokens, "quicklinksShowInLauncher", c.quicklinksShowInLauncher)
		if c.quicklinks == nil || !c.quicklinks.Available() {
			ui.Text(ctx, c.text("快捷链接库无法读取，原文件已保留", "The quicklink library is unreadable; the file was kept")).
				FontSize(theme.TrailingFont).TextColor(tokens.Destructive)
		} else {
			for _, link := range c.quicklinks.Links() {
				subtitle := link.Destination
				if link.Pinned {
					subtitle = c.text("置顶 · ", "Pinned · ") + subtitle
				}
				pinLabel := c.text("置顶", "Pin")
				if link.Pinned {
					pinLabel = c.text("取消置顶", "Unpin")
				}
				c.workflowRow(ctx, tokens, link.Name, subtitle, link.Hotkey, pinLabel, func() {
					c.toggleQuicklinkPin(link)
				}, func() {
					c.editQuicklink(link)
				}, func() {
					c.confirmWorkflowDelete(editors.Quicklink, link.ID, link.Name)
				})
			}
		}
		if ui.Button(ctx, c.text("新建快捷链接", "New quicklink")).AlignSelf(ui.Start).Clicked() {
			c.newQuicklink()
		}
	})
	c.renderWorkflowEditor(ctx, tokens)
}

func (c *controller) snippetsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("片段", "Snippets")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		c.workflowFeature(ctx, tokens, c.text("启用片段", "Enable snippets"), c.featureSnippets, func(enabled bool) {
			c.setSnippetFeature(enabled)
		})
		if !c.featureSnippets {
			return
		}
		c.launcherVisibility(ctx, tokens, "snippetsShowInLauncher", c.snippetsShowInLauncher)
		ui.Text(ctx, c.text("启用即允许在任意应用中监听键入的关键词；不保存按键记录。",
			"Enabling listens for typed keywords in other apps; no key log is kept.")).
			FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary)
		if c.snippetError != "" {
			ui.Text(ctx, c.snippetError).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
		}
		for _, issue := range c.snippetSnapshot.Issues {
			ui.Text(ctx, c.text("片段格式错误：", "Malformed snippet: ")+issue.Path+" · "+issue.Message).
				FontSize(theme.KeyCapFont).TextColor(tokens.Destructive).SingleLine()
		}
		for _, record := range c.snippetSnapshot.Records {
			subtitle := strings.TrimSpace(record.Keyword)
			if subtitle == "" {
				subtitle = c.text("无关键词", "No keyword")
			}
			if !record.Enabled {
				subtitle += c.text(" · 已停用", " · Disabled")
			}
			c.workflowRow(ctx, tokens, record.Name, subtitle, c.snippetHotkey(record.ID), "", nil, func() {
				c.editSnippet(record)
			}, func() {
				c.confirmWorkflowDelete(editors.Snippet, record.ID, record.Name)
			})
		}
		ui.Row(ctx).Gap(8).AlignItems(ui.Center).Children(func() {
			if ui.Button(ctx, c.text("新建片段", "New snippet")).Clicked() {
				c.newSnippet()
			}
			if ui.Button(ctx, c.text("片段目录", "Snippets folder")).Clicked() {
				if err := openExternalURL(c.snippets.Directory()); err != nil {
					c.settingsStatus = c.text("无法打开片段目录", "Could not open the snippets folder")
				}
			}
		})
	})
	c.renderWorkflowEditor(ctx, tokens)
}

func (c *controller) renderWorkflowEditor(ctx *ui.Context, tokens theme.Tokens) {
	if c.formModel != nil && c.formModel.Open {
		c.renderWorkflowForm(ctx, tokens)
	}
}

func (c *controller) notesSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("笔记", "Notes")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		c.workflowFeature(ctx, tokens, c.text("启用笔记", "Enable notes"), c.featureNotes, func(enabled bool) {
			c.setFeature("notesEnabled", enabled)
		})
		if !c.featureNotes {
			return
		}
		components.FormRow(ctx, tokens, c.text("笔记窗口", "Notes window"), func() {
			ui.Row(ctx).Gap(8).Children(func() {
				if ui.Button(ctx, c.text("打开笔记", "Open notes")).Clicked() {
					c.openEditor()
				}
				if ui.Button(ctx, c.text("笔记目录", "Notes folder")).Clicked() {
					c.openNotesFolder()
				}
			})
		})
	})
}

func (c *controller) fileSearchSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("文件搜索", "File search")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("启用", "Enable"), func() {
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
		components.FormRow(ctx, tokens, c.text("忽略规则", "Ignore patterns"), func() {
			ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.TextInput(ctx, &c.fileSearchIgnoresDraft).Grow(1).
					Label(c.text("名称或通配符，用分号分隔", "Names or globs, separated by semicolons"))
				if ui.Button(ctx, c.text("应用", "Apply")).Clicked() {
					c.applyFileSearchIgnores()
				}
			})
		})
	})
}

func (c *controller) windowSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("窗口管理", "Window management")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("启用", "Enable"), func() {
			enabled := c.featureWindowManagement
			ui.Switch(ctx, &enabled).Label(c.text("移动和调整前台窗口", "Move and resize the frontmost window"))
			if enabled != c.featureWindowManagement {
				c.setFeature("windowManagementEnabled", enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
			enabled := c.windowShowInLauncher
			ui.Switch(ctx, &enabled).Label(c.text("允许搜索窗口命令", "Make window commands searchable"))
			if enabled != c.windowShowInLauncher {
				c.setFeature("windowManagementShowInLauncher", enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("循环半屏", "Cycle halves"), func() {
			enabled := c.windowCycle
			ui.Switch(ctx, &enabled).Label(c.text("重复同一命令时轮换半屏比例", "Cycle half-screen fractions on repeat"))
			if enabled != c.windowCycle {
				c.setFeature("windowManagementCycleOnRepeat", enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("窗口间距", "Window gap"), func() {
			ui.MenuButton(ctx, fmt.Sprintf("%d DIP", c.windowGap), func(m *ui.Menu) {
				for _, gap := range windowGapPresets {
					if m.Item(fmt.Sprintf("%d DIP", gap)).Checked(c.windowGap == gap).Chosen() {
						c.setWindowGap(gap)
					}
				}
			}).AlignSelf(ui.Start)
		})
	})
}

func (c *controller) clipboardSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("剪贴板", "Clipboard")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("剪贴板监听", "Clipboard monitor"), func() {
			enabled := c.clipboardEnabled.Load()
			ui.Switch(ctx, &enabled).Label(c.text("保存文本和图片历史", "Keep text and image history"))
			if enabled != c.clipboardEnabled.Load() {
				c.setFeature("clipboardMonitorEnabled", enabled)
			}
		})
		components.FormRow(ctx, tokens, c.text("保留时间", "Retention"), func() {
			ui.MenuButton(ctx, clipboardRetentionLabel(c.language, c.clipboardRetention), func(m *ui.Menu) {
				for _, days := range clipboardRetentionPresets {
					label := clipboardRetentionLabel(c.language, days)
					if m.Item(label).Checked(c.clipboardRetention == days).Chosen() {
						c.setClipboardRetention(days)
					}
				}
			}).AlignSelf(ui.Start)
		})
		components.FormRow(ctx, tokens, c.text("剪贴板历史", "Clipboard history"), func() {
			if ui.Button(ctx, c.text("清空历史", "Clear history")).AlignSelf(ui.Start).Clicked() {
				c.clearClipboardHistory()
			}
			if c.clipboardError != "" {
				ui.Text(ctx, c.clipboardError).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
			}
		})
	})

	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("排除应用", "Excluded apps")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		ui.Text(ctx, c.text("来自这些应用的复制内容不会被记录。",
			"Copies from these applications are not recorded.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		for _, stem := range c.clipboardDisabled {
			stem := stem
			ui.Row(ctx).FillWidth().Padding(4, 8).Gap(8).AlignItems(ui.Center).Children(func() {
				title := c.clipboardExclusionTitle(stem)
				ui.Column(ctx).Grow(1).Gap(1).Children(func() {
					ui.Text(ctx, title).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
					if !strings.EqualFold(title, stem) {
						ui.Text(ctx, stem).FontSize(theme.TrailingFont).
							TextColor(tokens.TextTertiary).SingleLine()
					}
				})
				if ui.Button(ctx, c.text("移除", "Remove")).Clicked() {
					c.removeClipboardExcludedApp(stem)
				}
			})
		}
		if ui.Button(ctx, c.text("添加应用…", "Add Application…")).AlignSelf(ui.Start).Clicked() {
			c.pickClipboardExcludedApp()
		}
	})
}

func (c *controller) emojiSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("表情与符号", "Emoji & Symbols")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("肤色", "Skin tone"), func() {
			ui.MenuButton(ctx, c.view.EmojiTone.Label(), func(m *ui.Menu) {
				for _, tone := range emoji.AllTones {
					if m.Item(tone.Label()).Checked(c.view.EmojiTone == tone).Chosen() {
						c.setEmojiTone(tone)
					}
				}
			}).AlignSelf(ui.Start)
		})
	})
}

func (c *controller) calendarSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("日历", "Calendar")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		components.FormRow(ctx, tokens, c.text("启用", "Enable"), func() {
			enabled := c.featureCalendar
			ui.Switch(ctx, &enabled).Label(c.text("读取 Windows 日历和会议链接", "Read the Windows calendar and meeting links"))
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
				ui.MenuButton(ctx, fmt.Sprintf(c.text("%d 分钟", "%d min"), c.joinWindowMinutes), func(m *ui.Menu) {
					for _, minutes := range joinWindowPresets {
						label := fmt.Sprintf(c.text("%d 分钟", "%d min"), minutes)
						if m.Item(label).Checked(c.joinWindowMinutes == minutes).Chosen() {
							c.setJoinWindow(minutes)
						}
					}
				}).AlignSelf(ui.Start)
			})
			components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
				enabled := c.calendarShowInLauncher
				ui.Switch(ctx, &enabled).Label(c.text("允许搜索我的日程", "Make My Schedule searchable"))
				if enabled != c.calendarShowInLauncher {
					c.setFeature("calendarShowInLauncher", enabled)
				}
			})
			components.FormRow(ctx, tokens, c.text("摄像头预览", "Camera preview"), func() {
				enabled := false
				ui.Switch(ctx, &enabled).Label(c.text("当前构建不提供入会摄像头预览", "Unavailable in this build")).Disabled(true)
			})
			if c.calendarError != "" {
				ui.Text(ctx, c.calendarError).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
			}
		}
	})
}

func (c *controller) extensionsSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("扩展", "Extensions")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		ui.Text(ctx, c.text("此原生构建不运行第三方 JavaScript 扩展；扩展入口保持不可用。",
			"This native build does not run third-party JavaScript extensions; extension entry points stay unavailable.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		components.FormRow(ctx, tokens, c.text("启用扩展", "Enable extensions"), func() {
			enabled := c.extensionsEnabled
			ui.Switch(ctx, &enabled).Label(c.text("当前构建不可用", "Unavailable in this build")).Disabled(true)
		})
		components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
			enabled := c.extensionsShowInLauncher
			ui.Switch(ctx, &enabled).Label(c.text("当前构建不可用", "Unavailable in this build")).Disabled(true)
		})
	})
}

func (c *controller) permissionsSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("权限", "Permissions")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
		for _, permission := range settings.PermissionRows(zh) {
			components.FormRow(ctx, tokens, permission.Title, func() {
				ui.Row(ctx).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
					ui.Text(ctx, permission.Subtitle).FontSize(theme.TrailingFont).
						TextColor(tokens.TextTertiary).Grow(1).SingleLine()
					if ui.Button(ctx, c.text("打开设置", "Open Settings")).Clicked() {
						c.openPermissionSettings(permission.URI)
					}
				})
			})
		}
	})
}

func (c *controller) aboutSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	version := "dev"
	if info, ok := debug.ReadBuildInfo(); ok && info.Main.Version != "" && info.Main.Version != "(devel)" {
		version = info.Main.Version
	}
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, "Tinycast for Windows").FontSize(18).Bold()
		ui.Text(ctx, c.text("一个小型原生启动器。", "A small native launcher.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		components.FormRow(ctx, tokens, c.text("版本", "Version"), func() {
			ui.Text(ctx, version+" · Go "+runtime.Version()).
				FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		})
		components.FormRow(ctx, tokens, c.text("项目主页", "Project"), func() {
			if ui.Button(ctx, projectURL).AlignSelf(ui.Start).Clicked() {
				_ = c.openPermissionSettingsURL(projectURL)
			}
		})
		components.FormRow(ctx, tokens, c.text("支持 Tinycast…", "Support Tinycast…"), func() {
			if ui.Button(ctx, c.text("在 Polar 支持", "Support on Polar")).AlignSelf(ui.Start).Clicked() {
				_ = c.openPermissionSettingsURL(supportCheckoutURL)
			}
		})
		components.FormRow(ctx, tokens, c.text("支持提醒", "Support reminders"), func() {
			enabled := c.supportReminders
			ui.Switch(ctx, &enabled).Label(c.text("偶尔询问是否支持 Tinycast", "Occasionally ask whether to support Tinycast"))
			if enabled != c.supportReminders {
				c.setSupportReminders(enabled)
			}
		})
	})
}

func (c *controller) openPermissionSettingsURL(uri string) error {
	if err := openExternalURL(uri); err != nil {
		c.settingsStatus = c.text("无法打开链接", "Could not open the link")
		return err
	}
	return nil
}
