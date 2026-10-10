//! Shared plumbing for nested modal message loops (dialog, camera preview).

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PostQuitMessage, MSG};

/// `GetMessageW` for a nested modal loop. Returns `false` when the loop must
/// stop: on `WM_QUIT` (re-posted so the outer pump in `platform::run` still
/// sees it and the process exits) or on failure (`-1`, which `BOOL::as_bool`
/// would otherwise treat as success).
pub(crate) unsafe fn next_message(msg: &mut MSG) -> bool {
    match GetMessageW(msg, HWND::default(), 0, 0).0 {
        0 => {
            PostQuitMessage(msg.wParam.0 as i32);
            false
        }
        -1 => false,
        _ => true,
    }
}
