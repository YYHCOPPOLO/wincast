//! The Slint settings window: creation, the Win32 bits winit cannot express,
//! and the bridge that pushes AppCore's settings state into the surface.
//!
//! winit only creates its HWND once the event loop runs, so `create` is called
//! early from `platform::run` and finishes the setup on a short timer, exactly
//! like the palette host. Until then `show` and `invalidate` are remembered and
//! applied once the window is ready.
//!
//! Closing hides the window (Slint's default close response); the component is
//! kept alive by `SettingsWindow`, so opening Settings again shows the same
//! window with its current tab and scroll positions.

use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use tinycast_pure::i18n::settings_tab_title;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsWindow, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOP, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use crate::ui_host::app_core as core_from_host;

type Settings = tinycast_ui::SettingsWindow;

const SUBCLASS_ID: usize = 0x53;
/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's `skip_taskbar` leaves this bit set, which would hide the settings
/// window from the taskbar; the styles are corrected once the HWND exists.
const WS_EX_APPWINDOW_BIT: isize = WS_EX_APPWINDOW.0 as isize;

pub struct SettingsWindow {
    ui: Settings,
    inner: *mut Inner,
    /// Kept for the call sites the Direct2D window had. It stays invalid until
    /// the event loop creates the Slint window; use `hwnd()` when a real handle
    /// is needed (a file-dialog owner, for example).
    pub hwnd: HWND,
}

struct Inner {
    host: HWND,
    /// Upgradeable handle for the init timer; does not keep the window alive.
    weak: slint::Weak<Settings>,
    ready: bool,
    /// A show request that arrived before the HWND existed.
    pending_show: bool,
    /// The first show centres the window; later shows keep where the user left it.
    placed: bool,
    appearance: u8,
}

impl SettingsWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        let ui = Settings::new().map_err(crate::ui_host::platform_error)?;
        let appearance = core_from_host(host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or(1);
        tinycast_ui::apply_appearance(&ui, appearance);
        let inner = Box::into_raw(Box::new(Inner {
            host,
            weak: ui.as_weak(),
            ready: false,
            pending_show: false,
            placed: false,
            appearance,
        }));
        let window = Self {
            ui,
            inner,
            hwnd: HWND::default(),
        };
        // Closing hides the surface; the component and its state stay alive.
        let ptr = inner as usize;
        window.ui.window().on_close_requested(move || {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                close_requested(inner);
            }
            slint::CloseRequestResponse::HideWindow
        });
        // No HWND exists until the event loop runs; park the window off-screen.
        window.ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        window.ui.show().map_err(crate::ui_host::platform_error)?;
        arm_init(inner);
        Ok(window)
    }

    pub fn show(&self) {
        if !unsafe { (*self.inner).ready } {
            unsafe {
                (*self.inner).pending_show = true;
            }
            return;
        }
        show_impl(&self.ui, self.inner);
    }

    pub fn invalidate(&self) {
        refresh(&self.ui, self.inner);
    }

    pub fn set_locale(&self, locale: &str) {
        // Slint shapes with the system locale and every string is supplied by
        // the host, so there is no per-window text format to rebuild here.
        let _ = locale;
        self.invalidate();
    }

    pub fn on_tab_changed(&self) {
        refresh(&self.ui, self.inner);
    }

    /// Phase 7 has no alias or AI connection drafts; phase 8's editors will
    /// restore the real commit. The AppCore call site stays as it is.
    pub fn commit_pending(&self) -> bool {
        true
    }

    /// The real HWND once the event loop has created the Slint window.
    pub fn hwnd(&self) -> HWND {
        window_hwnd(&self.ui).unwrap_or_default()
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        let hwnd = self.hwnd();
        unsafe {
            // The event loop may have destroyed the window first; only unhook a
            // live HWND.
            if !hwnd.is_invalid() && IsWindow(hwnd).as_bool() {
                let _ =
                    RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(settings_subclass), SUBCLASS_ID);
            }
            if !self.inner.is_null() {
                drop(Box::from_raw(self.inner));
                self.inner = std::ptr::null_mut();
            }
        }
    }
}

/// Polls for the HWND winit creates on the first event-loop turn. Re-arms
/// itself until the window is ready, then stops for good.
fn arm_init(inner: *mut Inner) {
    let ptr = inner as usize;
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let inner = ptr as *mut Inner;
        if inner.is_null() {
            return;
        }
        let Some(ui) = ui_for(inner) else {
            return;
        };
        if unsafe { (*inner).ready } {
            return;
        }
        if !init_window(&ui, inner) {
            arm_init(inner);
            return;
        }
        window_ready(&ui, inner);
    });
}

/// The live handle for this window, if the component still exists.
fn ui_for(inner: *mut Inner) -> Option<Settings> {
    unsafe { (*inner).weak.upgrade() }
}

/// One-time setup, run as soon as winit has an HWND. Returns `false` while the
/// window is still pending.
fn init_window(ui: &Settings, inner: *mut Inner) -> bool {
    let Some(hwnd) = window_hwnd(ui) else {
        return false;
    };
    apply_taskbar_window(hwnd);
    if !unsafe {
        SetWindowSubclass(
            hwnd,
            SUBCLASSPROC::Some(settings_subclass),
            SUBCLASS_ID,
            inner as usize,
        )
        .as_bool()
    } {
        return false;
    }
    true
}

fn window_ready(ui: &Settings, inner: *mut Inner) {
    unsafe {
        (*inner).ready = true;
    }
    // The window was shown off-screen only so winit would create its HWND.
    let pending = unsafe { (*inner).pending_show };
    if pending {
        unsafe {
            (*inner).pending_show = false;
        }
        show_impl(ui, inner);
    } else {
        let _ = ui.hide();
    }
}

fn show_impl(ui: &Settings, inner: *mut Inner) {
    refresh(ui, inner);
    if ui.show().is_err() {
        return;
    }
    let hwnd = window_hwnd(ui).unwrap_or_default();
    if hwnd.is_invalid() {
        return;
    }
    // Showing re-applies winit's own extended style, so the taskbar bit has to
    // be set again afterwards.
    apply_taskbar_window(hwnd);
    if !unsafe { (*inner).placed } {
        unsafe {
            (*inner).placed = true;
        }
        center(hwnd);
    }
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Pulls the selected tab out of AppCore and pushes the strings the shell
/// draws. The sidebar and pages extend this in 7.2/7.3.
fn refresh(ui: &Settings, inner: *mut Inner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let core = unsafe { &*core };
    let appearance = core.appearance_key();
    if unsafe { (*inner).appearance } != appearance {
        unsafe {
            (*inner).appearance = appearance;
        }
        tinycast_ui::apply_appearance(ui, appearance);
    }
    let lang = core.ui_lang();
    let tab = core.settings_tab;
    let title = settings_tab_title(tab, lang);
    ui.set_window_title(title.into());
    ui.set_heading(title.into());
}

/// The close button only hides: any draft stays, and the window keeps its HWND.
fn close_requested(_inner: *mut Inner) {}

/// WS_EX_APPWINDOW and the taskbar: winit sets skip_taskbar, so the settings
/// window gets its own taskbar button only after clearing TOOLWINDOW.
fn apply_taskbar_window(hwnd: HWND) {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (before | WS_EX_APPWINDOW_BIT) & !(WS_EX_TOOLWINDOW.0 as isize);
        if next != before {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Centres the window on the monitor under the pointer, like the old window.
fn center(hwnd: HWND) {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    let screens = crate::platform::screens::screens_px();
    let screen = crate::platform::screens::target_screen_from_cursor_px((pt.x, pt.y), &screens);
    let (x, y) = match screen {
        Some(s) => {
            let sw = s.work.right - s.work.left;
            let sh = s.work.bottom - s.work.top;
            (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 2)
        }
        None => (100, 100),
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

fn window_hwnd(ui: &Settings) -> Option<HWND> {
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

unsafe extern "system" fn settings_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(settings_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}
