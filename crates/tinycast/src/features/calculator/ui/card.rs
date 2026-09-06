//! Two-column calculator card. Selection index 0 when a result is present.

use tinycast_pure::calc::CalcResult;
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL,
};

use crate::design_system::{fill_squircle, symbols};
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
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    expression: &str,
    display: &str,
    source_badge: Option<&str>,
    target_badge: Option<&str>,
    selected: bool,
    is_error: bool,
    y: f32,
    panel_w: f32,
    _h: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let card = tinycast_pure::layout::list::calc_card_rect(panel_w, y);
    fill_squircle(
        target,
        card,
        theme::radius::CARD,
        theme::colors::ramp_rgba(
            appearance,
            theme::colors::CARD_FILL_DARK_ALPHA,
            theme::colors::CARD_FILL_LIGHT_ALPHA,
        ),
    )?;
    if selected {
        fill_squircle(
            target,
            card,
            theme::radius::CARD,
            theme::colors::ramp_rgba(
                appearance,
                theme::colors::SELECTION_DARK_ALPHA,
                theme::colors::SELECTION_LIGHT_ALPHA,
            ),
        )?;
    }

    let inner = tinycast_pure::layout::list::calc_card_inner_rect(panel_w, y);
    if is_error {
        return paint_error(target, dwrite, fonts, display, inner, appearance);
    }

    let arrow_w = theme::size::HEADER_ICON_SLOT;
    let mid = inner.x + inner.w / 2.0;
    paint_column(
        target,
        fonts,
        expression,
        source_badge,
        D2D_RECT_F {
            left: inner.x,
            top: inner.y,
            right: mid - arrow_w / 2.0,
            bottom: inner.y + inner.h,
        },
        appearance,
    )?;
    let tertiary = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    );
    symbols::paint_fluent_in(
        target,
        dwrite,
        "arrow.right",
        DipRect {
            x: mid - arrow_w / 2.0,
            y: inner.y,
            w: arrow_w,
            h: inner.h,
        },
        tertiary,
    )?;
    paint_column(
        target,
        fonts,
        display,
        target_badge,
        D2D_RECT_F {
            left: mid + arrow_w / 2.0,
            top: inner.y,
            right: inner.x + inner.w,
            bottom: inner.y + inner.h,
        },
        appearance,
    )
}

fn paint_error(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    message: &str,
    inner: DipRect,
    appearance: u8,
) -> windows::core::Result<()> {
    let icon = tinycast_pure::layout::list::calc_error_icon_rect(inner);
    let secondary = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    );
    symbols::paint_fluent_in(target, dwrite, "exclamationmark.triangle", icon, secondary)?;
    let brush = unsafe {
        target.CreateSolidColorBrush(&crate::design_system::appearance::color(secondary), None)?
    };
    draw_text(
        target,
        &fonts.title,
        &brush,
        D2D_RECT_F {
            left: icon.x + icon.w + theme::spacing::MD,
            top: inner.y,
            right: inner.x + inner.w,
            bottom: inner.y + inner.h,
        },
        message,
    )
}

fn paint_column(
    target: &ID2D1RenderTarget,
    fonts: &ListFonts,
    text: &str,
    badge: Option<&str>,
    rect: D2D_RECT_F,
    appearance: u8,
) -> windows::core::Result<()> {
    let pad = theme::spacing::MD;
    let left = rect.left + pad;
    let right = rect.right - pad;
    let primary = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_PRIMARY_ALPHA,
        theme::colors::TEXT_PRIMARY_ALPHA,
    );
    let title = unsafe {
        target.CreateSolidColorBrush(&crate::design_system::appearance::color(primary), None)?
    };
    let value_bottom = if badge.is_some() {
        rect.top + (rect.bottom - rect.top) * 0.58
    } else {
        rect.bottom
    };
    draw_text(
        target,
        &fonts.calc_result,
        &title,
        D2D_RECT_F {
            left,
            top: rect.top,
            right,
            bottom: value_bottom,
        },
        text,
    )?;
    if let Some(badge) = badge {
        let secondary = theme::colors::ramp_rgba(
            appearance,
            theme::colors::TEXT_SECONDARY_ALPHA,
            theme::colors::TEXT_SECONDARY_ALPHA,
        );
        let muted_brush = unsafe {
            target.CreateSolidColorBrush(
                &crate::design_system::appearance::color(secondary),
                None,
            )?
        };
        let pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top: value_bottom + theme::spacing::XXS,
                right: (left + 160.0).min(right),
                bottom: rect.bottom - theme::spacing::XXS,
            },
            radiusX: theme::radius::KEY_CAP,
            radiusY: theme::radius::KEY_CAP,
        };
        fill_squircle(
            target,
            DipRect {
                x: pill.rect.left,
                y: pill.rect.top,
                w: pill.rect.right - pill.rect.left,
                h: pill.rect.bottom - pill.rect.top,
            },
            theme::radius::KEY_CAP,
            theme::colors::ramp_rgba(
                appearance,
                theme::colors::CONTROL_SURFACE_DARK_ALPHA,
                theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
            ),
        )?;
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


