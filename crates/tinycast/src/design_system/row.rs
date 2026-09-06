use tinycast_pure::layout::list;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use super::fill_squircle;

pub fn paint_row_fill(
    target: &ID2D1RenderTarget,
    panel_w: f32,
    row_y: f32,
    selected: bool,
    hovered: bool,
    appearance: u8,
) -> windows::core::Result<()> {
    let (dark, light) = if selected {
        (
            theme::colors::SELECTION_DARK_ALPHA,
            theme::colors::SELECTION_LIGHT_ALPHA,
        )
    } else if hovered {
        (
            theme::colors::ROW_HOVER_DARK_ALPHA,
            theme::colors::ROW_HOVER_LIGHT_ALPHA,
        )
    } else {
        return Ok(());
    };
    let rgba = theme::colors::ramp_rgba(appearance, dark, light);
    fill_squircle(
        target,
        list::row_fill_rect(panel_w, row_y),
        theme::radius::ROW,
        rgba,
    )
}

#[cfg(test)]
mod tests {
    use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

    fn render_selected_row() -> Vec<u8> {
        let (_w, _h, bits) = crate::design_system::test_render::with_offscreen(200, 40, |target| {
            unsafe {
                let clear = D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                target.Clear(Some(&clear));
            }
            crate::design_system::paint_row_fill(target, 200.0, 0.0, true, false, 0)
        })
        .expect("offscreen");
        bits
    }

    #[test]
    fn selected_row_is_white_10_on_dark() {
        let bits = render_selected_row();
        let i = (18 * 200 + 100) * 4;
        let a = bits[i + 3] as f32 / 255.0;
        assert!(a > 0.05 && a < 0.20, "selection alpha {a}");
    }
}
