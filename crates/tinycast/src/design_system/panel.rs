use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_GRADIENT_STOP, D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
};

use super::appearance;
use super::fill_squircle;
use super::text;

pub fn paint_panel_scrim(
    target: &ID2D1RenderTarget,
    width: f32,
    height: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    paint_scrim(target, width, height, theme::radius::PANEL, appearance)
}

pub fn paint_scrim(
    target: &ID2D1RenderTarget,
    width: f32,
    height: f32,
    radius: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    unsafe {
        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        target.Clear(Some(&clear));
    }
    let rgba = theme::colors::scrim_rgba(appearance);
    fill_squircle(
        target,
        DipRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        },
        radius,
        rgba,
    )
}

pub fn paint_sheen(
    target: &ID2D1RenderTarget,
    width: f32,
    height: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (r, g, b, a) = text::sheen(appearance);
    let top = appearance::color((r, g, b, a));
    let clear = appearance::color((r, g, b, 0.0));
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: top,
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: clear,
        },
    ];
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: D2D_POINT_2F { x: 0.0, y: 0.0 },
            endPoint: D2D_POINT_2F {
                x: 0.0,
                y: height * 0.45,
            },
        };
        let brush = target.CreateLinearGradientBrush(&props, None, &collection)?;
        target.FillRectangle(
            &D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width,
                bottom: height * 0.45,
            },
            &brush,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn dark_scrim_center_is_black_40() {
        let (w, h, bits) = crate::design_system::test_render::panel_scrim(100, 100, 0)
            .expect("offscreen");
        let i = ((h / 2) * w + (w / 2)) * 4;
        let b = bits[i] as f32;
        let g = bits[i + 1] as f32;
        let r = bits[i + 2] as f32;
        let a = bits[i + 3] as f32 / 255.0;
        assert!(a > 0.30 && a < 0.50, "alpha {a}");
        assert!(r < 8.0 && g < 8.0 && b < 8.0);
        let corner = 0;
        assert_eq!(bits[corner + 3], 0, "outside squircle must be transparent");
    }
}
