package app

import (
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/editors"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

func (c *controller) aiSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
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
			if ui.Button(ctx, c.aiModelLabel()).AlignSelf(ui.Start).Clicked() {
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
			if ui.Button(ctx, c.aiRetentionLabel()).AlignSelf(ui.Start).Clicked() {
				c.cycleAIRetention()
			}
		})
		components.FormRow(ctx, tokens, c.text("打开方式", "Opening"), func() {
			if ui.Button(ctx, c.aiOpensToLabel()).AlignSelf(ui.Start).Clicked() {
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
				if ui.Button(ctx, c.aiConnDraftProvider.Title()).AlignSelf(ui.Start).Clicked() {
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
			if ui.Button(ctx, c.text("添加连接…", "Add connection…")).AlignSelf(ui.Start).Clicked() {
				c.startNewAIConnection()
			}
		}
		components.FormRow(ctx, tokens, "ChatGPT", func() {
			ui.Text(ctx, c.text("订阅版暂不可用；请使用 API 连接。",
				"ChatGPT subscription is not available yet. Use an API connection.")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		})
	})
}

func (c *controller) quickActionsSettingsPage(ctx *ui.Context, tokens theme.Tokens, zh bool) {
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
			if ui.Button(ctx, c.quickActionLanguageLabel()).AlignSelf(ui.Start).Clicked() {
				c.cycleQuickActionLanguage()
			}
		})
		components.FormRow(ctx, tokens, c.text("模型", "Model"), func() {
			if ui.Button(ctx, c.quickActionModelLabel()).AlignSelf(ui.Start).Clicked() {
				c.cycleQuickActionModel()
			}
		})
	})
}

func (c *controller) renderWorkflowForm(ctx *ui.Context, tokens theme.Tokens) {
	if c.formModel == nil || !c.formModel.Open {
		return
	}
	editors.Render(ctx, tokens, c.formModel, editors.Callbacks{
		Save:   c.saveWorkflowForm,
		Cancel: c.cancelWorkflowForm,
		Delete: c.deleteWorkflowForm,
		SetRecording: func(recording bool) {
			c.formModel.Recording = recording
			c.formModel.Error = ""
			c.settings.Update(func() {})
		},
		Captured: func(accelerator string) {
			c.formModel.Hotkey = accelerator
			c.formModel.Recording = false
			c.formModel.Dirty = true
			c.settings.Update(func() {})
		},
	})
}
