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
