use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS,
};

use super::appearance;
use super::fonts::Fonts;
use super::squircle::{fill_squircle, stroke_squircle};

pub fn paint_keycap(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    text: &str,
    right: f32,
    row_y: f32,
    row_h: f32,
    outline: bool,
    appearance: u8,
) -> windows::core::Result<f32> {
    let pad = theme::spacing::XS;
    let min_w = theme::size::KEY_CAP;
    let cap_h = theme::size::KEY_CAP;
    let text_w = measure_width(fonts, text, 80.0, cap_h);
    let cap_w = (text_w + pad * 2.0).max(min_w);
    let cap_x = right - cap_w;
    let cap_y = row_y + (row_h - cap_h) / 2.0;
    let rect = DipRect {
        x: cap_x,
        y: cap_y,
        w: cap_w,
        h: cap_h,
    };
    if outline {
        let rgba = theme::colors::ramp_rgba(
            appearance,
            theme::colors::BORDER_DARK_ALPHA,
            theme::colors::BORDER_LIGHT_ALPHA,
        );
        stroke_squircle(target, rect, theme::radius::KEY_CAP, rgba, theme::size::HAIRLINE)?;
    } else {
        let rgba = theme::colors::ramp_rgba(
            appearance,
            theme::colors::CONTROL_SURFACE_DARK_ALPHA,
            theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
        );
        fill_squircle(target, rect, theme::radius::KEY_CAP, rgba)?;
    }

    let ink = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    );
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(ink), None)? };
    let mut text_top = cap_y;
    if text.contains('↵') {
        text_top += 1.1;
    }
    let layout = D2D_RECT_F {
        left: cap_x,
        top: text_top,
        right: cap_x + cap_w,
        bottom: text_top + cap_h,
    };
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            &fonts.keycap,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(cap_w)
}

#[cfg(test)]
mod tests {
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, DWRITE_FACTORY_TYPE_SHARED,
    };

    #[test]
    fn keycap_paints_chip_width() {
        let (_w, _h, bits) =
            crate::design_system::test_render::with_offscreen(200, 40, |target| {
                let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
                let fonts = crate::design_system::Fonts::new(&dwrite)?;
                let w = crate::design_system::paint_keycap(
                    target, &fonts, "↵", 180.0, 0.0, 36.0, true, 0,
                )?;
                assert!(w >= tinycast_pure::theme::size::KEY_CAP);
                Ok(())
            })
            .expect("keycap");
        assert!(!bits.is_empty());
    }
}

fn measure_width(fonts: &Fonts, text: &str, max_w: f32, h: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let Ok(layout) = fonts
            .dwrite
            .CreateTextLayout(&wide, &fonts.keycap, max_w.max(1.0), h)
        else {
            return 0.0;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        if layout.GetMetrics(&mut metrics).is_err() {
            return 0.0;
        }
        metrics.width
    }
}
