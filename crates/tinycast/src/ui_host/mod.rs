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

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWLP_USERDATA};

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
