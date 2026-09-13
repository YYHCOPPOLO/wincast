use std::sync::atomic::{AtomicU8, Ordering};

use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// Last resolved chrome: 0 Dark, 1 Light. Default Light.
static RESOLVED: AtomicU8 = AtomicU8::new(1);

pub fn set_resolved(key: u8) {
    RESOLVED.store(if key == 0 { 0 } else { 1 }, Ordering::Relaxed);
}

pub fn resolved() -> u8 {
    RESOLVED.load(Ordering::Relaxed)
}

/// Straight-alpha `D2D1_COLOR_F`. Direct2D premultiplies into the target.
pub fn color(rgba: (f32, f32, f32, f32)) -> D2D1_COLOR_F {
    let (r, g, b, a) = rgba;
    D2D1_COLOR_F { r, g, b, a }
}

pub fn ink_colorref(appearance: u8) -> COLORREF {
    if appearance == 0 {
        COLORREF(0x00FFFFFF)
    } else {
        COLORREF(0x00000000)
    }
}
