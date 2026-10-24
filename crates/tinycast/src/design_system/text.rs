use tinycast_pure::theme;

pub fn tertiary_ink(appearance: u8) -> (f32, f32, f32, f32) {
    theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    )
}

pub fn note_text(appearance: u8) -> (f32, f32, f32, f32) {
    theme::colors::ramp_rgba(appearance, 0.90, 0.85)
}
