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

pub fn ink_colorref(appearance: u8) -> COLORREF {
    if appearance == 0 {
        COLORREF(0x00FFFFFF)
    } else {
        COLORREF(0x00000000)
    }
}
