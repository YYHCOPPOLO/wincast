package app

import (
	"errors"
	"log"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/egoist/mygo"
)

const doubleTapWindow = 350 * time.Millisecond

func (c *controller) applyHotkey() {
	if err := c.registerHotkey(c.hotkey); err != nil {
		c.view.Status = c.view.Text("热键被占用或格式无效", "Shortcut is occupied or invalid")
		log.Printf("shortcut unavailable: %v", err)
	}
}

// registerHotkey installs accel and restores the previous shortcut if the new
// one cannot be registered. The caller only commits the setting on success.
func (c *controller) registerHotkey(accel string) error {
	accel = strings.TrimSpace(accel)
	if accel == "" {
		return errors.New("empty shortcut")
	}
	if c.hotkeyRegistered && c.hotkeyOwned == accel {
		return nil
	}
	previous := c.hotkeyOwned
	if c.hotkeyRegistered {
		mygo.GlobalShortcut.Unregister(previous)
		c.hotkeyRegistered = false
	}
	if err := mygo.GlobalShortcut.Register(accel, c.toggle); err != nil {
		if previous != "" {
			if restoreErr := mygo.GlobalShortcut.Register(previous, c.toggle); restoreErr == nil {
				c.hotkeyRegistered, c.hotkeyOwned = true, previous
			} else {
				log.Printf("shortcut restore failed: %v", restoreErr)
			}
		}
		return err
	}
	c.hotkeyRegistered, c.hotkeyOwned = true, accel
	log.Printf("shortcut registered %s", accel)
	return nil
}

func (c *controller) setHotkey(accel string) {
	if err := c.registerHotkey(accel); err != nil {
		c.settingsStatus = c.text("热键注册失败；已恢复原热键", "Could not register the shortcut; the previous one was restored")
		log.Printf("shortcut update failed: %v", err)
	} else {
		c.hotkey = strings.TrimSpace(accel)
		if c.settingsDocument != nil && c.settingsWritable {
			_ = c.settingsDocument.Set("hotkey", c.hotkey)
		}
		c.settingsStatus = c.text("热键已更新", "Shortcut updated")
		c.queueSettingsSave()
	}
}

func doubleTapVirtualKey(value string) uint32 {
	switch value {
	case "ctrl":
		return 0x11
	case "alt":
		return 0x12
	case "shift":
		return 0x10
	}
	return 0
}

func (c *controller) applyDoubleTap() {
	vk := doubleTapVirtualKey(c.doubleTap)
	c.doubleTapVK.Store(vk)
	c.doubleTapHeld.Store(false)
	c.lastDoubleTap.Store(0)
	if c.messageHost == nil {
		return
	}
	if vk == 0 {
		if c.keyboardInstalled {
			c.messageHost.StopKeyboard()
			c.keyboardInstalled = false
			log.Printf("keyboard monitor stopped")
		}
		return
	}
	if c.keyboardInstalled {
		return
	}
	if err := c.messageHost.MonitorKeyboard(c.onKeyboard); err != nil {
		log.Printf("keyboard monitor unavailable: %v", err)
		c.settingsStatus = c.text("双击修饰键不可用", "Double-tap modifier is unavailable")
		return
	}
	c.keyboardInstalled = true
	log.Printf("keyboard monitor installed vk=0x%02x", vk)
}

func (c *controller) setDoubleTap(value string) {
	switch value {
	case "none", "ctrl", "alt", "shift":
	default:
		value = "none"
	}
	c.doubleTap = value
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("doubleTapModifier", value)
	}
	c.queueSettingsSave()
	c.applyDoubleTap()
}

// onKeyboard runs on the message-host thread. It only compares atomic fields
// and posts the toggle to the UI thread.
func (c *controller) onKeyboard(event windows.KeyEvent) {
	if event.Injected {
		return
	}
	vk := c.doubleTapVK.Load()
	if vk == 0 {
		return
	}
	if modifierVirtualKey(event.VirtualKey) != vk {
		if event.Down {
			c.lastDoubleTap.Store(0) // a chord is not a standalone tap
		}
		return
	}
	if !event.Down {
		c.doubleTapHeld.Store(false)
		return
	}
	if c.doubleTapHeld.Swap(true) {
		return // key repeat is not a second tap
	}
	now := time.Now()
	previous := c.lastDoubleTap.Swap(now.UnixNano())
	if previous == 0 {
		return
	}
	gap := now.Sub(time.Unix(0, previous))
	if gap < 40*time.Millisecond || gap > doubleTapWindow {
		return
	}
	c.lastDoubleTap.Store(0)
	c.panel.Update(func() {
		if !c.closing {
			c.toggle()
		}
	})
}

func modifierVirtualKey(vk uint32) uint32 {
	switch vk {
	case 0xA0, 0xA1: // VK_LSHIFT, VK_RSHIFT
		return 0x10
	case 0xA2, 0xA3: // VK_LCONTROL, VK_RCONTROL
		return 0x11
	case 0xA4, 0xA5: // VK_LMENU, VK_RMENU
		return 0x12
	default:
		return vk
	}
}
