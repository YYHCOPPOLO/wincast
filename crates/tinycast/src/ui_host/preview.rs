//! The Slint camera preview card: Join continues, Cancel drops. Camera deny is
//! not fatal.
//!
//! The card rides the phase-3 modal continuation slot (`claim_external` /
//! `complete_external`), so a confirm dialog and the preview never stack, and
//! the join side effect only runs when the user accepts. The window is the
//! opaque 420×236 card (`size::CAMERA_PREVIEW`); it is created on use, born
//! off-screen so winit makes its HWND, then placed and focused like the
//! dialog card.

use std::cell::Cell;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use tinycast_pure::i18n::{
    camera_ready, camera_unavailable, chrome, join_now_accept, join_now_title, Chrome, UiLang,
};
use tinycast_pure::theme;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WS_EX_TOOLWINDOW,
};

use crate::design_system::appearance;
use crate::ui_host::modal::{self, Pending};

type Surface = tinycast_ui::PreviewWindow;

/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's skip_taskbar leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;

struct PreviewHost {
    ui: Surface,
    closing: Cell<bool>,
}

thread_local! {
    static PREVIEW: Cell<Option<PreviewHost>> = const { Cell::new(None) };
}

pub fn probe_camera_nonfatal() -> bool {
    match windows::Media::Capture::MediaCapture::new() {
        Ok(_) => true,
        Err(_) => true,
    }
}

/// Shows the card and registers the join continuation on the modal slot.
/// Returns false when the card could not be shown (slot busy or surface
/// failure); the caller then joins directly, like the old create failure did.
pub fn present(title: &str, lang: UiLang, on_join: Box<dyn FnOnce()>) -> bool {
    let camera_ok = windows::Media::Capture::MediaCapture::new().is_ok();
    let claim = modal::claim_external(Pending {
        on_accept: on_join,
        on_cancel: Box::new(|| {}),
    });
    if claim.is_err() {
        return false;
    }
    let Some(host) = create(title, camera_ok, lang) else {
        // Nothing was shown, so nothing may run.
        modal::release_external();
        return false;
    };
    PREVIEW.set(Some(host));
    true
}

fn create(title: &str, camera_ok: bool, lang: UiLang) -> Option<PreviewHost> {
    let ui = Surface::new().ok()?;
    tinycast_ui::apply_appearance(&ui, appearance::resolved());
    ui.set_heading(join_now_title(title, lang).into());
    let status = if camera_ok {
        camera_ready(lang)
    } else {
        camera_unavailable(lang)
    };
    ui.set_message(status.into());
    ui.set_accept_label(join_now_accept(lang).into());
    ui.set_cancel_label(chrome(Chrome::Cancel, lang).into());
    ui.on_accepted(|| request_close(true));
    ui.on_cancelled(|| request_close(false));
    // No HWND exists until the event loop runs; park the card out of sight.
    ui.window().set_position(slint::LogicalPosition {
        x: PARKED as f32,
        y: PARKED as f32,
    });
    ui.show().ok()?;
    let weak = ui.as_weak();
    arm_init(weak);
    Some(PreviewHost {
        ui,
        closing: Cell::new(false),
    })
}

/// Polls for the HWND winit creates on the first event-loop turn, then places
/// the card and starts the entrance.
fn arm_init(weak: slint::Weak<Surface>) {
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if init_window(&ui) {
            return;
        }
        arm_init(weak);
    });
}

/// One-time setup, run as soon as winit has an HWND. Returns false while the
/// window is still pending.
fn init_window(ui: &Surface) -> bool {
    let Some(hwnd) = window_hwnd(ui) else {
        return false;
    };
    apply_tool_window(hwnd);
    place(hwnd);
    ui.set_presented(true);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
    }
    ui.invoke_focus_accept();
    true
}

fn window_hwnd(ui: &Surface) -> Option<HWND> {
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::Win32(win32) = handle.as_raw() else {
                return None;
            };
            Some(HWND(win32.hwnd.get() as *mut core::ffi::c_void))
        })
        .flatten()
}

/// Centres the card on the primary work area, a third of the way down.
fn place(hwnd: HWND) {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let screens = crate::platform::screens::screens_px();
    let screen = screens
        .iter()
        .find(|s| s.origin_is_primary)
        .or_else(|| screens.first());
    let (x, y) = match screen {
        Some(s) => {
            let sw = s.work.right - s.work.left;
            let sh = s.work.bottom - s.work.top;
            (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 3)
        }
        None => (100, 100),
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOCOPYBITS,
        );
    }
}

/// WS_EX_TOOLWINDOW and the taskbar: winit sets skip_taskbar, which does not
/// clear APPWINDOW, so the style is corrected once the HWND exists.
fn apply_tool_window(hwnd: HWND) {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (before | WS_EX_TOOLWINDOW.0 as isize) & !WS_EX_APPWINDOW_BIT;
        if next != before {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Plays the exit fade, then drops the card and runs the chosen continuation.
fn request_close(accepted: bool) {
    let closing = PREVIEW.with(|slot| {
        if let Some(host) = slot.take() {
            if host.closing.get() {
                slot.set(Some(host));
                return false;
            }
            host.closing.set(true);
            host.ui.set_exit_fade(0.0);
            slot.set(Some(host));
            true
        } else {
            false
        }
    });
    if !closing {
        return;
    }
    let millis = (theme::duration::EXIT_SECS * 1000.0).round().max(0.0) as u64;
    slint::Timer::single_shot(Duration::from_millis(millis), move || {
        let host = PREVIEW.take();
        drop(host);
        modal::complete_external(accepted);
    });
}
