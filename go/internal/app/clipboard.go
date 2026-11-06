package app

import (
	"fmt"
	"log"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

// startClipboardHost owns the Win32 message window for clipboard and theme
// broadcasts. It never reads clipboard content; events are counted and logged
// as a type only, so logs stay free of user text.
func (c *controller) startClipboardHost() {
	host, err := windows.StartMessageHost()
	if err != nil {
		log.Printf("message host unavailable: %v", err)
		return
	}
	c.messageHost = host
	if err := host.WatchClipboard(func() {
		if !c.clipboardEnabled.Load() {
			return
		}
		c.clipboardEvents.Add(1)
		log.Printf("clipboard update")
		c.panel.Update(func() {
			if !c.closing && c.settings != nil {
				c.settings.Update(func() {})
			}
		})
	}); err != nil {
		log.Printf("clipboard listener unavailable: %v", err)
	}
	if err := host.WatchTheme(func(dark bool) {
		c.panel.Update(func() {
			if c.closing {
				return
			}
			c.systemDark = dark
			if c.settings != nil {
				c.settings.Update(func() {})
			}
		})
	}); err != nil {
		log.Printf("system theme listener unavailable: %v", err)
	}
}

func (c *controller) clipboardSummary() string {
	events := c.clipboardEvents.Load()
	if c.language == "zh" {
		return fmt.Sprintf("%d 次", events)
	}
	return fmt.Sprintf("%d events", events)
}
