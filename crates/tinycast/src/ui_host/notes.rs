//! The Slint Notes window: the 52 DIP chrome and the footer are Slint, the
//! body keeps the native RichEdit child and the switcher keeps a native EDIT
//! (probe conclusion 2: the IME needs real EDIT controls). The switcher itself
//! is the 300×240 frameless Slint popup.
//!
//! The host subclasses each Slint HWND (its `dwRefData` only — winit owns
//! `GWLP_USERDATA`): `WM_SIZE` lays the children out, `WM_COMMAND`/`EN_CHANGE`
//! drives the 300 ms debounced save, the footer count and the cue, and the
//! `WM_TIMER` flush keeps the last keystrokes. The frame position persists to
//! `notes-window.json` like the Direct2D window's did. Because winit only
//! makes the HWND once the event loop runs, `set_body`/`set_title` written
//! before then are buffered and applied at init.

use slint::Weak;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Model, VecModel};
use tinycast_pure::i18n::UiLang;
use tinycast_pure::theme;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, SetBkColor, SetTextColor, HBRUSH,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetClientRect, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindow,
    KillTimer, MoveWindow, SendMessageW, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow,
    EN_CHANGE, HTTRANSPARENT, HWND_TOP, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE,
    SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DPICHANGED,
    WM_KEYDOWN, WM_MOUSEWHEEL, WM_NCDESTROY, WM_NCHITTEST, WM_SIZE, WM_TIMER, WS_CHILD, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL,
};

use crate::platform::paths;
use crate::platform::screens::dip_scalar_to_px;
use crate::ui_host::app_core as core_from_host;

const NOTES_SUBCLASS_ID: usize = 0x61;
const SWITCHER_SUBCLASS_ID: usize = 0x62;
const EDIT_SUBCLASS_ID: usize = 0x63;
const SWITCHER_EDIT_SUBCLASS_ID: usize = 0x64;
const CUE_SUBCLASS_ID: usize = 0x65;

const ID_EDIT: usize = 101;
const ID_CUE: usize = 105;
const ID_SWITCHER_EDIT: usize = 201;
pub const SAVE_TIMER: usize = 1;
const SAVE_MS: u32 = 300;
pub const SCF_DEFAULT: usize = 0;
pub const SCF_ALL: usize = 4;
pub const CHARFORMAT_WPARAMS: [usize; 2] = [SCF_DEFAULT, SCF_ALL];
const ES_MULTILINE: WINDOW_STYLE = WINDOW_STYLE(0x0004);
const ES_AUTOVSCROLL: WINDOW_STYLE = WINDOW_STYLE(0x0040);
const ES_WANTRETURN: WINDOW_STYLE = WINDOW_STYLE(0x1000);

/// The switcher keeps the 300×240 popup size (`theme::size::NOTE_SWITCHER`).
pub const SWITCHER_WIDTH: i32 = theme::size::NOTE_SWITCHER.0 as i32;
pub const SWITCHER_HEIGHT: i32 = theme::size::NOTE_SWITCHER.1 as i32;

/// Old switcher EDIT geometry: 8 DIP gutter, 24 DIP field.
const SWITCHER_PAD_DIP: f32 = 8.0;
const SWITCHER_EDIT_H_DIP: f32 = 24.0;

/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// Both surfaces are born off-screen so winit can make their HWNDs.
const PARKED: i32 = -32_000;

pub struct NotesWindow {
    ui: tinycast_ui::NotesWindow,
    switcher: tinycast_ui::NoteSwitcherWindow,
    host: HWND,
    inner: *mut Inner,
}

struct Inner {
    host: HWND,
    notes: Weak<tinycast_ui::NotesWindow>,
    switcher: Weak<tinycast_ui::NoteSwitcherWindow>,
    edit: HWND,
    cue: HWND,
    cue_brush: HBRUSH,
    switcher_edit: HWND,
    switcher_open: bool,
    selected: usize,
    ready: bool,
    pending_show: bool,
    lang: UiLang,
    /// The switcher list rows; the model updates in place.
    entries: Rc<VecModel<tinycast_ui::NoteEntry>>,
    /// Buffered until the HWND exists (`create` returns before that).
    pending_title: String,
    pending_body: String,
    body_len: usize,
}

impl NotesWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let _ = LoadLibraryW(w!("Msftedit.dll"));
        }
        let ui = tinycast_ui::NotesWindow::new().map_err(crate::ui_host::platform_error)?;
        let switcher =
            tinycast_ui::NoteSwitcherWindow::new().map_err(crate::ui_host::platform_error)?;
        let appearance = core_from_host(host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or(1);
        tinycast_ui::apply_appearance(&ui, appearance);
        tinycast_ui::apply_appearance(&switcher, appearance);
        let lang = core_from_host(host)
            .map(|core| unsafe { (*core).ui_lang() })
            .unwrap_or(UiLang::ZhHans);
        let entries = Rc::new(VecModel::default());
        switcher.set_entries(slint::ModelRc::from(entries.clone()));
        switcher.set_empty_glyph("rectangle.stack".into());
        let cue_brush = unsafe { CreateSolidBrush(note_bg_colorref(appearance)) };
        let inner = Box::into_raw(Box::new(Inner {
            host,
            notes: ui.as_weak(),
            switcher: switcher.as_weak(),
            edit: HWND::default(),
            cue: HWND::default(),
            cue_brush,
            switcher_edit: HWND::default(),
            switcher_open: false,
            selected: 0,
            ready: false,
            pending_show: false,
            lang,
            entries,
            pending_title: String::new(),
            pending_body: String::new(),
            body_len: 0,
        }));
        let window = Self {
            ui,
            switcher,
            host,
            inner,
        };

        // No HWND exists until the event loop runs; park both surfaces.
        park(&window.ui);
        park(&window.switcher);
        window.ui.show().map_err(crate::ui_host::platform_error)?;
        window
            .switcher
            .show()
            .map_err(crate::ui_host::platform_error)?;

        let ptr = window.inner as usize;
        window.switcher.on_picked(move |index| {
            picked(ptr, index.max(0) as usize);
        });
        window.ui.on_create_clicked(move || {
            if let Some(core) = core_from_host(host_of(ptr)) {
                unsafe {
                    (*core).notes_create();
                }
            }
        });
        window.ui.on_switcher_clicked(move || {
            if let Some(core) = core_from_host(host_of(ptr)) {
                unsafe {
                    (*core).notes_toggle_switcher();
                }
            }
        });
        window.ui.on_folder_clicked(move || {
            if let Some(core) = core_from_host(host_of(ptr)) {
                unsafe {
                    (*core).notes_open_folder();
                }
            }
        });
        window.ui.window().on_close_requested(move || {
            // Closing hides; the debounce is flushed first, like WM_CLOSE did.
            close_requested(ptr);
            slint::CloseRequestResponse::HideWindow
        });
        apply_chrome(inner);
        arm_init(inner);
        Ok(window)
    }

    pub fn show(&self) {
        let ready = unsafe { (*self.inner).ready };
        if !ready {
            unsafe {
                (*self.inner).pending_show = true;
            }
            return;
        }
        let _ = self.ui.show();
        unsafe {
            if let Some(hwnd) = window_hwnd(&self.ui) {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
            }
        }
        if self.switcher_is_open() {
            self.place_switcher();
            let _ = self.switcher.show();
            self.focus_switcher();
        } else {
            self.focus_editor();
        }
    }

    pub fn hide(&self) {
        unsafe {
            (*self.inner).pending_show = false;
        }
        self.close_switcher();
        unsafe {
            if let Some(hwnd) = window_hwnd(&self.ui) {
                save_frame(hwnd);
            }
        }
        let _ = self.ui.hide();
    }

    /// Editor text typed since the last debounced save, if any. Cancels the
    /// pending save so the caller owns storing it.
    pub fn take_pending_body(&self) -> Option<String> {
        unsafe {
            let hwnd = window_hwnd(&self.ui)?;
            KillTimer(hwnd, SAVE_TIMER)
                .is_ok()
                .then(|| edit_text((*self.inner).edit))
        }
    }

    pub fn is_visible(&self) -> bool {
        unsafe {
            window_hwnd(&self.ui)
                .map(|hwnd| {
                    windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool()
                })
                .unwrap_or(false)
        }
    }

    pub fn set_locale(&self, _locale: &str) {
        let lang = core_from_host(self.host)
            .map(|core| unsafe { (*core).ui_lang() })
            .unwrap_or(UiLang::ZhHans);
        unsafe {
            (*self.inner).lang = lang;
        }
        apply_chrome(self.inner);
    }

    pub fn open_switcher(&self) {
        unsafe {
            (*self.inner).switcher_open = true;
        }
        self.place_switcher();
        let _ = self.switcher.show();
        self.focus_switcher();
    }

    pub fn close_switcher(&self) {
        unsafe {
            (*self.inner).switcher_open = false;
        }
        let _ = self.switcher.hide();
        self.focus_editor();
    }

    pub fn switcher_is_open(&self) -> bool {
        unsafe { (*self.inner).switcher_open }
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            (*self.inner).pending_title = title.to_string();
        }
        self.ui.set_note_title(title.into());
        if let Some(hwnd) = window_hwnd(&self.ui) {
            let mut wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_mut_ptr()));
            }
            wide.clear();
        }
    }

    pub fn set_body(&self, body: &str) {
        unsafe {
            (*self.inner).pending_body = body.to_string();
        }
        let edit = unsafe { (*self.inner).edit };
        if edit.is_invalid() {
            return;
        }
        let wide: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = SetWindowTextW(edit, PCWSTR(wide.as_ptr()));
            style_editor(edit);
        }
        self.sync_count(body.encode_utf16().count());
    }

    pub fn focus_editor(&self) {
        unsafe {
            let _ = SetFocus((*self.inner).edit);
        }
    }

    pub fn focus_switcher(&self) {
        unsafe {
            let _ = SetFocus((*self.inner).switcher_edit);
        }
    }

    pub fn fill_switcher(&self, titles: &[String]) {
        unsafe {
            let inner = &mut *self.inner;
            inner.entries.set_vec(
                titles
                    .iter()
                    .map(|title| tinycast_ui::NoteEntry {
                        title: title.clone().into(),
                    })
                    .collect::<Vec<_>>(),
            );
            inner.selected = 0;
        }
        self.switcher
            .set_selected(if titles.is_empty() { -1 } else { 0 });
        self.switcher.set_list_offset(0.0);
    }

    /// Places the popup under the notes title bar, horizontally centred on
    /// the notes window (the old `open_switcher`).
    fn place_switcher(&self) {
        let Some(notes) = window_hwnd(&self.ui) else {
            return;
        };
        let Some(switcher) = window_hwnd(&self.switcher) else {
            return;
        };
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(notes, &mut rect) }.is_err() {
            return;
        }
        let dpi = unsafe { GetDpiForWindow(notes) };
        let dpi = if dpi == 0 { 96 } else { dpi };
        let w = dip_scalar_to_px(SWITCHER_WIDTH as f32, dpi);
        let h = dip_scalar_to_px(SWITCHER_HEIGHT as f32, dpi);
        let x = rect.left + ((rect.right - rect.left) - w) / 2;
        let y = rect.top + dip_scalar_to_px(theme::size::NOTE_SWITCHER_DROP, dpi);
        unsafe {
            let _ = SetWindowPos(
                switcher,
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    }

    fn sync_count(&self, count: usize) {
        unsafe {
            (*self.inner).body_len = count;
        }
        self.ui.set_footer_count(count.to_string().into());
        self.sync_cue(count == 0);
    }

    fn sync_cue(&self, empty: bool) {
        let cue = unsafe { (*self.inner).cue };
        if cue.is_invalid() {
            return;
        }
        unsafe {
            let _ = ShowWindow(cue, if empty { SW_SHOW } else { SW_HIDE });
        }
    }
}

impl Drop for NotesWindow {
    fn drop(&mut self) {
        unsafe {
            if let Some(hwnd) = window_hwnd(&self.ui) {
                // Shutdown destroys this window with the backend; keep the
                // last keystrokes the debounce timer has not stored yet.
                if KillTimer(hwnd, SAVE_TIMER).is_ok() {
                    if let Some(core) = core_from_host(self.host) {
                        (*core).notes_body_changed(edit_text((*self.inner).edit));
                    }
                }
                if IsWindow(hwnd).as_bool() {
                    save_frame(hwnd);
                }
            }
            // The subclasses must not outlive `inner`.
            if let Some(hwnd) = window_hwnd(&self.ui) {
                if IsWindow(hwnd).as_bool() {
                    let _ = RemoveWindowSubclass(
                        hwnd,
                        SUBCLASSPROC::Some(notes_subclass),
                        NOTES_SUBCLASS_ID,
                    );
                }
            }
            if let Some(hwnd) = window_hwnd(&self.switcher) {
                if IsWindow(hwnd).as_bool() {
                    let _ = RemoveWindowSubclass(
                        hwnd,
                        SUBCLASSPROC::Some(switcher_subclass),
                        SWITCHER_SUBCLASS_ID,
                    );
                }
            }
            if IsWindow((*self.inner).edit).as_bool() {
                let _ = RemoveWindowSubclass(
                    (*self.inner).edit,
                    SUBCLASSPROC::Some(edit_subclass),
                    EDIT_SUBCLASS_ID,
                );
            }
            if IsWindow((*self.inner).switcher_edit).as_bool() {
                let _ = RemoveWindowSubclass(
                    (*self.inner).switcher_edit,
                    SUBCLASSPROC::Some(switcher_edit_subclass),
                    SWITCHER_EDIT_SUBCLASS_ID,
                );
            }
            if IsWindow((*self.inner).cue).as_bool() {
                let _ = RemoveWindowSubclass(
                    (*self.inner).cue,
                    SUBCLASSPROC::Some(cue_subclass),
                    CUE_SUBCLASS_ID,
                );
            }
            if !(*self.inner).cue_brush.is_invalid() {
                let _ = DeleteObject((*self.inner).cue_brush);
            }
            drop(Box::from_raw(self.inner));
            self.inner = std::ptr::null_mut();
        }
    }
}

fn host_of(ptr: usize) -> HWND {
    unsafe { (*(ptr as *mut Inner)).host }
}

/// A switcher row was activated (click or Enter): pick it through AppCore,
/// close the popup and hand the keyboard back to the editor.
fn picked(ptr: usize, index: usize) {
    unsafe {
        let inner = ptr as *mut Inner;
        if let Some(core) = core_from_host((*inner).host) {
            (*core).notes_pick_switcher(index);
        }
        (*inner).switcher_open = false;
        if let Some(switcher) = (*inner).switcher.upgrade() {
            let _ = switcher.hide();
        }
        if !(*inner).edit.is_invalid() {
            let _ = SetFocus((*inner).edit);
        }
    }
}

/// The notes window's close (caption X): flush the debounce, then hide
/// through AppCore so the palette state stays consistent.
fn close_requested(ptr: usize) {
    unsafe {
        let inner = ptr as *mut Inner;
        if let Some(hwnd) = (*inner).notes.upgrade().and_then(|ui| window_hwnd(&ui)) {
            if KillTimer(hwnd, SAVE_TIMER).is_ok() {
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).notes_body_changed(edit_text((*inner).edit));
                }
            }
        }
        if let Some(core) = core_from_host((*inner).host) {
            (*core).notes_hide();
        }
    }
}

fn park<T: ComponentHandle>(ui: &T) {
    ui.window().set_position(slint::LogicalPosition {
        x: PARKED as f32,
        y: PARKED as f32,
    });
}

fn arm_init(inner: *mut Inner) {
    let ptr = inner as usize;
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let inner = ptr as *mut Inner;
        if unsafe { (*inner).ready } {
            return;
        }
        let notes = unsafe { (*inner).notes.upgrade() };
        let switcher = unsafe { (*inner).switcher.upgrade() };
        let (Some(notes), Some(switcher)) = (notes, switcher) else {
            arm_init(inner);
            return;
        };
        let (Some(notes_hwnd), Some(switcher_hwnd)) = (window_hwnd(&notes), window_hwnd(&switcher))
        else {
            let _ = notes.hide();
            let _ = switcher.hide();
            arm_init(inner);
            return;
        };
        if !init_window(inner, notes_hwnd, switcher_hwnd) {
            arm_init(inner);
            return;
        }
        unsafe {
            (*inner).ready = true;
        }
        // Apply what arrived before the HWND existed.
        let title = unsafe { (*inner).pending_title.clone() };
        if !title.is_empty() {
            set_title_inner(inner, &title);
        }
        let body = unsafe { (*inner).pending_body.clone() };
        unsafe {
            let wide: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SetWindowTextW((*inner).edit, PCWSTR(wide.as_ptr()));
            style_editor((*inner).edit);
            (*inner).body_len = body.encode_utf16().count();
            if let Some(notes_ui) = (*inner).notes.upgrade() {
                notes_ui.set_footer_count((*inner).body_len.to_string().into());
            }
            sync_cue_inner(inner, (*inner).body_len == 0);
        }
        unsafe {
            layout_children(notes_hwnd, inner);
            layout_switcher_children(switcher_hwnd, inner);
            restore_frame(notes_hwnd);
        }
        let pending_show = unsafe { (*inner).pending_show };
        if !pending_show {
            let _ = notes.hide();
            let _ = switcher.hide();
        } else {
            unsafe {
                (*inner).pending_show = false;
            }
            show_inner(inner);
        }
    });
}

fn set_title_inner(inner: *mut Inner, title: &str) {
    unsafe {
        (*inner).pending_title = title.to_string();
        if let Some(ui) = (*inner).notes.upgrade() {
            ui.set_note_title(title.into());
            if let Some(hwnd) = window_hwnd(&ui) {
                let mut wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_mut_ptr()));
            }
        }
    }
}

fn show_inner(inner: *mut Inner) {
    unsafe {
        if let Some(notes) = (*inner).notes.upgrade() {
            let _ = notes.show();
            if let Some(hwnd) = window_hwnd(&notes) {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
            }
        }
        if (*inner).switcher_open {
            if let Some(switcher) = (*inner).switcher.upgrade() {
                let _ = switcher.show();
            }
            if !(*inner).switcher_edit.is_invalid() {
                let _ = SetFocus((*inner).switcher_edit);
            }
        } else if !(*inner).edit.is_invalid() {
            let _ = SetFocus((*inner).edit);
        }
    }
}

/// Builds the native children on the freshly created HWNDs and installs the
/// subclasses. Returns false to retry on the next poll tick.
fn init_window(inner: *mut Inner, notes_hwnd: HWND, switcher_hwnd: HWND) -> bool {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        // The body keeps the RichEdit; the IME needs a real EDIT control.
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("RICHEDIT50W"),
            w!(""),
            WS_CHILD
                | WS_VISIBLE
                | WS_VSCROLL
                | WS_TABSTOP
                | WINDOW_STYLE(ES_MULTILINE.0 | ES_AUTOVSCROLL.0 | ES_WANTRETURN.0),
            0,
            0,
            100,
            100,
            notes_hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_EDIT as *mut core::ffi::c_void),
            hinstance,
            None,
        )
        .unwrap_or_default();
        if edit.is_invalid() {
            return false;
        }
        style_editor(edit);
        // The placeholder cue rides above the edit and is hit-test
        // transparent, exactly like the Direct2D chrome's STATIC.
        let cue = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR::null(),
            WS_CHILD,
            0,
            0,
            100,
            24,
            notes_hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CUE as *mut core::ffi::c_void),
            hinstance,
            None,
        )
        .unwrap_or_default();
        if !cue.is_invalid() {
            let _ = ShowWindow(cue, SW_HIDE);
        }
        // Switcher search: a real EDIT inside the Slint popup.
        let switcher_edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0,
            0,
            100,
            24,
            switcher_hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(
                ID_SWITCHER_EDIT as *mut core::ffi::c_void,
            ),
            hinstance,
            None,
        )
        .unwrap_or_default();
        if switcher_edit.is_invalid() {
            return false;
        }

        (*inner).edit = edit;
        (*inner).cue = cue;
        (*inner).switcher_edit = switcher_edit;

        let ok = SetWindowSubclass(
            notes_hwnd,
            SUBCLASSPROC::Some(notes_subclass),
            NOTES_SUBCLASS_ID,
            inner as usize,
        )
        .as_bool()
            && SetWindowSubclass(
                switcher_hwnd,
                SUBCLASSPROC::Some(switcher_subclass),
                SWITCHER_SUBCLASS_ID,
                inner as usize,
            )
            .as_bool()
            && SetWindowSubclass(
                edit,
                SUBCLASSPROC::Some(edit_subclass),
                EDIT_SUBCLASS_ID,
                inner as usize,
            )
            .as_bool()
            && SetWindowSubclass(
                switcher_edit,
                SUBCLASSPROC::Some(switcher_edit_subclass),
                SWITCHER_EDIT_SUBCLASS_ID,
                inner as usize,
            )
            .as_bool();
        if !ok {
            return false;
        }
        if !cue.is_invalid() {
            let _ = SetWindowSubclass(cue, SUBCLASSPROC::Some(cue_subclass), CUE_SUBCLASS_ID, 0);
        }
        true
    }
}

/// Lays the RichEdit and the cue into the body inset (`size-note-editor-*`),
/// the old `layout`.
unsafe fn layout_children(hwnd: HWND, inner: *mut Inner) {
    let mut rc = RECT::default();
    if GetClientRect(hwnd, &mut rc).is_err() {
        return;
    }
    let dpi = GetDpiForWindow(hwnd);
    let dpi = if dpi == 0 { 96 } else { dpi };
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    let title_h = dip_scalar_to_px(theme::size::NOTE_TITLEBAR, dpi);
    let footer_h = dip_scalar_to_px(theme::size::NOTE_FOOTER_HEIGHT, dpi);
    let inset = dip_scalar_to_px(theme::size::NOTE_EDITOR_INSET, dpi);
    let top_inset = dip_scalar_to_px(theme::size::NOTE_EDITOR_TOP_INSET, dpi);
    let edit_y = title_h + top_inset;
    let edit_h = (h - edit_y - footer_h).max(20);
    let _ = MoveWindow(
        (*inner).edit,
        inset,
        edit_y,
        (w - inset * 2).max(20),
        edit_h,
        true,
    );
    if !(*inner).cue.is_invalid() {
        let cue_h = dip_scalar_to_px(24.0, dpi);
        let _ = MoveWindow(
            (*inner).cue,
            inset,
            edit_y,
            (w - inset * 2).max(20),
            cue_h,
            true,
        );
    }
}

/// Lays the switcher's search EDIT: 8 DIP gutter, 24 DIP field, per the old
/// `layout_switcher`.
unsafe fn layout_switcher_children(hwnd: HWND, inner: *mut Inner) {
    let mut rc = RECT::default();
    if GetClientRect(hwnd, &mut rc).is_err() {
        return;
    }
    let dpi = GetDpiForWindow(hwnd);
    let dpi = if dpi == 0 { 96 } else { dpi };
    let w = rc.right - rc.left;
    let pad = dip_scalar_to_px(SWITCHER_PAD_DIP, dpi);
    let edit_h = dip_scalar_to_px(SWITCHER_EDIT_H_DIP, dpi);
    let _ = MoveWindow(
        (*inner).switcher_edit,
        pad,
        pad,
        (w - pad * 2).max(20),
        edit_h,
        true,
    );
}

unsafe extern "system" fn notes_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let inner = dwrefdata as *mut Inner;
    match msg {
        // The body edit reports to this window: arm the debounce, refresh the
        // footer count and the cue.
        WM_COMMAND => {
            let notify = ((wparam.0 as u32) >> 16) as u16;
            let id = (wparam.0 as u16) as usize;
            if id == ID_EDIT && notify == EN_CHANGE as u16 && !inner.is_null() {
                let len = GetWindowTextLengthW((*inner).edit).max(0) as usize;
                let _ = SetTimer(hwnd, SAVE_TIMER, SAVE_MS, None);
                if let Some(notes) = (*inner).notes.upgrade() {
                    notes.set_footer_count(len.to_string().into());
                }
                (*inner).body_len = len;
                sync_cue_inner(inner, len == 0);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_TIMER => {
            if wparam.0 == SAVE_TIMER && !inner.is_null() {
                // One-shot debounce: EN_CHANGE re-arms it on the next keystroke.
                let _ = KillTimer(hwnd, SAVE_TIMER);
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).notes_body_changed(edit_text((*inner).edit));
                }
                return LRESULT(0);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_SIZE => {
            if !inner.is_null() {
                layout_children(hwnd, inner);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_DPICHANGED => {
            if !inner.is_null() {
                layout_children(hwnd, inner);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        // The cue STATIC paints against the note background.
        WM_CTLCOLORSTATIC => {
            if !inner.is_null() {
                let child = HWND(lparam.0 as *mut core::ffi::c_void);
                if child == (*inner).cue {
                    let hdc =
                        windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut core::ffi::c_void);
                    let appearance = core_from_host((*inner).host)
                        .map(|core| (*core).resolved_appearance())
                        .unwrap_or(1);
                    SetTextColor(hdc, note_cue_colorref(appearance));
                    SetBkColor(hdc, note_bg_colorref(appearance));
                    return LRESULT((*inner).cue_brush.0 as isize);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let _ =
                RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(notes_subclass), NOTES_SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn switcher_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let inner = dwrefdata as *mut Inner;
    match msg {
        // The search edit reports here; typing filters the list.
        WM_COMMAND => {
            let notify = ((wparam.0 as u32) >> 16) as u16;
            let id = (wparam.0 as u16) as usize;
            if id == ID_SWITCHER_EDIT && notify == EN_CHANGE as u16 && !inner.is_null() {
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).notes_filter_switcher(&window_text((*inner).switcher_edit));
                }
                (*inner).selected = 0;
                if let Some(switcher) = (*inner).switcher.upgrade() {
                    switcher.set_selected(0);
                    switcher.set_list_offset(0.0);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_SIZE | WM_DPICHANGED => {
            if !inner.is_null() {
                layout_switcher_children(hwnd, inner);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(
                hwnd,
                SUBCLASSPROC::Some(switcher_subclass),
                SWITCHER_SUBCLASS_ID,
            );
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let inner = dwrefdata as *mut Inner;
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x1B && !inner.is_null() {
        // Esc: close an open switcher, otherwise hide the window.
        if (*inner).switcher_open {
            close_switcher_inner(inner);
        } else if let Some(core) = core_from_host((*inner).host) {
            (*core).notes_hide();
        }
        return LRESULT(0);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn switcher_edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let inner = dwrefdata as *mut Inner;
    if !inner.is_null() {
        match msg {
            WM_KEYDOWN => match wparam.0 as u16 {
                0x1B => {
                    close_switcher_inner(inner);
                    return LRESULT(0);
                }
                0x0D => {
                    if (*inner).entries.row_count() > 0 {
                        let selected = (*inner).selected;
                        picked(inner as usize, selected);
                    }
                    return LRESULT(0);
                }
                0x26 => {
                    move_selection(inner, -1);
                    return LRESULT(0);
                }
                0x28 => {
                    move_selection(inner, 1);
                    return LRESULT(0);
                }
                _ => {}
            },
            WM_MOUSEWHEEL => {
                // The list Flickable owns the wheel; hand it to the popup.
                if let Some(switcher) = (*inner).switcher.upgrade() {
                    if let Some(target) = window_hwnd(&switcher) {
                        let _ = SendMessageW(target, WM_MOUSEWHEEL, wparam, lparam);
                    }
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

/// The cue is hit-test transparent so clicks reach the edit below.
unsafe extern "system" fn cue_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _dwrefdata: usize,
) -> LRESULT {
    if msg == WM_NCHITTEST {
        return LRESULT(HTTRANSPARENT as isize);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

fn close_switcher_inner(inner: *mut Inner) {
    unsafe {
        (*inner).switcher_open = false;
        if let Some(switcher) = (*inner).switcher.upgrade() {
            let _ = switcher.hide();
        }
        if !(*inner).edit.is_invalid() {
            let _ = SetFocus((*inner).edit);
        }
    }
}

/// Moves the Slint selection and keeps it inside the popup's viewport.
fn move_selection(inner: *mut Inner, delta: isize) {
    unsafe {
        let count = (*inner).entries.row_count() as isize;
        if count == 0 {
            return;
        }
        let next = ((*inner).selected as isize + delta).clamp(0, count - 1) as usize;
        if next == (*inner).selected {
            return;
        }
        (*inner).selected = next;
        if let Some(switcher) = (*inner).switcher.upgrade() {
            switcher.set_selected(next as i32);
        }
        let row = theme::size::MAGPIE_LIST_ITEM;
        let top = next as f32 * row;
        let view = theme::size::NOTE_SWITCHER.1 - theme::size::NOTE_SEARCH_HEIGHT;
        let offset = -(*inner)
            .switcher
            .upgrade()
            .map(|s| s.get_list_offset())
            .unwrap_or(0.0);
        if let Some(switcher) = (*inner).switcher.upgrade() {
            if top < offset {
                switcher.set_list_offset(-top);
            } else if top + row > offset + view {
                switcher.set_list_offset(-(top + row - view));
            }
        }
    }
}

fn sync_cue_inner(inner: *mut Inner, empty: bool) {
    unsafe {
        if (*inner).cue.is_invalid() {
            return;
        }
        let _ = ShowWindow((*inner).cue, if empty { SW_SHOW } else { SW_HIDE });
    }
}

fn edit_text(hwnd: HWND) -> String {
    window_text(hwnd)
}

fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        if n <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }
}

/// The chrome strings come from i18n on create and on every locale change.
fn apply_chrome(inner: *mut Inner) {
    unsafe {
        let lang = (*inner).lang;
        if let Some(ui) = (*inner).notes.upgrade() {
            ui.set_create_symbol("plus".into());
            ui.set_switcher_symbol("rectangle.stack".into());
            ui.set_folder_symbol("folder".into());
            ui.set_footer_count((*inner).body_len.to_string().into());
            ui.set_note_title((*inner).pending_title.clone().into());
        }
        if let Some(switcher) = (*inner).switcher.upgrade() {
            switcher.set_empty_label(
                tinycast_pure::i18n::empty_nothing_left(lang)
                    .to_string()
                    .into(),
            );
        }
    }
}

/// RichEdit colours: the note background (kept at the old `note_bg_rgb`) and
/// the note ink composited over it.
fn note_bg_rgb(appearance: u8) -> (u8, u8, u8) {
    if appearance == 0 {
        (0x1A, 0x1A, 0x1A)
    } else {
        (0xF2, 0xF2, 0xF2)
    }
}

fn composite_colorref(rgba: (f32, f32, f32, f32), bg: (u8, u8, u8)) -> COLORREF {
    let (r, g, b, a) = rgba;
    let comp = |c: f32, bc: u8| ((c * a + (bc as f32 / 255.0) * (1.0 - a)) * 255.0).round() as u32;
    COLORREF(comp(b, bg.2) << 16 | comp(g, bg.1) << 8 | comp(r, bg.0))
}

pub fn note_fg_colorref(appearance: u8) -> COLORREF {
    composite_colorref(
        crate::design_system::text::note_text(appearance),
        note_bg_rgb(appearance),
    )
}

fn note_bg_colorref(appearance: u8) -> COLORREF {
    let (r, g, b) = note_bg_rgb(appearance);
    COLORREF((b as u32) << 16 | (g as u32) << 8 | r as u32)
}

fn note_cue_colorref(appearance: u8) -> COLORREF {
    composite_colorref(
        crate::design_system::text::tertiary_ink(appearance),
        note_bg_rgb(appearance),
    )
}

/// RichEdit background and ink, applied to both the selection and the whole
/// document (the old `CHARFORMAT_WPARAMS`).
fn style_editor(edit: HWND) {
    const EM_SETBKGNDCOLOR: u32 = 0x0443;
    const EM_SETCHARFORMAT: u32 = 0x0444;
    const CFM_COLOR: u32 = 0x4000_0000;
    #[repr(C)]
    struct CharFormat {
        cb_size: u32,
        dw_mask: u32,
        dw_effects: u32,
        y_height: i32,
        y_offset: i32,
        cr_text_color: COLORREF,
        b_char_set: u8,
        b_pitch_and_family: u8,
        sz_face_name: [u16; 32],
    }
    unsafe {
        let bg = note_bg_colorref(0);
        let _ = SendMessageW(edit, EM_SETBKGNDCOLOR, WPARAM(0), LPARAM(bg.0 as isize));
        let mut cf = CharFormat {
            cb_size: std::mem::size_of::<CharFormat>() as u32,
            dw_mask: CFM_COLOR,
            dw_effects: 0,
            y_height: 0,
            y_offset: 0,
            cr_text_color: note_fg_colorref(0),
            b_char_set: 0,
            b_pitch_and_family: 0,
            sz_face_name: [0; 32],
        };
        for wparam in CHARFORMAT_WPARAMS {
            let _ = SendMessageW(
                edit,
                EM_SETCHARFORMAT,
                WPARAM(wparam),
                LPARAM(&mut cf as *mut CharFormat as isize),
            );
        }
    }
}

fn window_hwnd<T: ComponentHandle>(ui: &T) -> Option<HWND> {
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

fn restore_frame(notes_hwnd: HWND) {
    if let Some(frame) = load_saved_frame() {
        unsafe {
            let _ = SetWindowPos(
                notes_hwnd,
                HWND_TOP,
                frame.0,
                frame.1,
                frame.2,
                frame.3,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

fn placement_path() -> PathBuf {
    paths::roaming_dir().join("notes-window.json")
}

fn load_saved_frame() -> Option<(i32, i32, i32, i32)> {
    let bytes = std::fs::read(placement_path()).ok()?;
    let v = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    Some((
        v["x"].as_i64()? as i32,
        v["y"].as_i64()? as i32,
        v["w"].as_i64()? as i32,
        v["h"].as_i64()? as i32,
    ))
}

unsafe fn save_frame(hwnd: HWND) {
    let mut rc = RECT::default();
    if GetWindowRect(hwnd, &mut rc).is_err() {
        return;
    }
    let v = serde_json::json!({
        "name": "Notes Window",
        "x": rc.left,
        "y": rc.top,
        "w": rc.right - rc.left,
        "h": rc.bottom - rc.top,
    });
    let _ = std::fs::create_dir_all(paths::roaming_dir());
    let _ = std::fs::write(placement_path(), v.to_string());
}
