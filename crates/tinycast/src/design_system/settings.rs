use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_GRADIENT_STOP, D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL};

use super::appearance;
use super::fill_squircle;
use super::squircle::stroke_squircle;

pub const ROW_H: f32 = 52.0;
pub const TOGGLE_W: f32 = 40.0;
pub const TOGGLE_H: f32 = 22.0;
pub const HEADER_H: f32 = 22.0;
#[allow(dead_code)]
pub const FOOTER_H: f32 = 36.0;
pub const ACCENT: (f32, f32, f32, f32) = (0.0, 0.47, 0.83, 1.0);

pub fn footer_block_h(lines: u32) -> f32 {
    (18.0 * lines as f32).max(18.0)
}
pub const OVERFLOW_FADE: f32 = 24.0;
pub const CARD_INSET: f32 = theme::spacing::XXL;
pub const CARD_PAD: f32 = theme::spacing::XL;

/// Symmetric content inset: card origin plus inner pad.
pub fn content_pad() -> f32 {
    CARD_INSET + CARD_PAD
}

pub fn toggle_rect(width: f32, row_y: f32, pad_x: f32) -> DipRect {
    DipRect {
        x: width - pad_x - TOGGLE_W,
        y: row_y + (ROW_H - TOGGLE_H) / 2.0,
        w: TOGGLE_W,
        h: TOGGLE_H,
    }
}

pub fn approx_label_width(text: &str) -> f32 {
    let mut w = 0.0f32;
    for c in text.chars() {
        w += if (c as u32) > 0x2E80 { 13.0 } else { 7.5 };
    }
    (w + theme::spacing::XS).max(theme::spacing::XL)
}

pub fn sidebar_rgb(appearance: u8) -> (f32, f32, f32) {
    if appearance == 0 {
        theme::settings_chrome::SIDEBAR_DARK
    } else {
        theme::settings_chrome::SIDEBAR_LIGHT
    }
}

pub fn detail_rgb(appearance: u8) -> (f32, f32, f32) {
    if appearance == 0 {
        theme::settings_chrome::DETAIL_DARK
    } else {
        theme::settings_chrome::DETAIL_LIGHT
    }
}

pub fn card_fill(appearance: u8) -> (f32, f32, f32, f32) {
    theme::colors::ramp_rgba(
        appearance,
        theme::colors::CARD_FILL_DARK_ALPHA,
        theme::colors::CARD_FILL_LIGHT_ALPHA,
    )
}

pub fn card_stroke(appearance: u8) -> (f32, f32, f32, f32) {
    theme::colors::ramp_rgba(
        appearance,
        theme::colors::CARD_STROKE_ALPHA,
        theme::colors::CARD_STROKE_ALPHA,
    )
}

pub fn paint_window_background(
    target: &ID2D1RenderTarget,
    w: f32,
    h: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (dr, dg, db) = detail_rgb(appearance);
    let (sr, sg, sb) = sidebar_rgb(appearance);
    unsafe {
        target.Clear(Some(&D2D1_COLOR_F {
            r: dr,
            g: dg,
            b: db,
            a: 1.0,
        }));
        let brush = target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: sr,
                g: sg,
                b: sb,
                a: 1.0,
            },
            None,
        )?;
        let side = tinycast_pure::layout::settings::sidebar_rect(h);
        target.FillRectangle(
            &D2D_RECT_F {
                left: side.x,
                top: side.y,
                right: side.x + side.w,
                bottom: side.y + side.h,
            },
            &brush,
        );
    }
    let _ = w;
    Ok(())
}

pub struct GroupedSection {
    pub header: Option<&'static str>,
    pub footer: Option<&'static str>,
    pub footer_h: f32,
    pub y: f32,
    pub width: f32,
    pub body_h: f32,
}

impl GroupedSection {
    pub fn card_rect(&self) -> DipRect {
        let header_h = if self.header.is_some() {
            HEADER_H + theme::spacing::SECTION_HEADER_BOTTOM
        } else {
            0.0
        };
        DipRect {
            x: CARD_INSET,
            y: self.y + header_h,
            w: (self.width - CARD_INSET * 2.0).max(0.0),
            h: self.body_h,
        }
    }

    pub fn next_y(&self) -> f32 {
        let card = self.card_rect();
        let footer_h = if self.footer.is_some() {
            theme::spacing::SM + self.footer_h.max(footer_block_h(1))
        } else {
            0.0
        };
        card.y + card.h + footer_h + theme::spacing::SECTION_SPACING
    }
}

pub fn paint_grouped_section(
    target: &ID2D1RenderTarget,
    header_format: &IDWriteTextFormat,
    caption_format: &IDWriteTextFormat,
    section: &GroupedSection,
    appearance: u8,
) -> windows::core::Result<()> {
    let card = section.card_rect();
    if let Some(header) = section.header {
        let (r, g, b, a) = theme::colors::ramp_rgba(
            appearance,
            theme::colors::TEXT_TERTIARY_DARK_ALPHA,
            theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
        );
        draw_text(
            target,
            header_format,
            header,
            D2D_RECT_F {
                left: card.x,
                top: section.y,
                right: card.x + card.w,
                bottom: section.y + HEADER_H,
            },
            (r, g, b, a),
        )?;
    }
    fill_squircle(target, card, theme::radius::CARD, card_fill(appearance))?;
    stroke_squircle(
        target,
        card,
        theme::radius::CARD,
        card_stroke(appearance),
        theme::size::HAIRLINE,
    )?;
    if let Some(footer) = section.footer {
        let (r, g, b, a) = theme::colors::ramp_rgba(
            appearance,
            theme::colors::TEXT_TERTIARY_DARK_ALPHA,
            theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
        );
        draw_text(
            target,
            caption_format,
            footer,
            D2D_RECT_F {
                left: card.x,
                top: card.y + card.h + theme::spacing::SM,
                right: card.x + card.w,
                bottom: card.y
                    + card.h
                    + theme::spacing::SM
                    + section.footer_h.max(footer_block_h(1)),
            },
            (r, g, b, a),
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub enum RowTrailing<'a> {
    None,
    Toggle(bool),
    Label(&'a str),
}

pub fn trailing_width(trailing: RowTrailing<'_>, width: f32, pad_x: f32) -> f32 {
    let max_trail = ((width - pad_x * 2.0) * 0.5).max(48.0);
    match trailing {
        RowTrailing::Toggle(_) => TOGGLE_W + theme::spacing::SM,
        RowTrailing::Label(label) => approx_label_width(label).clamp(40.0, max_trail),
        RowTrailing::None => 0.0,
    }
}

pub fn paint_form_row(
    target: &ID2D1RenderTarget,
    title_format: &IDWriteTextFormat,
    caption_format: &IDWriteTextFormat,
    title: &str,
    subtitle: &str,
    y: f32,
    width: f32,
    enabled: bool,
    appearance: u8,
    trailing: RowTrailing<'_>,
) -> windows::core::Result<()> {
    paint_settings_row(
        target,
        title_format,
        caption_format,
        title,
        subtitle,
        y,
        width,
        content_pad(),
        enabled,
        appearance,
        trailing,
    )
}

pub fn paint_settings_row(
    target: &ID2D1RenderTarget,
    title_format: &IDWriteTextFormat,
    caption_format: &IDWriteTextFormat,
    title: &str,
    subtitle: &str,
    y: f32,
    width: f32,
    pad_x: f32,
    enabled: bool,
    appearance: u8,
    trailing: RowTrailing<'_>,
) -> windows::core::Result<()> {
    let dim = if enabled { 1.0 } else { 0.45 };
    let (tr, tg, tb, ta) = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_PRIMARY_ALPHA,
        theme::colors::TEXT_PRIMARY_ALPHA,
    );
    let (cr, cg, cb, ca) = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    );
    let trail_w = trailing_width(trailing, width, pad_x);
    let text_right = (width - pad_x - trail_w).max(pad_x + 48.0);
    let has_sub = !subtitle.is_empty();
    let title_top = if has_sub {
        y + 4.0
    } else {
        y + (ROW_H - 24.0) / 2.0
    };
    draw_text(
        target,
        title_format,
        title,
        D2D_RECT_F {
            left: pad_x,
            top: title_top,
            right: text_right,
            bottom: title_top + 24.0,
        },
        (tr, tg, tb, ta * dim),
    )?;
    if has_sub {
        draw_text(
            target,
            caption_format,
            subtitle,
            D2D_RECT_F {
                left: pad_x,
                top: y + 28.0,
                right: text_right,
                bottom: y + ROW_H - 4.0,
            },
            (cr, cg, cb, ca * dim),
        )?;
    }
    match trailing {
        RowTrailing::Toggle(on) => {
            paint_toggle(
                target,
                toggle_rect(width, y, pad_x),
                on,
                enabled,
                appearance,
            )?;
        }
        RowTrailing::Label(label) => {
            draw_text(
                target,
                caption_format,
                label,
                D2D_RECT_F {
                    left: text_right,
                    top: y,
                    right: width - pad_x,
                    bottom: y + ROW_H,
                },
                (cr, cg, cb, ca * dim),
            )?;
        }
        RowTrailing::None => {}
    }
    Ok(())
}

pub fn form_origin() -> f32 {
    CARD_INSET + HEADER_H + theme::spacing::SECTION_HEADER_BOTTOM + CARD_PAD
}

/// Ink that follows Appearance: white on Dark, black on Light (straight alpha).
pub fn ramp_color(appearance: u8, alpha: f32) -> D2D1_COLOR_F {
    appearance::color(theme::colors::ramp_rgba(appearance, alpha, alpha))
}

pub fn primary_ink(appearance: u8) -> D2D1_COLOR_F {
    ramp_color(appearance, theme::colors::TEXT_PRIMARY_ALPHA)
}

pub fn secondary_ink(appearance: u8) -> D2D1_COLOR_F {
    ramp_color(appearance, theme::colors::TEXT_SECONDARY_ALPHA)
}

pub fn tertiary_ink(appearance: u8) -> D2D1_COLOR_F {
    appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    ))
}

/// Content-space y after a FeatureSwitchSection card (scroll = 0).
pub fn switch_section_next_y(launcher_row: bool) -> f32 {
    let rows = if launcher_row { 2 } else { 1 };
    GroupedSection {
        header: Some(""),
        footer: None,
        footer_h: 0.0,
        y: CARD_INSET,
        width: 420.0,
        body_h: CARD_PAD * 2.0 + ROW_H * rows as f32,
    }
    .next_y()
}

pub fn feature_switch_section(
    width: f32,
    y: f32,
    header: &'static str,
    launcher_row: bool,
) -> (GroupedSection, DipRect, Option<DipRect>) {
    let rows = if launcher_row { 2 } else { 1 };
    let section = GroupedSection {
        header: Some(header),
        footer: None,
        footer_h: 0.0,
        y,
        width,
        body_h: CARD_PAD * 2.0 + ROW_H * rows as f32,
    };
    let card = section.card_rect();
    let enable = DipRect {
        x: card.x,
        y: card.y + CARD_PAD,
        w: card.w,
        h: ROW_H,
    };
    let show = if launcher_row {
        Some(DipRect {
            x: card.x,
            y: enable.y + ROW_H,
            w: card.w,
            h: ROW_H,
        })
    } else {
        None
    };
    (section, enable, show)
}

pub fn paint_overflow_fade(
    target: &ID2D1RenderTarget,
    width: f32,
    viewport_h: f32,
    content_h: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    if content_h <= viewport_h + scroll + 0.5 {
        return Ok(());
    }
    let band = OVERFLOW_FADE;
    let y1 = viewport_h;
    let y0 = (y1 - band).max(0.0);
    let (r, g, b) = detail_rgb(appearance);
    let clear = D2D1_COLOR_F { r, g, b, a: 0.0 };
    let outer = D2D1_COLOR_F { r, g, b, a: 1.0 };
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: clear,
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: outer,
        },
    ];
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: D2D_POINT_2F { x: 0.0, y: y0 },
            endPoint: D2D_POINT_2F { x: 0.0, y: y1 },
        };
        let brush = target.CreateLinearGradientBrush(&props, None, &collection)?;
        target.FillRectangle(
            &D2D_RECT_F {
                left: 0.0,
                top: y0,
                right: width,
                bottom: y1,
            },
            &brush,
        );
    }
    Ok(())
}

pub fn paint_toggle(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    on: bool,
    enabled: bool,
    appearance: u8,
) -> windows::core::Result<()> {
    let fill = if on {
        D2D1_COLOR_F {
            r: ACCENT.0,
            g: ACCENT.1,
            b: ACCENT.2,
            a: if enabled { ACCENT.3 } else { 0.45 },
        }
    } else {
        ramp_color(appearance, if enabled { 0.18 } else { 0.08 })
    };
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    let rounded = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.w,
            bottom: rect.y + rect.h,
        },
        radiusX: rect.h / 2.0,
        radiusY: rect.h / 2.0,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &brush);
    }
    let pad = 2.0;
    let kn = rect.h - pad * 2.0;
    let kx = if on {
        rect.x + rect.w - pad - kn
    } else {
        rect.x + pad
    };
    let knob = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.92,
            },
            None,
        )?
    };
    unsafe {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: kx,
                    top: rect.y + pad,
                    right: kx + kn,
                    bottom: rect.y + pad + kn,
                },
                radiusX: kn / 2.0,
                radiusY: kn / 2.0,
            },
            &knob,
        );
    }
    Ok(())
}

fn draw_text(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    text: &str,
    rect: D2D_RECT_F,
    rgba: (f32, f32, f32, f32),
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(rgba), None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

    #[test]
    fn feature_switch_rows_are_stacked_row_h() {
        let (_, enable, show) = feature_switch_section(420.0, CARD_INSET, "Notes", true);
        let show = show.expect("launcher row");
        assert!((show.y - enable.y - ROW_H).abs() < 0.001);
        assert!((show.y - enable.y - (ROW_H + theme::spacing::XL)).abs() > 1.0);
        assert!((enable.y - form_origin()).abs() < 0.001);
    }

    #[test]
    fn footer_height_grows_with_wrapped_copy() {
        assert!(footer_block_h(3) > 36.0);
    }

    #[test]
    fn primary_ink_inverts_with_appearance() {
        let dark = primary_ink(0);
        let light = primary_ink(1);
        assert!(dark.r > 0.9);
        assert!(light.r < 0.1);
    }

    #[test]
    fn toggle_stays_inside_card_content_inset() {
        for width in [theme::size::SETTINGS_DETAIL_MINIMUM, 645.0] {
            let (section, enable, show) = feature_switch_section(width, CARD_INSET, "Notes", true);
            let card = section.card_rect();
            let pad = content_pad();
            let on = toggle_rect(width, enable.y, pad);
            assert!(
                on.x + 0.01 >= card.x + CARD_PAD,
                "toggle x {} inside card {}+{}",
                on.x,
                card.x,
                CARD_PAD
            );
            assert!(
                on.x + on.w <= card.x + card.w - CARD_PAD + 0.01,
                "toggle right {} vs card inner {}",
                on.x + on.w,
                card.x + card.w - CARD_PAD
            );
            let show = show.expect("launcher row");
            let off = toggle_rect(width, show.y, pad);
            assert!(off.x + off.w <= card.x + card.w - CARD_PAD + 0.01);
            let leaked = width - theme::spacing::XL - TOGGLE_W;
            assert!(
                leaked + TOGGLE_W > card.x + card.w - CARD_PAD,
                "legacy XL pad must overflow the card so the new inset is a real fix"
            );
        }
    }

    #[test]
    fn long_trailing_label_does_not_eat_the_title_column() {
        let width = theme::size::SETTINGS_DETAIL_MINIMUM;
        let pad = content_pad();
        let label = "空闲 0 秒后返回（0 = 从不）。";
        let trail = trailing_width(RowTrailing::Label(label), width, pad);
        let text_right = width - pad - trail;
        assert!(text_right - pad >= 80.0, "title column {text_right}");
        assert!(trail <= (width - pad * 2.0) * 0.5 + 0.01);
        assert!(approx_label_width("Never") < approx_label_width(label));
    }

    #[test]
    fn toggle_thumb_is_visible_on_and_off() {
        let rect = DipRect {
            x: 8.0,
            y: 4.0,
            w: TOGGLE_W,
            h: TOGGLE_H,
        };
        let paint = |on: bool| {
            crate::design_system::test_render::with_offscreen(64, 32, |target| {
                unsafe {
                    target.Clear(Some(&D2D1_COLOR_F {
                        r: 0.16,
                        g: 0.16,
                        b: 0.16,
                        a: 1.0,
                    }));
                }
                paint_toggle(target, rect, on, true, 0)
            })
            .expect("toggle")
            .2
        };
        let on = paint(true);
        let off = paint(false);
        let sample = |bits: &[u8], x: i32| {
            let x = x.clamp(0, 63) as usize;
            let y = 14usize;
            let i = (y * 64 + x) * 4;
            (bits[i] as i16, bits[i + 1] as i16, bits[i + 2] as i16)
        };
        let on_right = sample(&on, (rect.x + rect.w - 6.0) as i32);
        let on_left = sample(&on, (rect.x + 6.0) as i32);
        let off_left = sample(&off, (rect.x + 6.0) as i32);
        assert!(
            on_right.2 > 180 && on_right.1 > 180 && on_right.0 > 180,
            "on thumb should be light, got {:?}",
            on_right
        );
        assert!(
            on_left.2 < 80 && on_left.1 > 60,
            "on track should be accent, got {:?}",
            on_left
        );
        assert!(
            off_left.2 > 180 && off_left.1 > 180,
            "off thumb should be light on the left, got {:?}",
            off_left
        );
    }
}
