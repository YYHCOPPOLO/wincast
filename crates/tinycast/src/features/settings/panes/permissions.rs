//! Settings → Permissions. No prompt at launch; Open Settings uses Windows URIs.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 56.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionHit {
    Open(usize),
}

pub struct PermissionRow {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub uri: &'static str,
}

pub const ROWS: &[PermissionRow] = &[
    PermissionRow {
        title: "UI Automation",
        subtitle: "Used to paste into the frontmost app.",
        uri: "ms-settings:privacy-general",
    },
    PermissionRow {
        title: "Low-level keyboard hook",
        subtitle: "Snippets, double-tap, and Hyper. Not prompted at launch.",
        uri: "ms-settings:privacy",
    },
    PermissionRow {
        title: "Calendar",
        subtitle: "Meetings stay empty until Calendar access is granted.",
        uri: "ms-settings:privacy-calendar",
    },
    PermissionRow {
        title: "Camera",
        subtitle: "Camera preview for joining meetings.",
        uri: "ms-settings:privacy-webcam",
    },
];

pub fn content_height() -> f32 {
    24.0 + ROW_H * ROWS.len() as f32 + 24.0
}

pub fn hit(x: f32, y: f32, scroll: f32, width: f32) -> Option<PermissionHit> {
    let y = y + scroll;
    let pad = theme::spacing::XL;
    for i in 0..ROWS.len() {
        let top = 24.0 + i as f32 * ROW_H;
        if y >= top && y < top + ROW_H && x > width - pad - 120.0 {
            return Some(PermissionHit::Open(i));
        }
    }
    None
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let origin = -scroll;
    let pad = theme::spacing::XL;
    let brush = unsafe { target.CreateSolidColorBrush(&ds::primary_ink(appearance), None)? };
    let muted_brush =
        unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    for (i, _) in ROWS.iter().enumerate() {
        let y = 24.0 + i as f32 * ROW_H + origin;
        let title: Vec<u16> = tinycast_pure::i18n::permission_title(i, formats.lang)
            .encode_utf16()
            .collect();
        let sub: Vec<u16> = tinycast_pure::i18n::permission_subtitle(i, formats.lang)
            .encode_utf16()
            .collect();
        let open: Vec<u16> = tinycast_pure::i18n::open_settings_action(formats.lang)
            .encode_utf16()
            .collect();
        unsafe {
            target.DrawText(
                &title,
                formats.body,
                &D2D_RECT_F {
                    left: pad,
                    top: y,
                    right: width - pad - 130.0,
                    bottom: y + 28.0,
                },
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            target.DrawText(
                &sub,
                formats.caption,
                &D2D_RECT_F {
                    left: pad,
                    top: y + 28.0,
                    right: width - pad - 130.0,
                    bottom: y + ROW_H - 4.0,
                },
                &muted_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            target.DrawText(
                &open,
                formats.body,
                &D2D_RECT_F {
                    left: width - pad - 120.0,
                    top: y + 16.0,
                    right: width - pad,
                    bottom: y + 40.0,
                },
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_rows_have_settings_uris() {
        assert_eq!(ROWS.len(), 4);
        assert!(ROWS.iter().all(|r| r.uri.starts_with("ms-settings:")));
        assert_eq!(hit(400.0, 30.0, 0.0, 500.0), Some(PermissionHit::Open(0)));
    }
}
