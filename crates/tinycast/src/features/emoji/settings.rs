//! Settings → Emoji & Symbols: skin tone.

use tinycast_pure::emoji::EmojiSkinTone;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;

pub fn section_header() -> &'static str {
    "Emoji & Symbols"
}

pub const SKIN_TONE_TITLE: &str = "Skin tone";
pub const SKIN_TONE_SUBTITLE: &str = "Applied to people emoji in Search Emoji.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiHit {
    SkinTone,
}

pub fn content_height() -> f32 {
    ds::form_origin() + ROW_H + 24.0
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<EmojiHit> {
    let y = y + scroll;
    let origin = ds::form_origin();
    if y >= origin && y < origin + ROW_H {
        Some(EmojiHit::SkinTone)
    } else {
        None
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    tone_raw: &str,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let (section, enable, _) =
        ds::feature_switch_section(width, ds::CARD_INSET - scroll, section_header(), false);
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, 0)?;
    let y = enable.y;
    let pad = theme::spacing::XL;
    let tone = EmojiSkinTone::from_raw(tone_raw);
    draw(
        target,
        formats.body,
        SKIN_TONE_TITLE,
        pad,
        y + 8.0,
        width - pad,
        y + 28.0,
        0.92,
    )?;
    draw(
        target,
        formats.caption,
        SKIN_TONE_SUBTITLE,
        pad,
        y + 28.0,
        width - 140.0,
        y + ROW_H - 4.0,
        0.55,
    )?;
    draw(
        target,
        formats.body,
        tone.label(),
        width - 120.0,
        y + 14.0,
        width - pad,
        y + 38.0,
        0.92,
    )?;
    Ok(())
}

fn draw(
    target: &ID2D1RenderTarget,
    format: &windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
    text: &str,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    alpha: f32,
) -> windows::core::Result<()> {
    let color = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: alpha,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&color, None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &D2D_RECT_F {
                left,
                top,
                right,
                bottom,
            },
            &brush,
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
    fn emoji_settings_exposes_skin_tone() {
        assert_eq!(SKIN_TONE_TITLE, "Skin tone");
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0),
            Some(EmojiHit::SkinTone)
        );
        assert_eq!(EmojiSkinTone::from_raw("light").label(), "Light");
    }
}
