use crate::theme;

pub fn volume_size() -> (f32, f32) {
    (theme::size::HUD_WIDTH, theme::size::HUD_HEIGHT)
}

pub fn message_max_width() -> f32 {
    theme::size::HUD_MAX_WIDTH
}

pub fn edge_offset() -> f32 {
    theme::size::HUD_EDGE_OFFSET
}
