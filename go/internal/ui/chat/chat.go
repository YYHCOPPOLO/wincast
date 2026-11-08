// Package chat paints the AI transcript. The transcript follows its tail
// unless the user has scrolled up, and assistant Markdown is rendered by the
// native MyGo Markdown view.
package chat

import (
	"math"

	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
	"github.com/egoist/mygo/ui/markdown"
)

// The options pointer must stay stable across frames so a streaming message
// keeps its parse when it becomes complete.
var renderOptions = &markdown.Options{ShareSelection: true, WrapCode: true}

func Body(c *ui.Context, tokens theme.Tokens, messages []domainai.ChatMessage, notice string, thinking, streaming, chinese bool, scroll *ui.ScrollState) {
	if scroll.Y >= scroll.MaxY {
		scroll.Y = math.MaxFloat32
	}
	ui.Scroll(c).Grow(1).TrackScroll(scroll).Selectable().Children(func() {
		ui.Column(c).FillWidth().Padding(12, 18).Gap(12).Children(func() {
			if len(messages) == 0 && notice == "" && !thinking {
				ui.Text(c, emptyLabel(chinese)).FontSize(theme.SectionFont).
					TextColor(tokens.TextTertiary).Padding(20, 0)
			}
			for index, message := range messages {
				messageRow(c, tokens, message, index == len(messages)-1 && streaming, chinese)
			}
			if thinking {
				ui.Text(c, "◌").FontSize(theme.SectionFont).TextColor(tokens.TextTertiary)
			}
			if notice != "" {
				ui.Text(c, notice).FontSize(theme.TrailingFont).TextColor(tokens.Destructive)
			}
		})
	})
}

func messageRow(c *ui.Context, tokens theme.Tokens, message domainai.ChatMessage, live, chinese bool) {
	if message.Role == domainai.ChatUser {
		ui.Column(c).AlignSelf(ui.End).AlignItems(ui.End).MaxWidthPercent(82).
			Padding(8, 12).Radius(10).Background(tokens.Muted).Children(func() {
			ui.Text(c, message.Text).FontSize(theme.RowFont).TextColor(tokens.Text)
		})
		return
	}
	ui.Column(c).FillWidth().Gap(4).Children(func() {
		ui.Text(c, assistantLabel(chinese)).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		switch {
		case live && message.State == domainai.StateStreaming:
			markdown.Stream(c, message.Text, renderOptions)
		default:
			markdown.View(c, message.Text, renderOptions)
		}
		if message.State == domainai.StateFailed {
			ui.Text(c, failedLabel(chinese)).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		}
	})
}

func emptyLabel(chinese bool) string {
	if chinese {
		return "提问或选择模型后开始对话。"
	}
	return "Ask a question, or choose a model in Settings."
}

func assistantLabel(chinese bool) string {
	if chinese {
		return "AI"
	}
	return "Assistant"
}

func failedLabel(chinese bool) string {
	if chinese {
		return "回复未完成"
	}
	return "Response incomplete"
}
