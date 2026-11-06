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
	Query    string
	Apps     []mygo.InstalledApp
	Results  []mygo.InstalledApp
	Loading  bool
	Status   string
	Chinese  bool
	Chosen   int
	Input    ui.Handle
	list     ui.ListState
	onOpen   func(mygo.InstalledApp)
	onHide   func()
	onMenu   func()
	onCopy   func(string, bool)
	icon     func(mygo.InstalledApp) *ui.Bitmap
	onFilter func(time.Duration, int)
}

func New(chinese bool, open func(mygo.InstalledApp), hide, menu func(), copyText func(string, bool), icon func(mygo.InstalledApp) *ui.Bitmap, filtered func(time.Duration, int)) *View {
	return &View{Chinese: chinese, Chosen: -1, onOpen: open, onHide: hide, onMenu: menu, onCopy: copyText, icon: icon, onFilter: filtered}
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
			if ui.TextInputBase(c, &v.Query).Bind(&v.Input).Grow(1).FontSize(20).
				Placeholder(v.Text("搜索应用程序…", "Search applications…")).
				Label(v.Text("搜索应用程序", "Search applications")).OnTextKey(v.textKey).Changed() {
				v.Filter()
			}
		})
		ui.Divider(c)
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
		ui.Divider(c)
		ui.Row(c).Padding(10, 16).Gap(10).AlignItems(ui.Center).Children(func() {
			status := v.Status
			if status == "" {
				status = v.Text("Go 预览 · Alt+Shift+Space", "Go Preview · Alt+Shift+Space")
			}
			ui.Text(c, status).FontSize(11).TextColor(theme.TextMuted).Grow(1).Role(ui.RoleStatus)
			ui.Text(c, v.Text("↑↓ 选择   ↵ 打开", "↑↓ Select   ↵ Open")).FontSize(11).TextColor(theme.TextMuted)
			if ui.Button(c, "···").Label(v.Text("操作菜单 Ctrl+K", "Actions Ctrl+K")).Clicked() {
				v.onMenu()
			}
		})
	})
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
		v.onCopy(key.Text(), key.Key == ui.KeyEnter)
		return true
	}
	if key.Mods != 0 {
		return false
	}
	switch key.Key {
	case ui.KeyEnter:
		if v.Query != key.Text() {
			v.Query = key.Text()
			v.Filter()
		}
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
		// Only Applications exists in P0. Keep the native input focused until
		// the complete mode contract is migrated by the launcher plan.
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
	v.Query = ""
	v.Filter()
	v.Input.Focus()
}

func (v *View) open() {
	if v.Chosen >= 0 && v.Chosen < len(v.Results) {
		v.onOpen(v.Results[v.Chosen])
	}
}

func (v *View) choose(i int) {
	if len(v.Results) == 0 {
		v.Chosen = -1
		return
	}
	v.Chosen = max(0, min(i, len(v.Results)-1))
	v.list.ScrollIntoView(v.Chosen)
}

func (v *View) Filter() {
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
