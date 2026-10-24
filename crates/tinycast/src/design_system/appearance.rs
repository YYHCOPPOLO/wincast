use std::sync::atomic::{AtomicU8, Ordering};

use windows::Win32::Foundation::COLORREF;

/// Last resolved chrome: 0 Dark, 1 Light. Default Light.
static RESOLVED: AtomicU8 = AtomicU8::new(1);

pub fn set_resolved(key: u8) {
    RESOLVED.store(if key == 0 { 0 } else { 1 }, Ordering::Relaxed);
}

pub fn resolved() -> u8 {
    RESOLVED.load(Ordering::Relaxed)
}

/// The native EDIT's text ink: the theme's `--fg` step, not raw black/white,
/// so the field's text and caret read on the card-2 field box in both
/// appearances. COLORREF is 0x00BBGGRR.
pub fn ink_colorref(appearance: u8) -> COLORREF {
    colorref(tinycast_pure::theme::colors::text_rgba(appearance))
}

pub fn search_background_colorref(appearance: u8) -> COLORREF {
    colorref(tinycast_pure::theme::colors::card2_rgba(appearance))
}

/// Empty-field hint ink: `--fg-2` reads clearly on card-2 in both appearances;
/// the tertiary step was too faint at the 20 DIP search size.
pub fn placeholder_colorref(appearance: u8) -> COLORREF {
    colorref(tinycast_pure::theme::colors::text_secondary_rgba(
        appearance,
    ))
}

fn colorref((red, green, blue, _): (f32, f32, f32, f32)) -> COLORREF {
    COLORREF(
        (red * 255.0).round() as u32
            | ((green * 255.0).round() as u32) << 8
            | ((blue * 255.0).round() as u32) << 16,
    )
}
