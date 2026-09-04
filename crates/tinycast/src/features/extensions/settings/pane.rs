//! Settings → Extensions: original switch, no JS runtime.

use tinycast_pure::extensions::RUNTIME_ABSENT;
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
const NOTICE_H: f32 = 40.0;

pub const ENABLE_TITLE: &str = "Enable extensions";
pub const ENABLE_SUBTITLE: &str =
    "Run Raycast extensions natively. A running command holds a JavaScript engine in memory until you leave it.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const LAUNCHER_SUBTITLE: &str = "List every extension's commands in launcher search.";
pub const RUNTIME_NOTICE: &str = RUNTIME_ABSENT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionsHit {
    Enable,
    ShowInLauncher,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H * 2.0 + theme::spacing::XL * 2.0 + NOTICE_H + 24.0
}

fn row_y(i: usize) -> f32 {
    24.0 + i as f32 * (ROW_H + theme::spacing::XL)
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<ExtensionsHit> {
    let y = y + scroll;
    if y >= row_y(0) && y < row_y(0) + ROW_H {
        return Some(ExtensionsHit::Enable);
    }
    if y >= row_y(1) && y < row_y(1) + ROW_H {
        return Some(ExtensionsHit::ShowInLauncher);
    }
    None
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    show_in_launcher: bool,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    paint_toggle(
        target,
        formats,
        ENABLE_TITLE,
        ENABLE_SUBTITLE,
        enabled,
        true,
        row_y(0) - scroll,
        width,
    )?;
    paint_toggle(
        target,
        formats,
        SHOW_IN_LAUNCHER,
        LAUNCHER_SUBTITLE,
        show_in_launcher,
        false,
        row_y(1) - scroll,
        width,
    )?;
    let notice_y = row_y(2) - scroll;
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.55,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let wide: Vec<u16> = RUNTIME_NOTICE.encode_utf16().collect();
    let pad = theme::spacing::XL;
    unsafe {
        target.DrawText(
            &wide,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: notice_y,
                right: width - pad,
                bottom: notice_y + NOTICE_H,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_toggle(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    disabled: bool,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let text_w = width - pad * 3.0 - TOGGLE_W;
    let title_a = if disabled { 0.45 } else { 0.92 };
    let sub_a = if disabled { 0.32 } else { 0.55 };
    let title_color = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: title_a,
    };
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: sub_a,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&title_color, None)? };
    let title_wide: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title_wide,
            formats.body,
            &D2D_RECT_F {
                left: pad,
                top: y + 8.0,
                right: pad + text_w,
                bottom: y + 28.0,
            },
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
            &D2D_RECT_F {
                left: pad,
                top: y + 28.0,
                right: pad + text_w,
                bottom: y + ROW_H - 4.0,
            },
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
            a: if disabled { 0.35 } else { 1.0 },
        }
    } else {
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: if disabled { 0.08 } else { 0.18 },
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
    fn enable_switch_is_visible_and_runtime_copy_matches_reject() {
        assert_eq!(ENABLE_TITLE, "Enable extensions");
        assert_eq!(SHOW_IN_LAUNCHER, "Show in launcher");
        assert_eq!(RUNTIME_NOTICE, RUNTIME_ABSENT);
        assert_eq!(hit(20.0, 30.0, 0.0), Some(ExtensionsHit::Enable));
        assert_eq!(
            hit(20.0, row_y(1) + 4.0, 0.0),
            Some(ExtensionsHit::ShowInLauncher)
        );
        assert!(content_height() > row_y(1) + ROW_H);
    }
}
