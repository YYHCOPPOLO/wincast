package app

import (
	"context"
	"log"
	"strings"
	"time"

	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	aifeature "github.com/YYHCOPPOLO/wincast/go/internal/features/ai"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/quickactions"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const (
	quickActionWidth  = 520
	quickActionHeight = 360
)

// runQuickAction captures the previous window's selection before the palette
// hides, then streams the transformation. Fix Grammar applies in place when
// complete; the other actions keep a preview open by default.
func (c *controller) runQuickAction(action domainai.QuickAction) {
	if c.closing {
		return
	}
	if !c.quickActionsEnabled {
		c.view.PendingToast = c.text("快捷操作已关闭", "Quick Actions are disabled")
		return
	}
	if c.qa == nil {
		c.qa = quickactions.New()
	}
	target := c.target
	if target.Window == 0 {
		target = windows.ForegroundTarget()
	}
	if target.Window == 0 {
		c.view.PendingToast = c.text("没有可编辑的目标窗口", "No editable target window")
		return
	}
	selection := c.quickActionSelection()
	if selection == nil {
		if c.quickActionModel != nil && c.quickActionModel.Kind == domainai.SelectionChatGPT {
			c.view.PendingToast = c.text("ChatGPT 订阅暂未提供。", "ChatGPT chat is not available yet.")
		} else {
			c.view.PendingToast = c.text("请在设置中选择快捷操作模型。", "Choose a Quick Action model in Settings.")
		}
		return
	}
	connection, ok := c.aiConnection(selection.Connection)
	if !ok {
		c.view.PendingToast = c.text("请在设置中选择 API 连接。", "Choose an API connection in Settings.")
		return
	}
	provider, err := aifeature.ProviderFor(connection, selection.Model, c.aiKeys)
	if err != nil {
		c.view.PendingToast = c.aiErrorText(err)
		return
	}
	if c.panel.IsVisible() {
		c.panel.Hide()
	}
	owner := c.panel.NativeHandle()
	c.clipboardSuppress.Store(true)
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		releaseClipboard := func() {
			time.Sleep(150 * time.Millisecond)
			c.clipboardSuppress.Store(false)
		}
		captureCtx, cancel := context.WithTimeout(c.ctx, 2*time.Second)
		text, captureErr := windows.CaptureSelection(captureCtx, target, owner)
		cancel()
		if c.ctx.Err() != nil {
			releaseClipboard()
			return
		}
		if captureErr != nil {
			releaseClipboard()
			log.Printf("quick action selection capture failed: %v", captureErr)
			c.panel.Update(func() {
				if c.closing {
					return
				}
				c.showFailure(c.text("无法读取选中文本", "Could not read the selection"),
					action.Title(), c.text("请选中文本后再试。", "Select text and try again."))
			})
			return
		}
		releaseClipboard()
		if c.ctx.Err() != nil {
			return
		}
		request := domainai.Request{
			Instructions:    action.Instructions(c.quickActionLanguage),
			Messages:        []domainai.Message{domainai.UserMessage(action.Message(text))},
			MaxOutputTokens: 2048,
		}
		preview := !action.ReplacesDirectlyByDefault() || action.AlwaysPreviews()
		streamCtx, generation := c.qa.Begin(action, target, preview)
		c.panel.Update(func() {
			if c.closing {
				return
			}
			c.qaApplied = false
			c.qaScroll.Y = 0
			c.quickActionChanged(generation)
		})
		var lastUpdate time.Time
		provider.Stream(streamCtx, request, func(event domainai.Event) {
			c.qa.Event(generation, event)
			terminal := event.Kind == domainai.EventDone || event.Kind == domainai.EventError
			if terminal || time.Since(lastUpdate) >= 50*time.Millisecond {
				lastUpdate = time.Now()
				c.panel.Update(func() {
					if c.closing {
						return
					}
					c.quickActionChanged(generation)
				})
			}
		})
	}()
}

func (c *controller) quickActionSelection() *domainai.ModelSelection {
	selection := c.quickActionModel
	if selection == nil {
		selection = c.aiDefaultModel
	}
	if selection == nil || !selection.Valid() {
		return nil
	}
	if selection.Kind == domainai.SelectionChatGPT {
		return nil
	}
	return selection
}

// quickActionChanged runs on the UI thread after events or state changes.
func (c *controller) quickActionChanged(generation uint64) {
	if c.qa == nil || c.closing {
		return
	}
	snapshot := c.qa.Snapshot()
	if generation != 0 && snapshot.Generation != generation {
		return
	}
	if snapshot.Preview {
		if c.qaWindow == nil {
			c.createQuickActionWindow()
		}
		if !c.qaWindow.IsVisible() {
			c.showQuickActionWindow()
		} else {
			c.qaWindow.Update(func() {})
		}
	}
	if !snapshot.Preview && snapshot.Accepted && !c.qaApplied {
		c.qaApplied = true
		c.applyQuickActionResult()
		return
	}
	if !snapshot.Preview && !snapshot.Running && !snapshot.Accepted {
		if snapshot.Failed {
			c.showFailure(c.text("快捷操作失败", "The Quick Action failed"),
				snapshot.Action.Title(), c.aiErrorTextText(snapshot.Result))
		}
		c.qa.Reset()
	}
}

func (c *controller) createQuickActionWindow() {
	c.qaWindow = c.newMaterialWindow(mygo.WindowOptions{
		Title:           "Tinycast Quick Action",
		Panel:           true,
		Hidden:          true,
		Frameless:       true,
		Width:           quickActionWidth,
		Height:          quickActionHeight,
		DisableResize:   true,
		BackgroundColor: "light-dark(#ffffff, #232327)",
		Content:         ui.View(c.renderQuickAction),
	})
	c.qaWindow.OnClose(func(event *mygo.CloseEvent) {
		if c.closing {
			return
		}
		event.PreventDefault()
		c.dismissQuickAction()
	})
	c.qaWindow.OnClosed(func() {
		if c.qaWindow != nil {
			c.qaWindow = nil
		}
	})
}

func (c *controller) showQuickActionWindow() {
	if c.qaWindow == nil {
		return
	}
	x, y := c.panel.Position()
	c.qaWindow.SetPosition(x+(int(theme.PanelWidth)-quickActionWidth)/2, y+48)
	showWindow(c.qaWindow)
}

func (c *controller) renderQuickAction(ctx *ui.Context) {
	tokens := theme.ForContext(ctx)
	ctx.SetTheme(tokens.Theme())
	defer ctx.Root().Background(tokens.PanelBackground(ctx.Vibrancy()))
	ctx.OnShortcut(0, ui.KeyEscape, c.dismissQuickAction)
	ctx.OnShortcut(ui.Ctrl, ui.KeyC, c.copyQuickActionResult)
	ctx.OnShortcut(ui.Ctrl, ui.KeyEnter, c.applyQuickActionResult)
	snapshot := c.qa.Snapshot()
	ui.Column(ctx).Fill().Padding(18, 20).Gap(12).Children(func() {
		title := snapshot.Action.Title()
		if snapshot.Running {
			title = snapshot.Action.ProgressTitle()
		}
		ui.Text(ctx, title).FontSize(theme.SectionFont + 2).Bold().Role(ui.RoleHeading)
		if snapshot.Failed {
			ui.Text(ctx, c.text("操作未完成", "The action did not finish")).
				FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
		}
		ui.Scroll(ctx).Grow(1).TrackScroll(&c.qaScroll).Children(func() {
			ui.Column(ctx).FillWidth().Padding(10, 12).Radius(theme.CardRadius).
				Background(tokens.Page).Children(func() {
				text := snapshot.Result
				if snapshot.Failed {
					text = c.aiErrorTextText(text)
				}
				if text == "" {
					if snapshot.Running {
						text = c.text("正在处理…", "Working…")
					} else {
						text = c.text("没有结果", "No result")
					}
				}
				ui.Text(ctx, text).FontSize(theme.RowFont).TextColor(tokens.Text).Selectable()
			})
		})
		ui.Row(ctx).Gap(10).AlignItems(ui.Center).Children(func() {
			if ui.Button(ctx, c.text("替换", "Replace")).Disabled(!snapshot.Accepted || snapshot.Canceled).Clicked() {
				c.applyQuickActionResult()
			}
			if ui.Button(ctx, c.text("复制", "Copy")).Disabled(!snapshot.Accepted).Clicked() {
				c.copyQuickActionResult()
			}
			ui.Text(ctx, c.text("Esc 关闭  Ctrl+Enter 替换", "Esc Close  Ctrl+Enter Replace")).
				FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Grow(1).
				TextAlign(ui.End).SingleLine()
		})
	})
}

func (c *controller) dismissQuickAction() {
	if c.qa != nil {
		c.qa.Cancel()
		c.qa.Reset()
	}
	if c.qaWindow != nil && c.qaWindow.IsVisible() {
		c.qaWindow.Hide()
	}
}

func (c *controller) cancelQuickAction() {
	c.dismissQuickAction()
}

func (c *controller) applyQuickActionResult() {
	if c.qa == nil || c.closing {
		return
	}
	snapshot := c.qa.Snapshot()
	if !snapshot.Accepted || snapshot.Canceled || snapshot.Result == "" {
		return
	}
	owner := c.panel.NativeHandle()
	if c.qaWindow != nil && c.qaWindow.IsVisible() {
		c.qaWindow.Hide()
	}
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		ctx, cancel := context.WithTimeout(c.ctx, 3*time.Second)
		err := c.qa.Apply(ctx, owner)
		cancel()
		if c.ctx.Err() != nil {
			return
		}
		if err != nil {
			log.Printf("quick action replace failed: %v", err)
			c.panel.Update(func() {
				if c.closing {
					return
				}
				c.copyText(snapshot.Result, false)
				c.showFailure(c.text("无法替换选中文本", "Could not replace the selection"),
					snapshot.Action.Title(), c.text("结果已复制到剪贴板，可手动粘贴。",
						"The result was copied to the clipboard; paste it manually."))
			})
		}
		c.panel.Update(func() {
			if c.closing || c.qa == nil {
				return
			}
			if c.qa.Snapshot().Generation == snapshot.Generation {
				c.qa.Reset()
			}
		})
	}()
}

func (c *controller) copyQuickActionResult() {
	if c.qa == nil {
		return
	}
	snapshot := c.qa.Snapshot()
	if strings.TrimSpace(snapshot.Result) == "" {
		return
	}
	c.copyText(snapshot.Result, false)
	c.dismissQuickAction()
}
