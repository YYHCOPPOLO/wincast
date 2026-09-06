use tinycast_pure::layout::palette_chrome::header_icon_rect;
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP,
};

use super::appearance;
use super::fill_squircle;

/// SF Symbol name → Segoe Fluent / MDL2 codepoint.
pub fn fluent_for_sf(sf: &str) -> Option<&'static str> {
    Some(match sf {
        "magnifyingglass" => "\u{E721}",
        "chevron.left" => "\u{E76B}",
        "arrow.right" => "\u{E72A}",
        "exclamationmark.triangle" => "\u{E7BA}",
        _ => return None,
    })
}

pub fn paint_header_glyph(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    symbol: &str,
    appearance: u8,
) -> windows::core::Result<()> {
    let slot = header_icon_rect();
    let Some(glyph) = fluent_for_sf(symbol) else {
        return paint_missing(target, slot, appearance);
    };
    let Ok(format) = fluent_format(dwrite) else {
        return paint_missing(target, slot, appearance);
    };
    let ink = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    );
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(ink), None)? };
    let wide: Vec<u16> = glyph.encode_utf16().collect();
    let layout = D2D_RECT_F {
        left: slot.x,
        top: slot.y,
        right: slot.x + slot.w,
        bottom: slot.y + slot.h,
    };
    unsafe {
        target.DrawText(
            &wide,
            &format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

pub fn paint_fluent_in(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    symbol: &str,
    slot: DipRect,
    rgba: (f32, f32, f32, f32),
) -> windows::core::Result<()> {
    let Some(glyph) = fluent_for_sf(symbol) else {
        return Ok(());
    };
    let format = fluent_format_sized(dwrite, slot.h.max(1.0))?;
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(rgba), None)? };
    let wide: Vec<u16> = glyph.encode_utf16().collect();
    let layout = D2D_RECT_F {
        left: slot.x,
        top: slot.y,
        right: slot.x + slot.w,
        bottom: slot.y + slot.h,
    };
    unsafe {
        target.DrawText(
            &wide,
            &format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn fluent_format(dwrite: &IDWriteFactory) -> windows::core::Result<IDWriteTextFormat> {
    fluent_format_sized(dwrite, theme::typography::HEADER_ICON)
}

fn fluent_format_sized(
    dwrite: &IDWriteFactory,
    size: f32,
) -> windows::core::Result<IDWriteTextFormat> {
    let format = match unsafe {
        dwrite.CreateTextFormat(
            w!("Segoe Fluent Icons"),
            None,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-US"),
        )
    } {
        Ok(format) => format,
        Err(_) => unsafe {
            dwrite.CreateTextFormat(
                w!("Segoe MDL2 Assets"),
                None,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-US"),
            )?
        },
    };
    unsafe {
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
    }
    Ok(format)
}

fn paint_missing(
    target: &ID2D1RenderTarget,
    slot: DipRect,
    appearance: u8,
) -> windows::core::Result<()> {
    let size = theme::typography::HEADER_ICON;
    let rect = DipRect {
        x: slot.x + (slot.w - size) / 2.0,
        y: slot.y + (slot.h - size) / 2.0,
        w: size,
        h: size,
    };
    let rgba = theme::colors::ramp_rgba(appearance, 0.06, 0.06);
    fill_squircle(target, rect, theme::radius::THUMBNAIL, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnifying_glass_maps() {
        assert!(fluent_for_sf("magnifyingglass").is_some());
        assert!(fluent_for_sf("chevron.left").is_some());
        assert!(fluent_for_sf("arrow.right").is_some());
        assert!(fluent_for_sf("exclamationmark.triangle").is_some());
    }
}
