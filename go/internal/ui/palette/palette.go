package palette

import (
	"fmt"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
)

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
	Chosen              int
	Input               ui.Handle
	inputText           *ui.TextBuffer
	list                ui.ListState
	onOpen              func(domain.Application)
	onHide              func()
	onMenu              func()
	onCopy              func(string, bool)
	icon                func(domain.Application) *ui.Bitmap
	onFilter            func(time.Duration, int)
	Dictionary          bool
	DictionaryAvailable bool
	DictionaryLoading   bool
	DictionaryError     string
	Definition          *Definition
	dictionaryScroll    ui.ScrollState
	OnLookup            func(string)
}

func New(chinese bool, open func(domain.Application), hide, menu func(), copyText func(string, bool), icon func(domain.Application) *ui.Bitmap, filtered func(time.Duration, int)) *View {
	return &View{Chinese: chinese, Chosen: -1, inputText: ui.NewTextBuffer(""), onOpen: open, onHide: hide, onMenu: menu, onCopy: copyText, icon: icon, onFilter: filtered}
}

func (v *View) Text(zh, en string) string {
	if v.Chinese {
		return zh
	}
	return en
}

func (v *View) Render(c *ui.Context) {
	dark := c.Theme().Dark
	if v.ForcedDark != nil {
		dark = *v.ForcedDark
	}
	tokens := theme.For(dark)
	c.SetTheme(tokens.Theme())
	c.Root().Background(tokens.Surface)
	c.OnShortcut(ui.Ctrl, ui.KeyK, v.onMenu)
	c.OnShortcut(0, ui.KeyEscape, v.escape)
	c.OnShortcut(0, ui.KeyDown, func() { v.choose(v.Chosen + 1) })
	c.OnShortcut(0, ui.KeyUp, func() { v.choose(v.Chosen - 1) })
	c.OnShortcut(0, ui.KeyEnter, v.open)
	ui.Column(c).Fill().Children(func() {
		ui.Row(c).Padding(18, 20).Gap(14).AlignItems(ui.Center).Children(func() {
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
		components.Footer(c, tokens, status, v.Text("Tab 词典   ↑↓ 选择", "Tab Dictionary   ↑↓ Select"), v.onMenu)
	})
}

func (v *View) applications(c *ui.Context, tokens theme.Tokens) {
	components.Section(c, tokens, v.Text("应用程序", "Applications"))
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
	components.ResultRow(c, tokens, components.Row{
		Icon: v.icon(application), Fallback: application.Name, Title: application.Name,
		Selected: i == v.Chosen,
	})
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
		v.onCopy(v.CopyText(), key.Key == ui.KeyEnter)
		return true
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
			v.onHide()
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

func (v *View) escape() {
	if v.Query == "" {
		v.onHide()
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
		v.onOpen(v.Results[v.Chosen])
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
	v.Results = domain.Filter(v.Apps, v.Query)
	v.choose(0)
	if v.onFilter != nil {
		v.onFilter(time.Since(started), len(v.Results))
	}
}

func (v *View) CountStatus() string {
	return fmt.Sprintf(v.Text("%d 个应用程序", "%d applications"), len(v.Apps))
}
