//! Shared plumbing for nested modal message loops (dialog, camera preview).

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PeekMessageW, PostQuitMessage, MSG, PM_REMOVE, WM_KEYFIRST, WM_KEYLAST,
    WM_MOUSEFIRST, WM_MOUSELAST,
};

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

/// Call after a modal surface has closed. Drops only keyboard/mouse input
/// still queued for this thread (typing or clicks that landed while the card
/// was animating out) so it cannot leak into the palette or editor
/// underneath. Everything else stays queued for the main pump: `WM_APP`
/// results posted by worker threads (clipboard, AI, indexes), hotkeys,
/// timers, paint and `WM_QUIT`.
pub(crate) unsafe fn discard_stray_input() {
    let mut msg = MSG::default();
    for (first, last) in [(WM_KEYFIRST, WM_KEYLAST), (WM_MOUSEFIRST, WM_MOUSELAST)] {
        while PeekMessageW(&mut msg, HWND::default(), first, last, PM_REMOVE).as_bool() {}
    }
}
