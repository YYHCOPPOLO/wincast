package app

import (
	"fmt"
	"log"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/extensions"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/settings"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/editors"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// extensionAppearance is the launcher icon override for one extension.
type extensionAppearance struct {
	Symbol string `json:"symbol"`
	Tint   string `json:"tint"`
}

var extensionAppearanceSymbols = []string{
	"puzzlepiece.extension", "bolt", "star", "heart", "folder", "globe",
	"gear", "magnifyingglass", "clipboard", "calendar", "clock", "link",
	"person", "bell", "check", "plus", "doc.text", "photo", "terminal",
}

var extensionAppearanceTints = []string{
	"red", "maroon", "rose", "pink", "purple", "indigo", "blue", "cyan",
	"teal", "mint", "green", "lime", "yellow", "orange", "tan", "brown",
	"gray", "slate",
}

func (c *controller) extensionAppearanceFor(name string) *extensionAppearance {
	if appearance, ok := c.extensionAppearances[name]; ok && appearance.Symbol != "" {
		return &appearance
	}
	return nil
}

// setExtensionAppearance stores or clears one launcher icon override.
func (c *controller) setExtensionAppearance(name string, appearance *extensionAppearance) {
	if c.extensionAppearances == nil {
		c.extensionAppearances = make(map[string]extensionAppearance)
	}
	if appearance == nil || appearance.Symbol == "" {
		delete(c.extensionAppearances, name)
	} else {
		c.extensionAppearances[name] = *appearance
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("extensionAppearances", c.extensionAppearances)
	}
	c.queueSettingsSave()
	c.refreshExtras()
	c.updateSettings(func() {})
}

// extensionsSettingsPage is the native Extensions pane: the consent switch,
// the installed set, the three install routes and the preference editor.
func (c *controller) extensionsSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	c.extensionEnableAlert(ctx, tokens, zh)
	c.extensionUninstallAlert(ctx, tokens, zh)
	if c.extensionPrefsOpen {
		c.extensionPreferencesCard(ctx, tokens, zh)
		return
	}
	if c.extensionSearchOpen {
		c.extensionSearchCard(ctx, tokens, zh)
		return
	}
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("扩展", "Extensions")).FontSize(theme.SectionFont).
			TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		ui.Text(ctx, c.text("运行 Raycast 扩展。开启即表示允许第三方 JavaScript 在你的电脑上运行。",
			"Run Raycast extensions. Turning this on is consent to run third-party JavaScript on your computer.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		components.FormRow(ctx, tokens, c.text("启用扩展", "Enable extensions"), func() {
			enabled := c.extensionsEnabled
			ui.Switch(ctx, &enabled).Label(c.text("关闭时不扫描、不加载任何扩展", "Nothing is scanned or loaded while off"))
			if enabled != c.extensionsEnabled {
				if enabled {
					c.extensionEnableConfirm = true
				} else {
					c.setExtensionsEnabled(false)
				}
			}
		})
		components.FormRow(ctx, tokens, c.text("启动器显示", "Show in launcher"), func() {
			enabled := c.extensionsShowInLauncher
			sw := ui.Switch(ctx, &enabled).Label(c.text("在搜索中列出扩展命令", "List extension commands in search"))
			if !c.extensionsEnabled {
				sw.Disabled(true)
			}
			if enabled != c.extensionsShowInLauncher {
				c.setExtensionsShowInLauncher(enabled)
			}
		})
		if c.extensionInstallStatus != "" {
			ui.Text(ctx, c.extensionInstallStatus).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		}
	})

	if !c.extensionsEnabled || c.extensions == nil {
		return
	}
	c.installedExtensionsCard(ctx, tokens, zh)
	c.installExtensionsCard(ctx, tokens, zh)
	c.registryCard(ctx, tokens, zh)
}

func (c *controller) catalogFailures() []string { return nil }

func (c *controller) extensionEnableAlert(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	if !c.extensionEnableConfirm {
		return
	}
	open := true
	choice := ui.AlertDialog(ctx, &open,
		c.text("启用扩展？", "Enable Extensions?"),
		c.text("扩展是第三方 JavaScript，可访问网络、文件和剪贴板。只安装你信任的扩展。",
			"Extensions are third-party JavaScript with access to the network, files and the clipboard. Install only extensions you trust."),
		c.text("取消", "Cancel"), c.text("启用", "Enable"))
	if choice == 1 {
		c.extensionEnableConfirm = false
		c.setExtensionsEnabled(true)
	}
	if choice == ui.AlertDismissed {
		c.extensionEnableConfirm = false
	}
}

func (c *controller) extensionUninstallAlert(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	if c.extensionUninstallConfirm == "" || c.extensions == nil {
		return
	}
	open := true
	choice := ui.AlertDialog(ctx, &open,
		c.text("卸载扩展？", "Uninstall Extension?"),
		c.text("扩展文件、偏好和本地存储都会被删除。", "The extension, its preferences and local storage are removed."),
		c.text("取消", "Cancel"), c.text("卸载", "Uninstall"))
	if choice == 1 {
		name := c.extensionUninstallConfirm
		c.extensionUninstallConfirm = ""
		for _, owner := range c.extensions.Installed() {
			if owner.ID() != name {
				continue
			}
			if err := c.extensions.Uninstall(owner); err != nil {
				c.extensionInstallStatus = err.Error()
			} else {
				c.extensionInstallStatus = c.text("已卸载 ", "Uninstalled ") + owner.Title()
			}
			break
		}
		c.refreshExtras()
		c.updateSettings(func() {})
	}
	if choice == ui.AlertDismissed {
		c.extensionUninstallConfirm = ""
	}
}

func (c *controller) installedExtensionsCard(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	installed := c.extensions.Installed()
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("已安装", "Installed")).FontSize(theme.SectionFont).
			TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		if len(installed) == 0 {
			ui.Text(ctx, c.text("还没有安装扩展。", "No extensions are installed yet.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			return
		}
		for _, owner := range installed {
			ui.Column(ctx).FillWidth().Padding(8, 0).Gap(4).Children(func() {
				ui.Row(ctx).FillWidth().Gap(10).AlignItems(ui.Center).Children(func() {
					ui.Column(ctx).Grow(1).Gap(1).Children(func() {
						ui.Text(ctx, owner.Title()).FontSize(theme.RowFont).FontWeight(600).TextColor(tokens.Text)
						meta := owner.Manifest.Description
						if meta == "" {
							meta = fmt.Sprintf("%s · %d commands", owner.Manifest.Author, len(owner.Manifest.Commands))
						}
						ui.Text(ctx, meta).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
					})
					if ui.Button(ctx, c.text("命令", "Commands")).Clicked() {
						if c.extensionSettingsName == owner.ID() {
							c.extensionSettingsName = ""
							c.extensionSettingsCommand = ""
						} else {
							c.extensionSettingsName = owner.ID()
							c.extensionSettingsCommand = ""
						}
					}
					if ui.Button(ctx, c.text("文件夹", "Folder")).Clicked() {
						_ = mygo.Shell.OpenPath(owner.Directory)
					}
					if ui.Button(ctx, c.text("偏好", "Preferences")).Clicked() {
						c.openExtensionPreferences(owner.ID(), "")
					}
					if ui.Button(ctx, c.text("卸载", "Uninstall")).Clicked() {
						c.extensionUninstallConfirm = owner.ID()
					}
				})
				if c.extensionSettingsName == owner.ID() {
					c.extensionAppearanceRow(ctx, tokens, owner)
					for _, command := range owner.Manifest.Commands {
						entryID := extensions.CommandRef{ExtensionName: owner.ID(), CommandName: command.Name}.EntryID()
						ui.Row(ctx).FillWidth().Padding(0, 8).Gap(8).AlignItems(ui.Center).Children(func() {
							ui.Text(ctx, command.Title).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).Grow(1)
							if command.Mode.Supported() {
								c.extensionCommandShortcutRecorder(ctx, tokens, entryID)
							} else {
								ui.Text(ctx, c.text("菜单栏命令", "Menu bar command")).
									FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
							}
							if ui.Button(ctx, c.text("命令偏好", "Command Preferences")).Clicked() {
								c.openExtensionPreferences(owner.ID(), command.Name)
							}
						})
					}
				}
			})
		}
	})
}

// extensionAppearanceRow edits the launcher icon override: a symbol and a
// tint, or the extension's own artwork when no override is set.
func (c *controller) extensionAppearanceRow(ctx *ui.Context, tokens theme.Tokens, owner *extensions.Installed) {
	appearance := c.extensionAppearanceFor(owner.ID())
	symbol := "puzzlepiece.extension"
	tint := "purple"
	if appearance != nil {
		symbol = appearance.Symbol
		if appearance.Tint != "" {
			tint = appearance.Tint
		}
	}
	ui.Row(ctx).FillWidth().Padding(0, 8).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(ctx, c.text("启动器图标", "Launcher Icon")).FontSize(theme.TrailingFont).
			TextColor(tokens.TextSecondary).Width(120)
		ui.Column(ctx).Width(22).Height(22).Center().Radius(5).
			Background(palette.ExtensionTintColor(tint)).Children(func() {
			ui.Text(ctx, palette.ExtensionSymbolGlyph(symbol)).Font("Segoe Fluent Icons").
				FontSize(13).TextColor(ui.RGBA(255, 255, 255, 255)).TextAlign(ui.Center)
		})
		ui.MenuButton(ctx, symbol, func(menu *ui.Menu) {
			for _, candidate := range extensionAppearanceSymbols {
				if menu.Item(candidate).Checked(candidate == symbol).Chosen() {
					c.setExtensionAppearance(owner.ID(), &extensionAppearance{Symbol: candidate, Tint: tint})
				}
			}
		}).MaxWidth(190)
		ui.MenuButton(ctx, strings.ToUpper(tint[:1])+tint[1:], func(menu *ui.Menu) {
			for _, candidate := range extensionAppearanceTints {
				label := strings.ToUpper(candidate[:1]) + candidate[1:]
				if menu.Item(label).Checked(candidate == tint).Chosen() {
					c.setExtensionAppearance(owner.ID(), &extensionAppearance{Symbol: symbol, Tint: candidate})
				}
			}
		}).MaxWidth(120)
		if appearance != nil {
			if ui.Button(ctx, c.text("恢复", "Reset")).Clicked() {
				c.setExtensionAppearance(owner.ID(), nil)
			}
		}
	})
}

// extensionCommandShortcutRecorder edits one extension command's global
// shortcut. The recording id is namespaced so a built-in recorder and an
// extension recorder can never claim the same session.
func (c *controller) extensionCommandShortcutRecorder(ctx *ui.Context, tokens theme.Tokens, entryID string) {
	recordingID := "extension:" + entryID
	if c.commandShortcutRecording == recordingID {
		capture := ""
		ui.TextInput(ctx, &capture).Width(180).
			Placeholder(c.text("按下组合键…", "Press a shortcut…")).
			AutoFocus().
			OnTextKey(func(key ui.TextKey) bool {
				if key.Mods == 0 && key.Key == ui.KeyEscape {
					c.cancelCommandShortcutRecording()
					return true
				}
				if key.Mods == 0 && key.Key == ui.KeyBackspace {
					c.setExtensionHotkey(entryID, "")
					return true
				}
				accelerator, ok := editors.FormatAccelerator(key.Mods, key.Key)
				if !ok {
					return true
				}
				c.setExtensionHotkey(entryID, accelerator)
				return true
			})
		return
	}
	value := c.extensionHotkey(entryID)
	label := value
	if label == "" {
		label = c.text("设置快捷键…", "Set shortcut…")
	}
	ui.Row(ctx).Gap(6).AlignItems(ui.Center).Children(func() {
		if ui.Button(ctx, label).Clicked() {
			c.beginCommandShortcutRecording(recordingID)
		}
		if value != "" {
			if ui.Button(ctx, c.text("清除", "Clear")).Clicked() {
				c.setExtensionHotkey(entryID, "")
			}
		}
	})
}

func (c *controller) installExtensionsCard(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("安装新扩展", "Install New")).FontSize(theme.SectionFont).
			TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		ui.Row(ctx).Gap(8).Children(func() {
			if ui.Button(ctx, c.text("搜索扩展商店…", "Search Registries…")).Clicked() {
				c.extensionSearchOpen = true
				c.extensionSearchQuery = ""
				c.extensionSearchResults = nil
				c.extensionSearchFailures = nil
			}
			if ui.Button(ctx, c.text("从 Raycast 导入", "Import from Raycast")).Clicked() {
				c.importFromRaycast()
			}
			if ui.Button(ctx, c.text("添加文件夹…", "Add Folder…")).Clicked() {
				c.installFromFolder()
			}
		})
		if c.extensionInstallStatus != "" {
			ui.Text(ctx, c.extensionInstallStatus).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		}
		ui.Text(ctx, c.text("仅复制 package.json、已构建的命令和 assets；从不运行安装脚本。",
			"Only package.json, built command bundles and assets are copied; install scripts never run.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
	})
}

func (c *controller) registryCard(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Text(ctx, c.text("注册表", "Registries")).FontSize(theme.SectionFont).
			TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		for _, registry := range extensions.DefaultRegistries() {
			components.FormRow(ctx, tokens, registry.Name, func() {
				ui.Text(ctx, c.text("已启用", "Enabled")).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			})
		}
		ui.Text(ctx, c.text("GitHub 源安装需要 Node.js 和 pnpm、Bun、Yarn 或 npm；扩展商店安装不需要。",
			"GitHub source installs need Node.js and pnpm, Bun, Yarn or npm; Store installs do not.")).
			FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
	})
}

func (c *controller) extensionSearchCard(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Row(ctx).FillWidth().Gap(8).AlignItems(ui.Center).Children(func() {
			if ui.Button(ctx, c.text("返回", "Back")).Clicked() {
				c.extensionSearchOpen = false
			}
			ui.Text(ctx, c.text("搜索扩展", "Search Extensions")).FontSize(theme.SectionFont).
				TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		})
		ui.Row(ctx).FillWidth().Gap(8).Children(func() {
			ui.TextInput(ctx, &c.extensionSearchQuery).Grow(1).Placeholder(c.text("扩展名称…", "Extension name…"))
			if ui.Button(ctx, c.text("搜索", "Search")).Clicked() {
				c.searchExtensions()
			}
		})
		if c.extensionSearchBusy {
			ui.Row(ctx).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.Spinner(ctx).Size(14, 14)
				ui.Text(ctx, c.text("正在搜索…", "Searching…")).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			})
		}
		for _, failure := range c.extensionSearchFailures {
			ui.Text(ctx, failure).FontSize(theme.TrailingFont).TextColor(tokens.Destructive)
		}
		for index := range c.extensionSearchResults {
			listing := c.extensionSearchResults[index]
			ui.Row(ctx).FillWidth().Padding(8, 0).Gap(10).AlignItems(ui.Center).Children(func() {
				ui.Column(ctx).Grow(1).Gap(1).Children(func() {
					ui.Text(ctx, listing.Title).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
					summary := listing.Summary
					if len(summary) > 140 {
						summary = summary[:140] + "…"
					}
					if summary != "" {
						ui.Text(ctx, summary).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
					}
					meta := fmt.Sprintf("%s · %d commands · %d installs", listing.Author, listing.CommandCount, listing.Downloads)
					ui.Text(ctx, meta).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine()
				})
				if ui.Button(ctx, c.text("安装", "Install")).Clicked() {
					c.installListing(listing)
				}
			})
		}
		if !c.extensionSearchBusy && len(c.extensionSearchResults) == 0 && c.extensionSearchQuery != "" {
			ui.Text(ctx, c.text("没有结果。", "No results.")).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		}
	})
}

func (c *controller) extensionPreferencesCard(ctx *ui.Context, tokens theme.Tokens, zh bool) {
	components.Card(ctx, tokens, func() {
		ui.Row(ctx).FillWidth().Gap(8).AlignItems(ui.Center).Children(func() {
			if ui.Button(ctx, c.text("返回", "Back")).Clicked() {
				c.extensionPrefsOpen = false
			}
			title := c.extensionPrefsName
			if c.extensionPrefsCommand != "" {
				title += " · " + c.extensionPrefsCommand
			}
			ui.Text(ctx, title).FontSize(theme.SectionFont).TextColor(tokens.TextSecondary).Role(ui.RoleHeading)
		})
		owner, command, ok := c.extensionPreferenceOwner()
		if !ok {
			ui.Text(ctx, c.text("扩展未安装。", "The extension is not installed.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			return
		}
		schemas := append([]extension.Preference(nil), owner.Manifest.Preferences...)
		if command != nil {
			schemas = append(schemas, command.Preferences...)
		}
		if len(schemas) == 0 {
			ui.Text(ctx, c.text("此命令没有偏好设置。", "This command has no preferences.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
			return
		}
		c.ensurePreferenceDrafts(owner, schemas)
		for _, schema := range schemas {
			c.extensionPreferenceRow(ctx, tokens, schema)
		}
		ui.Row(ctx).Gap(8).Children(func() {
			if ui.Button(ctx, c.text("保存", "Save")).Clicked() {
				c.saveExtensionPreferences(owner, schemas)
			}
			if ui.Button(ctx, c.text("取消", "Cancel")).Clicked() {
				c.extensionPrefsOpen = false
			}
		})
		if c.extensionPrefsStatus != "" {
			ui.Text(ctx, c.extensionPrefsStatus).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		}
	})
}

func (c *controller) extensionPreferenceOwner() (*extensions.Installed, *extension.Command, bool) {
	if c.extensions == nil || c.extensionPrefsName == "" {
		return nil, nil, false
	}
	for _, owner := range c.extensions.Installed() {
		if owner.ID() != c.extensionPrefsName {
			continue
		}
		if c.extensionPrefsCommand == "" {
			return owner, nil, true
		}
		if command, ok := owner.Command(c.extensionPrefsCommand); ok {
			return owner, &command, true
		}
		return owner, nil, true
	}
	return nil, nil, false
}

func (c *controller) ensurePreferenceDrafts(owner *extensions.Installed, schemas []extension.Preference) {
	if c.extensionPrefsText == nil {
		c.extensionPrefsText = make(map[string]string)
	}
	if c.extensionPrefsBool == nil {
		c.extensionPrefsBool = make(map[string]bool)
	}
	resolved := c.extensions.Storage().ResolvePreferences(owner.ID(), schemas)
	for _, schema := range schemas {
		value := resolved[schema.Name]
		if schema.Kind == extension.PreferenceCheckbox {
			if _, ok := c.extensionPrefsBool[schema.Name]; !ok {
				c.extensionPrefsBool[schema.Name] = value.BoolValue()
			}
			continue
		}
		if _, ok := c.extensionPrefsText[schema.Name]; !ok {
			c.extensionPrefsText[schema.Name] = value.StringValue()
		}
	}
}

func (c *controller) extensionPreferenceRow(ctx *ui.Context, tokens theme.Tokens, schema extension.Preference) {
	components.FormRow(ctx, tokens, schema.DisplayTitle(), func() {
		ui.Column(ctx).Grow(1).Gap(2).Children(func() {
			switch schema.Kind {
			case extension.PreferenceCheckbox:
				checked := c.extensionPrefsBool[schema.Name]
				ui.Switch(ctx, &checked)
				c.extensionPrefsBool[schema.Name] = checked
			case extension.PreferenceDropdown:
				titles := make([]string, 0, len(schema.Options))
				values := make(map[string]string, len(schema.Options))
				current := ""
				for _, option := range schema.Options {
					titles = append(titles, option.Title)
					values[option.Title] = option.Value
					if option.Value == c.extensionPrefsText[schema.Name] {
						current = option.Title
					}
				}
				if current == "" && len(titles) > 0 {
					current = titles[0]
				}
				ui.Select(ctx, &current, titles)
				if value, ok := values[current]; ok {
					c.extensionPrefsText[schema.Name] = value
				}
			case extension.PreferenceFile, extension.PreferenceDirectory:
				ui.Row(ctx).Gap(8).Children(func() {
					value := c.extensionPrefsText[schema.Name]
					input := ui.TextInput(ctx, &value).Grow(1)
					if input.Changed() {
						c.extensionPrefsText[schema.Name] = value
					}
					if ui.Button(ctx, c.text("选择…", "Choose…")).Clicked() {
						paths, err := mygo.Dialog.Open(mygo.OpenDialogOptions{
							Parent: c.settings, Title: schema.DisplayTitle(),
							Directory: schema.Kind == extension.PreferenceDirectory,
							Multiple:  false,
						})
						if err == nil && len(paths) > 0 {
							c.extensionPrefsText[schema.Name] = paths[0]
						}
					}
				})
			default:
				value := c.extensionPrefsText[schema.Name]
				input := ui.TextInput(ctx, &value).Grow(1)
				if input.Changed() {
					c.extensionPrefsText[schema.Name] = value
				}
				if schema.Kind == extension.PreferencePassword {
					input.Password()
				}
				if schema.Placeholder != "" {
					input.Placeholder(schema.Placeholder)
				}
			}
			if schema.Description != "" {
				ui.Text(ctx, schema.Description).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
			}
		})
	})
}

func (c *controller) saveExtensionPreferences(owner *extensions.Installed, schemas []extension.Preference) {
	for _, schema := range schemas {
		if schema.Kind == extension.PreferenceCheckbox {
			value := extension.PreferenceValue{Kind: extension.ValueBool, Bool: c.extensionPrefsBool[schema.Name]}
			c.extensions.Storage().SetPreference(owner.ID(), schema.Name, value.Any())
			continue
		}
		value := extension.PreferenceValue{Kind: extension.ValueString, String: c.extensionPrefsText[schema.Name]}
		c.extensions.Storage().SetPreference(owner.ID(), schema.Name, value.Any())
	}
	c.extensions.Storage().Flush()
	c.extensionPrefsStatus = c.text("偏好已保存", "Preferences saved")
}

func (c *controller) searchExtensions() {
	query := strings.TrimSpace(c.extensionSearchQuery)
	if query == "" || c.extensions == nil || c.extensionSearchBusy {
		return
	}
	c.extensionSearchBusy = true
	c.extensionSearchResults = nil
	c.extensionSearchFailures = nil
	c.updateSettings(func() {})
	go func() {
		listings, failures := c.extensions.SearchRegistries(query)
		c.updateSettings(func() {
			c.extensionSearchBusy = false
			c.extensionSearchResults = listings
			c.extensionSearchFailures = failures
		})
	}()
}

func (c *controller) installListing(listing extensions.Listing) {
	if c.extensions == nil {
		return
	}
	c.extensionInstallStatus = c.text("正在安装 ", "Installing ") + listing.Title + "…"
	c.updateSettings(func() {})
	go func() {
		installed, err := c.extensions.InstallListing(listing)
		c.updateSettings(func() {
			if err != nil {
				c.extensionInstallStatus = err.Error()
				log.Printf("extension install failed: %v", err)
				return
			}
			c.extensionInstallStatus = c.text("已安装 ", "Installed ") + installed.Title()
			c.extensionSearchOpen = false
			if c.panel != nil {
				c.panel.Update(func() {
					c.refreshExtras()
					c.reconcileItemHotkeys()
				})
			}
		})
	}()
}

func (c *controller) installFromFolder() {
	if c.extensions == nil {
		return
	}
	paths, err := mygo.Dialog.Open(mygo.OpenDialogOptions{
		Parent: c.settings, Title: c.text("选择扩展文件夹", "Choose Extension Folder"),
		Directory: true, Multiple: false,
	})
	if err != nil || len(paths) == 0 {
		return
	}
	c.extensionInstallStatus = c.text("正在安装…", "Installing…")
	c.updateSettings(func() {})
	go func() {
		installed, err := c.extensions.InstallFolder(paths[0])
		c.updateSettings(func() {
			if err != nil {
				c.extensionInstallStatus = err.Error()
				return
			}
			c.extensionInstallStatus = c.text("已安装 ", "Installed ") + installed.Title()
			if c.panel != nil {
				c.panel.Update(func() {
					c.refreshExtras()
					c.reconcileItemHotkeys()
				})
			}
		})
	}()
}

func (c *controller) importFromRaycast() {
	if c.extensions == nil {
		return
	}
	c.extensionInstallStatus = c.text("正在从 Raycast 导入…", "Importing from Raycast…")
	c.updateSettings(func() {})
	go func() {
		imported, failures := c.extensions.ImportRaycast()
		c.updateSettings(func() {
			if len(imported) == 0 && len(failures) == 0 {
				c.extensionInstallStatus = c.text("没有找到可导入的 Raycast 扩展", "No Raycast extensions were found")
				return
			}
			c.extensionInstallStatus = fmt.Sprintf(c.text("已导入 %d 个扩展", "Imported %d extensions"), len(imported))
			if len(failures) > 0 {
				c.extensionInstallStatus += " · " + failures[0]
			}
			if c.panel != nil {
				c.panel.Update(func() {
					c.refreshExtras()
					c.reconcileItemHotkeys()
				})
			}
		})
	}()
}

// openExtensionPreferences opens the pane and seeds the editor for one
// extension or command. Empty names open the pane's main view.
func (c *controller) openExtensionPreferences(extensionName, commandName string) {
	c.extensionSettingsName = extensionName
	c.extensionSettingsCommand = commandName
	if extensionName == "" {
		c.extensionPrefsOpen = false
	} else {
		c.extensionPrefsOpen = true
		c.extensionPrefsName = extensionName
		c.extensionPrefsCommand = commandName
		c.extensionPrefsText = nil
		c.extensionPrefsBool = nil
		c.extensionPrefsStatus = ""
	}
	c.settingsTab = settings.Extensions
	c.openSettings()
	if c.settings != nil {
		c.updateSettings(func() {})
	}
}
