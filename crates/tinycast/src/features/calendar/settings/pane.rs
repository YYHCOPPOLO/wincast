//! Settings → Calendar: enable (consent), auto-join, join window, camera preview.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarHit {
    Enable,
    AutoJoin,
    Camera,
    JoinWindow,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H * 4.0 + theme::spacing::XL * 3.0
}

fn row_y(i: usize) -> f32 {
    24.0 + i as f32 * (ROW_H + theme::spacing::XL)
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<CalendarHit> {
    let y = y + scroll;
    let hits = [
        CalendarHit::Enable,
        CalendarHit::AutoJoin,
        CalendarHit::Camera,
        CalendarHit::JoinWindow,
    ];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    None
}

pub fn join_window_title(minutes: i64) -> &'static str {
    match minutes {
        1 => "1 minute",
        2 => "2 minutes",
        10 => "10 minutes",
        15 => "15 minutes",
        _ => "5 minutes",
    }
}

pub fn cycle_join_window(minutes: i64) -> i64 {
    match minutes {
        1 => 2,
        2 => 5,
        5 => 10,
        10 => 15,
        _ => 1,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    auto_join: bool,
    camera: bool,
    join_minutes: i64,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    paint_toggle(
        target,
        formats,
        "Enable Calendar",
        "Read this PC’s calendar. Off by default.",
        enabled,
        row_y(0) - scroll,
        width,
    )?;
    paint_toggle(
        target,
        formats,
        "Auto-join meetings",
        "Open the join link when a meeting starts. Once per meeting per launch.",
        auto_join,
        row_y(1) - scroll,
        width,
    )?;
    paint_toggle(
        target,
        formats,
        "Camera preview",
        "Optional preview before joining. Deny is not fatal.",
        camera,
        row_y(2) - scroll,
        width,
    )?;
    paint_row(
        target,
        formats,
        "Join window",
        join_window_title(join_minutes),
        row_y(3) - scroll,
        width,
    )?;
    Ok(())
}

fn paint_toggle(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    paint_row(target, formats, title, subtitle, y, width)?;
    let pad = theme::spacing::XL;
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
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(&toggle, &brush);
    }
    Ok(())
}

fn paint_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
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
    let t: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &t,
            formats.body,
            &D2D_RECT_F {
                left: pad,
                top: y + 8.0,
                right: width - pad * 2.0 - TOGGLE_W,
                bottom: y + 28.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let muted_brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let s: Vec<u16> = subtitle.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &s,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: y + 28.0,
                right: width - pad * 2.0 - TOGGLE_W,
                bottom: y + ROW_H - 4.0,
            },
            &muted_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_is_first_row() {
        assert_eq!(hit(20.0, 30.0, 0.0), Some(CalendarHit::Enable));
        assert_eq!(cycle_join_window(5), 10);
    }
}
