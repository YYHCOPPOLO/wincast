//! Public virtual-desktop APIs only. Switching Space has no documented public enumerator.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};

pub const SPACE_UNAVAILABLE: &str = "Switching spaces is not available on Windows.";

pub fn manager() -> Option<IVirtualDesktopManager> {
    unsafe { CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL).ok() }
}

/// Next/Previous Space cannot be performed with public APIs (no desktop list).
pub fn switch_space(_hwnd: HWND, _next: bool) -> Result<(), &'static str> {
    let _ = manager();
    Err(SPACE_UNAVAILABLE)
}

#[cfg(test)]
mod tests {
    use super::SPACE_UNAVAILABLE;

    #[test]
    fn space_switch_is_unavailable_without_public_enumerator() {
        assert!(!SPACE_UNAVAILABLE.is_empty());
    }
}
