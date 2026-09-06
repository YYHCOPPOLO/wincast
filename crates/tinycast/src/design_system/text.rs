use tinycast_pure::palette_placement::DipRect;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE};
use windows::Win32::Graphics::DirectWrite::{IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL};

use super::appearance;

pub fn draw(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    text: &str,
    rect: DipRect,
    rgba: (f32, f32, f32, f32),
) -> windows::core::Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(rgba), None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    let layout = D2D_RECT_F {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.w,
        bottom: rect.y + rect.h,
    };
    unsafe {
        target.DrawText(
            &wide,
            format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

pub fn control_surface(appearance: u8) -> (f32, f32, f32, f32) {
    tinycast_pure::theme::colors::ramp_rgba(
        appearance,
        tinycast_pure::theme::colors::CONTROL_SURFACE_DARK_ALPHA,
        tinycast_pure::theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
    )
}

pub fn primary_ink(appearance: u8) -> (f32, f32, f32, f32) {
    tinycast_pure::theme::colors::ramp_rgba(
        appearance,
        tinycast_pure::theme::colors::TEXT_PRIMARY_ALPHA,
        tinycast_pure::theme::colors::TEXT_PRIMARY_ALPHA,
    )
}

pub fn secondary_ink(appearance: u8) -> (f32, f32, f32, f32) {
    tinycast_pure::theme::colors::ramp_rgba(
        appearance,
        tinycast_pure::theme::colors::TEXT_SECONDARY_ALPHA,
        tinycast_pure::theme::colors::TEXT_SECONDARY_ALPHA,
    )
}

#[allow(dead_code)]
pub fn tertiary_ink(appearance: u8) -> (f32, f32, f32, f32) {
    tinycast_pure::theme::colors::ramp_rgba(
        appearance,
        tinycast_pure::theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        tinycast_pure::theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    )
}

#[allow(dead_code)]
pub fn note_text(appearance: u8) -> (f32, f32, f32, f32) {
    tinycast_pure::theme::colors::ramp_rgba(appearance, 0.90, 0.85)
}

#[allow(dead_code)]
pub fn sheen(appearance: u8) -> (f32, f32, f32, f32) {
    tinycast_pure::theme::colors::ramp_rgba(appearance, 0.04, 0.04)
}

pub const DESTRUCTIVE: (f32, f32, f32, f32) = (0.86, 0.22, 0.22, 1.0);
pub const SUCCESS: (f32, f32, f32, f32) = (0.20, 0.78, 0.35, 1.0);
#[allow(dead_code)]
pub const PROGRESS: (f32, f32, f32, f32) = (0.0, 0.47, 0.83, 1.0);
#[allow(dead_code)]
pub const BRAND: (f32, f32, f32, f32) = (0.525, 0.231, 1.0, 1.0);
