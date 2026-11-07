package palette

import (
	"fmt"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	domaincalc "github.com/YYHCOPPOLO/wincast/go/internal/domain/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/emoji"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

type Mode uint8

const (
	ModeLauncher Mode = iota
	ModeDictionary
	ModeClipboard
	ModeCalculatorHistory
	ModeEmoji
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

	ClipboardSearch    func(query string, filter clipboard.Filter) []clipboard.Item
	ClipboardPaste     func(clipboard.Item)
	ClipboardCopy      func(clipboard.Item)
	ClipboardPin       func(clipboard.Item)
	ClipboardDelete    func(clipboard.Item)
	ClipboardImage     func(clipboard.Item) *ui.Bitmap
	ClipboardPinned    func(byte)
	SetClipboardFilter func(clipboard.Filter)

	Calculate             func(query string) *domaincalc.Result
	CalculatorHistory     func(query string) []domaincalc.HistoryEntry
	CopyCalculator        func(domaincalc.Result)
	CopyCalculatorHistory func(domaincalc.HistoryEntry)
	OpenCalculatorHistory func()

	EmojiSearch    func(query string, tone emoji.SkinTone) []emoji.Emoji
	EmojiPaste     func(string)
	CycleEmojiTone func() emoji.SkinTone
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
	Mode                Mode
	launcherQuery       string
	DictionaryAvailable bool
	DictionaryLoading   bool
	DictionaryError     string
	Definition          *Definition
	Suggestions         []string
	PendingToast        string
	dictionaryScroll    ui.ScrollState
	OnLookup            func(string)

	ClipboardItems     []clipboard.Item
	ClipboardFilter    clipboard.Filter
	ClipboardAvailable bool
	ClipboardError     string
	ClipboardChosen    int
	clipboardList      ui.ListState
	clipboardPreview   ui.ScrollState

	CalcResult      *domaincalc.Result
	CalcHistory     []domaincalc.HistoryEntry
	calcHistoryList ui.ListState

	EmojiItems  []emoji.Emoji
	EmojiTone   emoji.SkinTone
	EmojiChosen int
	emojiScroll ui.ScrollState
}

func New(chinese bool, callbacks Callbacks) *View {
	return &View{Chinese: chinese, Expanded: true, Chosen: -1, ClipboardChosen: -1,
		inputText: ui.NewTextBuffer(""), callbacks: callbacks}
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
	c.OnShortcut(0, ui.KeyDown, func() { v.moveVertical(1) })
	c.OnShortcut(0, ui.KeyUp, func() { v.moveVertical(-1) })
	c.OnShortcut(0, ui.KeyLeft, func() {
		if v.Mode == ModeEmoji {
			v.moveHorizontal(-1)
		}
	})
	c.OnShortcut(0, ui.KeyRight, func() {
		if v.Mode == ModeEmoji {
			v.moveHorizontal(1)
		}
	})
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
			switch v.Mode {
			case ModeDictionary:
				placeholder = v.Text("查询词典…", "Look up a word…")
			case ModeClipboard:
				placeholder = v.Text("搜索剪贴板…", "Search clipboard…")
			case ModeCalculatorHistory:
				placeholder = v.Text("搜索计算历史…", "Search calculator history…")
			case ModeEmoji:
				placeholder = v.Text("搜索 Emoji…", "Search emoji…")
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
		switch v.Mode {
		case ModeDictionary:
			ui.Text(c, v.Text("词典", "Dictionary")).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Padding(10, 18)
			v.dictionaryBody(c, tokens)
		case ModeClipboard:
			v.clipboardBody(c, tokens)
		case ModeCalculatorHistory:
			v.calculatorHistoryBody(c, tokens)
		case ModeEmoji:
			v.emojiBody(c, tokens)
		default:
			v.applications(c, tokens)
		}
		ui.Divider(c)
		status := v.Status
		if status == "" {
			status = v.defaultStatus()
		}
		components.Footer(c, tokens, status, v.footerHints(), v.callbacks.Menu)
	})
	if v.PendingToast != "" {
		c.Toast(v.PendingToast)
		v.PendingToast = ""
	}
}

func (v *View) defaultStatus() string {
	switch v.Mode {
	case ModeClipboard:
		if v.ClipboardError != "" {
			return v.ClipboardError
		}
		if !v.ClipboardAvailable {
			return v.Text("剪贴板存储不可用", "Clipboard storage is unavailable")
		}
		return v.Text("剪贴板历史", "Clipboard history")
	case ModeDictionary:
		return v.Text("ECDICT 词典", "ECDICT dictionary")
	case ModeCalculatorHistory:
		return v.Text("计算历史", "Calculator history")
	case ModeEmoji:
		return v.Text("Emoji 与符号 · 肤色 "+v.EmojiTone.Label(), "Emoji & Symbols · "+v.EmojiTone.Label())
	default:
		return v.Text("Go 预览 · Alt+Shift+Space", "Go Preview · Alt+Shift+Space")
	}
}

func (v *View) footerHints() string {
	switch v.Mode {
	case ModeClipboard:
		return v.Text("Enter 粘回  Ctrl+C 复制  Ctrl+. 置顶  Ctrl+P 筛选", "Enter Paste  Ctrl+C Copy  Ctrl+. Pin  Ctrl+P Filter")
	case ModeCalculatorHistory:
		return v.Text("Enter 复制结果  Esc 返回", "Enter Copy  Esc Back")
	case ModeEmoji:
		return v.Text("Enter 粘回  Ctrl+T 肤色  Esc 返回", "Enter Paste  Ctrl+T Skin tone  Esc Back")
	default:
		if v.CalcResult != nil {
			return v.Text("Enter 复制答案  ↑↓ 选择", "Enter Copy answer  ↑↓ Select")
		}
		return v.Text("Tab 词典   ↑↓ 选择", "Tab Dictionary   ↑↓ Select")
	}
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
	v.calcCard(c, tokens)
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

func (v *View) calcCard(c *ui.Context, tokens theme.Tokens) {
	if v.CalcResult == nil {
		return
	}
	fill := ui.Transparent
	if v.Chosen < 0 {
		fill = tokens.Selection
	}
	result := v.CalcResult
	ui.Column(c).FillWidth().Padding(8, 18).Gap(3).Radius(theme.RowRadius).
		Background(fill).Children(func() {
		ui.Text(c, result.Expression).FontSize(theme.TrailingFont).
			TextColor(tokens.TextTertiary).SingleLine()
		color := tokens.Text
		if result.IsError() {
			color = tokens.Destructive
		}
		ui.Text(c, result.Display).FontSize(theme.HeadingFont).TextColor(color).SingleLine().Role(ui.RoleStatus)
		if result.SourceBadge != "" || result.TargetBadge != "" {
			ui.Row(c).Gap(8).Children(func() {
				if result.SourceBadge != "" {
					ui.Text(c, result.SourceBadge).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
				}
				if result.TargetBadge != "" {
					ui.Text(c, "→ "+result.TargetBadge).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
				}
			})
		}
	})
}

func (v *View) calculatorHistoryBody(c *ui.Context, tokens theme.Tokens) {
	v.calcCard(c, tokens)
	v.calcHistoryList.Selected = &v.Chosen
	v.calcHistoryList.Key = func(i int) any {
		return v.CalcHistory[i].Expression + "\x00" + v.CalcHistory[i].Result
	}
	v.calcHistoryList.Label = func(i int) string { return v.CalcHistory[i].Expression }
	list := ui.List(c, &v.calcHistoryList, len(v.CalcHistory), func(i int) {
		entry := v.CalcHistory[i]
		components.ResultRow(c, tokens, components.Row{
			Fallback: "=",
			Title:    entry.Expression,
			Subtitle: entry.Result,
			Selected: i == v.Chosen,
		})
	}).Grow(1).Padding(0, 8).Label(v.Text("计算历史", "Calculator history"))
	if list.Submitted() {
		v.open()
	}
	if len(v.CalcHistory) == 0 && v.CalcResult == nil {
		message := v.Text("没有计算历史", "No calculator history")
		if v.Query != "" {
			message = v.Text("没有匹配的计算", "No matching calculations")
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) emojiBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(6, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, v.Text("Emoji 与符号", "Emoji & Symbols")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Grow(1).Role(ui.RoleHeading)
		if ui.Button(c, v.Text("肤色: ", "Skin: ")+v.EmojiTone.Label()).Clicked() {
			v.cycleEmojiTone()
		}
	})
	ui.Scroll(c).Grow(1).TrackScroll(&v.emojiScroll).Children(func() {
		ui.Column(c).FillWidth().Padding(8, 18).Gap(8).Children(func() {
			if len(v.EmojiItems) == 0 {
				message := v.Text("没有匹配的 Emoji", "No matching emoji")
				if v.Query == "" {
					message = v.Text("正在载入 Emoji…", "Loading emoji…")
				}
				ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
				return
			}
			group := ""
			var row []int
			flush := func() {
				if len(row) == 0 {
					return
				}
				indexes := row
				row = nil
				ui.Grid(c).Columns(emoji.GridColumns).Gap(6).Children(func() {
					for _, index := range indexes {
						v.emojiCell(c, index, tokens)
					}
				})
			}
			for index, item := range v.EmojiItems {
				if item.Group != group {
					flush()
					group = item.Group
					ui.Text(c, group).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
				}
				row = append(row, index)
				if len(row) >= emoji.GridColumns {
					flush()
				}
			}
			flush()
		})
	})
}

func (v *View) emojiCell(c *ui.Context, index int, tokens theme.Tokens) {
	item := v.EmojiItems[index]
	fill := ui.Transparent
	if index == v.EmojiChosen {
		fill = tokens.Selection
	}
	ui.Text(c, item.Glyph).FontSize(26).Height(50).TextAlign(ui.Center).
		Background(fill).Radius(10).Label(item.Name).Role(ui.RoleButton).
		OnClick(func() { v.EmojiChosen = index })
}

func (v *View) cycleEmojiTone() {
	if v.callbacks.CycleEmojiTone == nil {
		return
	}
	v.EmojiTone = v.callbacks.CycleEmojiTone()
	v.Filter()
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
	if v.Mode == ModeClipboard {
		if key.Mods == ui.Ctrl && key.Key == ui.KeyC {
			v.copySelectedClipboard()
			return true
		}
		if key.Mods == ui.Ctrl|ui.Shift && key.Key == ui.KeyEnter {
			v.pasteSelectedClipboard()
			return true
		}
		if key.Mods == 0 && key.Key == ui.KeyDelete {
			v.deleteSelectedClipboard()
			return true
		}
	}
	if key.Mods == ui.Ctrl|ui.Shift && (key.Key == ui.KeyEnter || key.Key == ui.KeyC) {
		v.syncQuery(key.Text())
		v.callbacks.CopyText(v.CopyText(), key.Key == ui.KeyEnter)
		return true
	}
	if key.Mods == ui.Ctrl {
		if v.Mode == ModeClipboard {
			switch key.Key {
			case ui.KeyP:
				v.CycleClipboardFilter()
				return true
			case ui.KeyPeriod:
				v.pinSelectedClipboard()
				return true
			}
			if digit, ok := digitKey(key.Key); ok {
				v.pastePinned(digit)
				return true
			}
		}
		if digit, ok := digitKey(key.Key); ok {
			if v.Mode == ModeLauncher && v.callbacks.Favorite != nil {
				v.callbacks.Favorite(digit)
			}
			return true
		}
		switch key.Key {
		case ui.KeyD:
			if v.Mode == ModeLauncher && v.callbacks.ToggleFavorite != nil {
				v.callbacks.ToggleFavorite()
			}
			return true
		case ui.KeyE:
			if v.callbacks.ToggleExpanded != nil {
				v.callbacks.ToggleExpanded()
			}
			return true
		case ui.KeyT:
			if v.Mode == ModeEmoji {
				v.cycleEmojiTone()
				return true
			}
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
		v.escape()
	case ui.KeyDown:
		v.moveVertical(1)
	case ui.KeyUp:
		v.moveVertical(-1)
	case ui.KeyTab:
		if v.Mode != ModeLauncher {
			return false
		}
		if strings.TrimSpace(key.Text()) == "" {
			v.SetModeClipboard()
			return true
		}
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
		switch v.Mode {
		case ModeClipboard, ModeDictionary, ModeCalculatorHistory, ModeEmoji:
			v.SetModeLauncher()
			return
		}
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
	switch v.Mode {
	case ModeDictionary:
		return
	case ModeClipboard:
		v.pasteSelectedClipboard()
		return
	case ModeCalculatorHistory:
		if v.Chosen < 0 && v.CalcResult != nil {
			v.copyCalculator()
			return
		}
		if v.Chosen >= 0 && v.Chosen < len(v.CalcHistory) && v.callbacks.CopyCalculatorHistory != nil {
			v.callbacks.CopyCalculatorHistory(v.CalcHistory[v.Chosen])
		}
		return
	case ModeEmoji:
		if v.EmojiChosen >= 0 && v.EmojiChosen < len(v.EmojiItems) && v.callbacks.EmojiPaste != nil {
			v.callbacks.EmojiPaste(v.EmojiItems[v.EmojiChosen].Glyph)
		}
		return
	}
	if v.Chosen < 0 && v.CalcResult != nil {
		v.copyCalculator()
		return
	}
	if v.Chosen >= 0 && v.Chosen < len(v.Results) {
		v.callbacks.Open(v.Results[v.Chosen])
	}
}

func (v *View) copyCalculator() {
	if v.CalcResult != nil && v.callbacks.CopyCalculator != nil {
		v.callbacks.CopyCalculator(*v.CalcResult)
	}
}

func (v *View) currentChosen() int {
	switch v.Mode {
	case ModeClipboard:
		return v.ClipboardChosen
	case ModeEmoji:
		return v.EmojiChosen
	}
	return v.Chosen
}

func (v *View) moveVertical(step int) {
	if v.Mode == ModeEmoji {
		v.choose(v.EmojiChosen + step*emoji.GridColumns)
		return
	}
	v.choose(v.currentChosen() + step)
}

func (v *View) moveHorizontal(step int) {
	if v.Mode == ModeEmoji {
		v.choose(v.EmojiChosen + step)
	}
}

func (v *View) choose(i int) {
	switch v.Mode {
	case ModeDictionary:
		return
	case ModeClipboard:
		if len(v.ClipboardItems) == 0 {
			v.ClipboardChosen = -1
			return
		}
		v.ClipboardChosen = max(0, min(i, len(v.ClipboardItems)-1))
		v.clipboardList.ScrollIntoView(v.ClipboardChosen)
		return
	case ModeCalculatorHistory:
		minimum := 0
		if v.CalcResult != nil {
			minimum = -1
		}
		maximum := len(v.CalcHistory) - 1
		if maximum < minimum {
			v.Chosen = -1
			return
		}
		v.Chosen = max(minimum, min(i, maximum))
		if v.Chosen >= 0 {
			v.calcHistoryList.ScrollIntoView(v.Chosen)
		}
		return
	case ModeEmoji:
		if len(v.EmojiItems) == 0 {
			v.EmojiChosen = -1
			return
		}
		v.EmojiChosen = max(0, min(i, len(v.EmojiItems)-1))
		return
	}
	minimum := 0
	if v.CalcResult != nil {
		minimum = -1
	}
	if len(v.Results) == 0 {
		v.Chosen = minimum
		return
	}
	v.Chosen = max(minimum, min(i, len(v.Results)-1))
	if v.Chosen >= 0 {
		v.list.ScrollIntoView(v.Chosen)
	}
}

func (v *View) Filter() {
	switch v.Mode {
	case ModeDictionary:
		v.CalcResult = nil
		v.CalcHistory = nil
		v.Definition = nil
		v.Suggestions = nil
		v.DictionaryError = ""
		v.dictionaryScroll.X, v.dictionaryScroll.Y = 0, 0
		if v.OnLookup != nil {
			v.OnLookup(v.Query)
		}
		return
	case ModeClipboard:
		v.CalcResult = nil
		v.CalcHistory = nil
		if v.callbacks.ClipboardSearch != nil {
			v.ClipboardItems = v.callbacks.ClipboardSearch(v.Query, v.ClipboardFilter)
		} else {
			v.ClipboardItems = nil
		}
		v.choose(0)
		return
	case ModeCalculatorHistory:
		if v.callbacks.Calculate != nil {
			v.CalcResult = v.callbacks.Calculate(v.Query)
		}
		if v.callbacks.CalculatorHistory != nil {
			v.CalcHistory = v.callbacks.CalculatorHistory(v.Query)
		}
		if v.CalcResult != nil {
			v.choose(-1)
		} else {
			v.choose(0)
		}
		return
	case ModeEmoji:
		v.CalcResult = nil
		v.CalcHistory = nil
		if v.callbacks.EmojiSearch != nil {
			v.EmojiItems = v.callbacks.EmojiSearch(v.Query, v.EmojiTone)
		} else {
			v.EmojiItems = nil
		}
		v.choose(0)
		return
	}
	if v.OnLookup != nil {
		v.OnLookup("") // cancel a lookup after returning to Applications
	}
	if v.callbacks.Calculate != nil {
		v.CalcResult = v.callbacks.Calculate(v.Query)
	}
	started := time.Now()
	if v.callbacks.Search != nil {
		v.Results = v.callbacks.Search(v.Query, v.ShowHidden)
	} else {
		v.Results = domain.Filter(v.Apps, v.Query)
	}
	if v.CalcResult != nil {
		v.choose(-1)
	} else {
		v.choose(0)
	}
	if v.callbacks.Filtered != nil {
		v.callbacks.Filtered(time.Since(started), len(v.Results))
	}
}

func (v *View) CountStatus() string {
	return fmt.Sprintf(v.Text("%d 个应用程序", "%d applications"), len(v.Apps))
}

func (v *View) SetModeLauncher() {
	v.Mode = ModeLauncher
	v.Query = v.launcherQuery
	v.inputText.Set(v.launcherQuery)
	v.launcherQuery = ""
	v.Chosen, v.ClipboardChosen, v.EmojiChosen = -1, -1, -1
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.Filter()
	v.Input.Focus()
}

func (v *View) SetModeEmoji() {
	if v.Mode == ModeEmoji {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeEmoji
	v.Query = ""
	v.inputText.Set("")
	v.EmojiChosen = -1
	v.Filter()
	v.Input.Focus()
}

func (v *View) SetModeCalculatorHistory() {
	if v.Mode == ModeCalculatorHistory {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeCalculatorHistory
	v.Query = ""
	v.inputText.Set("")
	v.Chosen = -1
	v.EmojiItems = nil
	v.Filter()
	v.Input.Focus()
}

func (v *View) SetModeClipboard() {
	if v.Mode == ModeClipboard {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeClipboard
	v.Query = ""
	v.inputText.Set("")
	v.ClipboardChosen = -1
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.Filter()
	v.Input.Focus()
}

func (v *View) Clipboard() bool { return v.Mode == ModeClipboard }

func (v *View) InDictionary() bool { return v.Mode == ModeDictionary }

// ChooseClipboard moves the clipboard selection after a store mutation.
func (v *View) ChooseClipboard(index int) {
	if index < 0 || index >= len(v.ClipboardItems) {
		return
	}
	v.ClipboardChosen = index
	v.clipboardList.ScrollIntoView(index)
}

func (v *View) ToggleDictionary() {
	if v.Mode == ModeDictionary {
		v.SetModeLauncher()
		return
	}
	if !v.DictionaryAvailable {
		v.Status = v.Text("词典库不可用", "Dictionary database unavailable")
		return
	}
	v.launcherQuery = v.inputText.String()
	v.Query = v.inputText.String()
	v.Mode = ModeDictionary
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.dictionaryScroll.X, v.dictionaryScroll.Y = 0, 0
	v.Filter()
	v.Input.Focus()
}

func (v *View) clipboardBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(4, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, clipboardFilterLabel(v.Chinese, v.ClipboardFilter)).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Grow(1).Role(ui.RoleHeading)
		if v.ClipboardChosen >= 0 && v.ClipboardChosen < len(v.ClipboardItems) {
			label := v.Text("置顶", "Pin")
			if v.ClipboardItems[v.ClipboardChosen].Pinned() {
				label = v.Text("取消置顶", "Unpin")
			}
			if ui.Button(c, label).Clicked() {
				v.pinSelectedClipboard()
			}
		}
		if ui.Button(c, v.Text("筛选 Ctrl+P", "Filter Ctrl+P")).Clicked() {
			v.CycleClipboardFilter()
		}
	})
	v.clipboardPreviewBody(c, tokens)
	v.clipboardList.Selected = &v.ClipboardChosen
	v.clipboardList.Key = func(i int) any { return v.ClipboardItems[i].ID }
	v.clipboardList.Label = func(i int) string { return clipboardRowTitle(v.ClipboardItems[i], v.Chinese) }
	list := ui.List(c, &v.clipboardList, len(v.ClipboardItems), func(i int) {
		v.clipboardRow(c, i, tokens)
	}).Grow(1).Padding(0, 8).Label(v.Text("剪贴板历史", "Clipboard history"))
	if list.Submitted() {
		v.open()
	}
	if len(v.ClipboardItems) == 0 {
		message := clipboardEmptyLabel(v.Chinese, v.ClipboardFilter)
		if !v.ClipboardAvailable {
			message = v.Text("剪贴板存储不可用，已有历史记录已保留", "Clipboard storage is unavailable; existing history is preserved")
		}
		if v.ClipboardError != "" {
			message = v.ClipboardError
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) clipboardPreviewBody(c *ui.Context, tokens theme.Tokens) {
	if v.ClipboardChosen < 0 || v.ClipboardChosen >= len(v.ClipboardItems) {
		return
	}
	item := v.ClipboardItems[v.ClipboardChosen]
	ui.Column(c).FillWidth().Padding(8, 18).Gap(6).Background(tokens.Card2).Children(func() {
		if item.Kind == clipboard.KindImage {
			if v.callbacks.ClipboardImage != nil {
				if bitmap := v.callbacks.ClipboardImage(item); bitmap != nil {
					ui.Image(c, bitmap).Size(160, 100).Fit(ui.Contain).Role(ui.RoleNone)
					return
				}
			}
			ui.Text(c, v.Text("图片", "Image")+" · "+item.ImagePath).FontSize(theme.TrailingFont).
				TextColor(tokens.TextTertiary).SingleLine()
			return
		}
		text := item.Text
		if len([]rune(text)) > 600 {
			text = string([]rune(text)[:600])
		}
		ui.Text(c, text).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).MaxLines(3)
	})
}

func (v *View) clipboardRow(c *ui.Context, i int, tokens theme.Tokens) {
	item := v.ClipboardItems[i]
	trailing := ""
	if item.Pinned() {
		trailing = v.Text("置顶", "Pinned")
	}
	components.ResultRow(c, tokens, components.Row{
		Fallback: clipboardRowFallback(item, v.Chinese),
		Title:    clipboardRowTitle(item, v.Chinese),
		Trailing: trailing,
		Selected: i == v.ClipboardChosen,
	})
}

func clipboardRowTitle(item clipboard.Item, chinese bool) string {
	if item.Kind == clipboard.KindImage {
		if chinese {
			return "[图片]"
		}
		return "[Image]"
	}
	return collapseClipboardText(item.Text, 200)
}

func clipboardRowFallback(item clipboard.Item, chinese bool) string {
	if item.Kind == clipboard.KindImage {
		if chinese {
			return "图"
		}
		return "IMG"
	}
	return "TXT"
}

func collapseClipboardText(text string, maxChars int) string {
	var builder strings.Builder
	chars := 0
	for _, r := range text {
		if chars == maxChars {
			break
		}
		if r == '\n' || r == '\r' || r == '\t' || r == ' ' {
			if builder.Len() > 0 && !strings.HasSuffix(builder.String(), " ") {
				builder.WriteByte(' ')
				chars++
			}
			continue
		}
		builder.WriteRune(r)
		chars++
	}
	return strings.TrimRight(builder.String(), " ")
}

func (v *View) CycleClipboardFilter() {
	next := (v.ClipboardFilter + 1) % 5
	v.ClipboardFilter = next
	if v.callbacks.SetClipboardFilter != nil {
		v.callbacks.SetClipboardFilter(next)
	}
	v.ClipboardChosen = -1
	v.Filter()
}

func (v *View) pinSelectedClipboard() {
	if v.ClipboardChosen < 0 || v.ClipboardChosen >= len(v.ClipboardItems) {
		return
	}
	item := v.ClipboardItems[v.ClipboardChosen]
	if v.callbacks.ClipboardPin != nil {
		v.callbacks.ClipboardPin(item)
	}
}

func (v *View) pasteSelectedClipboard() {
	if v.ClipboardChosen < 0 || v.ClipboardChosen >= len(v.ClipboardItems) {
		return
	}
	item := v.ClipboardItems[v.ClipboardChosen]
	if v.callbacks.ClipboardPaste != nil {
		v.callbacks.ClipboardPaste(item)
	}
}

func (v *View) copySelectedClipboard() {
	if v.ClipboardChosen < 0 || v.ClipboardChosen >= len(v.ClipboardItems) {
		return
	}
	item := v.ClipboardItems[v.ClipboardChosen]
	if v.callbacks.ClipboardCopy != nil {
		v.callbacks.ClipboardCopy(item)
	}
}

func (v *View) deleteSelectedClipboard() {
	if v.ClipboardChosen < 0 || v.ClipboardChosen >= len(v.ClipboardItems) {
		return
	}
	item := v.ClipboardItems[v.ClipboardChosen]
	if v.callbacks.ClipboardDelete != nil {
		v.callbacks.ClipboardDelete(item)
	}
}

func (v *View) pastePinned(digit byte) {
	if v.callbacks.ClipboardPinned != nil {
		v.callbacks.ClipboardPinned(digit)
	}
}

func clipboardFilterLabel(chinese bool, filter clipboard.Filter) string {
	if chinese {
		switch filter {
		case clipboard.FilterText:
			return "文本"
		case clipboard.FilterImages:
			return "图片"
		case clipboard.FilterLinks:
			return "链接"
		case clipboard.FilterEmails:
			return "邮箱"
		default:
			return "全部"
		}
	}
	switch filter {
	case clipboard.FilterText:
		return "Text"
	case clipboard.FilterImages:
		return "Images"
	case clipboard.FilterLinks:
		return "Links"
	case clipboard.FilterEmails:
		return "Emails"
	default:
		return "All"
	}
}

func clipboardEmptyLabel(chinese bool, filter clipboard.Filter) string {
	if chinese {
		switch filter {
		case clipboard.FilterText:
			return "剪贴板历史中没有文本"
		case clipboard.FilterImages:
			return "剪贴板历史中没有图片"
		case clipboard.FilterLinks:
			return "剪贴板历史中没有链接"
		case clipboard.FilterEmails:
			return "剪贴板历史中没有邮箱"
		default:
			return "剪贴板历史为空"
		}
	}
	switch filter {
	case clipboard.FilterText:
		return "No text in clipboard history"
	case clipboard.FilterImages:
		return "No images in clipboard history"
	case clipboard.FilterLinks:
		return "No links in clipboard history"
	case clipboard.FilterEmails:
		return "No email addresses in clipboard history"
	default:
		return "Clipboard history is empty"
	}
}
