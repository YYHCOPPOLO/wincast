package palette

import (
	"fmt"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	domaincalc "github.com/YYHCOPPOLO/wincast/go/internal/domain/calculator"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/emoji"
	filesearch "github.com/YYHCOPPOLO/wincast/go/internal/domain/filesearch"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/meeting"
	domainuninstall "github.com/YYHCOPPOLO/wincast/go/internal/domain/uninstall"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/clipboard"
	files "github.com/YYHCOPPOLO/wincast/go/internal/features/filesearch"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/chat"
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
	ModeArguments
	ModeFileSearch
	ModeSchedule
	ModeUninstall
	ModeAI
	ModeAIHistory
)

// Callbacks are the user's intent. The view never performs I/O and never
// holds a repository; the app decides what an action means.
type Callbacks struct {
	Open             func(domain.Application)
	Hide             func()
	Menu             func()
	AppMenu          func()
	CopyText         func(string, bool)
	Icon             func(domain.Application) *ui.Bitmap
	Filtered         func(time.Duration, int)
	Search           func(query string, showHidden bool) []domain.Application
	Favorite         func(byte)
	FavoriteSlot     func(byte) string
	ToggleFavorite   func()
	ToggleExpanded   func()
	ExpandedChanged  func()
	SaveAlias        func(domain.Application, string)
	SetHidden        func(domain.Application, bool)
	ToggleShowHidden func()

	ClipboardSearch    func(query string, filter clipboard.Filter) []clipboard.Item
	ClipboardPaste     func(clipboard.Item)
	ClipboardCopy      func(clipboard.Item)
	ClipboardPin       func(clipboard.Item)
	ClipboardDelete    func(clipboard.Item)
	ClipboardImage     func(clipboard.Item) (*ui.Bitmap, bool)
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

	SubmitArguments func(domain.Application, []string)

	FileSearch   func(query string)
	OpenFile     func(filesearch.Hit)
	RevealFile   func(filesearch.Hit)
	CopyFilePath func(filesearch.Hit)

	JoinMeeting     func(meeting.Event)
	CopyMeetingLink func(meeting.Event)

	ConfirmUninstall    func()
	CopyUninstallPath   func(string)
	RevealUninstallPath func(string)

	AISend         func(string)
	AICancel       func()
	AINew          func()
	AIHistory      func(string) []domainai.Conversation
	AIOpen         func(string)
	AIModelMenu    func()
	AILeave        func(string)
	AIHistoryClose func()

	ApplyVolume func(float64)
	CloseVolume func()
}

// View is UI-thread-owned display state. Callbacks send user intent back to
// the app; no disk, SQL or shell work runs as the view is constructed.
type View struct {
	Query               string
	Apps                []domain.Application
	Results             []domain.Application
	Loading             bool
	Status              string
	Shortcut            string
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
	clipboardPreviewID string

	CalcResult      *domaincalc.Result
	CalcHistory     []domaincalc.HistoryEntry
	calcHistoryList ui.ListState

	EmojiItems  []emoji.Emoji
	EmojiTone   emoji.SkinTone
	EmojiChosen int
	emojiScroll ui.ScrollState
	emojiReveal bool

	Argument     *ArgumentForm
	argumentList ui.ListState

	FileSearchItems []filesearch.Hit
	FileSearchState files.State
	FileSearchError string
	fileSearchList  ui.ListState

	ScheduleEvents  []meeting.Event
	ScheduleItems   []meeting.Event
	ScheduleLoading bool
	ScheduleError   string
	scheduleList    ui.ListState

	UninstallTarget  string
	UninstallAll     []domainuninstall.Candidate
	UninstallItems   []domainuninstall.Candidate
	UninstallLoading bool
	UninstallError   string
	uninstallList    ui.ListState

	AIMessages      []domainai.ChatMessage
	AINotice        string
	AIThinking      bool
	AIStreaming     bool
	AIAvailable     bool
	AIModelLabel    string
	aiScroll        ui.ScrollState
	aiDraft         string
	AIHistoryItems  []domainai.Conversation
	AIHistoryChosen int
	aiHistoryList   ui.ListState

	VolumeOpen  bool
	VolumeValue float64
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
	if v.Mode == ModeEmoji {
		c.OnShortcut(0, ui.KeyLeft, func() { v.moveHorizontal(-1) })
		c.OnShortcut(0, ui.KeyRight, func() { v.moveHorizontal(1) })
	}
	c.OnShortcut(0, ui.KeyEnter, v.open)
	if v.Mode == ModeAI && v.callbacks.AINew != nil {
		c.OnShortcut(ui.Ctrl, ui.KeyN, v.callbacks.AINew)
	}
	v.renderAlias(c, tokens)
	v.renderVolume(c, tokens)
	ui.Column(c).Fill().Children(func() {
		ui.Row(c).Height(theme.HeaderHeight+2*theme.HeaderPadding).
			Padding(theme.HeaderPadding, 20).Gap(14).AlignItems(ui.Center).Children(func() {
			if v.Mode == ModeLauncher {
				ui.Text(c, "⌕").FontSize(24).TextColor(tokens.TextTertiary)
			} else {
				v.backButton(c, tokens)
			}
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
			case ModeFileSearch:
				placeholder = v.Text("搜索文件和文件夹…", "Search files and folders…")
			case ModeSchedule:
				placeholder = v.Text("搜索日程…", "Search schedule…")
			case ModeUninstall:
				placeholder = v.Text("搜索卸载项…", "Search items…")
			case ModeAI:
				placeholder = v.Text("问点什么…", "Ask anything…")
			case ModeAIHistory:
				placeholder = v.Text("搜索对话…", "Search chats…")
			case ModeArguments:
				if current := v.argumentCurrent(); current != nil {
					placeholder = current.Name
				} else {
					placeholder = v.Text("填写参数…", "Enter arguments…")
				}
			}
			if ui.TextInputBufferBase(c, v.inputText).Bind(&v.Input).Grow(1).Height(theme.HeaderHeight).FontSize(theme.SearchFont).
				Placeholder(placeholder).Label(placeholder).OnTextKey(v.textKey).Changed() {
				v.syncQuery(v.inputText.String())
			}
			if v.Mode == ModeClipboard && v.Expanded {
				v.clipboardFilterButton(c, tokens)
			}
			if v.Mode == ModeAI && v.Expanded {
				v.aiModelButton(c, tokens)
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
		case ModeFileSearch:
			v.fileSearchBody(c, tokens)
		case ModeSchedule:
			v.scheduleBody(c, tokens)
		case ModeUninstall:
			v.uninstallBody(c, tokens)
		case ModeAI:
			chat.Body(c, tokens, v.AIMessages, v.AINotice, v.AIThinking, v.AIStreaming, v.Chinese, &v.aiScroll)
		case ModeAIHistory:
			v.aiHistoryBody(c, tokens)
		case ModeArguments:
			v.argumentsBody(c, tokens)
		default:
			v.applications(c, tokens)
		}
		ui.Divider(c)
		status := v.Status
		if status == "" {
			status = v.defaultStatus()
		}
		components.Footer(c, tokens, status, v.footerHints(), v.callbacks.Menu, v.callbacks.AppMenu)
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
	case ModeFileSearch:
		if v.FileSearchError != "" {
			return v.FileSearchError
		}
		if v.FileSearchState == files.Failed {
			return v.Text("文件搜索不可用", "File search is unavailable")
		}
		return v.Text("文件搜索", "File search")
	case ModeSchedule:
		if v.ScheduleError != "" {
			return v.ScheduleError
		}
		if v.ScheduleLoading {
			return v.Text("正在读取日历…", "Loading calendar…")
		}
		return v.Text("我的日程", "My schedule")
	case ModeUninstall:
		if v.UninstallError != "" {
			return v.UninstallError
		}
		if v.UninstallLoading {
			return v.Text("正在查找应用文件…", "Finding application files…")
		}
		selected := domainuninstall.SelectedCount(v.UninstallAll)
		return fmt.Sprintf(v.Text("已选择 %d / %d 项", "%d of %d selected"), selected, len(v.UninstallAll))
	case ModeAI:
		if v.AINotice != "" {
			return v.AINotice
		}
		if v.AIStreaming {
			return v.Text("正在回复…", "Streaming…")
		}
		return v.Text("AI 对话", "AI Chat")
	case ModeAIHistory:
		return fmt.Sprintf(v.Text("%d 段对话", "%d chats"), len(v.AIHistoryItems))
	case ModeArguments:
		return v.Text("填写模板参数", "Template arguments")
	default:
		return v.Text("Go 预览", "Go Preview") + " · " + v.Shortcut
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
	case ModeFileSearch:
		return v.Text("Enter 打开  Ctrl+Shift+C 复制路径  Esc 返回", "Enter Open  Ctrl+Shift+C Copy path  Esc Back")
	case ModeSchedule:
		return v.Text("Enter 入会  Ctrl+Shift+C 复制链接  Esc 返回", "Enter Join  Ctrl+Shift+C Copy link  Esc Back")
	case ModeUninstall:
		return v.Text("Enter 卸载  Ctrl+Enter 选择  Ctrl+Shift+C 复制路径  Esc 返回", "Enter Uninstall  Ctrl+Enter Select  Ctrl+Shift+C Copy path  Esc Back")
	case ModeAI:
		if v.AIStreaming {
			return v.Text("Enter 停止  Ctrl+N 新对话  Ctrl+K 操作", "Enter Stop  Ctrl+N New chat  Ctrl+K Actions")
		}
		return v.Text("Enter 发送  Ctrl+N 新对话  Ctrl+K 操作", "Enter Send  Ctrl+N New chat  Ctrl+K Actions")
	case ModeAIHistory:
		return v.Text("Enter 打开  Esc 返回", "Enter Open  Esc Back")
	case ModeArguments:
		return v.Text("Enter 确认  Backspace 上一步  Esc 取消", "Enter Confirm  Backspace Back  Esc Cancel")
	default:
		if v.CalcResult != nil {
			return v.Text("Enter 复制答案  ↑↓ 选择", "Enter Copy answer  ↑↓ Select")
		}
		return v.Text("Tab 词典   ↑↓ 选择", "Tab Dictionary   ↑↓ Select")
	}
}

func (v *View) renderVolume(c *ui.Context, tokens theme.Tokens) {
	if !v.VolumeOpen {
		return
	}
	components.Dialog(c, &v.VolumeOpen, func() {
		ui.Text(c, v.Text("设置音量", "Set volume")).FontSize(16).Bold().Role(ui.RoleHeading)
		ui.Text(c, fmt.Sprintf("%.0f%%", v.VolumeValue*100)).FontSize(theme.SectionFont).TextColor(tokens.TextSecondary)
		ui.Slider(c, &v.VolumeValue, 0, 1).Grow(1).Label(v.Text("音量", "Volume"))
		ui.Row(c).Gap(8).Children(func() {
			if ui.Button(c, v.Text("应用", "Apply")).Clicked() {
				if v.callbacks.ApplyVolume != nil {
					v.callbacks.ApplyVolume(v.VolumeValue)
				}
				v.VolumeOpen = false
			}
			if ui.Button(c, v.Text("取消", "Cancel")).Clicked() {
				if v.callbacks.CloseVolume != nil {
					v.callbacks.CloseVolume()
				}
				v.VolumeOpen = false
			}
		})
	})
	if !v.VolumeOpen && v.callbacks.CloseVolume != nil {
		v.callbacks.CloseVolume()
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
	v.list.Label = func(i int) string { return v.Results[i].Name }
	list := components.ResultList(c, tokens, &v.list, len(v.Results), func(i int) { v.row(c, i, tokens) }).Grow(1).Padding(0, 8).
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

// calcCard mirrors the original CalculatorCard: expression and answer as two
// equal columns around an arrow, each with its own pill, or one warning line
// for an error. A click chooses the card; Enter still copies it.
func (v *View) calcCard(c *ui.Context, tokens theme.Tokens) {
	if v.CalcResult == nil {
		return
	}
	result := v.CalcResult
	gap := float32(0)
	if result.IsError() {
		gap = theme.CalcColumnGap
	}
	ui.Row(c).FillWidth().Padding(theme.CalcCardGapY, theme.CalcCardInset).Children(func() {
		card := ui.Row(c).FillWidth().
			Padding(theme.CalcCardPaddingY, theme.CalcCardPaddingX).
			Gap(gap).AlignItems(ui.Center).Radius(theme.CalcCardRadius)
		fill := tokens.CardFill
		switch {
		case v.Chosen < 0:
			fill = tokens.Selection
		case card.Hovered():
			fill = tokens.CardHover
		}
		card.Background(fill).Cursor(ui.CursorPointer).Children(func() {
			if result.IsError() {
				ui.Text(c, "\uE7BA").Font("Segoe Fluent Icons").FontSize(theme.RowFont).
					TextColor(tokens.TextSecondary).Shrink(0)
				ui.Text(c, result.Display).FontSize(theme.RowFont).TextColor(tokens.TextSecondary).
					SingleLine().Grow(1)
				return
			}
			v.calcColumn(c, tokens, result.Expression, result.SourceBadge, 500)
			ui.Text(c, "\uE72A").Font("Segoe Fluent Icons").FontSize(theme.CalcArrowFont).
				TextColor(tokens.TextTertiary).Shrink(0)
			v.calcColumn(c, tokens, result.Display, result.TargetBadge, 600)
		})
		if card.Clicked() {
			v.choose(-1)
		}
	})
}

// calcColumn is one side of the answer card: the value with its connectors
// dimmed, and the word-name pill beneath it.
func (v *View) calcColumn(c *ui.Context, tokens theme.Tokens, text, badge string, weight int) {
	ui.Column(c).Grow(1).Basis(0).MinWidth(0).Padding(0, theme.CalcColumnPadding).
		Gap(theme.CalcColumnGap).AlignItems(ui.Center).Children(func() {
		ui.RichText(c, calcTextSpans(text, tokens)...).FontSize(theme.CalcResultFont).
			FontWeight(weight).TextColor(tokens.Text).TextAlign(ui.Center).
			SingleLine().FillWidth()
		if badge == "" {
			return
		}
		ui.Row(c).Padding(theme.CalcBadgePadY, theme.CalcBadgePadX).
			Radius(theme.CalcBadgeRadius).Background(tokens.ControlSurface).Children(func() {
			ui.Text(c, badge).FontSize(theme.CalcBadgeFont).
				TextColor(tokens.TextSecondary).SingleLine()
		})
	})
}

// calcConnectors are the words and operators the original dims, so the values
// they join read first.
var calcConnectors = map[string]bool{
	"to": true, "of": true, "off": true, "on": true, "as": true, "from": true,
	"ago": true, "at": true, "tip": true, "ratio": true, "average": true,
	"avg": true, "mean": true, "sum": true, "total": true, "round": true,
	"nearest": true, "and": true, "is": true, "what": true, "the": true,
	"next": true, "last": true, "+": true, "-": true, "×": true, "÷": true,
	"^": true, "→": true, "->": true, "mod": true,
}

// calcTextSpans builds one styled paragraph, dimming connectors. `in` is a
// unit after a value, so only the joining `in` in `10 in in cm` counts.
func calcTextSpans(text string, tokens theme.Tokens) []ui.Span {
	words := strings.Split(text, " ")
	spans := make([]ui.Span, 0, len(words)*2)
	for index, word := range words {
		if index > 0 {
			spans = append(spans, ui.Span{Text: " "})
		}
		color := tokens.Text
		if calcConnector(word, index, words) {
			color = tokens.TextTertiary
		}
		spans = append(spans, ui.Span{Text: word, Color: color})
	}
	return spans
}

func calcConnector(word string, index int, words []string) bool {
	lowered := strings.ToLower(word)
	if calcConnectors[lowered] {
		return true
	}
	if lowered != "in" || index == 0 || index+1 >= len(words) {
		return false
	}
	return strings.ToLower(words[index+1]) != "in"
}

func (v *View) calculatorHistoryBody(c *ui.Context, tokens theme.Tokens) {
	v.calcCard(c, tokens)
	v.calcHistoryList.Selected = &v.Chosen
	v.calcHistoryList.Label = func(i int) string { return v.CalcHistory[i].Expression }
	list := components.ResultList(c, tokens, &v.calcHistoryList, len(v.CalcHistory), func(i int) {
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
			for _, section := range emoji.Sections(v.EmojiItems) {
				ui.Text(c, section.Group).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Role(ui.RoleHeading)
				for start := section.Start; start < section.Start+section.Count; start += emoji.GridColumns {
					end := min(start+emoji.GridColumns, section.Start+section.Count)
					ui.Grid(c).FillWidth().Columns(emoji.GridColumns).Gap(6).Children(func() {
						for index := start; index < end; index++ {
							v.emojiCell(c, index, tokens)
						}
						// Empty cells retain the columns of a partial category row.
						for missing := end - start; missing < emoji.GridColumns; missing++ {
							ui.Spacer(c).Height(emoji.CellDip)
						}
					})
				}
			}
		})
	})
	v.emojiReveal = false
}

func (v *View) emojiCell(c *ui.Context, index int, tokens theme.Tokens) {
	item := v.EmojiItems[index]
	fill := ui.Transparent
	if index == v.EmojiChosen {
		fill = tokens.Selection
	}
	cell := ui.Text(c, item.Glyph).FontSize(30).Height(emoji.CellDip).TextAlign(ui.Center).
		Background(fill).Radius(10).Label(item.Name).Role(ui.RoleButton).
		OnClick(func() { v.choose(index) })
	if v.emojiReveal && index == v.EmojiChosen {
		sections := emoji.Sections(v.EmojiItems)
		if len(sections) > 0 && index >= min(emoji.GridColumns, sections[0].Count) {
			cell.ScrollIntoView()
		}
	}
}

func (v *View) fileSearchBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(6, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, v.Text("文件与文件夹", "Files and folders")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Grow(1).Role(ui.RoleHeading)
		if v.FileSearchState == files.Searching {
			ui.Text(c, v.Text("搜索中…", "Searching…")).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		}
	})
	v.fileSearchList.Selected = &v.Chosen
	v.fileSearchList.Label = func(i int) string { return v.FileSearchItems[i].Name }
	list := components.ResultList(c, tokens, &v.fileSearchList, len(v.FileSearchItems), func(i int) {
		hit := v.FileSearchItems[i]
		fallback := v.Text("文件", "FILE")
		if hit.IsDirectory {
			fallback = v.Text("目录", "DIR")
		}
		components.ResultRow(c, tokens, components.Row{
			Fallback: fallback, Title: hit.Name, Subtitle: hit.Parent, Selected: i == v.Chosen,
		})
	}).Grow(1).Padding(0, 8).Label(v.Text("文件搜索结果", "File search results"))
	if list.Submitted() {
		v.open()
	}
	if len(v.FileSearchItems) == 0 {
		message := v.Text("没有找到文件", "No files found")
		switch {
		case v.Query == "":
			message = v.Text("输入以搜索文件和文件夹", "Type to search files and folders")
		case v.FileSearchState == files.Searching || v.FileSearchState == files.Idle:
			message = v.Text("正在搜索文件…", "Searching files…")
		case v.FileSearchState == files.Failed:
			message = v.Text("文件搜索不可用", "File search is unavailable")
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) scheduleBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(6, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, v.Text("日程", "Schedule")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Grow(1).Role(ui.RoleHeading)
		if v.ScheduleLoading {
			ui.Text(c, v.Text("正在读取…", "Loading…")).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		}
	})
	v.scheduleList.Selected = &v.Chosen
	v.scheduleList.Label = func(i int) string { return v.ScheduleItems[i].Title }
	list := components.ResultList(c, tokens, &v.scheduleList, len(v.ScheduleItems), func(i int) {
		event := v.ScheduleItems[i]
		fallback := event.CalendarName
		if fallback == "" {
			fallback = v.Text("日程", "CAL")
		}
		title := event.Title
		if title == "" {
			title = v.Text("(无标题)", "(Untitled)")
		}
		components.ResultRow(c, tokens, components.Row{
			Fallback: fallback, Title: title, Trailing: v.scheduleCountdown(event.Start),
			Selected: i == v.Chosen,
		})
	}).Grow(1).Padding(0, 8).Label(v.Text("日程列表", "Schedule"))
	if list.Submitted() {
		v.open()
	}
	if len(v.ScheduleItems) == 0 {
		message := v.Text("没有即将开始的日程", "No upcoming events")
		switch {
		case v.ScheduleError != "":
			message = v.ScheduleError
		case v.ScheduleLoading:
			message = v.Text("正在读取日历…", "Loading calendar…")
		case strings.TrimSpace(v.Query) != "":
			message = v.Text("没有匹配的日程", "No matching events")
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) scheduleCountdown(start int64) string {
	now := time.Now().Unix()
	delta := start - now
	if delta > 0 {
		return fmt.Sprintf(v.Text("%d 分钟后", "in %d min"), (delta+59)/60)
	}
	elapsed := -delta / 60
	if elapsed == 0 {
		return v.Text("现在", "now")
	}
	return fmt.Sprintf(v.Text("%d 分钟前", "%d min ago"), elapsed)
}

// SetScheduleEvents replaces the snapshot and filters it again. A selection
// is kept by event ID so a minute tick does not jump the highlight.
func (v *View) SetScheduleEvents(events []meeting.Event, errText string) {
	v.ScheduleEvents = events
	v.ScheduleError = errText
	v.ScheduleLoading = false
	v.filterSchedule(false)
}

func (v *View) filterSchedule(reset bool) {
	selected := ""
	if !reset && v.Chosen >= 0 && v.Chosen < len(v.ScheduleItems) {
		selected = v.ScheduleItems[v.Chosen].ID
	}
	query := strings.ToLower(strings.TrimSpace(v.Query))
	items := make([]meeting.Event, 0, len(v.ScheduleEvents))
	for _, event := range v.ScheduleEvents {
		if query != "" && !strings.Contains(strings.ToLower(event.Title), query) &&
			!strings.Contains(strings.ToLower(event.CalendarName), query) {
			continue
		}
		items = append(items, event)
	}
	v.ScheduleItems = items
	v.Chosen = -1
	if selected != "" {
		for index, event := range items {
			if event.ID == selected {
				v.Chosen = index
				break
			}
		}
	}
	if v.Chosen < 0 && len(items) > 0 {
		v.Chosen = 0
	}
	if v.Chosen >= 0 {
		v.scheduleList.ScrollIntoView(v.Chosen)
	}
}

func (v *View) uninstallBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(6, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		label := v.Text("卸载", "Uninstall")
		if v.UninstallTarget != "" {
			label += " · " + v.UninstallTarget
		}
		ui.Text(c, label).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Grow(1).SingleLine().Role(ui.RoleHeading)
		if v.UninstallLoading {
			ui.Text(c, v.Text("正在扫描…", "Scanning…")).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		}
	})
	v.uninstallList.Selected = &v.Chosen
	v.uninstallList.Label = func(i int) string { return v.UninstallItems[i].Name }
	list := components.ResultList(c, tokens, &v.uninstallList, len(v.UninstallItems), func(i int) {
		item := v.UninstallItems[i]
		fill := ui.Transparent
		if i == v.Chosen {
			fill = tokens.Selection
		}
		row := ui.Row(c).Height(theme.RowHeight).Padding(6, 10).Gap(12).AlignItems(ui.Center).
			Radius(theme.RowRadius).Background(fill).Children(func() {
			box := ui.Box(c).Size(16, 16).Radius(4).Shrink(0)
			if item.Removable {
				box.Cursor(ui.CursorPointer)
			}
			if item.Selected {
				box.Background(tokens.Accent).DrawOver(func(p *ui.Painter, r ui.Rect) {
					var check ui.Path
					check.MoveTo(r.X+r.W*0.20, r.Y+r.H*0.52).
						LineTo(r.X+r.W*0.42, r.Y+r.H*0.74).
						LineTo(r.X+r.W*0.80, r.Y+r.H*0.28)
					p.StrokePath(&check, 1.8, tokens.AccentText)
				})
			} else {
				border := tokens.Border.Mix(tokens.Text, 0.25)
				if !item.Removable {
					border = tokens.Line2
				}
				box.Background(tokens.Surface).Border(1, border)
			}
			if item.Removable {
				box.OnClick(func() { v.setUninstallSelected(i, !item.Selected) })
			}
			ui.Column(c).Grow(1).Gap(1).Children(func() {
				color := tokens.Text
				if !item.Removable {
					color = tokens.TextTertiary
				}
				ui.Text(c, item.Name).FontSize(theme.RowFont).TextColor(color).SingleLine()
				ui.Text(c, item.DisplayPath).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine()
			})
			reason := item.Error
			if reason == "" {
				reason = item.LockReason
			}
			if reason != "" {
				ui.Text(c, reason).FontSize(theme.KeyCapFont).TextColor(tokens.Destructive).SingleLine()
			}
		})
		if row.RowDoubleClicked() {
			v.setUninstallSelected(i, !item.Selected)
		}
	}).Grow(1).Padding(0, 8).Label(v.Text("卸载项目", "Uninstall items"))
	if list.Submitted() {
		if v.callbacks.ConfirmUninstall != nil {
			v.callbacks.ConfirmUninstall()
		}
	}
	if len(v.UninstallItems) == 0 {
		message := v.Text("没有可卸载的项目", "Nothing to remove")
		switch {
		case v.UninstallError != "":
			message = v.UninstallError
		case v.UninstallLoading:
			message = v.Text("正在查找应用文件…", "Finding application files…")
		case strings.TrimSpace(v.Query) != "":
			message = v.Text("没有匹配的项目", "No matching items")
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) ToggleUninstall(index int) {
	if index < 0 || index >= len(v.UninstallItems) {
		return
	}
	v.setUninstallSelected(index, !v.UninstallItems[index].Selected)
}

func (v *View) setUninstallSelected(index int, selected bool) {
	if index < 0 || index >= len(v.UninstallItems) || !v.UninstallItems[index].Removable {
		return
	}
	id := v.UninstallItems[index].ID
	v.UninstallItems[index].Selected = selected
	for i := range v.UninstallAll {
		if v.UninstallAll[i].ID == id {
			v.UninstallAll[i].Selected = selected
		}
	}
}

// UninstallSelection returns the checked rows from the unfiltered snapshot.
func (v *View) UninstallSelection() []domainuninstall.Candidate {
	return domainuninstall.Selected(v.UninstallAll)
}

func (v *View) filterUninstall(reset bool) {
	selected := ""
	if !reset && v.Chosen >= 0 && v.Chosen < len(v.UninstallItems) {
		selected = v.UninstallItems[v.Chosen].ID
	}
	items := domainuninstall.Filter(v.UninstallAll, v.Query)
	v.UninstallItems = items
	v.Chosen = -1
	if selected != "" {
		for index, item := range items {
			if item.ID == selected {
				v.Chosen = index
				break
			}
		}
	}
	if v.Chosen < 0 && len(items) > 0 {
		v.Chosen = 0
	}
	if v.Chosen >= 0 {
		v.uninstallList.ScrollIntoView(v.Chosen)
	}
}

func (v *View) SetModeUninstall(target string) {
	v.SetExpanded(true)
	if v.Mode == ModeUninstall {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeUninstall
	v.Query = ""
	v.inputText.Set("")
	v.Chosen = -1
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.FileSearchItems = nil
	v.ScheduleItems = nil
	v.UninstallTarget = target
	v.UninstallAll = nil
	v.UninstallItems = nil
	v.UninstallError = ""
	v.UninstallLoading = true
	v.Status = ""
	v.Filter()
	v.Input.Focus()
}

func (v *View) SetUninstallResults(items []domainuninstall.Candidate, errText string) {
	v.UninstallAll = items
	v.UninstallError = errText
	v.UninstallLoading = false
	v.filterUninstall(true)
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
		Icon: v.callbacks.Icon(application), Fallback: v.entryFallback(application), Title: application.Name,
		Trailing: trailing, Selected: i == v.Chosen,
	})
}

// entryFallback is the avatar text for a row without an icon. Feature rows
// name their feature instead of repeating the entry name.
func (v *View) entryFallback(application domain.Application) string {
	switch application.Kind {
	case domain.KindCommand:
		return v.Text("命令", "CMD")
	case domain.KindQuicklink:
		return v.Text("链接", "LINK")
	case domain.KindSnippet:
		return v.Text("片段", "SNIP")
	case domain.KindFileSearch:
		return v.Text("文件", "FILE")
	case domain.KindSystemAction:
		return v.Text("系统", "SYS")
	case domain.KindWindowCommand:
		return v.Text("窗口", "WIN")
	case domain.KindCalendar:
		return v.Text("日程", "CAL")
	}
	return application.Name
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

func (v *View) textKey(key ui.TextKey) bool {
	// MyGo gives composition keys to the IME before OnTextKey. Read Text here
	// because a rapid Enter may arrive before the next view has bound Query.
	if v.Mode == ModeArguments {
		if key.Mods == 0 {
			switch key.Key {
			case ui.KeyEnter, ui.KeyTab:
				v.argumentCommit()
				return true
			case ui.KeyEscape:
				v.cancelArguments()
				return true
			case ui.KeyBackspace:
				if v.Query == "" {
					v.argumentStepBack()
					return true
				}
			case ui.KeyDown:
				v.argumentMoveChoice(1)
				return true
			case ui.KeyUp:
				v.argumentMoveChoice(-1)
				return true
			}
		}
		return false
	}
	if key.Key == ui.KeyTab && (key.Mods == 0 || key.Mods == ui.Shift) {
		v.syncQuery(key.Text())
		v.tab(key.Mods == ui.Shift)
		return true
	}
	if v.Mode == ModeEmoji && key.Mods == 0 {
		switch key.Key {
		case ui.KeyLeft:
			v.moveHorizontal(-1)
			return true
		case ui.KeyRight:
			v.moveHorizontal(1)
			return true
		}
	}
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
	if key.Mods == ui.Ctrl|ui.Shift && key.Key == ui.KeyC && v.Mode == ModeFileSearch {
		if v.Chosen >= 0 && v.Chosen < len(v.FileSearchItems) && v.callbacks.CopyFilePath != nil {
			v.callbacks.CopyFilePath(v.FileSearchItems[v.Chosen])
		}
		return true
	}
	if key.Mods == ui.Ctrl|ui.Shift && key.Key == ui.KeyC && v.Mode == ModeSchedule {
		if v.Chosen >= 0 && v.Chosen < len(v.ScheduleItems) && v.callbacks.CopyMeetingLink != nil {
			v.callbacks.CopyMeetingLink(v.ScheduleItems[v.Chosen])
		}
		return true
	}
	if key.Mods == ui.Ctrl|ui.Shift && key.Key == ui.KeyC && v.Mode == ModeUninstall {
		if v.Chosen >= 0 && v.Chosen < len(v.UninstallItems) && v.callbacks.CopyUninstallPath != nil {
			v.callbacks.CopyUninstallPath(v.UninstallItems[v.Chosen].Path)
		}
		return true
	}
	if key.Mods == ui.Ctrl|ui.Shift && (key.Key == ui.KeyEnter || key.Key == ui.KeyC) {
		v.syncQuery(key.Text())
		v.callbacks.CopyText(v.CopyText(), key.Key == ui.KeyEnter)
		return true
	}
	if key.Mods == ui.Ctrl {
		if v.Mode == ModeUninstall && key.Key == ui.KeyEnter {
			v.ToggleUninstall(v.Chosen)
			return true
		}
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
	default:
		return false
	}
	return true
}

func (v *View) tab(backward bool) {
	query := v.Query
	if v.Mode == ModeLauncher {
		if !backward && strings.TrimSpace(query) != "" && v.DictionaryAvailable {
			v.ToggleDictionary()
		} else if !backward && strings.TrimSpace(query) == "" && v.AIAvailable {
			if v.callbacks.AILeave != nil {
				v.callbacks.AILeave(query)
			}
			v.SetModeAI("")
		} else {
			v.SetModeClipboard()
			v.SetQuery(query)
		}
		return
	}
	if v.Mode == ModeAI || v.Mode == ModeAIHistory {
		v.SetModeClipboard()
		return
	}
	v.SetModeLauncher()
	v.SetQuery(query)
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
	if v.Mode == ModeArguments {
		v.cancelArguments()
		return
	}
	if v.Query == "" {
		switch v.Mode {
		case ModeAIHistory:
			v.back()
			return
		case ModeAI:
			v.back()
			return
		case ModeClipboard, ModeDictionary, ModeCalculatorHistory, ModeEmoji, ModeFileSearch, ModeSchedule, ModeUninstall:
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
	if strings.TrimSpace(text) != "" {
		v.SetExpanded(true)
	}
	v.Filter()
}

func (v *View) syncQuery(text string) {
	if v.Query != text {
		v.Query = text
		if strings.TrimSpace(text) != "" {
			v.SetExpanded(true)
		}
		v.Filter()
	}
}

func (v *View) open() {
	if v.Mode == ModeArguments {
		v.argumentCommit()
		return
	}
	if !v.Expanded {
		v.SetExpanded(true)
		return
	}
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
	case ModeFileSearch:
		if v.Chosen >= 0 && v.Chosen < len(v.FileSearchItems) && v.callbacks.OpenFile != nil {
			v.callbacks.OpenFile(v.FileSearchItems[v.Chosen])
		}
		return
	case ModeSchedule:
		if v.Chosen >= 0 && v.Chosen < len(v.ScheduleItems) && v.callbacks.JoinMeeting != nil {
			v.callbacks.JoinMeeting(v.ScheduleItems[v.Chosen])
		}
		return
	case ModeUninstall:
		if v.callbacks.ConfirmUninstall != nil {
			v.callbacks.ConfirmUninstall()
		}
		return
	case ModeAI:
		if v.AIStreaming {
			if v.callbacks.AICancel != nil {
				v.callbacks.AICancel()
			}
			return
		}
		if strings.TrimSpace(v.Query) != "" && v.callbacks.AISend != nil {
			v.callbacks.AISend(v.Query)
		}
		return
	case ModeAIHistory:
		if v.AIHistoryChosen >= 0 && v.AIHistoryChosen < len(v.AIHistoryItems) &&
			v.callbacks.AIOpen != nil {
			v.callbacks.AIOpen(v.AIHistoryItems[v.AIHistoryChosen].ID)
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
	case ModeAIHistory:
		return v.AIHistoryChosen
	}
	return v.Chosen
}

func (v *View) moveVertical(step int) {
	if v.Mode == ModeArguments {
		v.argumentMoveChoice(step)
		return
	}
	if !v.Expanded {
		v.SetExpanded(true)
		return
	}
	if v.Mode == ModeEmoji {
		v.choose(emoji.VerticalSelection(v.EmojiItems, v.EmojiChosen, step))
		return
	}
	if v.Mode == ModeAI {
		v.aiScroll.Y = max(0, v.aiScroll.Y+float32(step)*theme.RowHeight*2)
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
	case ModeAI:
		return
	case ModeClipboard:
		v.chooseClipboard(i, false)
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
		chosen := max(0, min(i, len(v.EmojiItems)-1))
		if chosen != v.EmojiChosen {
			v.emojiReveal = true
		}
		sections := emoji.Sections(v.EmojiItems)
		if len(sections) > 0 && chosen < min(emoji.GridColumns, sections[0].Count) {
			v.emojiScroll.X, v.emojiScroll.Y = 0, 0
		}
		v.EmojiChosen = chosen
		return
	case ModeFileSearch:
		if len(v.FileSearchItems) == 0 {
			v.Chosen = -1
			return
		}
		v.Chosen = max(0, min(i, len(v.FileSearchItems)-1))
		v.fileSearchList.ScrollIntoView(v.Chosen)
		return
	case ModeSchedule:
		if len(v.ScheduleItems) == 0 {
			v.Chosen = -1
			return
		}
		v.Chosen = max(0, min(i, len(v.ScheduleItems)-1))
		v.scheduleList.ScrollIntoView(v.Chosen)
		return
	case ModeUninstall:
		if len(v.UninstallItems) == 0 {
			v.Chosen = -1
			return
		}
		v.Chosen = max(0, min(i, len(v.UninstallItems)-1))
		v.uninstallList.ScrollIntoView(v.Chosen)
		return
	case ModeAIHistory:
		if len(v.AIHistoryItems) == 0 {
			v.AIHistoryChosen = -1
			return
		}
		v.AIHistoryChosen = max(0, min(i, len(v.AIHistoryItems)-1))
		v.aiHistoryList.ScrollIntoView(v.AIHistoryChosen)
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
	if v.Mode == ModeArguments {
		return
	}
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
		v.filterClipboard(true)
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
		v.emojiScroll.X, v.emojiScroll.Y = 0, 0
		v.emojiReveal = true
		v.choose(0)
		return
	case ModeFileSearch:
		v.CalcResult = nil
		v.CalcHistory = nil
		v.EmojiItems = nil
		v.FileSearchError = ""
		if strings.TrimSpace(v.Query) == "" {
			v.FileSearchState = files.Idle
			v.FileSearchItems = nil
			v.Chosen = -1
		}
		if v.callbacks.FileSearch != nil {
			v.callbacks.FileSearch(v.Query)
		}
		return
	case ModeSchedule:
		v.CalcResult = nil
		v.CalcHistory = nil
		v.EmojiItems = nil
		v.FileSearchItems = nil
		v.filterSchedule(true)
		return
	case ModeUninstall:
		v.CalcResult = nil
		v.CalcHistory = nil
		v.EmojiItems = nil
		v.FileSearchItems = nil
		v.ScheduleItems = nil
		v.filterUninstall(true)
		return
	case ModeAI:
		return
	case ModeAIHistory:
		v.filterAIHistory()
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
	if v.Mode == ModeAI && v.callbacks.AILeave != nil {
		v.callbacks.AILeave(v.Query)
	}
	v.Mode = ModeLauncher
	v.Argument = nil
	v.Query = v.launcherQuery
	v.inputText.Set(v.launcherQuery)
	v.launcherQuery = ""
	v.Chosen, v.ClipboardChosen, v.EmojiChosen = -1, -1, -1
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.Filter()
	v.Input.Focus()
}

// ResetLauncher applies the summon preference without persisting temporary
// expansion caused by a query or by entering a feature.
func (v *View) ResetLauncher(expanded bool) {
	v.launcherQuery = ""
	v.Argument = nil
	v.SetModeLauncher()
	v.Status = ""
	v.AliasOpen = false
	v.SetExpanded(expanded)
}

func (v *View) SetExpanded(expanded bool) {
	if v.Expanded == expanded {
		return
	}
	v.Expanded = expanded
	if v.callbacks.ExpandedChanged != nil {
		v.callbacks.ExpandedChanged()
	}
}

func (v *View) SetModeEmoji() {
	v.SetExpanded(true)
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
	v.SetExpanded(true)
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

func (v *View) SetModeFileSearch() {
	v.SetExpanded(true)
	if v.Mode == ModeFileSearch {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeFileSearch
	v.Query = ""
	v.inputText.Set("")
	v.Chosen = -1
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.Filter()
	v.Input.Focus()
}

func (v *View) SetFileSearchResults(state files.State, hits []filesearch.Hit, errText string) {
	if state == files.Searching {
		v.FileSearchState = state
		return
	}
	v.FileSearchState = state
	v.FileSearchItems = hits
	v.FileSearchError = errText
	v.fileSearchList.ScrollIntoView(0)
	v.choose(0)
}

func (v *View) SetModeSchedule() {
	v.SetExpanded(true)
	if v.Mode == ModeSchedule {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeSchedule
	v.Query = ""
	v.inputText.Set("")
	v.Chosen = -1
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.FileSearchItems = nil
	v.Status = ""
	v.Filter()
	v.Input.Focus()
}

func (v *View) SetModeAI(draft string) {
	v.SetExpanded(true)
	if v.Mode == ModeAI {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	v.Mode = ModeAI
	v.Query = draft
	v.inputText.Set(draft)
	v.Chosen, v.ClipboardChosen, v.EmojiChosen, v.AIHistoryChosen = -1, -1, -1, -1
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.FileSearchItems = nil
	v.ScheduleItems = nil
	v.UninstallItems = nil
	v.Input.Focus()
}

func (v *View) SetModeAIHistory() {
	v.SetExpanded(true)
	if v.Mode == ModeAIHistory {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeLauncher {
		v.launcherQuery = v.Query
	}
	if v.Mode == ModeAI {
		v.aiDraft = v.Query
	}
	v.Mode = ModeAIHistory
	v.Query = ""
	v.inputText.Set("")
	v.AIHistoryChosen = -1
	v.CalcResult = nil
	v.CalcHistory = nil
	v.EmojiItems = nil
	v.FileSearchItems = nil
	v.ScheduleItems = nil
	v.UninstallItems = nil
	v.Filter()
	v.Input.Focus()
}

// SetAIChat replaces the transcript snapshot. Messages are shared with the
// controller; the last assistant text grows in place while streaming.
func (v *View) SetAIChat(messages []domainai.ChatMessage, notice string, thinking, streaming bool) {
	v.AIMessages = messages
	v.AINotice = notice
	v.AIThinking = thinking
	v.AIStreaming = streaming
}

func (v *View) SetAIStreaming(streaming bool) {
	v.AIStreaming = streaming
}

// ClearAIComposer empties the input after a message was sent. The transcript
// keeps the turn; only the composer is cleared.
func (v *View) ClearAIComposer() {
	v.Query = ""
	v.inputText.Set("")
	v.Input.Focus()
}

func (v *View) SetAIHistory(items []domainai.Conversation) {
	v.AIHistoryItems = items
	selected := ""
	if v.AIHistoryChosen >= 0 && v.AIHistoryChosen < len(items) {
		selected = items[v.AIHistoryChosen].ID
	}
	v.AIHistoryChosen = -1
	if selected != "" {
		for index, item := range items {
			if item.ID == selected {
				v.AIHistoryChosen = index
				break
			}
		}
	}
	if v.AIHistoryChosen < 0 && len(items) > 0 {
		v.AIHistoryChosen = 0
	}
	if v.AIHistoryChosen >= 0 {
		v.aiHistoryList.ScrollIntoView(v.AIHistoryChosen)
	}
}

func (v *View) filterAIHistory() {
	selected := ""
	if v.AIHistoryChosen >= 0 && v.AIHistoryChosen < len(v.AIHistoryItems) {
		selected = v.AIHistoryItems[v.AIHistoryChosen].ID
	}
	if v.callbacks.AIHistory != nil {
		v.AIHistoryItems = v.callbacks.AIHistory(v.Query)
	} else {
		query := strings.ToLower(strings.TrimSpace(v.Query))
		items := make([]domainai.Conversation, 0, len(v.AIHistoryItems))
		for _, item := range v.AIHistoryItems {
			if query != "" && !strings.Contains(strings.ToLower(item.Title), query) &&
				!strings.Contains(strings.ToLower(item.Preview), query) {
				continue
			}
			items = append(items, item)
		}
		v.AIHistoryItems = items
	}
	v.AIHistoryChosen = -1
	if selected != "" {
		for index, item := range v.AIHistoryItems {
			if item.ID == selected {
				v.AIHistoryChosen = index
				break
			}
		}
	}
	if v.AIHistoryChosen < 0 && len(v.AIHistoryItems) > 0 {
		v.AIHistoryChosen = 0
	}
	if v.AIHistoryChosen >= 0 {
		v.aiHistoryList.ScrollIntoView(v.AIHistoryChosen)
	}
}

func (v *View) aiHistoryBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(6, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, v.Text("对话历史", "Chat History")).FontSize(theme.SectionFont).
			TextColor(tokens.TextTertiary).Grow(1).Role(ui.RoleHeading)
	})
	v.aiHistoryList.Selected = &v.AIHistoryChosen
	v.aiHistoryList.Label = func(i int) string { return v.AIHistoryItems[i].Title }
	list := components.ResultList(c, tokens, &v.aiHistoryList, len(v.AIHistoryItems), func(i int) {
		item := v.AIHistoryItems[i]
		components.ResultRow(c, tokens, components.Row{
			Fallback: "AI", Title: item.Title, Subtitle: item.Preview,
			Trailing: fmt.Sprintf("%d", item.MessageCount), Selected: i == v.AIHistoryChosen,
		})
	}).Grow(1).Padding(0, 8).Label(v.Text("对话历史", "Chat history"))
	if list.Submitted() {
		v.open()
	}
	if len(v.AIHistoryItems) == 0 {
		message := v.Text("还没有对话", "No chats yet")
		if strings.TrimSpace(v.Query) != "" {
			message = v.Text("没有匹配的对话", "No matching chats")
		}
		ui.Text(c, message).TextColor(tokens.TextTertiary).Padding(20)
	}
}

func (v *View) SetModeClipboard() {
	v.SetExpanded(true)
	if v.Mode == ModeClipboard {
		v.Input.Focus()
		return
	}
	if v.Mode == ModeAI && v.callbacks.AILeave != nil {
		v.callbacks.AILeave(v.Query)
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
	v.chooseClipboard(index, false)
}

// RefreshClipboard retains the selected business ID when capture or a store
// mutation changes row order. Only a new query/filter intentionally starts at
// the first row; deletion chooses the nearest surviving row.
func (v *View) RefreshClipboard() {
	if v.Mode == ModeClipboard {
		v.filterClipboard(false)
	}
}

func (v *View) selectedClipboardID() string {
	if v.ClipboardChosen >= 0 && v.ClipboardChosen < len(v.ClipboardItems) {
		return v.ClipboardItems[v.ClipboardChosen].ID
	}
	return ""
}

func (v *View) chooseClipboard(index int, reveal bool) {
	previous := v.selectedClipboardID()
	if len(v.ClipboardItems) == 0 {
		v.ClipboardChosen = -1
		return
	}
	v.ClipboardChosen = max(0, min(index, len(v.ClipboardItems)-1))
	if reveal || v.selectedClipboardID() != previous {
		v.clipboardList.ScrollIntoView(v.ClipboardChosen)
	}
}

func (v *View) filterClipboard(reset bool) {
	previous, index := v.selectedClipboardID(), v.ClipboardChosen
	if v.callbacks.ClipboardSearch != nil {
		v.ClipboardItems = v.callbacks.ClipboardSearch(v.Query, v.ClipboardFilter)
	} else {
		v.ClipboardItems = nil
	}
	if reset {
		index = 0
	} else {
		for candidate, item := range v.ClipboardItems {
			if item.ID == previous {
				index = candidate
				break
			}
		}
	}
	if len(v.ClipboardItems) == 0 {
		v.ClipboardChosen = -1
		return
	}
	v.ClipboardChosen = max(0, min(index, len(v.ClipboardItems)-1))
	if reset || v.selectedClipboardID() != previous {
		v.clipboardList.ScrollIntoView(v.ClipboardChosen)
	}
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
	v.SetExpanded(true)
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

// backButton is the header's return control on every non-launcher screen: a
// quiet hover surface with a stroked arrow instead of a boxed text button.
func (v *View) backButton(c *ui.Context, tokens theme.Tokens) {
	button := ui.ButtonBase(c).Size(30, 30).Radius(theme.ControlRadius).
		Tooltip(v.Text("返回启动器 (Esc)", "Back to launcher (Esc)")).
		Label(v.Text("返回启动器", "Back to launcher")).
		Cursor(ui.CursorPointer)
	fill := ui.Transparent
	switch {
	case button.Pressed():
		fill = tokens.SurfacePress
	case button.Hovered():
		fill = tokens.SurfaceHover
	}
	color := tokens.TextSecondary
	if button.Hovered() {
		color = tokens.Text
	}
	button.Background(fill).Draw(func(p *ui.Painter, r ui.Rect) {
		var head ui.Path
		head.MoveTo(r.X+r.W*0.40, r.Y+r.H*0.30).
			LineTo(r.X+r.W*0.24, r.Y+r.H*0.50).
			LineTo(r.X+r.W*0.40, r.Y+r.H*0.70)
		p.StrokePath(&head, 1.7, color)
		p.Line(r.X+r.W*0.26, r.Y+r.H*0.50, r.X+r.W*0.76, r.Y+r.H*0.50, 1.7, color)
	})
	if button.Clicked() {
		v.back()
	}
}

// back leaves history to the chat and any other mode to the launcher. Leaving
// chat remembers its draft before the mode changes.
func (v *View) back() {
	switch v.Mode {
	case ModeAIHistory:
		v.SetModeAI(v.aiDraft)
	case ModeAI:
		v.SetModeLauncher()
	default:
		v.SetModeLauncher()
	}
}

// clipboardFilterButton states the active type filter in the header and opens
// the type menu. The filter is global to the history, so it stays out of the
// per-entry action row.
func (v *View) clipboardFilterButton(c *ui.Context, tokens theme.Tokens) {
	button := ui.ButtonBase(c).Height(28).Padding(3, 10).Gap(6).Radius(theme.ControlRadius).
		Tooltip(v.Text("按类型筛选 (Ctrl+P)", "Filter by type (Ctrl+P)")).
		Label(v.Text("筛选类型", "Filter type")).
		Cursor(ui.CursorPointer)
	fill := tokens.Muted
	switch {
	case button.Pressed():
		fill = tokens.Border
	case button.Hovered():
		fill = tokens.SurfacePress
	}
	color := tokens.Fg2
	if button.Hovered() {
		color = tokens.Text
	}
	button.Background(fill).TextColor(color).Children(func() {
		ui.Text(c, clipboardFilterLabel(v.Chinese, v.ClipboardFilter)).FontSize(theme.TrailingFont).SingleLine()
		ui.Box(c).Size(10, 10).Shrink(0).Draw(func(p *ui.Painter, r ui.Rect) {
			var chevron ui.Path
			chevron.MoveTo(r.X+r.W*0.20, r.Y+r.H*0.38).
				LineTo(r.X+r.W*0.50, r.Y+r.H*0.66).
				LineTo(r.X+r.W*0.80, r.Y+r.H*0.38)
			p.StrokePath(&chevron, 1.5, tokens.TextTertiary)
		})
	})
	button.Menu(func(m *ui.Menu) {
		for _, option := range clipboardFilterOptions {
			if m.Item(v.Text(option.zh, option.en)).Checked(v.ClipboardFilter == option.filter).Chosen() {
				v.SetClipboardFilter(option.filter)
			}
		}
	})
}

// aiModelButton shows the current default model and opens the controller's
// model menu. The menu is native so it can carry every saved connection.
func (v *View) aiModelButton(c *ui.Context, tokens theme.Tokens) {
	label := v.AIModelLabel
	if label == "" {
		label = v.Text("选择模型", "Choose model")
	}
	button := ui.ButtonBase(c).Height(28).Padding(3, 10).Gap(6).Radius(theme.ControlRadius).
		MaxWidth(280).Tooltip(v.Text("选择 AI 模型", "Choose AI model")).
		Label(v.Text("选择 AI 模型", "Choose AI model")).Cursor(ui.CursorPointer)
	fill := tokens.Muted
	switch {
	case button.Pressed():
		fill = tokens.Border
	case button.Hovered():
		fill = tokens.SurfacePress
	}
	color := tokens.Fg2
	if button.Hovered() {
		color = tokens.Text
	}
	button.Background(fill).TextColor(color).Children(func() {
		ui.Text(c, label).FontSize(theme.TrailingFont).SingleLine()
		ui.Box(c).Size(10, 10).Shrink(0).Draw(func(p *ui.Painter, r ui.Rect) {
			var chevron ui.Path
			chevron.MoveTo(r.X+r.W*0.20, r.Y+r.H*0.38).
				LineTo(r.X+r.W*0.50, r.Y+r.H*0.66).
				LineTo(r.X+r.W*0.80, r.Y+r.H*0.38)
			p.StrokePath(&chevron, 1.5, tokens.TextTertiary)
		})
	})
	if button.Clicked() && v.callbacks.AIModelMenu != nil {
		v.callbacks.AIModelMenu()
	}
}

func (v *View) clipboardBody(c *ui.Context, tokens theme.Tokens) {
	ui.Row(c).Padding(6, 18).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, v.Text("剪贴板历史", "Clipboard history")).FontSize(theme.SectionFont).
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
	})
	ui.Row(c).FillWidth().Grow(1).Children(func() {
		ui.Column(c).Width(290).FillHeight().Children(func() {
			v.clipboardList.Selected = &v.ClipboardChosen
			v.clipboardList.Label = func(i int) string { return clipboardRowTitle(v.ClipboardItems[i], v.Chinese) }
			list := components.ResultList(c, tokens, &v.clipboardList, len(v.ClipboardItems), func(i int) {
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
		})
		ui.Box(c).Width(1).FillHeight().Background(tokens.Line)
		v.clipboardPreviewBody(c, tokens)
	})
}

func (v *View) clipboardPreviewBody(c *ui.Context, tokens theme.Tokens) {
	id := v.selectedClipboardID()
	if id != v.clipboardPreviewID {
		v.clipboardPreviewID = id
		v.clipboardPreview.X, v.clipboardPreview.Y = 0, 0
	}
	ui.Scroll(c).Grow(1).FillHeight().TrackScroll(&v.clipboardPreview).Children(func() {
		ui.Column(c).FillWidth().Padding(16, 18).Gap(12).Children(func() {
			if id == "" {
				ui.Text(c, v.Text("选择一条记录查看完整内容", "Select an entry to preview its full contents")).
					TextColor(tokens.TextTertiary).Role(ui.RoleStatus)
				return
			}
			item := v.ClipboardItems[v.ClipboardChosen]
			if item.Kind == clipboard.KindImage {
				var bitmap *ui.Bitmap
				loading := false
				if v.callbacks.ClipboardImage != nil {
					bitmap, loading = v.callbacks.ClipboardImage(item)
				}
				if bitmap != nil {
					ui.Image(c, bitmap).Size(360, 240).Fit(ui.Contain).Label(v.Text("剪贴板图片预览", "Clipboard image preview"))
				} else {
					message := v.Text("无法加载图片预览", "Image preview unavailable")
					if loading {
						message = v.Text("正在加载图片预览…", "Loading image preview…")
					}
					ui.Text(c, message).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Role(ui.RoleStatus)
				}
				return
			}
			ui.Text(c, item.Text).FillWidth().FontSize(theme.RowFont).TextColor(tokens.TextSecondary).Selectable()
		})
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
	lastSpace := false
	for _, r := range text {
		if chars == maxChars {
			break
		}
		if r == '\n' || r == '\r' || r == '\t' || r == ' ' {
			if builder.Len() > 0 && !lastSpace {
				builder.WriteByte(' ')
				chars++
				lastSpace = true
			}
			continue
		}
		builder.WriteRune(r)
		chars++
		lastSpace = false
	}
	return strings.TrimRight(builder.String(), " ")
}

func (v *View) CycleClipboardFilter() {
	next := (int(v.ClipboardFilter) + 1) % len(clipboardFilterOptions)
	v.SetClipboardFilter(clipboard.Filter(next))
}

// SetClipboardFilter applies one type filter and starts the list over, so a
// menu choice and the Ctrl+P cycle share the same reset and reload.
func (v *View) SetClipboardFilter(filter clipboard.Filter) {
	if v.ClipboardFilter == filter {
		return
	}
	v.ClipboardFilter = filter
	if v.callbacks.SetClipboardFilter != nil {
		v.callbacks.SetClipboardFilter(filter)
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
	if !v.Expanded {
		v.SetExpanded(true)
		return
	}
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

// clipboardFilterOptions is the menu order, and the order Ctrl+P cycles.
var clipboardFilterOptions = [...]struct {
	filter clipboard.Filter
	zh     string
	en     string
}{
	{clipboard.FilterAll, "全部类型", "All Types"},
	{clipboard.FilterText, "仅文本", "Text Only"},
	{clipboard.FilterImages, "仅图片", "Images Only"},
	{clipboard.FilterLinks, "仅链接", "Links Only"},
	{clipboard.FilterEmails, "仅邮箱", "Emails Only"},
}

func clipboardFilterLabel(chinese bool, filter clipboard.Filter) string {
	for _, option := range clipboardFilterOptions {
		if option.filter == filter {
			if chinese {
				return option.zh
			}
			return option.en
		}
	}
	if chinese {
		return "全部类型"
	}
	return "All Types"
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
