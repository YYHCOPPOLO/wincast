//! Settings → Notes: feature switch.

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

pub const ENABLE_TITLE: &str = "Enable Notes";
pub const ENABLE_SUBTITLE: &str =
    "A local Markdown editor. Off by default; enabling does not create notes until you ask.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotesHit {
    Enable,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H + 24.0
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<NotesHit> {
    let y = y + scroll;
    if y >= 24.0 && y < 24.0 + ROW_H {
        Some(NotesHit::Enable)
    } else {
        None
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let y = 24.0 - scroll;
    let pad = theme::spacing::XL;
    let text_w = width - pad * 3.0 - TOGGLE_W;
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
    let title: Vec<u16> = ENABLE_TITLE.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title,
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
    let sub: Vec<u16> = ENABLE_SUBTITLE.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &sub,
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
    let fill = if enabled {
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
    fn enable_is_hittable() {
        assert_eq!(hit(20.0, 30.0, 0.0), Some(NotesHit::Enable));
    }
}
