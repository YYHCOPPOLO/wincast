//! Clipboard settings: retention, disabled apps, clear history.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL};

use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 36.0;

const RETENTION_DAYS: [i64; 7] = [1, 7, 30, 90, 180, 365, -1];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardHit {
    Retention,
    Clear,
    RemoveApp(usize),
}

pub fn retention_title(days: i64) -> &'static str {
    match days {
        1 => "1 Day",
        7 => "1 Week",
        30 => "1 Month",
        90 => "3 Months",
        180 => "6 Months",
        365 => "1 Year",
        _ => "Forever",
    }
}

pub fn cycle_retention(days: i64) -> i64 {
    let idx = RETENTION_DAYS
        .iter()
        .position(|d| *d == days)
        .unwrap_or(3);
    RETENTION_DAYS[(idx + 1) % RETENTION_DAYS.len()]
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    retention_days: i64,
    disabled: &[String],
    detail_w: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let origin = theme::spacing::XXL - scroll;
    let inset = theme::spacing::XXL;
    let mut y = origin;
    draw_header(target, formats.header, inset, y, detail_w, "History")?;
    y += 24.0;
    draw_row(
        target,
        formats,
        inset,
        y,
        detail_w,
        "Keep history for",
        retention_title(retention_days),
    )?;
    y += ROW_H + theme::spacing::XL;
    draw_header(
        target,
        formats.header,
        inset,
        y,
        detail_w,
        "Disabled Applications",
    )?;
    y += 24.0;
    if disabled.is_empty() {
        draw_caption(
            target,
            formats.caption,
            inset,
            y,
            detail_w,
            "Copies from these apps are not recorded.",
        )?;
        y += ROW_H;
    } else {
        for name in disabled {
            draw_row(target, formats, inset, y, detail_w, name, "Remove")?;
            y += ROW_H;
        }
    }
    y += theme::spacing::XL;
    draw_header(target, formats.header, inset, y, detail_w, "Clear")?;
    y += 24.0;
    draw_row(
        target,
        formats,
        inset,
        y,
        detail_w,
        "Clear history",
        "Clear",
    )?;
    Ok(())
}

pub fn hit(x: f32, y: f32, scroll: f32, disabled_len: usize) -> Option<ClipboardHit> {
    let origin = theme::spacing::XXL - scroll;
    let mut row_y = origin + 24.0;
    if in_row(y, row_y) && x > theme::spacing::XXL {
        return Some(ClipboardHit::Retention);
    }
    row_y += ROW_H + theme::spacing::XL + 24.0;
    for i in 0..disabled_len.max(1) {
        if disabled_len > 0 && in_row(y, row_y) {
            return Some(ClipboardHit::RemoveApp(i));
        }
        row_y += ROW_H;
    }
    if disabled_len == 0 {
        row_y += 0.0;
    }
    row_y += theme::spacing::XL + 24.0;
    if in_row(y, row_y) {
        return Some(ClipboardHit::Clear);
    }
    let _ = x;
    None
}

fn in_row(y: f32, row_y: f32) -> bool {
    y >= row_y && y < row_y + ROW_H
}

fn draw_header(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    x: f32,
    y: f32,
    w: f32,
    text: &str,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&muted(), None)? };
    draw_text(
        target,
        format,
        &brush,
        D2D_RECT_F {
            left: x,
            top: y,
            right: w - theme::spacing::XXL,
            bottom: y + 20.0,
        },
        text,
    )
}

fn draw_caption(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    x: f32,
    y: f32,
    w: f32,
    text: &str,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&muted(), None)? };
    draw_text(
        target,
        format,
        &brush,
        D2D_RECT_F {
            left: x,
            top: y,
            right: w - theme::spacing::XXL,
            bottom: y + 20.0,
        },
        text,
    )
}

fn draw_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    x: f32,
    y: f32,
    w: f32,
    title: &str,
    trailing: &str,
) -> windows::core::Result<()> {
    let pill = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x - 4.0,
            top: y,
            right: w - theme::spacing::XL,
            bottom: y + ROW_H,
        },
        radiusX: theme::radius::ROW,
        radiusY: theme::radius::ROW,
    };
    let fill = unsafe { target.CreateSolidColorBrush(&row_fill(), None)? };
    unsafe {
        target.FillRoundedRectangle(&pill, &fill);
    }
    let title_brush = unsafe { target.CreateSolidColorBrush(&title_color(), None)? };
    draw_text(
        target,
        formats.body,
        &title_brush,
        D2D_RECT_F {
            left: x + theme::spacing::SM,
            top: y,
            right: w - 140.0,
            bottom: y + ROW_H,
        },
        title,
    )?;
    let trail = unsafe { target.CreateSolidColorBrush(&muted(), None)? };
    draw_text(
        target,
        formats.caption,
        &trail,
        D2D_RECT_F {
            left: w - 130.0,
            top: y,
            right: w - theme::spacing::XXL,
            bottom: y + ROW_H,
        },
        trailing,
    )
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
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn title_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.1,
        g: 0.1,
        b: 0.1,
        a: 1.0,
    }
}

fn muted() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.35,
        g: 0.35,
        b: 0.35,
        a: 1.0,
    }
}

fn row_fill() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.65,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_cycles_through_presets() {
        assert_eq!(cycle_retention(90), 180);
        assert_eq!(retention_title(90), "3 Months");
        assert_eq!(retention_title(-1), "Forever");
    }
}
