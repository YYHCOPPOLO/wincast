// Package editors draws the shared command, quicklink and snippet forms.
// They are one modal shape so save, cancel, delete and dirty states behave
// the same everywhere.
package editors

import (
	"fmt"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

type Kind uint8

const (
	Command Kind = iota
	Quicklink
	Snippet
)

// Model is the draft being edited. The app owns it; the view only renders
// and forwards intent.
type Model struct {
	Open bool
	Kind Kind
	ID   string

	Name     string
	Text     string
	Keyword  string
	Body     *ui.TextBuffer
	Enabled  bool
	Confirm  bool
	ShowRoot bool
	ShowHud  bool
	Hotkey   string

	Recording   bool
	InsertToken string
	Error       string
	Dirty       bool
}

type Callbacks struct {
	Save         func()
	Cancel       func()
	Delete       func()
	SetRecording func(bool)
	Captured     func(string)
}

var insertTokens = []string{
	"{clipboard}",
	`{clipboard offset=1}`,
	"{selection}",
	"{date}",
	`{date format="yyyy-MM-dd"}`,
	"{time}",
	"{datetime}",
	"{day}",
	"{uuid}",
	`{argument name="Name"}`,
	"{snippet:Name}",
	"{cursor}",
}

func Render(c *ui.Context, tokens theme.Tokens, model *Model, callbacks Callbacks) {
	if model == nil {
		return
	}
	ui.Modal(c, &model.Open, func() {
		ui.Column(c).Gap(12).Children(func() {
			ui.Text(c, title(model.Kind)).FontSize(16).Bold().Role(ui.RoleHeading)
			ui.Text(c, subtitle(model.Kind)).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary)

			fieldRow(c, tokens, "名称 Name", func() {
				if ui.TextInput(c, &model.Name).Grow(1).Changed() {
					model.Dirty = true
				}
			})
			switch model.Kind {
			case Command:
				fieldRow(c, tokens, "命令 Command", func() {
					if ui.TextInput(c, &model.Text).Grow(1).Changed() {
						model.Dirty = true
					}
				})
				ui.Text(c, "原样交给 CreateProcessW：PATH 查找、工作目录为用户主目录、无控制台。 / Passed to CreateProcessW verbatim: PATH lookup, user profile working directory, no console.").
					FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary)
				toggleRow(c, tokens, "运行前确认 Confirm", &model.Confirm, func() { model.Dirty = true })
			case Quicklink:
				fieldRow(c, tokens, "目标 Destination", func() {
					if ui.TextInput(c, &model.Text).Grow(1).Changed() {
						model.Dirty = true
					}
				})
				ui.Text(c, "支持 {clipboard}、{selection}、{date}、{argument name=\"Name\"} 等占位符；URL 会自动编码。 / Supports {clipboard}, {selection}, {date}, {argument name=\"Name\"} and more; URLs are encoded automatically.").
					FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary)
				toggleRow(c, tokens, "显示在启动器 Show in launcher", &model.ShowRoot, func() { model.Dirty = true })
			case Snippet:
				fieldRow(c, tokens, "关键词 Keyword", func() {
					if ui.TextInput(c, &model.Keyword).Grow(1).Changed() {
						model.Dirty = true
					}
				})
				toggleRow(c, tokens, "启用 Enabled", &model.Enabled, func() { model.Dirty = true })
				toggleRow(c, tokens, "展开提示 Confirmation", &model.ShowHud, func() { model.Dirty = true })
				ui.Text(c, "内容 Content").FontSize(theme.FormLabelFont).TextColor(tokens.TextSecondary)
				ui.TextAreaBufferBase(c, model.Body).Height(220).Padding(10, 12).FontSize(13).
					Label("Snippet body").OnChange(func() { model.Dirty = true })
				ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
					ui.Select(c, &model.InsertToken, insertTokens).Width(260)
					if ui.Button(c, "插入 Insert").Clicked() && model.InsertToken != "" {
						model.Body.Set(model.Body.String() + model.InsertToken)
						model.Dirty = true
					}
				})
			}
			hotkeyRow(c, tokens, model, callbacks)
			if model.Error != "" {
				ui.Text(c, model.Error).FontSize(theme.TrailingFont).TextColor(tokens.Destructive).SingleLine()
			}
			ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
				if model.ID != "" && callbacks.Delete != nil {
					if ui.Button(c, "删除 Delete").Clicked() {
						callbacks.Delete()
					}
				}
				ui.Spacer(c)
				if model.Dirty {
					ui.Text(c, "未保存 Unsaved").FontSize(theme.KeyCapFont).TextColor(tokens.TextTertiary)
				}
				if ui.Button(c, "取消 Cancel").Clicked() {
					callbacks.Cancel()
				}
				if ui.PrimaryButton(c, "保存 Save").Clicked() {
					callbacks.Save()
				}
			})
		})
	})
}

func title(kind Kind) string {
	switch kind {
	case Command:
		return "编辑命令 Edit command"
	case Quicklink:
		return "编辑快捷链接 Edit quicklink"
	default:
		return "编辑片段 Edit snippet"
	}
}

func subtitle(kind Kind) string {
	switch kind {
	case Command:
		return "命名、命令文本与运行前确认。"
	case Quicklink:
		return "命名与可含占位符的目标地址。"
	default:
		return "Markdown 正文与可选关键词。"
	}
}

func fieldRow(c *ui.Context, tokens theme.Tokens, label string, control func()) {
	ui.Row(c).Gap(12).AlignItems(ui.Center).Children(func() {
		ui.Text(c, label).FontSize(theme.FormLabelFont).TextColor(tokens.TextSecondary).Width(150).SingleLine()
		control()
	})
}

func toggleRow(c *ui.Context, tokens theme.Tokens, label string, value *bool, changed func()) {
	fieldRow(c, tokens, label, func() {
		if ui.Switch(c, value).Changed() {
			changed()
		}
	})
}

func hotkeyRow(c *ui.Context, tokens theme.Tokens, model *Model, callbacks Callbacks) {
	fieldRow(c, tokens, "快捷键 Shortcut", func() {
		ui.Row(c).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
			if model.Recording {
				capture := ""
				ui.TextInput(c, &capture).Grow(1).Placeholder("按下组合键… Press a shortcut…").
					AutoFocus().OnTextKey(func(key ui.TextKey) bool {
					if key.Mods == 0 && key.Key == ui.KeyEscape {
						callbacks.SetRecording(false)
						return true
					}
					if key.Mods == 0 && key.Key == ui.KeyBackspace {
						callbacks.Captured("")
						return true
					}
					accelerator, ok := FormatAccelerator(key.Mods, key.Key)
					if !ok {
						return true
					}
					callbacks.Captured(accelerator)
					return true
				})
			} else {
				if ui.TextInput(c, &model.Hotkey).Grow(1).Changed() {
					model.Dirty = true
				}
				if ui.Button(c, "录制 Record").Clicked() {
					callbacks.SetRecording(true)
				}
			}
		})
	})
}

// FormatAccelerator converts one chord to mygo's accelerator syntax. A bare
// modifier returns false so holding it cannot end the recording.
func FormatAccelerator(mods ui.Modifiers, key ui.Key) (string, bool) {
	if mods == 0 && key == ui.KeyEscape {
		return "", false
	}
	name, ok := keyName(key)
	if !ok {
		return "", false
	}
	parts := make([]string, 0, 4)
	if mods&ui.Super != 0 {
		parts = append(parts, "Super")
	}
	if mods&ui.Ctrl != 0 {
		parts = append(parts, "Ctrl")
	}
	if mods&ui.Alt != 0 {
		parts = append(parts, "Alt")
	}
	if mods&ui.Shift != 0 {
		parts = append(parts, "Shift")
	}
	parts = append(parts, name)
	if mods&(ui.Ctrl|ui.Alt|ui.Super) == 0 && !(key >= ui.KeyF1 && key <= ui.KeyF12) {
		// A bare letter, digit or space would be taken from every application.
		return "", false
	}
	return strings.Join(parts, "+"), true
}

func keyName(key ui.Key) (string, bool) {
	switch {
	case key >= ui.KeyA && key <= ui.KeyZ:
		return string(rune('A' + (key - ui.KeyA))), true
	case key >= ui.Key0 && key <= ui.Key9:
		return string(rune('0' + (key - ui.Key0))), true
	case key >= ui.KeyF1 && key <= ui.KeyF12:
		return fmt.Sprintf("F%d", 1+int(key-ui.KeyF1)), true
	}
	switch key {
	case ui.KeyEnter:
		return "Enter", true
	case ui.KeyTab:
		return "Tab", true
	case ui.KeySpace:
		return "Space", true
	case ui.KeyBackspace:
		return "Backspace", true
	case ui.KeyDelete:
		return "Delete", true
	case ui.KeyInsert:
		return "Insert", true
	case ui.KeyHome:
		return "Home", true
	case ui.KeyEnd:
		return "End", true
	case ui.KeyPageUp:
		return "PageUp", true
	case ui.KeyPageDown:
		return "PageDown", true
	case ui.KeyLeft:
		return "Left", true
	case ui.KeyRight:
		return "Right", true
	case ui.KeyUp:
		return "Up", true
	case ui.KeyDown:
		return "Down", true
	case ui.KeyMinus:
		return "-", true
	case ui.KeyEqual:
		return "=", true
	case ui.KeyComma:
		return ",", true
	case ui.KeyPeriod:
		return ".", true
	case ui.KeySlash:
		return "/", true
	case ui.KeySemicolon:
		return ";", true
	case ui.KeyQuote:
		return "'", true
	case ui.KeyBracketLeft:
		return "[", true
	case ui.KeyBracketRight:
		return "]", true
	case ui.KeyBackslash:
		return "\\", true
	case ui.KeyBackquote:
		return "`", true
	}
	return "", false
}
