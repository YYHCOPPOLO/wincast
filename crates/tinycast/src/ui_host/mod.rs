pub mod editor;
pub mod hud;
pub mod modal;
pub mod notes;
pub mod onboarding;
pub mod preview;
pub mod result;
pub mod settings;
mod settings_pages;
pub mod snapshot;
pub mod support;

use std::cell::RefCell;
use std::collections::HashSet;

use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::Graphics::Dwm::{DwmEnableBlurBehindWindow, DWM_BB_ENABLE, DWM_BLURBEHIND};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWLP_USERDATA};

thread_local! {
    /// HWNDs whose DWM blur has already been cleared. winit enables the effect
    /// once at creation and never removes it, so every Slint window needs this
    /// exactly once after its HWND exists.
    static BACKDROP_CLEARED: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
}

/// Remove the DWM blur-behind winit installs for a transparent window.
///
/// Slint decides window transparency from the window background *before* the
/// host applies the theme, and also whenever `no-frame` is set. Winit reacts
/// by calling `DwmEnableBlurBehindWindow` with an empty blur region; it never
/// disables the effect when the background later becomes opaque. The effect
/// forces every frame through DWM's alpha compositor (slow scrolling) and
/// hides native child controls such as the palette's search EDIT. Clearing it
/// keeps the window's own per-pixel alpha (rounded card corners still work)
/// while restoring normal composition and native children.
pub(crate) fn clear_backdrop(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    let key = hwnd.0 as isize;
    if !BACKDROP_CLEARED.with(|cleared| cleared.borrow_mut().insert(key)) {
        return;
    }
    let blur = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE,
        fEnable: BOOL(0),
        hRgnBlur: Default::default(),
        fTransitionOnMaximized: BOOL(0),
    };
    unsafe {
        let _ = DwmEnableBlurBehindWindow(hwnd, &blur);
    }
}

/// The AppCore behind the tray host. A modal continuation runs after its caller
/// returned, so it re-enters through this pointer instead of borrowing a caller
/// that is already gone.
pub(crate) fn app_core(host: HWND) -> Option<*mut crate::app_core::AppCore> {
    if host.is_invalid() {
        return None;
    }
    let ptr = unsafe { GetWindowLongPtrW(host, GWLP_USERDATA) } as *mut crate::app_core::AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

/// Slint's platform errors are not `windows::core::Error`s. One conversion keeps
/// the host code from repeating the HRESULT.
pub fn platform_error(error: slint::PlatformError) -> windows::core::Error {
    windows::core::Error::new(
        windows::core::HRESULT(0x8000_4005u32 as i32),
        error.to_string(),
    )
}
