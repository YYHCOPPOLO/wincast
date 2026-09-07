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

pub fn footer_block_h(lines: u32) -> f32 {
    (18.0 * lines as f32).max(18.0)
}
pub const OVERFLOW_FADE: f32 = 24.0;
pub const CARD_INSET: f32 = theme::spacing::XXL;
pub const CARD_PAD: f32 = theme::spacing::XL;

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
                bottom: card.y + card.h + theme::spacing::SM + section.footer_h.max(footer_block_h(1)),
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
    let trail_w = match trailing {
        RowTrailing::Toggle(_) => TOGGLE_W + theme::spacing::SM,
        RowTrailing::Label(_) => 120.0,
        RowTrailing::None => 0.0,
    };
    let text_right = width - pad_x - trail_w;
    draw_text(
        target,
        title_format,
        title,
        D2D_RECT_F {
            left: pad_x,
            top: y + 8.0,
            right: text_right,
            bottom: y + 28.0,
        },
        (tr, tg, tb, ta * dim),
    )?;
    if !subtitle.is_empty() {
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
            let ty = y + (ROW_H - TOGGLE_H) / 2.0;
            paint_toggle(
                target,
                DipRect {
                    x: width - pad_x - TOGGLE_W,
                    y: ty,
                    w: TOGGLE_W,
                    h: TOGGLE_H,
                },
                on,
                enabled,
                appearance,
            )?;
        }
        RowTrailing::Label(label) => {
            draw_text(
                target,
                title_format,
                label,
                D2D_RECT_F {
                    left: text_right,
                    top: y + 14.0,
                    right: width - pad_x,
                    bottom: y + 38.0,
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
    let clear = D2D1_COLOR_F {
        r,
        g,
        b,
        a: 0.0,
    };
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

fn paint_toggle(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    on: bool,
    enabled: bool,
    appearance: u8,
) -> windows::core::Result<()> {
    let fill = if on {
        D2D1_COLOR_F {
            r: 0.0,
            g: 0.47,
            b: 0.83,
            a: if enabled { 1.0 } else { 0.45 },
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
}
