// Package components builds the shared native pieces: rows, section headers,
// key caps, cards, forms and menus. Views compose these instead of repeating
// spacing and color decisions.
package components

import (
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

func Section(c *ui.Context, tokens theme.Tokens, label string) {
	ui.Text(c, label).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).
		Height(theme.SectionHeaderHeight).Padding(0, 10).Role(ui.RoleHeading)
}

// Row is one application result. Icon is drawn at the row's pixel size; when
// it is not ready yet the avatar keeps the row stable.
type Row struct {
	Icon     *ui.Bitmap
	Fallback string
	Title    string
	Subtitle string
	Trailing string
	Selected bool
}

// ResultList leaves selection painting to ResultRow. MyGo's default list
// paints its own accent-colored row behind the content; a larger content
// radius otherwise exposes that darker fill at all four corners.
func ResultList(c *ui.Context, tokens theme.Tokens, state *ui.ListState, count int, row func(int)) ui.Element {
	previous := c.Theme()
	listTheme := *previous
	listTheme.Accent = ui.Transparent
	listTheme.AccentText = tokens.Text
	listTheme.Radius = theme.RowRadius
	c.SetTheme(&listTheme)
	defer c.SetTheme(previous)
	return ui.List(c, state, count, row)
}

func ResultRow(c *ui.Context, tokens theme.Tokens, row Row) {
	fill := ui.Transparent
	if row.Selected {
		fill = tokens.Selection
	}
	ui.Row(c).Height(theme.RowHeight).Padding(6, 10).Gap(12).AlignItems(ui.Center).
		Radius(theme.RowRadius).Background(fill).Children(func() {
		if row.Icon != nil {
			ui.Image(c, row.Icon).Size(theme.ResultRowIcon, theme.ResultRowIcon).Role(ui.RoleNone)
		} else {
			ui.Avatar(c, row.Fallback, nil).Size(theme.ResultRowIcon, theme.ResultRowIcon).Role(ui.RoleNone)
		}
		ui.Column(c).Grow(1).Gap(1).Children(func() {
			ui.Text(c, row.Title).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
			if row.Subtitle != "" {
				ui.Text(c, row.Subtitle).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine()
			}
		})
		if row.Trailing != "" {
			KeyCap(c, tokens, row.Trailing)
		}
	})
}

func KeyCap(c *ui.Context, tokens theme.Tokens, label string) {
	ui.Text(c, label).FontSize(theme.KeyCapFont).TextColor(tokens.CtlFg).TextAlign(ui.Center).
		Height(theme.KeyCapHeight).MinWidth(theme.KeyCapHeight).Padding(1, 5).
		Radius(theme.KeyCapRadius).Background(tokens.Muted)
}

// Footer is the palette's permanent action bar. onMenu is nil when the view
// has no menu in this mode.
func Footer(c *ui.Context, tokens theme.Tokens, status, hints string, onMenu func()) {
	ui.Row(c).Height(theme.BottomBarHeight).Padding(10, 16).Gap(10).AlignItems(ui.Center).Children(func() {
		ui.Text(c, status).FontSize(theme.SectionFont).TextColor(tokens.TextSecondary).
			Grow(1).SingleLine().Role(ui.RoleStatus)
		if hints != "" {
			ui.Text(c, hints).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).SingleLine()
		}
		if onMenu != nil {
			if ui.Button(c, "···").Label("Actions").Clicked() {
				onMenu()
			}
		}
	})
}

// Card is a settings or dialog surface: its own fill, hairline and inset.
func Card(c *ui.Context, tokens theme.Tokens, fn func()) {
	ui.Column(c).FillWidth().Padding(16).Gap(12).Radius(theme.CardRadius).
		Background(tokens.Surface).Border(1, tokens.Line).Children(fn)
}

// FormRow aligns a label with a control that may grow.
func FormRow(c *ui.Context, tokens theme.Tokens, label string, control func()) {
	ui.Row(c).FillWidth().Gap(12).AlignItems(ui.Center).Children(func() {
		ui.Text(c, label).FontSize(theme.FormLabelFont).TextColor(tokens.TextSecondary).
			Width(110).SingleLine()
		ui.Column(c).Grow(1).Children(control)
	})
}

func ScrollPane(c *ui.Context, state *ui.ScrollState, fn func()) {
	ui.Scroll(c).Grow(1).TrackScroll(state).Children(fn)
}

// Dialog keeps the framework's themed modal and only owns the content gap.
func Dialog(c *ui.Context, open *bool, fn func()) {
	ui.Modal(c, open, func() {
		ui.Column(c).Gap(12).Children(fn)
	})
}

// MenuAction is one native menu entry. Separator entries ignore the other
// fields; Click runs on the UI thread after the menu closes.
type MenuAction struct {
	Label       string
	Accelerator string
	Disabled    bool
	Separator   bool
	Click       func()
}

func Menu(actions []MenuAction) *mygo.Menu {
	items := make([]*mygo.MenuItem, 0, len(actions))
	for _, action := range actions {
		if action.Separator {
			items = append(items, mygo.Separator())
			continue
		}
		click := action.Click
		items = append(items, &mygo.MenuItem{
			Label: action.Label, Accelerator: action.Accelerator, Disabled: action.Disabled,
			Click: func(*mygo.MenuItem, *mygo.Window) {
				if click != nil {
					click()
				}
			},
		})
	}
	return mygo.NewMenu(items)
}
