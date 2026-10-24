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
    if appearance == 0 {
        // Dark fg #EDEDF1.
        COLORREF(0x00F1EDED)
    } else {
        // Light fg #1C1C21.
        COLORREF(0x0021_1C1C)
    }
}
