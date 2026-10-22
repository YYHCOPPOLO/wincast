//! The palette's Slint window, the window styles winit cannot express, and the
//! native search field parented to it.
//!
//! Everything user-visible inside the frame is a Slint surface; the search
//! field stays a Win32 `EDIT` so the IME candidate window docks to a real
//! caret (phase 0 probe, conclusion 2).
//!
//! winit only creates its HWND once the event loop runs, so `create` shows the
//! window off-screen and finishes the setup on a short timer: window styles,
//! the child `EDIT` and the Win32 subclass.

use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_placement::{compact_size, expanded_size};
use tinycast_pure::theme;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, EN_CHANGE,
    GWL_EXSTYLE, GWLP_USERDATA, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOCOPYBITS,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WM_COMMAND, WM_NCDESTROY, WS_EX_TOOLWINDOW,
};

use super::edit::SearchEdit;
use super::physical;
use crate::app_core::AppCore;
use crate::platform::screens::dip_scalar_to_px;

const SUBCLASS_ID: usize = 1;
const INIT_MS: u64 = 16;
/// winit's `skip_taskbar` leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;

type Palette = tinycast_ui::PaletteWindow;

pub struct PaletteWindow {
    pub hwnd: HWND,
    ui: Palette,
    inner: *mut PaletteInner,
}

struct PaletteInner {
    host: HWND,
    /// Upgradeable handle for the init timer; does not keep the window alive.
    weak: slint::Weak<Palette>,
    edit: Option<SearchEdit>,
    appearance: u8,
    ready: bool,
    /// A show request that arrived before the HWND existed.
    pending_show: Option<physical::Rect>,
}

impl PaletteWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        let ui = Palette::new().map_err(crate::ui_host::platform_error)?;
        let appearance = core_from_host(host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or(0);
        tinycast_ui::apply_appearance(&ui, appearance);
        // No HWND exists until the event loop runs; park the window out of sight.
        ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        ui.show().map_err(crate::ui_host::platform_error)?;
        let inner = Box::into_raw(Box::new(PaletteInner {
            host,
            weak: ui.as_weak(),
            edit: None,
            appearance,
            ready: false,
            pending_show: None,
        }));
        let window = Self {
            hwnd: HWND::default(),
            ui,
            inner,
        };
        arm_init(inner);
        Ok(window)
    }

    pub fn show_at(&self, frame_px: physical::Rect) {
        if !unsafe { (*self.inner).ready } {
            unsafe {
                (*self.inner).pending_show = Some(frame_px);
            }
            return;
        }
        show_frame(&self.ui, self.inner, frame_px);
    }

    pub fn reset_search(&self) {
        self.set_search_text("");
    }

    pub fn set_search_text(&self, text: &str) {
        if let Some(edit) = unsafe { (*self.inner).edit.as_ref() } {
            edit.set_text(text);
        }
    }

    pub fn focus_search(&self) {
        if let Some(edit) = unsafe { (*self.inner).edit.as_ref() } {
            edit.focus();
        }
    }

    pub fn set_search_caret_visible(&self, visible: bool) {
        if let Some(edit) = unsafe { (*self.inner).edit.as_ref() } {
            let _ = if visible {
                unsafe { windows::Win32::UI::WindowsAndMessaging::ShowCaret(edit.hwnd) }
            } else {
                unsafe { windows::Win32::UI::WindowsAndMessaging::HideCaret(edit.hwnd) }
            };
        }
    }

    pub fn invalidate(&self) {
        refresh(&self.ui, self.inner);
    }

    pub fn set_locale(&self, locale: &str) {
        // Slint keeps its own locale for shaping; the search text is a native
        // control, so there is nothing to re-shape here.
        let _ = locale;
        self.invalidate();
    }

    /// Hides immediately. The original `orderOut`s before it pops back to root,
    /// so returning `false` lets `settle_hidden_palette` run right away.
    pub fn hide(&self) -> bool {
        let _ = self.ui.hide();
        if let Some(edit) = unsafe { (*self.inner).edit.as_ref() } {
            edit.set_visible(false);
        }
        false
    }

    pub fn set_expanded(&self, expanded: bool, anchor_px: physical::Point) {
        if !unsafe { (*self.inner).ready } {
            return;
        }
        let frame = frame_at(expanded, anchor_px, unsafe { GetDpiForWindow(self.hwnd) });
        sync_size(&self.ui, expanded);
        place(self.hwnd, frame);
        layout_search(&self.ui, self.inner);
        refresh(&self.ui, self.inner);
    }
}

impl Drop for PaletteWindow {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = RemoveWindowSubclass(
                    self.hwnd,
                    SUBCLASSPROC::Some(palette_subclass),
                    SUBCLASS_ID,
                );
            }
            if !self.inner.is_null() {
                drop(Box::from_raw(self.inner));
                self.inner = std::ptr::null_mut();
            }
        }
        self.hwnd = HWND::default();
    }
}

/// Polls for the HWND winit creates on the first event-loop turn. Re-arms
/// itself until the window is ready, then stops for good.
fn arm_init(inner: *mut PaletteInner) {
    let ptr = inner as usize;
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let inner = ptr as *mut PaletteInner;
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

/// The live handle for this palette, if the component still exists.
fn ui_for(inner: *mut PaletteInner) -> Option<Palette> {
    unsafe { (*inner).weak.upgrade() }
}

/// One-time setup, run as soon as winit has an HWND. Returns `false` while the
/// window is still pending.
fn init_window(ui: &Palette, inner: *mut PaletteInner) -> bool {
    let Some(hwnd) = window_hwnd(ui) else {
        return false;
    };
    apply_tool_window(hwnd);
    let edit = match SearchEdit::create(hwnd, unsafe { (*inner).host }) {
        Ok(edit) => edit,
        Err(_) => return false,
    };
    let subclassed = unsafe {
        SetWindowSubclass(
            hwnd,
            SUBCLASSPROC::Some(palette_subclass),
            SUBCLASS_ID,
            inner as usize,
        )
        .as_bool()
    };
    if !subclassed {
        return false;
    }
    unsafe {
        (*inner).edit = Some(edit);
    }
    true
}

fn window_ready(ui: &Palette, inner: *mut PaletteInner) {
    unsafe {
        (*inner).ready = true;
    }
    sync_size(ui, false);
    refresh(ui, inner);
    layout_search(ui, inner);
    match unsafe { (*inner).pending_show.take() } {
        Some(frame) => show_frame(ui, inner, frame),
        None => {
            let _ = ui.hide();
        }
    }
}

fn show_frame(ui: &Palette, inner: *mut PaletteInner, frame: physical::Rect) {
    let hwnd = window_hwnd(ui).unwrap_or_default();
    sync_size(ui, false);
    place(hwnd, frame);
    if ui.show().is_err() {
        return;
    }
    apply_tool_window(hwnd);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    refresh(ui, inner);
    layout_search(ui, inner);
    if let Some(edit) = unsafe { (*inner).edit.as_ref() } {
        edit.set_visible(true);
        edit.focus();
    }
}

fn frame_at(expanded: bool, anchor_px: physical::Point, dpi: u32) -> physical::Rect {
    let (w_dip, h_dip) = if expanded {
        expanded_size()
    } else {
        compact_size()
    };
    physical::Rect {
        x: anchor_px.x,
        y: anchor_px.y,
        w: dip_scalar_to_px(w_dip, dpi),
        h: dip_scalar_to_px(h_dip, dpi),
    }
}

/// Keeps the Slint window size and the squircle path in step with the frame.
fn sync_size(ui: &Palette, expanded: bool) {
    let (w_dip, h_dip) = if expanded {
        expanded_size()
    } else {
        compact_size()
    };
    ui.set_expanded(expanded);
    ui.set_squircle_commands(tinycast_ui::squircle_commands(
        0.0,
        0.0,
        w_dip,
        h_dip,
        theme::radius::PANEL,
    ));
}

fn layout_search(ui: &Palette, inner: *mut PaletteInner) {
    let trailing = core_from_host(unsafe { (*inner).host })
        .map(|core| unsafe { (*core).search_trailing_width() })
        .unwrap_or(0.0);
    let hwnd = window_hwnd(ui).unwrap_or_default();
    if let Some(edit) = unsafe { (*inner).edit.as_mut() } {
        edit.layout_with_trailing(hwnd, trailing);
    }
}

/// Pulls the whole screen out of `AppCore` and pushes it into the surface.
fn refresh(ui: &Palette, inner: *mut PaletteInner) {
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
    let header_symbol = if core.palette.mode == PaletteMode::Launcher {
        "magnifyingglass"
    } else {
        "chevron.left"
    };
    ui.set_header_symbol(header_symbol.into());
    let panel_h = if ui.get_expanded() {
        expanded_size().1
    } else {
        compact_size().1
    };
    let (scroll, content, _view) = core.scroll_metrics(panel_h);
    ui.set_content_height(content);
    ui.set_scroll_y(scroll);
}

fn window_hwnd(ui: &Palette) -> Option<HWND> {
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

fn place(hwnd: HWND, frame: physical::Rect) {
    unsafe {
        // NOZORDER: restacking on every height step fights DWM and flickers.
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            frame.x,
            frame.y,
            frame.w,
            frame.h,
            SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_NOZORDER,
        );
    }
}

/// `WS_EX_TOOLWINDOW` and the taskbar: winit sets `skip_taskbar`, which does not
/// clear `APPWINDOW`, so the style is corrected once the HWND exists.
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

fn core_from_host(host: HWND) -> Option<*mut AppCore> {
    if host.is_invalid() {
        return None;
    }
    let ptr = unsafe { GetWindowLongPtrW(host, GWLP_USERDATA) } as *mut AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe extern "system" fn palette_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let inner = dwrefdata as *mut PaletteInner;
    match msg {
        // The search field's parent is this window now, so the edit
        // notifications stop here instead of reaching the tray host.
        WM_COMMAND => {
            let notify = ((wparam.0 as u32) >> 16) & 0xffff;
            if notify == EN_CHANGE && !inner.is_null() {
                if let Some(core) = core_from_host((*inner).host) {
                    let edit = HWND(lparam.0 as *mut core::ffi::c_void);
                    (*core).set_query(super::edit::window_text(edit));
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(
                hwnd,
                SUBCLASSPROC::Some(palette_subclass),
                SUBCLASS_ID,
            );
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}
