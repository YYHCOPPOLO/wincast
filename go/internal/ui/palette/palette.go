package palette

import (
	"cmp"
	"fmt"
	"slices"
	"strings"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// View is UI-thread-owned display state. Callbacks send user intent back to
// the app; no disk, SQL or shell work runs as the view is constructed.
type View struct {
	Query               string
	Apps                []mygo.InstalledApp
	Results             []mygo.InstalledApp
	Loading             bool
	Status              string
	Chinese             bool
	Chosen              int
	Input               ui.Handle
	inputText           *ui.TextBuffer
	list                ui.ListState
	onOpen              func(mygo.InstalledApp)
	onHide              func()
	onMenu              func()
	onCopy              func(string, bool)
	icon                func(mygo.InstalledApp) *ui.Bitmap
	onFilter            func(time.Duration, int)
	Dictionary          bool
	DictionaryAvailable bool
	DictionaryLoading   bool
	DictionaryError     string
	Definition          *Definition
	dictionaryScroll    ui.ScrollState
	OnLookup            func(string)
}

func New(chinese bool, open func(mygo.InstalledApp), hide, menu func(), copyText func(string, bool), icon func(mygo.InstalledApp) *ui.Bitmap, filtered func(time.Duration, int)) *View {
	return &View{Chinese: chinese, Chosen: -1, inputText: ui.NewTextBuffer(""), onOpen: open, onHide: hide, onMenu: menu, onCopy: copyText, icon: icon, onFilter: filtered}
}

func (v *View) Text(zh, en string) string {
	if v.Chinese {
		return zh
	}
	return en
}

func (v *View) Render(c *ui.Context) {
	theme := magpie(c.Theme().Dark)
	c.SetTheme(theme)
	c.Root().Background(theme.Background)
	c.OnShortcut(ui.Ctrl, ui.KeyK, v.onMenu)
	c.OnShortcut(0, ui.KeyEscape, v.escape)
	c.OnShortcut(0, ui.KeyDown, func() { v.choose(v.Chosen + 1) })
	c.OnShortcut(0, ui.KeyUp, func() { v.choose(v.Chosen - 1) })
	c.OnShortcut(0, ui.KeyEnter, v.open)
	ui.Column(c).Fill().Children(func() {
		ui.Row(c).Padding(18, 20).Gap(14).AlignItems(ui.Center).Children(func() {
			ui.Text(c, "⌕").FontSize(24).TextColor(theme.TextMuted)
			placeholder := v.Text("搜索应用程序…", "Search applications…")
			if v.Dictionary {
				placeholder = v.Text("查询词典…", "Look up a word…")
			}
			if ui.TextInputBufferBase(c, v.inputText).Bind(&v.Input).Grow(1).FontSize(20).
				Placeholder(placeholder).Label(placeholder).OnTextKey(v.textKey).Changed() {
				v.syncQuery(v.inputText.String())
			}
		})
		ui.Divider(c)
		if v.Dictionary {
			ui.Text(c, v.Text("词典", "Dictionary")).FontSize(12).TextColor(theme.TextMuted).Padding(10, 18)
			v.dictionaryBody(c, theme)
		} else {
			v.applications(c, theme)
		}
		ui.Divider(c)
		ui.Row(c).Padding(10, 16).Gap(10).AlignItems(ui.Center).Children(func() {
			status := v.Status
			if status == "" {
				status = v.Text("Go 预览 · Alt+Shift+Space", "Go Preview · Alt+Shift+Space")
			}
			ui.Text(c, status).FontSize(11).TextColor(theme.TextMuted).Grow(1).Role(ui.RoleStatus)
			ui.Text(c, v.Text("Tab 词典   ↑↓ 选择", "Tab Dictionary   ↑↓ Select")).FontSize(11).TextColor(theme.TextMuted)
			if ui.Button(c, "···").Label(v.Text("操作菜单 Ctrl+K", "Actions Ctrl+K")).Clicked() {
				v.onMenu()
			}
		})
	})
}

func (v *View) applications(c *ui.Context, theme *ui.Theme) {
	ui.Text(c, v.Text("应用程序", "Applications")).FontSize(12).TextColor(theme.TextMuted).Padding(10, 18)
	v.list.Selected = &v.Chosen
	v.list.Key = func(i int) any { return v.Results[i].Path }
	v.list.Label = func(i int) string { return v.Results[i].Name }
	list := ui.List(c, &v.list, len(v.Results), func(i int) { v.row(c, i, theme) }).Grow(1).Padding(0, 8).
		Label(v.Text("应用程序搜索结果", "Application search results"))
	if list.Submitted() {
		v.open()
	}
	if len(v.Results) == 0 {
		message := v.Text("没有匹配的应用程序", "No matching applications")
		if v.Loading {
			message = v.Text("正在读取应用程序…", "Loading applications…")
		}
		ui.Text(c, message).TextColor(theme.TextMuted).Padding(20)
	}
}

func (v *View) row(c *ui.Context, i int, theme *ui.Theme) {
	application := v.Results[i]
	fill := ui.Transparent
	if i == v.Chosen {
		fill = theme.Selection
	}
	ui.Row(c).Height(44).Padding(6, 10).Gap(12).AlignItems(ui.Center).Radius(8).Background(fill).Children(func() {
		if image := v.icon(application); image != nil {
			ui.Image(c, image).Size(28, 28).Role(ui.RoleNone)
		} else {
			ui.Avatar(c, application.Name, nil).Size(28, 28).Role(ui.RoleNone)
		}
		ui.Text(c, application.Name).TextColor(theme.Text).Grow(1).SingleLine()
	})
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
	query := strings.ToLower(strings.TrimSpace(v.Query))
	type ranked struct {
		application mygo.InstalledApp
		rank        int
	}
	items := make([]ranked, 0, len(v.Apps))
	for _, application := range v.Apps {
		if score := rank(strings.ToLower(application.Name), query); score >= 0 {
			items = append(items, ranked{application, score})
		}
	}
	slices.SortStableFunc(items, func(a, b ranked) int { return cmp.Compare(a.rank, b.rank) })
	v.Results = v.Results[:0]
	for _, item := range items {
		v.Results = append(v.Results, item.application)
	}
	v.choose(0)
	if v.onFilter != nil {
		v.onFilter(time.Since(started), len(v.Results))
	}
}

func rank(name, query string) int {
	if query == "" || strings.HasPrefix(name, query) {
		return 0
	}
	if strings.Contains(name, " "+query) {
		return 1
	}
	if strings.Contains(name, query) {
		return 2
	}
	rest := name
	for _, r := range query {
		i := strings.IndexRune(rest, r)
		if i < 0 {
			return -1
		}
		rest = rest[i+len(string(r)):]
	}
	return 3
}

func magpie(dark bool) *ui.Theme {
	t := *ui.LightTheme()
	t.Background, t.Surface, t.Text, t.TextMuted = ui.Hex("#ffffff"), ui.Hex("#f1f1f4"), ui.Hex("#1c1c21"), ui.Hex("#63636b")
	t.Border, t.Accent, t.Selection = ui.Hex("#e3e3e8"), ui.Hex("#4f46e5"), ui.Hex("#ecebfb")
	if dark {
		t = *ui.DarkTheme()
		t.Background, t.Surface, t.Text, t.TextMuted = ui.Hex("#232327"), ui.Hex("#2d2d33"), ui.Hex("#ededf1"), ui.Hex("#a1a1aa")
		t.Border, t.Accent, t.Selection = ui.Hex("#333339"), ui.Hex("#a5b4fc"), ui.Hex("#33345a")
	}
	t.Radius, t.FontSize = 8, 14
	return &t
}

func (v *View) CountStatus() string {
	return fmt.Sprintf(v.Text("%d 个应用程序", "%d applications"), len(v.Apps))
}
