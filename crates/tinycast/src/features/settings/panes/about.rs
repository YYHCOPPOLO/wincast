use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutHit {
    Support,
}

pub fn content_height() -> f32 {
    120.0
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<AboutHit> {
    let y = y + scroll;
    if y >= 24.0 && y < 80.0 {
        Some(AboutHit::Support)
    } else {
        None
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let y = 24.0 - scroll;
    let brush = unsafe { target.CreateSolidColorBrush(&ds::primary_ink(appearance), None)? };
    let t: Vec<u16> = tinycast_pure::i18n::about_product(formats.lang)
        .encode_utf16()
        .collect();
    unsafe {
        target.DrawText(
            &t,
            formats.body,
            &D2D_RECT_F {
                left: theme::spacing::XL,
                top: y,
                right: width - theme::spacing::XL,
                bottom: y + 24.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let s: Vec<u16> = tinycast_pure::i18n::about_support(formats.lang)
        .encode_utf16()
        .collect();
    unsafe {
        target.DrawText(
            &s,
            formats.body,
            &D2D_RECT_F {
                left: theme::spacing::XL,
                top: y + 36.0,
                right: width - theme::spacing::XL,
                bottom: y + 60.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}
