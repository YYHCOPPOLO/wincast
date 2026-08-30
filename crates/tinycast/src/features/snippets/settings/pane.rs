//! Settings → Snippets: feature switch (keyword-expansion consent) and launcher visibility.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::features::launcher::settings::items::{ConfirmCopy, Formats};

const ROW_H: f32 = 52.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

pub const ENABLE_TITLE: &str = "Snippets";
pub const ENABLE_SUBTITLE: &str =
    "Keyword expansion requires listening to keystrokes. Keystrokes stay on this PC.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const ENABLE_CONFIRM_TITLE: &str = "Enable snippets?";
pub const ENABLE_CONFIRM_MESSAGE: &str =
    "Keyword expansion requires the Accessibility permission. Keystrokes stay on this PC.";
pub const ENABLE_CONFIRM_ACTION: &str = "Continue";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnippetsHit {
    Enable,
    ShowInLauncher,
}

pub fn enable_copy() -> ConfirmCopy {
    ConfirmCopy {
        title: ENABLE_CONFIRM_TITLE,
        message: ENABLE_CONFIRM_MESSAGE,
        accept: ENABLE_CONFIRM_ACTION,
        cancel: crate::features::launcher::settings::items::RESET_CONFIRM_CANCEL,
    }
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<SnippetsHit> {
    let y = y + scroll;
    if y >= 24.0 && y < 24.0 + ROW_H {
        return Some(SnippetsHit::Enable);
    }
    if y >= 24.0 + ROW_H + theme::spacing::XL && y < 24.0 + ROW_H * 2.0 + theme::spacing::XL {
        return Some(SnippetsHit::ShowInLauncher);
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
    let origin = -scroll;
    paint_row(
        target,
        formats,
        ENABLE_TITLE,
        ENABLE_SUBTITLE,
        enabled,
        24.0 + origin,
        width,
    )?;
    paint_row(
        target,
        formats,
        SHOW_IN_LAUNCHER,
        "Hide the Snippets section without turning keyword expansion off.",
        show_in_launcher,
        24.0 + ROW_H + theme::spacing::XL + origin,
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
    fn enable_copy_matches_oracle_title() {
        assert_eq!(enable_copy().title, "Enable snippets?");
        assert_eq!(hit(20.0, 30.0, 0.0), Some(SnippetsHit::Enable));
        assert_eq!(
            hit(20.0, 24.0 + ROW_H + theme::spacing::XL + 4.0, 0.0),
            Some(SnippetsHit::ShowInLauncher)
        );
    }
}
