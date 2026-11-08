// Package theme carries the design tokens shared by every native view.
// Values follow the original macOS app's design system (../tinycast, tag
// v0.10.2), adapted to Windows' system font and control metrics.
package theme

import (
	"math"

	"github.com/egoist/mygo/ui"
)

// Metrics used across the launcher, settings and future feature windows.
const (
	PanelWidth          float32 = 750
	PanelHeight         float32 = 475
	CompactHeight       float32 = 64
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

	PanelRadius   float32 = 26
	CardRadius    float32 = 10
	ControlRadius float32 = 8
	RowRadius     float32 = 10

	SettingsWidth   float32 = 900
	SettingsHeight  float32 = 700
	SettingsSidebar float32 = 215
	SettingsRow     float32 = 48

	BodyFont      float32 = 13
	SearchFont    float32 = 20
	RowFont       float32 = 13
	TrailingFont  float32 = 12
	SectionFont   float32 = 11
	KeyCapFont    float32 = 10
	HeadingFont   float32 = 26
	FormLabelFont float32 = 13

	// Calculator answer card, from the original CalculatorCard and LeadCard.
	CalcResultFont    float32 = 22
	CalcBadgeFont     float32 = 10
	CalcArrowFont     float32 = 15
	CalcCardRadius    float32 = 10
	CalcCardInset     float32 = 8
	CalcCardGapY      float32 = 4
	CalcCardPaddingY  float32 = 28
	CalcCardPaddingX  float32 = 12
	CalcColumnPadding float32 = 8
	CalcColumnGap     float32 = 8
	CalcBadgePadX     float32 = 6
	CalcBadgePadY     float32 = 2
	CalcBadgeRadius   float32 = 6
)

// Tokens is one appearance. Views use ForContext to follow the desktop accent.
type Tokens struct {
	Dark bool

	Page           ui.Color
	Surface        ui.Color
	SurfaceHover   ui.Color
	SurfacePress   ui.Color
	Border         ui.Color
	Line           ui.Color
	Line2          ui.Color
	Card2          ui.Color
	CardFill       ui.Color
	CardStroke     ui.Color
	CardHover      ui.Color
	GlassFrost     ui.Color
	ControlSurface ui.Color
	ControlHover   ui.Color
	ControlPress   ui.Color
	Muted          ui.Color
	Text           ui.Color
	TextSecondary  ui.Color
	TextTertiary   ui.Color
	NoteText       ui.Color
	Fg2            ui.Color
	Faint          ui.Color
	CtlFg          ui.Color
	Accent         ui.Color
	AccentHover    ui.Color
	AccentPressed  ui.Color
	AccentSoft     ui.Color
	AccentText     ui.Color
	Focus          ui.Color
	Selection      ui.Color
	RowHover       ui.Color
	TextSelection  ui.Color
	PillHover      ui.Color
	Destructive    ui.Color
	Success        ui.Color
	GreenSoft      ui.Color
	RedSoft        ui.Color
	Amber          ui.Color
	AmberSoft      ui.Color
	Scrim          ui.Color
	EmojiCell      ui.Color
	EmojiHover     ui.Color
	EmojiSelected  ui.Color
}

func For(dark bool) Tokens {
	if dark {
		return Dark()
	}
	return Light()
}

// ForContext keeps the original's neutral ink ramp while using the desktop's
// accent for native controls. Preferences are refreshed by MyGo on system changes.
func ForContext(c *ui.Context) Tokens {
	t := For(c.Theme().Dark)
	if accent := c.Preferences().Accent; accent.A > 0 {
		t = t.withAccent(accent)
	}
	return t
}

func (t Tokens) withAccent(accent ui.Color) Tokens {
	black, white := ui.RGB(0, 0, 0), ui.RGB(255, 255, 255)
	accent = ui.RGB(accent.R, accent.G, accent.B)
	t.Accent = accent
	// Compare contrast in linear sRGB, including for bright Windows accents.
	linear := func(channel uint8) float64 {
		x := float64(channel) / 255
		if x <= 0.04045 {
			return x / 12.92
		}
		return math.Pow((x+0.055)/1.055, 2.4)
	}
	luminance := 0.2126*linear(accent.R) + 0.7152*linear(accent.G) + 0.0722*linear(accent.B)
	t.AccentText = white
	shade := black
	if (luminance+0.05)/0.05 > 1.05/(luminance+0.05) {
		t.AccentText = black
		shade = white
	}
	// Move away from the text color, so hover/press never weaken its contrast.
	t.AccentHover = accent.Mix(shade, 0.10)
	t.AccentPressed = accent.Mix(shade, 0.18)
	t.AccentSoft = accent.Alpha(0.12)
	t.TextSelection = accent.Alpha(0.25)
	t.Focus = accent.Alpha(0.55)
	if t.Dark {
		t.AccentSoft = accent.Alpha(0.18)
		t.TextSelection = accent.Alpha(0.35)
		t.Focus = accent.Mix(white, 0.2).Alpha(0.60)
	}
	return t
}

// PanelBackground leaves Windows Acrylic visible. DWM supplies its own tint
// and contrast treatment; adding the macOS scrim on top washes out the blur.
// Unsupported systems and disabled glass use a fully opaque surface.
func (t Tokens) PanelBackground(material bool) ui.Color {
	if material {
		return ui.Transparent
	}
	return t.Surface
}

func (t Tokens) PageBackground(material bool) ui.Color {
	if material {
		return ui.Transparent
	}
	return t.Page
}

func Light() Tokens {
	return Tokens{
		Page:           ui.Hex("#f5f5f5"),
		Surface:        ui.Hex("#ffffff"),
		SurfaceHover:   ui.RGBA(0, 0, 0, 0.05),
		SurfacePress:   ui.RGBA(0, 0, 0, 0.10),
		Border:         ui.RGBA(0, 0, 0, 0.18),
		Line:           ui.RGBA(0, 0, 0, 0.12),
		Line2:          ui.RGBA(0, 0, 0, 0.06),
		Card2:          ui.RGBA(0, 0, 0, 0.03),
		CardFill:       ui.RGBA(0, 0, 0, 0.04),
		CardStroke:     ui.RGBA(0, 0, 0, 0.10),
		CardHover:      ui.RGBA(0, 0, 0, 0.07),
		GlassFrost:     ui.RGBA(255, 255, 255, 0.25),
		ControlSurface: ui.RGBA(0, 0, 0, 0.08),
		ControlHover:   ui.RGBA(0, 0, 0, 0.045),
		ControlPress:   ui.RGBA(0, 0, 0, 0.09),
		Muted:          ui.RGBA(0, 0, 0, 0.08),
		Text:           ui.RGB(0, 0, 0),
		TextSecondary:  ui.RGBA(0, 0, 0, 0.60),
		TextTertiary:   ui.RGBA(0, 0, 0, 0.42),
		NoteText:       ui.RGBA(0, 0, 0, 0.85),
		Fg2:            ui.RGBA(0, 0, 0, 0.60),
		Faint:          ui.RGBA(0, 0, 0, 0.16),
		CtlFg:          ui.RGBA(0, 0, 0, 0.60),
		Selection:      ui.RGBA(0, 0, 0, 0.09),
		RowHover:       ui.RGBA(0, 0, 0, 0.045),
		PillHover:      ui.RGBA(0, 0, 0, 0.10),
		Destructive:    ui.Hex("#dc2626"),
		Success:        ui.Hex("#16a34a"),
		GreenSoft:      ui.RGBA(22, 163, 74, 0.12),
		RedSoft:        ui.RGBA(220, 38, 38, 0.10),
		Amber:          ui.Hex("#d97706"),
		AmberSoft:      ui.RGBA(217, 119, 6, 0.12),
		Scrim:          ui.RGBA(255, 255, 255, 0.55),
		EmojiCell:      ui.RGBA(0, 0, 0, 0.04),
		EmojiHover:     ui.RGBA(0, 0, 0, 0.34),
		EmojiSelected:  ui.RGBA(0, 0, 0, 0.72),
	}.withAccent(ui.Hex("#0078d4"))
}

func Dark() Tokens {
	return Tokens{
		Dark:           true,
		Page:           ui.Hex("#1c1c1c"),
		Surface:        ui.Hex("#242424"),
		SurfaceHover:   ui.RGBA(255, 255, 255, 0.06),
		SurfacePress:   ui.RGBA(255, 255, 255, 0.12),
		Border:         ui.RGBA(255, 255, 255, 0.20),
		Line:           ui.RGBA(255, 255, 255, 0.10),
		Line2:          ui.RGBA(255, 255, 255, 0.06),
		Card2:          ui.RGBA(255, 255, 255, 0.04),
		CardFill:       ui.RGBA(255, 255, 255, 0.05),
		CardStroke:     ui.RGBA(255, 255, 255, 0.10),
		CardHover:      ui.RGBA(255, 255, 255, 0.08),
		GlassFrost:     ui.RGBA(255, 255, 255, 0.05),
		ControlSurface: ui.RGBA(255, 255, 255, 0.10),
		ControlHover:   ui.RGBA(255, 255, 255, 0.10),
		ControlPress:   ui.RGBA(255, 255, 255, 0.15),
		Muted:          ui.RGBA(255, 255, 255, 0.10),
		Text:           ui.RGB(255, 255, 255),
		TextSecondary:  ui.RGBA(255, 255, 255, 0.60),
		TextTertiary:   ui.RGBA(255, 255, 255, 0.40),
		NoteText:       ui.RGBA(255, 255, 255, 0.90),
		Fg2:            ui.RGBA(255, 255, 255, 0.60),
		Faint:          ui.RGBA(255, 255, 255, 0.22),
		CtlFg:          ui.RGBA(255, 255, 255, 0.60),
		Selection:      ui.RGBA(255, 255, 255, 0.10),
		RowHover:       ui.RGBA(255, 255, 255, 0.05),
		PillHover:      ui.RGBA(255, 255, 255, 0.12),
		Destructive:    ui.Hex("#f87171"),
		Success:        ui.Hex("#4ade80"),
		GreenSoft:      ui.RGBA(74, 222, 128, 0.15),
		RedSoft:        ui.RGBA(248, 113, 113, 0.15),
		Amber:          ui.Hex("#fbbf24"),
		AmberSoft:      ui.RGBA(251, 191, 36, 0.15),
		Scrim:          ui.RGBA(0, 0, 0, 0.40),
		EmojiCell:      ui.RGBA(255, 255, 255, 0.045),
		EmojiHover:     ui.RGBA(255, 255, 255, 0.42),
		EmojiSelected:  ui.RGBA(255, 255, 255, 0.92),
	}.withAccent(ui.Hex("#0078d4"))
}

// Theme maps the tokens onto MyGo's widget theme. Background is the panel's
// own surface; Page is what a settings window paints behind its cards.
func (t Tokens) Theme() *ui.Theme {
	base := ui.LightTheme()
	if t.Dark {
		base = ui.DarkTheme()
	}
	base.Background = t.Surface
	base.Surface = t.GlassFrost
	base.SurfaceHover = t.ControlHover
	base.SurfacePressed = t.ControlPress
	base.Border = t.CardStroke
	base.Text = t.Text
	base.TextMuted = t.TextSecondary
	base.Accent = t.Accent
	base.AccentHover = t.AccentHover
	base.AccentPressed = t.AccentPressed
	base.AccentText = t.AccentText
	base.Danger = t.Destructive
	base.Warning = t.Amber
	base.Success = t.Success
	base.Selection = t.TextSelection
	base.Focus = t.Focus
	base.Inverse = t.Text
	base.InverseText = t.Surface
	base.Scrollbar = t.Faint
	base.ScrollbarWidth = 4
	base.Radius = ControlRadius
	base.Spacing = 4
	base.FontSize = BodyFont
	return base
}
