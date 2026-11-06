package palette

import (
	"fmt"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

// Callbacks are the user's intent. The view never performs I/O and never
// holds a repository; the app decides what an action means.
type Callbacks struct {
	Open             func(domain.Application)
	Hide             func()
	Menu             func()
	CopyText         func(string, bool)
	Icon             func(domain.Application) *ui.Bitmap
	Filtered         func(time.Duration, int)
	Search           func(query string, showHidden bool) []domain.Application
	Favorite         func(byte)
	FavoriteSlot     func(byte) string
	ToggleFavorite   func()
	ToggleExpanded   func()
	SaveAlias        func(domain.Application, string)
	SetHidden        func(domain.Application, bool)
	ToggleShowHidden func()
}

// View is UI-thread-owned display state. Callbacks send user intent back to
// the app; no disk, SQL or shell work runs as the view is constructed.
type View struct {
	Query               string
	Apps                []domain.Application
	Results             []domain.Application
	Loading             bool
	Status              string
	Chinese             bool
	ForcedDark          *bool
	Expanded            bool
	ShowHidden          bool
	Chosen              int
	Input               ui.Handle
	inputText           *ui.TextBuffer
	list                ui.ListState
	AliasOpen           bool
	AliasText           string
	AliasTarget         domain.Application
	aliasInput          ui.Handle
	callbacks           Callbacks
	Dictionary          bool
	DictionaryAvailable bool
	DictionaryLoading   bool
	DictionaryError     string
	Definition          *Definition
	dictionaryScroll    ui.ScrollState
	OnLookup            func(string)
}

func New(chinese bool, callbacks Callbacks) *View {
	return &View{Chinese: chinese, Expanded: true, Chosen: -1, inputText: ui.NewTextBuffer(""), callbacks: callbacks}
}

func (v *View) Text(zh, en string) string {
	if v.Chinese {
		return zh
	}
	return en
}

// FocusAlias moves the keyboard focus into the alias field after the modal
// has been built.
func (v *View) FocusAlias() { v.aliasInput.Focus() }

func (v *View) Render(c *ui.Context) {
	dark := c.Theme().Dark
	if v.ForcedDark != nil {
		dark = *v.ForcedDark
	}
	tokens := theme.For(dark)
	c.SetTheme(tokens.Theme())
	c.Root().Background(tokens.Surface)
	c.OnShortcut(ui.Ctrl, ui.KeyK, v.callbacks.Menu)
	c.OnShortcut(0, ui.KeyEscape, v.escape)
	c.OnShortcut(0, ui.KeyDown, func() { v.choose(v.Chosen + 1) })
	c.OnShortcut(0, ui.KeyUp, func() { v.choose(v.Chosen - 1) })
	c.OnShortcut(0, ui.KeyEnter, v.open)
	v.renderAlias(c, tokens)
	ui.Column(c).Fill().Children(func() {
		padding := float32(18)
		if !v.Expanded {
			padding = theme.HeaderPadding
		}
		ui.Row(c).Padding(padding, 20).Gap(14).AlignItems(ui.Center).Children(func() {
			ui.Text(c, "⌕").FontSize(24).TextColor(tokens.TextTertiary)
			placeholder := v.Text("搜索应用程序…", "Search applications…")
			if v.Dictionary {
				placeholder = v.Text("查询词典…", "Look up a word…")
			}
			if ui.TextInputBufferBase(c, v.inputText).Bind(&v.Input).Grow(1).FontSize(theme.SearchFont).
				Placeholder(placeholder).Label(placeholder).OnTextKey(v.textKey).Changed() {
				v.syncQuery(v.inputText.String())
			}
		})
		if !v.Expanded {
			return
		}
		ui.Divider(c)
		if v.Dictionary {
			ui.Text(c, v.Text("词典", "Dictionary")).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Padding(10, 18)
			v.dictionaryBody(c, tokens)
		} else {
			v.applications(c, tokens)
		}
		ui.Divider(c)
		status := v.Status
		if status == "" {
			status = v.Text("Go 预览 · Alt+Shift+Space", "Go Preview · Alt+Shift+Space")
		}
		components.Footer(c, tokens, status, v.Text("Tab 词典   ↑↓ 选择", "Tab Dictionary   ↑↓ Select"), v.callbacks.Menu)
	})
}

func (v *View) renderAlias(c *ui.Context, tokens theme.Tokens) {
	if !v.AliasOpen {
		return
	}
	components.Dialog(c, &v.AliasOpen, func() {
		ui.Text(c, v.Text("设置别名", "Set alias")).FontSize(16).Bold().Role(ui.RoleHeading)
		ui.Text(c, v.AliasTarget.Name).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary)
		ui.TextInput(c, &v.AliasText).Bind(&v.aliasInput).Label(v.Text("别名", "Alias")).
			Placeholder(v.Text("输入别名后保存", "Type an alias and save")).Grow(1)
		ui.Row(c).Gap(8).Children(func() {
			if ui.Button(c, v.Text("保存", "Save")).Clicked() {
				if v.callbacks.SaveAlias != nil {
					v.callbacks.SaveAlias(v.AliasTarget, v.AliasText)
				}
				v.AliasOpen = false
			}
			if ui.Button(c, v.Text("取消", "Cancel")).Clicked() {
				v.AliasOpen = false
			}
		})
	})
}

func (v *View) applications(c *ui.Context, tokens theme.Tokens) {
	label := v.Text("应用程序", "Applications")
	if v.ShowHidden {
		label = v.Text("应用程序（含隐藏）", "Applications (including hidden)")
	}
	components.Section(c, tokens, label)
	v.list.Selected = &v.Chosen
	v.list.Key = func(i int) any { return stableKey(v.Results[i]) }
	v.list.Label = func(i int) string { return v.Results[i].Name }
	list := ui.List(c, &v.list, len(v.Results), func(i int) { v.row(c, i, tokens) }).Grow(1).Padding(0, 8).
		Label(v.Text("应用程序搜索结果", "Application search results"))
	if list.Submitted() {
		v.open()
	}
	if len(v.Results) == 0 {
		message := v.Text("没有匹配的应用程序", "No matching applications")
		if v.Loading {
			message = v.Text("正在读取应用程序…", "Loading applications…")
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) row(c *ui.Context, i int, tokens theme.Tokens) {
	application := v.Results[i]
	trailing := ""
	if digits := v.favoriteDigit(application.ID); digits != 0 {
		trailing = string([]byte{digits})
	}
	components.ResultRow(c, tokens, components.Row{
		Icon: v.callbacks.Icon(application), Fallback: application.Name, Title: application.Name,
		Trailing: trailing, Selected: i == v.Chosen,
	})
}

func (v *View) favoriteDigit(id string) byte {
	if v.callbacks.FavoriteSlot == nil {
		return 0
	}
	for digit := byte('1'); digit <= '9'; digit++ {
		if v.callbacks.FavoriteSlot(digit) == id {
			return digit
		}
	}
	if v.callbacks.FavoriteSlot('0') == id {
		return '0'
	}
	return 0
}

func stableKey(application domain.Application) string {
	if application.ID != "" {
		return application.ID
	}
	return application.Path
}

func (v *View) textKey(key ui.TextKey) bool {
	// MyGo gives composition keys to the IME before OnTextKey. Read Text here
	// because a rapid Enter may arrive before the next view has bound Query.
	if key.Mods == ui.Ctrl|ui.Shift && (key.Key == ui.KeyEnter || key.Key == ui.KeyC) {
		v.syncQuery(key.Text())
		v.callbacks.CopyText(v.CopyText(), key.Key == ui.KeyEnter)
		return true
	}
	if key.Mods == ui.Ctrl {
		if digit, ok := digitKey(key.Key); ok {
			if !v.Dictionary && v.callbacks.Favorite != nil {
				v.callbacks.Favorite(digit)
			}
			return true
		}
		switch key.Key {
		case ui.KeyD:
			if v.callbacks.ToggleFavorite != nil {
				v.callbacks.ToggleFavorite()
			}
			return true
		case ui.KeyE:
			if v.callbacks.ToggleExpanded != nil {
				v.callbacks.ToggleExpanded()
			}
			return true
		}
	}
	if key.Mods != 0 {
		return false
	}
	switch key.Key {
	case ui.KeyEnter:
		v.syncQuery(key.Text())
		v.open()
	case ui.KeyEscape:
		if key.Text() == "" {
			v.callbacks.Hide()
		} else {
			key.SetText("")
			v.Query = ""
			v.Filter()
		}
	case ui.KeyDown:
		v.choose(v.Chosen + 1)
	case ui.KeyUp:
		v.choose(v.Chosen - 1)
	case ui.KeyTab:
		v.Query = key.Text()
		v.ToggleDictionary()
		return true
	default:
		return false
	}
	return true
}

func digitKey(key ui.Key) (byte, bool) {
	switch key {
	case ui.Key1:
		return '1', true
	case ui.Key2:
		return '2', true
	case ui.Key3:
		return '3', true
	case ui.Key4:
		return '4', true
	case ui.Key5:
		return '5', true
	case ui.Key6:
		return '6', true
	case ui.Key7:
		return '7', true
	case ui.Key8:
		return '8', true
	case ui.Key9:
		return '9', true
	case ui.Key0:
		return '0', true
	}
	return 0, false
}

func (v *View) escape() {
	if v.Query == "" {
		v.callbacks.Hide()
		return
	}
	v.SetQuery("")
	v.Input.Focus()
}

// SetQuery updates the actual input as well as its derived search state.
// The persistent buffer also lets native menu actions read committed input
// between frames without keeping an Element or a Context alive.
func (v *View) SetQuery(text string) {
	if v.inputText.String() != text {
		v.inputText.Set(text)
	}
	v.Query = text
	v.Filter()
}

func (v *View) syncQuery(text string) {
	if v.Query != text {
		v.Query = text
		v.Filter()
	}
}

func (v *View) open() {
	if v.Dictionary {
		return
	}
	if v.Chosen >= 0 && v.Chosen < len(v.Results) {
		v.callbacks.Open(v.Results[v.Chosen])
	}
}

func (v *View) choose(i int) {
	if v.Dictionary {
		return
	}
	if len(v.Results) == 0 {
		v.Chosen = -1
		return
	}
	v.Chosen = max(0, min(i, len(v.Results)-1))
	v.list.ScrollIntoView(v.Chosen)
}

func (v *View) Filter() {
	if v.Dictionary {
		v.Definition = nil
		v.DictionaryError = ""
		v.dictionaryScroll.X, v.dictionaryScroll.Y = 0, 0
		if v.OnLookup != nil {
			v.OnLookup(v.Query)
		}
		return
	}
	if v.OnLookup != nil {
		v.OnLookup("") // cancel a lookup after returning to Applications
	}
	started := time.Now()
	if v.callbacks.Search != nil {
		v.Results = v.callbacks.Search(v.Query, v.ShowHidden)
	} else {
		v.Results = domain.Filter(v.Apps, v.Query)
	}
	v.choose(0)
	if v.callbacks.Filtered != nil {
		v.callbacks.Filtered(time.Since(started), len(v.Results))
	}
}

func (v *View) CountStatus() string {
	return fmt.Sprintf(v.Text("%d 个应用程序", "%d applications"), len(v.Apps))
}
