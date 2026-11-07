package app

import (
	"log"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/emoji"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

func (c *controller) clampEmojiTone() {
	tone := emoji.ToneFromRaw(c.emojiTone)
	c.emojiTone = tone.Raw()
}

func (c *controller) emojiSearch(query string, tone emoji.SkinTone) []emoji.Emoji {
	return emoji.Search(query, tone)
}

func (c *controller) pasteEmoji(glyph string) {
	if glyph == "" || c.closing {
		return
	}
	if err := windows.WriteClipboardText(c.panel.NativeHandle(), glyph, true); err != nil {
		c.view.Status = c.view.Text("复制失败，请重试", "Could not copy; please retry")
		log.Printf("emoji copy failed: %v", err)
		return
	}
	c.view.Status = c.view.Text("已复制 Emoji", "Emoji copied")
	c.pasteToRememberedTarget()
}

func (c *controller) cycleEmojiTone() emoji.SkinTone {
	next := emoji.ToneFromRaw(c.emojiTone).Cycle()
	c.emojiTone = next.Raw()
	c.view.EmojiTone = next
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("emojiSkinTone", c.emojiTone)
	}
	c.queueSettingsSave()
	return next
}

func (c *controller) openEmoji() {
	if c.closing {
		return
	}
	c.view.SetModeEmoji()
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.show()
}
