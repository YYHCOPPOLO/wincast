//! Two-column calculator card. Selection index 0 when a result is present.

use tinycast_pure::calc::CalcResult;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL,
};

use crate::features::launcher::ui::list::{ListFonts, PaintItem};

pub fn paint_item(result: &CalcResult, selected: bool) -> PaintItem {
    PaintItem::Calc {
        expression: result.expression.clone(),
        display: result.display.clone(),
        source_badge: result.source_badge.clone(),
        target_badge: result.target_badge.clone(),
        selected,
        is_error: is_error(result),
    }
}

pub fn is_error(result: &CalcResult) -> bool {
    result.source_badge.is_none() && result.target_badge.is_none()
}

pub fn is_actionable(result: &CalcResult) -> bool {
    !is_error(result)
}

pub fn paint(
    target: &ID2D1RenderTarget,
    _dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    expression: &str,
    display: &str,
    source_badge: Option<&str>,
    target_badge: Option<&str>,
    selected: bool,
    is_error: bool,
    y: f32,
    panel_w: f32,
    h: f32,
) -> windows::core::Result<()> {
    let inset = theme::spacing::MD;
    let card = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: inset,
            top: y + theme::spacing::XS,
            right: panel_w - inset,
            bottom: y + h - theme::spacing::XS,
        },
        radiusX: theme::radius::CARD,
        radiusY: theme::radius::CARD,
    };
    let fill = if selected {
        selection_color()
    } else {
        card_fill()
    };
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(&card, &brush);
    }

    if is_error {
        let text_brush = unsafe { target.CreateSolidColorBrush(&muted(0.70), None)? };
        return draw_text(
            target,
            &fonts.title,
            &text_brush,
            D2D_RECT_F {
                left: card.rect.left + theme::spacing::XL,
                top: card.rect.top,
                right: card.rect.right - theme::spacing::XL,
                bottom: card.rect.bottom,
            },
            display,
        );
    }

    let mid = (card.rect.left + card.rect.right) / 2.0;
    let arrow_w = 28.0;
    paint_column(
        target,
        fonts,
        expression,
        source_badge,
        D2D_RECT_F {
            left: card.rect.left + theme::spacing::MD,
            top: card.rect.top + theme::spacing::SM,
            right: mid - arrow_w / 2.0,
            bottom: card.rect.bottom - theme::spacing::SM,
        },
        false,
    )?;
    let arrow_brush = unsafe { target.CreateSolidColorBrush(&muted(0.45), None)? };
    draw_text(
        target,
        &fonts.title,
        &arrow_brush,
        D2D_RECT_F {
            left: mid - arrow_w / 2.0,
            top: card.rect.top,
            right: mid + arrow_w / 2.0,
            bottom: card.rect.bottom,
        },
        "→",
    )?;
    paint_column(
        target,
        fonts,
        display,
        target_badge,
        D2D_RECT_F {
            left: mid + arrow_w / 2.0,
            top: card.rect.top + theme::spacing::SM,
            right: card.rect.right - theme::spacing::MD,
            bottom: card.rect.bottom - theme::spacing::SM,
        },
        true,
    )
}

fn paint_column(
    target: &ID2D1RenderTarget,
    fonts: &ListFonts,
    text: &str,
    badge: Option<&str>,
    rect: D2D_RECT_F,
    semibold: bool,
) -> windows::core::Result<()> {
    let title = unsafe { target.CreateSolidColorBrush(&title_color(), None)? };
    let format: &IDWriteTextFormat = if semibold {
        &fonts.calc_result
    } else {
        &fonts.calc_result
    };
    let value_bottom = if badge.is_some() {
        rect.top + (rect.bottom - rect.top) * 0.58
    } else {
        rect.bottom
    };
    draw_text(
        target,
        format,
        &title,
        D2D_RECT_F {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: value_bottom,
        },
        text,
    )?;
    if let Some(badge) = badge {
        let muted_brush = unsafe { target.CreateSolidColorBrush(&muted(0.55), None)? };
        let pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left,
                top: value_bottom + theme::spacing::XXS,
                right: (rect.left + 160.0).min(rect.right),
                bottom: rect.bottom - theme::spacing::XXS,
            },
            radiusX: theme::radius::KEY_CAP,
            radiusY: theme::radius::KEY_CAP,
        };
        let pill_fill = unsafe { target.CreateSolidColorBrush(&muted(0.12), None)? };
        unsafe {
            target.FillRoundedRectangle(&pill, &pill_fill);
        }
        draw_text(target, &fonts.calc_badge, &muted_brush, pill.rect, badge)?;
    }
    Ok(())
}

fn draw_text(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    text: &str,
) -> windows::core::Result<()> {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn selection_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: theme::colors::SELECTION_DARK_ALPHA,
    }
}

fn card_fill() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.06,
    }
}

fn title_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    }
}

fn muted(a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
    }
}
