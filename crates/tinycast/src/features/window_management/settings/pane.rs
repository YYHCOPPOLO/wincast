//! Settings → Window Management: enable, show in launcher, gap, cycle on repeat.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

pub const ENABLE_TITLE: &str = "Window Management";
pub const ENABLE_SUBTITLE: &str = "Move and resize the frontmost window. Off by default.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const CYCLE_TITLE: &str = "Cycle halves on repeat";
pub const GAP_TITLE: &str = "Gap";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowHit {
    Enable,
    ShowInLauncher,
    Cycle,
    Gap,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H * 4.0 + theme::spacing::XL * 3.0
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<WindowHit> {
    let y = y + scroll;
    let mut row = 24.0;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::Enable);
    }
    row += ROW_H + theme::spacing::XL;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::ShowInLauncher);
    }
    row += ROW_H + theme::spacing::XL;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::Cycle);
    }
    row += ROW_H + theme::spacing::XL;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::Gap);
    }
    None
}

pub fn cycle_gap(current: i32) -> i32 {
    match current {
        0 => 8,
        8 => 16,
        16 => 24,
        _ => 0,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    show_in_launcher: bool,
    cycle: bool,
    gap: i32,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let origin = -scroll;
    let mut y = 24.0 + origin;
    paint_row(target, formats, ENABLE_TITLE, ENABLE_SUBTITLE, enabled, y, width)?;
    y += ROW_H + theme::spacing::XL;
    paint_row(
        target,
        formats,
        SHOW_IN_LAUNCHER,
        "Hide the Window Management section without disabling shortcuts.",
        show_in_launcher,
        y,
        width,
    )?;
    y += ROW_H + theme::spacing::XL;
    paint_row(
        target,
        formats,
        CYCLE_TITLE,
        "Repeated Left/Right/Top/Bottom Half cycles ½ → ⅓ → ⅔.",
        cycle,
        y,
        width,
    )?;
    y += ROW_H + theme::spacing::XL;
    paint_row(
        target,
        formats,
        GAP_TITLE,
        &format!("{gap} pt between tiles and screen edges."),
        gap > 0,
        y,
        width,
    )?;
    Ok(())
}

fn paint_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let text_w = width - pad * 3.0 - TOGGLE_W;
    let title_rect = D2D_RECT_F {
        left: pad,
        top: y + 8.0,
        right: pad + text_w,
        bottom: y + 28.0,
    };
    let sub_rect = D2D_RECT_F {
        left: pad,
        top: y + 28.0,
        right: pad + text_w,
        bottom: y + ROW_H - 4.0,
    };
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    };
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.55,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&white, None)? };
    let title_wide: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title_wide,
            formats.body,
            &title_rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let muted_brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let sub_wide: Vec<u16> = subtitle.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &sub_wide,
            formats.caption,
            &sub_rect,
            &muted_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let toggle = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: width - pad - TOGGLE_W,
            top: y + (ROW_H - TOGGLE_H) / 2.0,
            right: width - pad,
            bottom: y + (ROW_H - TOGGLE_H) / 2.0 + TOGGLE_H,
        },
        radiusX: TOGGLE_H / 2.0,
        radiusY: TOGGLE_H / 2.0,
    };
    let fill = if on {
        D2D1_COLOR_F {
            r: 0.2,
            g: 0.55,
            b: 1.0,
            a: 1.0,
        }
    } else {
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.18,
        }
    };
    let toggle_brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(&toggle, &toggle_brush);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_management_hits_four_rows() {
        assert_eq!(hit(20.0, 30.0, 0.0), Some(WindowHit::Enable));
        assert_eq!(cycle_gap(0), 8);
        assert_eq!(cycle_gap(24), 0);
    }
}
