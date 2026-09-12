use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP};
use windows::Win32::Graphics::DirectWrite::{IDWriteFactory, DWRITE_MEASURING_MODE_NATURAL};

use crate::design_system::settings as ds;
use crate::design_system::symbols;
use crate::features::launcher::settings::items::Formats;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutHit {
    Support,
}

const ICON: f32 = 88.0;

pub fn version_line() -> String {
    format!("Version {}", env!("CARGO_PKG_VERSION"))
}

pub fn copyright_line() -> &'static str {
    "© 2026 Abue Ammar · AGPL-3.0"
}

pub fn content_height() -> f32 {
    420.0
}

pub fn support_rect(width: f32, scroll: f32) -> DipRect {
    let pad = ds::content_pad();
    DipRect {
        x: pad,
        y: 248.0 - scroll,
        w: (width - pad * 2.0).max(80.0),
        h: ds::ROW_H,
    }
}

pub fn hit(x: f32, y: f32, scroll: f32, width: f32) -> Option<AboutHit> {
    let r = support_rect(width, scroll);
    if x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h {
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
    dwrite: &IDWriteFactory,
) -> windows::core::Result<()> {
    let pad = ds::content_pad();
    let lang = formats.lang;
    let icon = DipRect {
        x: (width - ICON) / 2.0,
        y: 24.0 - scroll,
        w: ICON,
        h: ICON,
    };
    symbols::paint_fluent_in(
        target,
        dwrite,
        "heart",
        icon,
        crate::design_system::text::BRAND,
    )?;
    let title_top = icon.y + icon.h + theme::spacing::XL;
    draw(
        target,
        formats.body,
        tinycast_pure::i18n::about_product(lang),
        pad,
        title_top,
        width - pad,
        title_top + 24.0,
        appearance,
        true,
    )?;
    let ver = version_line();
    draw(
        target,
        formats.caption,
        &ver,
        pad,
        title_top + 28.0,
        width - pad,
        title_top + 48.0,
        appearance,
        false,
    )?;
    draw(
        target,
        formats.caption,
        tinycast_pure::i18n::support_keeps_independent(lang),
        pad,
        title_top + 52.0,
        width - pad,
        title_top + 92.0,
        appearance,
        false,
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::about_support(lang),
        tinycast_pure::i18n::support_built_with_love(lang),
        support_rect(width, scroll).y,
        width,
        true,
        appearance,
        ds::RowTrailing::None,
    )?;
    let copy_y = 360.0 - scroll;
    draw(
        target,
        formats.caption,
        copyright_line(),
        pad,
        copy_y,
        width - pad,
        copy_y + 20.0,
        appearance,
        false,
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
    appearance: u8,
    primary: bool,
) -> windows::core::Result<()> {
    let color = if primary {
        ds::primary_ink(appearance)
    } else {
        ds::secondary_ink(appearance)
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
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_row_is_hittable() {
        let width = 420.0;
        let r = support_rect(width, 0.0);
        assert_eq!(
            hit(r.x + 4.0, r.y + 4.0, 0.0, width),
            Some(AboutHit::Support)
        );
        assert_eq!(hit(1.0, 1.0, 0.0, width), None);
        assert!(content_height() > r.y + r.h);
    }

    #[test]
    fn about_shows_version_support_and_copyright() {
        let ver = version_line();
        assert!(ver.contains(env!("CARGO_PKG_VERSION")));
        assert!(copyright_line().contains("AGPL-3.0"));
        assert!(copyright_line().contains("Abue Ammar"));
        assert_eq!(support_rect(420.0, 0.0).h, ds::ROW_H);
    }
}
