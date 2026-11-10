package app

import (
	"errors"
	"log"
	"strconv"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
)

const doubleTapWindow = 350 * time.Millisecond

// The clipboard history command has a default shortcut even before the user
// opens Settings, matching the requested Ctrl+` chord. A user-recorded value
// in commandHotkeys overrides it, including an explicit empty value.
const defaultClipboardHotkey = "Ctrl+`"

const (
	vkShift    = 0x10
	vkControl  = 0x11
	vkMenu     = 0x12
	vkLeftWin  = 0x5B
	vkRightWin = 0x5C
)

func (c *controller) applyHotkey() {
	if err := c.registerHotkey(c.hotkey); err != nil {
		c.view.Status = c.view.Text("热键被占用或格式无效", "Shortcut is occupied or invalid")
		log.Printf("shortcut unavailable: %v", err)
	}
}

// shortcutChord is the subset of the accelerator syntax the message-host
// keyboard hook can observe: modifiers plus one virtual key.
type shortcutChord struct {
	ctrl  bool
	alt   bool
	shift bool
	super bool
	key   uint32
}

// registerShortcut prefers RegisterHotKey. When another application already
// owns the chord, a low-level keyboard listener still delivers it to Wincast;
// the listener never consumes the key. An accelerator already claimed by this
// app is never double-bound.
func (c *controller) registerShortcut(accel string, fn func()) error {
	err := mygo.GlobalShortcut.Register(accel, fn)
	if err == nil {
		return nil
	}
	if strings.HasPrefix(err.Error(), "mygo: shortcut ") {
		return err
	}
	chord, ok := parseShortcutChord(accel)
	if !ok || c.messageHost == nil {
		return err
	}
	held := false
	stop, hookErr := c.messageHost.AddKeyboardListener(func(event windows.KeyEvent) {
		if event.Injected || event.VirtualKey != chord.key {
			return
		}
		if !event.Down {
			held = false
			return
		}
		if held {
			return
		}
		held = true
		if !chord.pressed() {
			return
		}
		fn()
	})
	if hookErr != nil {
		return hookErr
	}
	c.shortcutHooks = append(c.shortcutHooks, stop)
	log.Printf("shortcut uses keyboard hook %s", accel)
	return nil
}

// parseShortcutChord converts an accelerator the recorder can produce into a
// hook matcher. Bare characters are refused: they would be taken from every
// application without a modifier.
func parseShortcutChord(accel string) (shortcutChord, bool) {
	var chord shortcutChord
	parts := strings.Split(strings.TrimSpace(accel), "+")
	if len(parts) < 2 {
		return chord, false
	}
	for _, part := range parts[:len(parts)-1] {
		switch strings.ToLower(strings.TrimSpace(part)) {
		case "ctrl", "control":
			chord.ctrl = true
		case "alt", "option":
			chord.alt = true
		case "shift":
			chord.shift = true
		case "super", "win", "meta", "command", "cmd":
			chord.super = true
		default:
			return chord, false
		}
	}
	key, ok := shortcutVirtualKey(parts[len(parts)-1])
	if !ok {
		return chord, false
	}
	chord.key = key
	if !chord.ctrl && !chord.alt && !chord.super && !(key >= 0x70 && key <= 0x87) {
		return chord, false
	}
	return chord, true
}

func (ch shortcutChord) pressed() bool {
	if windows.KeyDown(vkControl) != ch.ctrl {
		return false
	}
	if windows.KeyDown(vkMenu) != ch.alt {
		return false
	}
	if windows.KeyDown(vkShift) != ch.shift {
		return false
	}
	super := windows.KeyDown(vkLeftWin) || windows.KeyDown(vkRightWin)
	return super == ch.super
}

var shortcutKeys = map[string]uint32{
	"enter": 0x0D, "return": 0x0D, "tab": 0x09, "space": 0x20,
	"backspace": 0x08, "delete": 0x2E, "del": 0x2E, "insert": 0x2D, "ins": 0x2D,
	"home": 0x24, "end": 0x23, "pageup": 0x21, "pagedown": 0x22,
	"escape": 0x1B, "esc": 0x1B, "left": 0x25, "right": 0x27, "up": 0x26, "down": 0x28,
	"-": 0xBD, "=": 0xBB, ",": 0xBC, ".": 0xBE, "/": 0xBF, ";": 0xBA,
	"'": 0xDE, "[": 0xDB, "]": 0xDD, "\\": 0xDC, "`": 0xC0,
}

func shortcutVirtualKey(raw string) (uint32, bool) {
	key := strings.TrimSpace(raw)
	if len(key) == 1 {
		c := key[0]
		switch {
		case c >= 'a' && c <= 'z':
			return uint32(c - 'a' + 'A'), true
		case c >= 'A' && c <= 'Z':
			return uint32(c), true
		case c >= '0' && c <= '9':
			return uint32(c), true
		}
	}
	lower := strings.ToLower(key)
	if vk, ok := shortcutKeys[lower]; ok {
		return vk, true
	}
	if len(lower) >= 2 && lower[0] == 'f' {
		n, err := strconv.Atoi(lower[1:])
		if err == nil && n >= 1 && n <= 24 {
			return 0x70 + uint32(n-1), true
		}
	}
	return 0, false
}

// normalizeShortcut makes comparison independent of modifier order and case.
func normalizeShortcut(value string) string {
	parts := strings.Split(strings.TrimSpace(value), "+")
	if len(parts) < 2 {
		return strings.ToLower(strings.TrimSpace(value))
	}
	var ctrl, alt, shift, super bool
	for _, part := range parts[:len(parts)-1] {
		switch strings.ToLower(strings.TrimSpace(part)) {
		case "ctrl", "control":
			ctrl = true
		case "alt", "option":
			alt = true
		case "shift":
			shift = true
		case "super", "win", "meta", "command", "cmd":
			super = true
		}
	}
	key := strings.ToLower(strings.TrimSpace(parts[len(parts)-1]))
	normalized := make([]string, 0, 5)
	if super {
		normalized = append(normalized, "super")
	}
	if ctrl {
		normalized = append(normalized, "ctrl")
	}
	if alt {
		normalized = append(normalized, "alt")
	}
	if shift {
		normalized = append(normalized, "shift")
	}
	normalized = append(normalized, key)
	return strings.Join(normalized, "+")
}

func sameShortcut(left, right string) bool {
	return left != "" && right != "" && normalizeShortcut(left) == normalizeShortcut(right)
}

// suspendMainHotkey releases the launcher chord while a settings recorder is
// capturing, so recording Alt+Space does not toggle the palette.
func (c *controller) suspendMainHotkey() {
	if c.hotkeyRegistered {
		mygo.GlobalShortcut.Unregister(c.hotkeyOwned)
		c.hotkeyRegistered = false
	}
}

func (c *controller) beginCommandShortcutRecording(id string) {
	if c.commandShortcutRecording == id {
		return
	}
	c.commandShortcutRecording = id
	c.settingsStatus = ""
	c.suspendMainHotkey()
	c.reconcileItemHotkeys()
	if c.settings != nil {
		c.settings.Update(func() {})
	}
}

func (c *controller) endCommandShortcutRecording() {
	// Also reached by the Clear button, which is not a recording session: the
	// bindings still have to be rebuilt after the map changed.
	c.commandShortcutRecording = ""
	c.applyHotkey()
	c.reconcileItemHotkeys()
	if c.view != nil {
		c.view.ClipboardShortcut = c.commandHotkey("command:clipboard-history")
	}
	if c.settings != nil {
		c.settings.Update(func() {})
	}
}

func (c *controller) cancelCommandShortcutRecording() {
	if c.commandShortcutRecording == "" {
		return
	}
	c.settingsStatus = ""
	c.endCommandShortcutRecording()
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
	for _, command := range builtinCommands {
		if command.visible != nil && !command.visible(c) {
			continue
		}
		if sameShortcut(c.commandHotkey(command.id), accel) {
			c.settingsStatus = c.text("该快捷键已用于应用内命令", "That shortcut is already used by an in-app command")
			return
		}
	}
	if c.itemShortcutInUse(accel) {
		c.settingsStatus = c.text("该快捷键已用于其他项目", "That shortcut is already used by another item")
		return
	}
	if err := c.registerHotkey(accel); err != nil {
		c.settingsStatus = c.text("热键注册失败；已恢复原热键", "Could not register the shortcut; the previous one was restored")
		log.Printf("shortcut update failed: %v", err)
	} else {
		c.hotkey = strings.TrimSpace(accel)
		c.view.Shortcut = c.hotkey
		if c.settingsDocument != nil && c.settingsWritable {
			_ = c.settingsDocument.Set("hotkey", c.hotkey)
		}
		c.settingsStatus = c.text("热键已更新", "Shortcut updated")
		c.queueSettingsSave()
	}
}

// commandHotkey returns the user's binding for a built-in command. The
// clipboard history command keeps Ctrl+` as its shipped default until the
// user explicitly records another chord or clears it.
func (c *controller) commandHotkey(id string) string {
	if value, ok := c.commandHotkeys[id]; ok {
		return strings.TrimSpace(value)
	}
	if id == "command:clipboard-history" {
		return defaultClipboardHotkey
	}
	return ""
}

// extensionHotkey returns the recorded chord for one extension launcher entry.
func (c *controller) extensionHotkey(entryID string) string {
	return strings.TrimSpace(c.extensionHotkeys[entryID])
}

// setExtensionHotkey validates and installs a per-command extension binding,
// then persists it. The settings recorder keeps the old binding on failure.
func (c *controller) setExtensionHotkey(entryID, accel string) bool {
	accel = strings.TrimSpace(accel)
	previous := c.extensionHotkey(entryID)
	if accel == previous {
		c.endCommandShortcutRecording()
		return true
	}
	if c.extensions != nil && accel != "" {
		_, command, ok := c.extensions.Resolve(entryID)
		if !ok {
			c.settingsStatus = c.text("扩展命令不存在", "That extension command is not installed")
			c.endCommandShortcutRecording()
			return false
		}
		if !command.Mode.Supported() {
			c.settingsStatus = c.text("该命令暂不支持快捷键", "That command cannot run from a shortcut")
			c.endCommandShortcutRecording()
			return false
		}
	}
	if accel != "" {
		if sameShortcut(accel, c.hotkey) {
			c.settingsStatus = c.text("该快捷键已用于应用启动器", "That shortcut already launches the app")
			c.endCommandShortcutRecording()
			return false
		}
		for _, command := range builtinCommands {
			if command.visible != nil && !command.visible(c) {
				continue
			}
			if sameShortcut(c.commandHotkey(command.id), accel) {
				c.settingsStatus = c.text("该快捷键已用于其他命令", "That shortcut is already used by another command")
				c.endCommandShortcutRecording()
				return false
			}
		}
		for otherID, other := range c.extensionHotkeys {
			if otherID != entryID && sameShortcut(other, accel) {
				c.settingsStatus = c.text("该快捷键已用于其他扩展命令", "That shortcut is already used by another extension command")
				c.endCommandShortcutRecording()
				return false
			}
		}
		if c.itemShortcutInUse(accel) {
			c.settingsStatus = c.text("该快捷键已用于其他项目", "That shortcut is already used by another item")
			c.endCommandShortcutRecording()
			return false
		}
		if err := c.registerShortcut(accel, func() {
			c.panel.Update(func() {
				if !c.closing {
					c.runExtensionHotkey(entryID)
				}
			})
		}); err != nil {
			c.settingsStatus = c.text("快捷键不可用：", "Shortcut unavailable: ") + err.Error()
			log.Printf("extension shortcut unavailable entry=%q shortcut=%q error=%v", entryID, accel, err)
			c.endCommandShortcutRecording()
			return false
		}
		if c.itemHotkeys == nil {
			c.itemHotkeys = make(map[string]bool)
		}
		c.itemHotkeys[normalizeShortcut(accel)] = true
	}
	if c.extensionHotkeys == nil {
		c.extensionHotkeys = make(map[string]string)
	}
	if accel == "" {
		delete(c.extensionHotkeys, entryID)
	} else {
		c.extensionHotkeys[entryID] = accel
	}
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("extensionHotkeys", c.extensionHotkeys)
	}
	c.queueSettingsSave()
	if accel == "" {
		c.settingsStatus = c.text("快捷键已清除", "Shortcut cleared")
	} else {
		c.settingsStatus = c.text("快捷键已更新", "Shortcut updated")
	}
	c.endCommandShortcutRecording()
	return true
}

// runExtensionHotkey runs a bound command, closing its palette screen when the
// same command is already showing.
func (c *controller) runExtensionHotkey(entryID string) {
	if c.extensions == nil || !c.extensions.Enabled() {
		return
	}
	if c.extensionEntryID == entryID && c.panel != nil && c.panel.IsVisible() {
		if c.view.Mode == palette.ModeExtension && c.view.ExtensionScreen != nil {
			c.leaveExtension()
			return
		}
	}
	c.startExtension(entryID)
}

func (c *controller) itemShortcutInUse(accel string) bool {
	for _, value := range c.extensionHotkeys {
		if sameShortcut(value, accel) {
			return true
		}
	}
	if c.featureCommands && c.commands != nil {
		for _, command := range c.commands.Commands() {
			if sameShortcut(command.Hotkey, accel) {
				return true
			}
		}
	}
	if c.featureQuicklinks && c.quicklinks != nil {
		for _, link := range c.quicklinks.Links() {
			if sameShortcut(link.Hotkey, accel) {
				return true
			}
		}
	}
	if c.featureSnippets {
		for _, record := range c.snippetSnapshot.Records {
			if record.Enabled && sameShortcut(c.snippetHotkey(record.ID), accel) {
				return true
			}
		}
	}
	return false
}

// setCommandHotkey validates and installs a binding, then persists it. The
// settings recorder keeps the old binding when registration fails.
func (c *controller) setCommandHotkey(id, accel string) bool {
	accel = strings.TrimSpace(accel)
	previous := c.commandHotkey(id)
	if accel == previous {
		c.endCommandShortcutRecording()
		return true
	}
	command, ok := builtinCommandByID(id)
	if !ok {
		return false
	}
	if accel != "" {
		if sameShortcut(accel, c.hotkey) {
			c.settingsStatus = c.text("该快捷键已用于应用启动器", "That shortcut already launches the app")
			c.endCommandShortcutRecording()
			return false
		}
		for _, other := range builtinCommands {
			if other.id == id || (other.visible != nil && !other.visible(c)) {
				continue
			}
			if sameShortcut(c.commandHotkey(other.id), accel) {
				c.settingsStatus = c.text("该快捷键已用于其他命令", "That shortcut is already used by another command")
				c.endCommandShortcutRecording()
				return false
			}
		}
		if c.itemShortcutInUse(accel) {
			c.settingsStatus = c.text("该快捷键已用于其他项目", "That shortcut is already used by another item")
			c.endCommandShortcutRecording()
			return false
		}
		if err := c.registerShortcut(accel, func() {
			c.panel.Update(func() {
				if !c.closing {
					c.runBuiltinCommand(command.id)
				}
			})
		}); err != nil {
			c.settingsStatus = c.text("快捷键不可用：", "Shortcut unavailable: ") + err.Error()
			log.Printf("command shortcut unavailable command=%q shortcut=%q error=%v", id, accel, err)
			c.endCommandShortcutRecording()
			return false
		}
		// Track the trial registration so ending the recording unregisters it
		// before the full table is rebuilt from the saved bindings.
		if c.itemHotkeys == nil {
			c.itemHotkeys = make(map[string]bool)
		}
		c.itemHotkeys[normalizeShortcut(accel)] = true
	}
	if c.commandHotkeys == nil {
		c.commandHotkeys = make(map[string]string)
	}
	c.commandHotkeys[id] = accel
	if c.settingsDocument != nil && c.settingsWritable {
		_ = c.settingsDocument.Set("commandHotkeys", c.commandHotkeys)
	}
	c.queueSettingsSave()
	if accel == "" {
		c.settingsStatus = c.text("快捷键已清除", "Shortcut cleared")
	} else {
		c.settingsStatus = c.text("快捷键已更新", "Shortcut updated")
	}
	c.endCommandShortcutRecording()
	return true
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
		if c.doubleTapStop != nil {
			c.doubleTapStop()
			c.doubleTapStop = nil
			log.Printf("keyboard monitor stopped")
		}
		return
	}
	if c.doubleTapStop != nil {
		return
	}
	stop, err := c.messageHost.AddKeyboardListener(c.onKeyboard)
	if err != nil {
		log.Printf("keyboard monitor unavailable: %v", err)
		c.settingsStatus = c.text("双击修饰键不可用", "Double-tap modifier is unavailable")
		return
	}
	c.doubleTapStop = stop
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
