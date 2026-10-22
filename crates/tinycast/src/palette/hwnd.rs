//! The palette's Slint window, the window styles winit cannot express, and the
//! native search field parented to it.
//!
//! Everything user-visible inside the frame is a Slint surface; the search
//! field stays a Win32 `EDIT` so the IME candidate window docks to a real
//! caret (phase 0 probe, conclusion 2). The `EDIT` paints only text and caret:
//! `WM_CTLCOLOREDIT` hands it a hollow brush so the scrim and the Slint
//! placeholder show through.
//!
//! winit only creates its HWND once the event loop runs, so `create` shows the
//! window off-screen and finishes the setup on a short timer: window styles,
//! the child `EDIT` and the Win32 subclass.

use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, ModelRc, VecModel};
use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_placement::{compact_size, expanded_size};
use tinycast_pure::palette_state::should_draw_placeholder;
use tinycast_pure::theme;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetStockObject, ScreenToClient, SetBkMode, SetTextColor, HDC, NULL_BRUSH, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetForegroundWindow, GetWindowLongPtrW, KillTimer, PostMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, EN_CHANGE, GA_ROOT,
    GWLP_USERDATA, GWL_EXSTYLE, HWND_TOPMOST, MA_NOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOCOPYBITS, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WA_INACTIVE, WM_ACTIVATE, WM_COMMAND,
    WM_CTLCOLOREDIT, WM_DPICHANGED, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_TIMER, WS_EX_TOOLWINDOW,
};

use super::edit::SearchEdit;
use super::physical;
use crate::app_core::AppCore;
use crate::design_system::symbols::fluent_for_sf;
use crate::features::launcher::ui::list::IconCache;
use crate::platform::messages::WM_RESIGN_PALETTE;
use crate::platform::screens::dip_scalar_to_px;
use crate::ui_host::snapshot::{self, RowImages};

const SUBCLASS_ID: usize = 1;
const RESIGN_TIMER_ID: usize = 2;
const INIT_MS: u64 = 16;
/// Room for the "Tab opens AI chat" hint between the field and the trailing gutter.
const TAB_HINT_WIDTH: f32 = 104.0;
/// winit's `skip_taskbar` leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;
/// The old Direct2D preview drew at most 2000 characters; keep layout bounded.
const PREVIEW_CHARS: usize = 2000;

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
    /// Visible slice only; `AppCore` still owns the whole screen.
    model: Rc<VecModel<tinycast_ui::PaletteRow>>,
    icons: IconCache,
    images: RowImages,
    appearance: u8,
    /// DIP reserved to the right of the native field, last applied.
    trailing: f32,
    /// Hover stays dark until the pointer actually moves over the palette.
    hover_armed: bool,
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
        let inner = Box::into_raw(Box::new(PaletteInner {
            host,
            weak: ui.as_weak(),
            edit: None,
            model: Rc::new(VecModel::default()),
            icons: IconCache::new(),
            images: RowImages::new(),
            appearance,
            // Forces the first layout.
            trailing: -1.0,
            hover_armed: true,
            ready: false,
            pending_show: None,
        }));
        let window = Self {
            hwnd: HWND::default(),
            ui,
            inner,
        };
        // The model is created once: the host replaces its contents, never it,
        // so a scroll position or a hover never resets with the data.
        window
            .ui
            .set_rows(ModelRc::from(unsafe { (*inner).model.clone() }));
        let ptr = inner as usize;
        window.ui.on_row_clicked(move |index, x, y| {
            let inner = ptr as *mut PaletteInner;
            if !inner.is_null() {
                row_clicked(inner, index, x, y);
            }
        });
        let ptr = inner as usize;
        window.ui.on_filter_clicked(move || {
            let inner = ptr as *mut PaletteInner;
            if !inner.is_null() {
                filter_clicked(inner);
            }
        });
        let ptr: usize = inner as usize;
        window.ui.on_scrolled(move |offset| {
            let inner = ptr as *mut PaletteInner;
            if !inner.is_null() {
                scrolled(inner, offset);
            }
        });
        let ptr: usize = inner as usize;
        window.ui.on_primary_clicked(move || {
            let inner = ptr as *mut PaletteInner;
            if inner.is_null() {
                return;
            }
            if let Some(core) = core_from_host(unsafe { (*inner).host }) {
                unsafe {
                    (*core).activate_selected();
                }
            }
        });
        let ptr: usize = inner as usize;
        window.ui.on_actions_clicked(move || {
            let inner = ptr as *mut PaletteInner;
            if inner.is_null() {
                return;
            }
            if let Some(core) = core_from_host(unsafe { (*inner).host }) {
                unsafe {
                    (*core).toggle_actions();
                }
            }
        });
        let ptr: usize = inner as usize;
        window.ui.on_apps_clicked(move || {
            let inner = ptr as *mut PaletteInner;
            if inner.is_null() {
                return;
            }
            apps_clicked(inner);
        });
        // Clicking the header glyph or the placeholder focuses the native field.
        let ptr = inner as usize;
        window.ui.on_header_clicked(move || {
            let inner = ptr as *mut PaletteInner;
            if inner.is_null() {
                return;
            }
            if let Some(edit) = unsafe { (*inner).edit.as_ref() } {
                edit.focus();
            }
        });
        // No HWND exists until the event loop runs; park the window out of sight.
        window.ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        window.ui.show().map_err(crate::ui_host::platform_error)?;
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
        unsafe {
            let _ = KillTimer(self.hwnd, RESIGN_TIMER_ID);
        }
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
        refresh(&self.ui, self.inner);
        layout_search(&self.ui, self.inner);
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
                let _ = KillTimer(self.hwnd, RESIGN_TIMER_ID);
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
    // Showing re-applies winit's own extended style, so the tool-window bit
    // has to be set again afterwards.
    apply_tool_window(hwnd);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    // Hover must stay dark until the pointer moves over the fresh window.
    ui.set_hover_armed(false);
    unsafe {
        (*inner).hover_armed = false;
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
    let hwnd = window_hwnd(ui).unwrap_or_default();
    let trailing = unsafe { (*inner).trailing };
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
    // Slint draws the fluent codepoint, not the SF name.
    let header_symbol = if core.palette.mode == PaletteMode::Launcher {
        "magnifyingglass"
    } else {
        "chevron.left"
    };
    ui.set_header_symbol(fluent_for_sf(header_symbol).unwrap_or_default().into());
    ui.set_empty_symbol(fluent_for_sf("magnifyingglass").unwrap_or_default().into());
    ui.set_arrow_symbol(fluent_for_sf("arrow.right").unwrap_or_default().into());
    ui.set_placeholder(core.search_placeholder().into());
    ui.set_show_placeholder(should_draw_placeholder(&core.palette));
    let hint = core.tab_hint().unwrap_or("");
    ui.set_tab_hint(hint.into());
    // The native field shrinks by whatever sits to its right, the hint included.
    let mut trailing = core.search_trailing_width();
    if !hint.is_empty() {
        trailing += TAB_HINT_WIDTH;
    }
    ui.set_search_trailing(trailing);
    let expanded = ui.get_expanded();
    let panel_h = snapshot::panel_height(expanded);
    let (scroll, content, view) = core.scroll_metrics(panel_h);
    let items = core.launcher_paint_items();
    // Clipboard is the one split screen: 290 DIP list, rule, wrapped preview.
    let preview = core.clipboard_preview();
    let split = preview.is_some() && !items.is_empty();
    ui.set_split(split);
    ui.set_split_x(theme::size::CLIPBOARD_LIST_WIDTH);
    ui.set_preview_text(
        preview
            .unwrap_or_default()
            .chars()
            .take(PREVIEW_CHARS)
            .collect::<String>()
            .into(),
    );
    attach_filter(ui, core);
    let empty = if items.is_empty() && core.ai_chat_messages().is_none() {
        core.empty_results_text().unwrap_or("")
    } else {
        ""
    };
    let dpi = unsafe { GetDpiForWindow(window_hwnd(ui).unwrap_or_default()) };
    let inner_mut = unsafe { &mut *inner };
    let visible = snapshot::visible_rows(
        &items,
        scroll,
        view,
        &mut inner_mut.icons,
        &mut inner_mut.images,
        dpi,
        appearance,
    );
    inner_mut.model.set_vec(visible.rows);
    ui.set_content_height(content.max(visible.content_height));
    // Slint's Flickable offset is negative while scrolled down; AppCore keeps
    // the positive distance-from-top.
    ui.set_scroll_y(-scroll);
    ui.set_empty_text(empty.into());
    attach_footer(ui, core);
    let changed = (unsafe { (*inner).trailing } - trailing).abs() > 0.01;
    if changed {
        unsafe {
            (*inner).trailing = trailing;
        }
        layout_search(ui, inner);
    }
}

/// The clipboard type filter: AppCore owns the label, the open state and the rect.
fn attach_filter(ui: &Palette, core: &AppCore) {
    let filter = if core.palette.mode == PaletteMode::Clipboard {
        core.clipboard_filter_paint()
    } else {
        None
    };
    ui.set_filter_visible(filter.is_some());
    if let Some(button) = filter {
        ui.set_filter_label(button.title.as_str().into());
        ui.set_filter_open(button.open);
        ui.set_filter_x(button.rect.x);
        ui.set_filter_y(button.rect.y);
        ui.set_filter_w(button.rect.w);
        ui.set_filter_h(button.rect.h);
    }
}

/// Footer labels come from the mode; the actions chord is split into two caps.
fn attach_footer(ui: &Palette, core: &AppCore) {
    let footer = core.footer_paint();
    ui.set_primary_label(footer.primary_label.into());
    ui.set_actions_label(footer.actions_label.into());
    ui.set_primary_shortcut("↵".into());
    ui.set_primary_modifier("".into());
    let mut caps = tinycast_pure::palette_menu::ACTIONS_SHORTCUT.split('+');
    let modifier = caps.next().unwrap_or_default();
    let key = caps.next().unwrap_or_default();
    ui.set_actions_modifier(modifier.into());
    ui.set_actions_shortcut(key.into());
    ui.set_show_actions(footer.show_action_group);
}

/// The apps circle opens the same menu the footer circle always did.
fn apps_clicked(inner: *mut PaletteInner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let panel_h = snapshot::panel_height(
        unsafe { (*inner).weak.upgrade() }
            .map(|ui| ui.get_expanded())
            .unwrap_or(false),
    );
    let circle = tinycast_pure::palette_menu::menu_button_rect(panel_h);
    unsafe {
        (*core).pointer_down(
            circle.x + circle.w / 2.0,
            circle.y + circle.h / 2.0,
            theme::size::PANEL_WIDTH,
            panel_h,
            false,
        );
    }
}

/// A row click re-enters the existing pointer path, which owns the selection.
/// Slint hands over the window DIP coordinates, so emoji cells and rows resolve
/// to the same pointer semantics the Direct2D layer had.
fn row_clicked(inner: *mut PaletteInner, _index: i32, x: f32, y: f32) {
    let inner_ref = unsafe { &*inner };
    let Some(core) = core_from_host(inner_ref.host) else {
        return;
    };
    let expanded = inner_ref
        .weak
        .upgrade()
        .map(|ui| ui.get_expanded())
        .unwrap_or(false);
    let panel_h = snapshot::panel_height(expanded);
    unsafe {
        (*core).pointer_down(x, y, theme::size::PANEL_WIDTH, panel_h, false);
    }
}

/// The filter's own rect goes back through the pointer path that owns the menu.
fn filter_clicked(inner: *mut PaletteInner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let Some(button) = (unsafe { (*core).clipboard_filter_paint() }) else {
        return;
    };
    let expanded = unsafe { (*inner).weak.upgrade() }
        .map(|ui| ui.get_expanded())
        .unwrap_or(false);
    let panel_h = snapshot::panel_height(expanded);
    unsafe {
        (*core).pointer_down(
            button.rect.x + button.rect.w / 2.0,
            button.rect.y + button.rect.h / 2.0,
            theme::size::PANEL_WIDTH,
            panel_h,
            false,
        );
    }
}

/// The Flickable reports drags and clamped offsets; `AppCore` stays authoritative.
fn scrolled(inner: *mut PaletteInner, offset: f32) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    // Flickable `content-y` is negative while scrolled down.
    let offset = -offset;
    let current = unsafe { (*core).list_scroll() };
    if (offset - current).abs() < 0.5 {
        return;
    }
    unsafe {
        (*core).set_list_scroll(offset);
    }
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

fn resign_should_hide(palette_visible: bool, foreground_is_self: bool) -> bool {
    palette_visible && !foreground_is_self
}

fn foreground_is_palette_or_child(palette: HWND) -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return false;
    }
    fg == palette || unsafe { GetAncestor(fg, GA_ROOT) == palette }
}

/// Focus left the palette: the original `orderOut`s, so hide now.
fn apply_resign(palette: HWND, inner: *mut PaletteInner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let visible = unsafe { (*core).palette_visible };
    if resign_should_hide(visible, foreground_is_palette_or_child(palette)) {
        unsafe {
            (*core).hide_palette();
        }
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
        // The EDIT paints only its text; the scrim and placeholder are Slint.
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let appearance = if inner.is_null() {
                1
            } else {
                core_from_host((*inner).host)
                    .map(|core| (*core).appearance_key())
                    .unwrap_or(1)
            };
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(
                hdc,
                crate::design_system::appearance::ink_colorref(appearance),
            );
            LRESULT(GetStockObject(NULL_BRUSH).0 as isize)
        }
        WM_MOUSEWHEEL => {
            if !inner.is_null() {
                // Over the clipboard preview the wheel belongs to its Flickable,
                // so hand the message to winit/Slint instead of the list.
                if wheel_over_preview(hwnd, inner, lparam) {
                    return DefSubclassProc(hwnd, msg, wparam, lparam);
                }
                if let Some(core) = core_from_host((*inner).host) {
                    let delta = ((wparam.0 as u32) >> 16) as i16;
                    (*core).scroll_list(delta);
                }
            }
            LRESULT(0)
        }
        // A click anywhere in the palette must not take the keyboard away from
        // the native field: rows, footer and the apps circle all re-focus it.
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            if !inner.is_null() {
                if let Some(edit) = (*inner).edit.as_ref() {
                    edit.focus();
                }
            }
            result
        }
        WM_MOUSEMOVE => {
            if !inner.is_null() && !(*inner).hover_armed {
                (*inner).hover_armed = true;
                if let Some(ui) = (*inner).weak.upgrade() {
                    ui.set_hover_armed(true);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEACTIVATE => {
            let menu_open = if inner.is_null() {
                false
            } else {
                core_from_host((*inner).host)
                    .map(|core| (*core).menu_is_open())
                    .unwrap_or(false)
            };
            if menu_open {
                // Clicking into an open menu must not take focus from the field.
                LRESULT(MA_NOACTIVATE as isize)
            } else {
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        WM_ACTIVATE => {
            if (wparam.0 as u32) & 0xffff == WA_INACTIVE {
                // Defer hide: a tray click deactivates us before WM_TOGGLE_PALETTE.
                let _ = SetTimer(hwnd, RESIGN_TIMER_ID, 1, None);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_TIMER => {
            if wparam.0 == RESIGN_TIMER_ID {
                let _ = KillTimer(hwnd, RESIGN_TIMER_ID);
                let _ = PostMessageW(hwnd, WM_RESIGN_PALETTE, WPARAM(0), LPARAM(0));
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_RESIGN_PALETTE => {
            if !inner.is_null() {
                apply_resign(hwnd, inner);
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            if !inner.is_null() {
                layout_search_of(hwnd, inner);
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).relayout_palette();
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(palette_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

/// True while the pointer sits right of the 290 DIP list column in clipboard mode.
fn wheel_over_preview(hwnd: HWND, inner: *mut PaletteInner, lparam: LPARAM) -> bool {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return false;
    };
    if unsafe { (*core).palette.mode } != PaletteMode::Clipboard {
        return false;
    }
    // WM_MOUSEWHEEL carries screen coordinates.
    let mut point = POINT {
        x: (lparam.0 & 0xffff) as u16 as i16 as i32,
        y: ((lparam.0 >> 16) & 0xffff) as u16 as i16 as i32,
    };
    if !unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
        return false;
    }
    let split = dip_scalar_to_px(theme::size::CLIPBOARD_LIST_WIDTH, unsafe {
        GetDpiForWindow(hwnd)
    });
    point.x > split
}

fn layout_search_of(hwnd: HWND, inner: *mut PaletteInner) {
    let trailing = unsafe { (*inner).trailing };
    if let Some(edit) = unsafe { (*inner).edit.as_mut() } {
        edit.layout_with_trailing(hwnd, trailing);
    }
}

#[cfg(test)]
mod tests {
    use super::resign_should_hide;

    #[test]
    fn resign_hides_only_when_visible_and_focus_left() {
        assert!(resign_should_hide(true, false));
        assert!(!resign_should_hide(true, true));
        assert!(!resign_should_hide(false, false));
        assert!(!resign_should_hide(false, true));
    }

    #[test]
    fn entrance_scale_matches_v0102() {
        assert_eq!(tinycast_pure::motion::ENTER_SCALE, 0.94);
    }
}
