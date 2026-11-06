// Package theme carries the Magpie design tokens shared by every native view.
// Values come from crates/tinycast-pure/src/theme.rs; the Rust implementation
// remains the behavior oracle until the Go application replaces it.
package theme

import "github.com/egoist/mygo/ui"

// Metrics used across the launcher, settings and future feature windows.
const (
	PanelWidth          float32 = 750
	PanelHeight         float32 = 475
	HeaderHeight        float32 = 44
	HeaderPadding       float32 = 10
	SearchHeight        float32 = 40
	RowHeight           float32 = 38
	ResultRowIcon       float32 = 26
	SectionHeaderHeight float32 = 22
	BottomBarHeight     float32 = 52
	MenuButton          float32 = 36
	KeyCapHeight        float32 = 18
	KeyCapRadius        float32 = 6

	PanelRadius   float32 = 16
	CardRadius    float32 = 12
	ControlRadius float32 = 8
	RowRadius     float32 = 10

	SettingsWidth   float32 = 900
	SettingsHeight  float32 = 700
	SettingsSidebar float32 = 215
	SettingsRow     float32 = 48

	BodyFont      float32 = 14
	SearchFont    float32 = 20
	RowFont       float32 = 13
	TrailingFont  float32 = 12
	SectionFont   float32 = 11
	KeyCapFont    float32 = 10
	HeadingFont   float32 = 26
	FormLabelFont float32 = 12
)

// Tokens is one appearance. Build it with For, Light or Dark, never by hand.
type Tokens struct {
	Dark bool

	Page          ui.Color
	Surface       ui.Color
	SurfaceHover  ui.Color
	SurfacePress  ui.Color
	Border        ui.Color
	Line          ui.Color
	Line2         ui.Color
	Card2         ui.Color
	Muted         ui.Color
	Text          ui.Color
	TextSecondary ui.Color
	TextTertiary  ui.Color
	Fg2           ui.Color
	Faint         ui.Color
	CtlFg         ui.Color
	Accent        ui.Color
	AccentSoft    ui.Color
	AccentText    ui.Color
	Selection     ui.Color
	PillHover     ui.Color
	Destructive   ui.Color
	Success       ui.Color
	GreenSoft     ui.Color
	RedSoft       ui.Color
	Amber         ui.Color
	AmberSoft     ui.Color
	Scrim         ui.Color
}

func For(dark bool) Tokens {
	if dark {
		return Dark()
	}
	return Light()
}

func Light() Tokens {
	return Tokens{
		Page:          ui.Hex("#f4f4f6"),
		Surface:       ui.Hex("#ffffff"),
		SurfaceHover:  ui.Hex("#f1f1f4"),
		SurfacePress:  ui.Hex("#e9e9ee"),
		Border:        ui.Hex("#e3e3e8"),
		Line:          ui.Hex("#e3e3e8"),
		Line2:         ui.Hex("#ececf0"),
		Card2:         ui.Hex("#f9f9fb"),
		Muted:         ui.Hex("#f1f1f4"),
		Text:          ui.Hex("#1c1c21"),
		TextSecondary: ui.Hex("#63636b"),
		TextTertiary:  ui.Hex("#85858d"),
		Fg2:           ui.Hex("#4b4b53"),
		Faint:         ui.Hex("#b9b9c2"),
		CtlFg:         ui.Hex("#63636b"),
		Accent:        ui.Hex("#4f46e5"),
		AccentSoft:    ui.Hex("#eeedfd"),
		AccentText:    ui.Hex("#ffffff"),
		Selection:     ui.Hex("#ecebfb"),
		PillHover:     ui.Hex("#e9e9ee"),
		Destructive:   ui.Hex("#d64545"),
		Success:       ui.Hex("#189a58"),
		GreenSoft:     ui.Hex("#e4f6ec"),
		RedSoft:       ui.Hex("#fbe9e9"),
		Amber:         ui.Hex("#b45309"),
		AmberSoft:     ui.Hex("#fbf0df"),
		Scrim:         ui.RGBA(255, 255, 255, 0.55),
	}
}

func Dark() Tokens {
	return Tokens{
		Dark:          true,
		Page:          ui.Hex("#1a1a1e"),
		Surface:       ui.Hex("#232327"),
		SurfaceHover:  ui.Hex("#2d2d33"),
		SurfacePress:  ui.Hex("#36363d"),
		Border:        ui.Hex("#333339"),
		Line:          ui.Hex("#333339"),
		Line2:         ui.Hex("#2c2c32"),
		Card2:         ui.Hex("#1f1f23"),
		Muted:         ui.Hex("#2d2d33"),
		Text:          ui.Hex("#ededf1"),
		TextSecondary: ui.Hex("#a1a1aa"),
		TextTertiary:  ui.Hex("#8b8b94"),
		Fg2:           ui.Hex("#b9b9c1"),
		Faint:         ui.Hex("#56565f"),
		CtlFg:         ui.Hex("#a1a1aa"),
		Accent:        ui.Hex("#a5b4fc"),
		AccentSoft:    ui.Hex("#2b2c47"),
		AccentText:    ui.Hex("#14142a"),
		Selection:     ui.Hex("#33345a"),
		PillHover:     ui.Hex("#36363d"),
		Destructive:   ui.Hex("#f28b82"),
		Success:       ui.Hex("#6fd99b"),
		GreenSoft:     ui.Hex("#1e3a2b"),
		RedSoft:       ui.Hex("#45272a"),
		Amber:         ui.Hex("#f2b544"),
		AmberSoft:     ui.Hex("#3f3220"),
		Scrim:         ui.RGBA(0, 0, 0, 0.40),
	}
}

// Theme maps the tokens onto MyGo's widget theme. Background is the panel's
// own surface; Page is what a settings window paints behind its cards.
func (t Tokens) Theme() *ui.Theme {
	base := ui.LightTheme()
	if t.Dark {
		base = ui.DarkTheme()
	}
	base.Background = t.Surface
	base.Surface = t.Muted
	base.SurfaceHover = t.SurfaceHover
	base.SurfacePressed = t.SurfacePress
	base.Border = t.Border
	base.Text = t.Text
	base.TextMuted = t.TextSecondary
	base.Accent = t.Accent
	base.AccentHover = t.Accent
	base.AccentPressed = t.Accent
	base.AccentText = t.AccentText
	base.Danger = t.Destructive
	base.Warning = t.Amber
	base.Success = t.Success
	base.Selection = t.Selection
	base.Focus = t.AccentSoft
	base.Inverse = t.Text
	base.InverseText = t.Surface
	base.Scrollbar = t.Faint
	base.Radius = ControlRadius
	base.Spacing = 4
	base.FontSize = BodyFont
	return base
}
