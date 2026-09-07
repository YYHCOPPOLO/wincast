//! Titled Notes window: RichEdit source editor and a 300×240 switcher.

use std::path::PathBuf;

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, SetBkColor, SetTextColor, HBRUSH, HDC,
};

use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindow, LoadCursorW,
    MoveWindow, RegisterClassW, SendMessageW, SetTimer, SetWindowLongPtrW, SetWindowPos,
    SetWindowTextW, ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, GWLP_WNDPROC,
    HWND_TOP, IDC_ARROW, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, MINMAXINFO,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SIZE,
    WM_TIMER, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_TOOLWINDOW,
    WS_OVERLAPPEDWINDOW, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
};

use crate::app_core::AppCore;
use crate::design_system::host::OverlayPainter;
use crate::design_system::squircle::fill_squircle;
use crate::design_system::text;
use crate::platform::paths;
use crate::platform::screens::dip_scalar_to_px;
use crate::platform::window::{apply_captioned_client_dip, captioned_outer_px};

const CLASS: windows::core::PCWSTR = w!("TinycastNotes");
const SWITCHER_CLASS: windows::core::PCWSTR = w!("TinycastNoteSwitcher");
pub const SWITCHER_WIDTH: i32 = 300;
pub const SWITCHER_HEIGHT: i32 = 240;
const ID_EDIT: usize = 101;
const ID_CREATE: usize = 102;
const ID_BROWSE: usize = 103;
const ID_FOLDER: usize = 104;
const ID_CUE: usize = 105;
const SCF_DEFAULT: usize = 0;
const SCF_ALL: usize = 4;
const CHARFORMAT_WPARAMS: [usize; 2] = [SCF_DEFAULT, SCF_ALL];
const ID_SWITCHER_EDIT: usize = 201;
const ID_SWITCHER_LIST: usize = 202;
const SAVE_TIMER: usize = 1;
const SAVE_MS: u32 = 300;
const ES_MULTILINE: WINDOW_STYLE = WINDOW_STYLE(0x0004);
const ES_AUTOVSCROLL: WINDOW_STYLE = WINDOW_STYLE(0x0040);
const ES_WANTRETURN: WINDOW_STYLE = WINDOW_STYLE(0x1000);
const LBS_NOTIFY: WINDOW_STYLE = WINDOW_STYLE(0x0001);
const EN_CHANGE: u16 = 0x0300;
const LBN_SELCHANGE: u16 = 1;
const LBN_DBLCLK: u16 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwitcherAction {
    None,
    Filter,
    Pick,
}

/// Child EDIT/LISTBOX notify the popup; the popup must forward these to the notes owner.
pub fn switcher_command_action(id: usize, notify: u16) -> SwitcherAction {
    match (id, notify) {
        (ID_SWITCHER_EDIT, EN_CHANGE) => SwitcherAction::Filter,
        (ID_SWITCHER_LIST, LBN_SELCHANGE) | (ID_SWITCHER_LIST, LBN_DBLCLK) => SwitcherAction::Pick,
        _ => SwitcherAction::None,
    }
}

pub struct NotesWindow {
    pub hwnd: HWND,
}

struct Inner {
    host: HWND,
    edit: HWND,
    switcher: HWND,
    switcher_edit: HWND,
    switcher_list: HWND,
    switcher_open: bool,
    edit_prev: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
    switcher_prev: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
    list_prev: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
    painter: Option<OverlayPainter>,
    switcher_painter: Option<OverlayPainter>,
    cue: HWND,
    cue_brush: HBRUSH,
    cue_prev: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
    create_rect: DipRect,
    browse_rect: DipRect,
    folder_rect: DipRect,
    title_rect: DipRect,
}

impl NotesWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let _ = LoadLibraryW(w!("Msftedit.dll"));
            let hinstance = GetModuleHandleW(None)?;
            register(CLASS, Some(wndproc))?;
            register(SWITCHER_CLASS, Some(switcher_wndproc))?;
            let (ow, oh) =
                captioned_outer_px(theme::size::NOTE_WINDOW, 96, notes_style(), notes_ex());
            let hwnd = CreateWindowExW(
                notes_ex(),
                CLASS,
                w!("Notes"),
                notes_style(),
                120,
                120,
                ow,
                oh,
                host,
                None,
                hinstance,
                Some(host.0 as *const core::ffi::c_void),
            )?;
            if let Some(frame) = load_saved_frame() {
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    frame.0,
                    frame.1,
                    frame.2,
                    frame.3,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            } else {
                apply_captioned_client_dip(
                    hwnd,
                    theme::size::NOTE_WINDOW,
                    notes_style(),
                    notes_ex(),
                );
            }
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);
        }
    }

    pub fn hide(&self) {
        unsafe {
            close_switcher_of(self.hwnd);
            save_frame(self.hwnd);
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(self.hwnd).as_bool() }
    }

    pub fn open_switcher(&self) {
        unsafe {
            open_switcher_of(self.hwnd);
        }
    }

    pub fn close_switcher(&self) {
        unsafe {
            close_switcher_of(self.hwnd);
        }
    }

    pub fn switcher_is_open(&self) -> bool {
        unsafe {
            inner_from(self.hwnd)
                .map(|i| (*i).switcher_open)
                .unwrap_or(false)
        }
    }

    pub fn set_title(&self, title: &str) {
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = SetWindowTextW(self.hwnd, PCWSTR(wide.as_ptr()));
        }
    }

    pub fn set_body(&self, body: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                let wide: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = SetWindowTextW((*inner).edit, PCWSTR(wide.as_ptr()));
                style_editor((*inner).edit);
                sync_cue(self.hwnd);
            }
        }
    }

    pub fn focus_editor(&self) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                let _ = SetFocus((*inner).edit);
            }
        }
    }

    pub fn focus_switcher(&self) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                let _ = SetFocus((*inner).switcher_edit);
            }
        }
    }

    pub fn fill_switcher(&self, titles: &[String]) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                let _ = SendMessageW(
                    (*inner).switcher_list,
                    LB_RESETCONTENT,
                    WPARAM(0),
                    LPARAM(0),
                );
                for title in titles {
                    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = SendMessageW(
                        (*inner).switcher_list,
                        LB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(wide.as_ptr() as isize),
                    );
                }
                if !titles.is_empty() {
                    let _ =
                        SendMessageW((*inner).switcher_list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
                }
            }
        }
    }
}

impl Drop for NotesWindow {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                save_frame(self.hwnd);
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

fn register(
    class: PCWSTR,
    proc: Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>,
) -> windows::core::Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: proc,
            hInstance: hinstance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let atom = RegisterClassW(&wc);
        if atom == 0 {
            let last = windows::Win32::Foundation::GetLastError();
            if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(last.into());
            }
        }
        Ok(())
    }
}

unsafe fn inner_from(hwnd: HWND) -> Option<*mut Inner> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn core_from_host(host: HWND) -> Option<*mut AppCore> {
    let ptr = GetWindowLongPtrW(host, GWLP_USERDATA) as *mut AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let host = HWND(cs.lpCreateParams);
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("RICHEDIT50W"),
                w!(""),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WS_TABSTOP
                    | ES_MULTILINE
                    | ES_AUTOVSCROLL
                    | ES_WANTRETURN,
                0,
                52,
                400,
                300,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_EDIT as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            style_editor(edit);
            let cue = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Start writing…"),
                WS_CHILD,
                0,
                0,
                100,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CUE as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let _ = ShowWindow(cue, SW_HIDE);
            let create_btn = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Create"),
                WS_CHILD | WS_TABSTOP,
                8,
                6,
                72,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CREATE as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let browse_btn = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Browse"),
                WS_CHILD | WS_TABSTOP,
                88,
                6,
                72,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_BROWSE as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let folder_btn = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Folder"),
                WS_CHILD | WS_TABSTOP,
                168,
                6,
                72,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_FOLDER as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let _ = ShowWindow(create_btn, SW_HIDE);
            let _ = ShowWindow(browse_btn, SW_HIDE);
            let _ = ShowWindow(folder_btn, SW_HIDE);
            let switcher = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                SWITCHER_CLASS,
                w!(""),
                WS_POPUP | WS_BORDER | WINDOW_STYLE(0x02000000),
                0,
                0,
                SWITCHER_WIDTH,
                SWITCHER_HEIGHT,
                hwnd,
                None,
                hinstance,
                None,
            )
            .unwrap_or_default();
            let switcher_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                8,
                8,
                SWITCHER_WIDTH - 16,
                24,
                switcher,
                windows::Win32::UI::WindowsAndMessaging::HMENU(
                    ID_SWITCHER_EDIT as *mut core::ffi::c_void,
                ),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let switcher_list = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("LISTBOX"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_VSCROLL | LBS_NOTIFY,
                8,
                40,
                SWITCHER_WIDTH - 16,
                SWITCHER_HEIGHT - 52,
                switcher,
                windows::Win32::UI::WindowsAndMessaging::HMENU(
                    ID_SWITCHER_LIST as *mut core::ffi::c_void,
                ),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let cue_brush = CreateSolidBrush(note_bg_colorref(0));
            let inner = Box::new(Inner {
                host,
                edit,
                switcher,
                switcher_edit,
                switcher_list,
                switcher_open: false,
                edit_prev: None,
                switcher_prev: None,
                list_prev: None,
                painter: OverlayPainter::new().ok(),
                switcher_painter: OverlayPainter::new().ok(),
                cue,
                cue_brush,
                cue_prev: None,
                create_rect: empty_rect(),
                browse_rect: empty_rect(),
                folder_rect: empty_rect(),
                title_rect: empty_rect(),
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
            if let Some(inner) = inner_from(hwnd) {
                let prev =
                    SetWindowLongPtrW((*inner).edit, GWLP_WNDPROC, edit_subclass as usize as isize);
                (*inner).edit_prev = Some(std::mem::transmute(prev));
                let prev = SetWindowLongPtrW(
                    (*inner).switcher_edit,
                    GWLP_WNDPROC,
                    switcher_edit_subclass as usize as isize,
                );
                (*inner).switcher_prev = Some(std::mem::transmute(prev));
                let prev = SetWindowLongPtrW(
                    (*inner).switcher_list,
                    GWLP_WNDPROC,
                    switcher_list_subclass as usize as isize,
                );
                (*inner).list_prev = Some(std::mem::transmute(prev));
                if !(*inner).cue.is_invalid() {
                    let prev = SetWindowLongPtrW(
                        (*inner).cue,
                        GWLP_WNDPROC,
                        cue_subclass as usize as isize,
                    );
                    (*inner).cue_prev = Some(std::mem::transmute(prev));
                }
            }
            LRESULT(1)
        }
        WM_SIZE => {
            layout(hwnd);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let suggested = lparam.0 as *const RECT;
            if !suggested.is_null() {
                let r = *suggested;
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            if let Some(inner) = inner_from(hwnd) {
                style_editor((*inner).edit);
            }
            layout(hwnd);
            layout_switcher(hwnd);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let child = HWND(lparam.0 as *mut core::ffi::c_void);
            if let Some(inner) = inner_from(hwnd) {
                if child == (*inner).cue {
                    let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
                    SetTextColor(hdc, note_cue_colorref(0));
                    SetBkColor(hdc, note_bg_colorref(0));
                    return LRESULT((*inner).cue_brush.0 as isize);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint_notes(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            hit_titlebar(hwnd, lparam);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let info = lparam.0 as *mut MINMAXINFO;
            if !info.is_null() {
                let dpi = GetDpiForWindow(hwnd);
                let (ow, oh) = captioned_outer_px((320.0, 220.0), dpi, notes_style(), notes_ex());
                (*info).ptMinTrackSize.x = ow;
                (*info).ptMinTrackSize.y = oh;
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            let notify = ((wparam.0 as u32) >> 16) as u16;
            match id {
                ID_CREATE => {
                    if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                        (*core).notes_create();
                    }
                }
                ID_BROWSE => {
                    if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                        (*core).notes_toggle_switcher();
                    }
                }
                ID_FOLDER => {
                    if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                        (*core).notes_open_folder();
                    }
                }
                ID_EDIT if notify == EN_CHANGE => {
                    let _ = SetTimer(hwnd, SAVE_TIMER, SAVE_MS, None);
                    sync_cue(hwnd);
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
                }
                _ => match switcher_command_action(id, notify) {
                    SwitcherAction::Filter => {
                        if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host))
                        {
                            (*core).notes_filter_switcher(&switcher_query(hwnd));
                        }
                    }
                    SwitcherAction::Pick => pick_switcher(hwnd),
                    SwitcherAction::None => {}
                },
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == SAVE_TIMER => {
            if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                (*core).notes_body_changed(edit_text(hwnd));
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u16 == 0x1B => {
            escape(hwnd);
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                (*core).notes_hide();
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            save_frame(hwnd);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let inner = Box::from_raw(ptr as *mut Inner);
                if !inner.cue_brush.is_invalid() {
                    let _ = DeleteObject(inner.cue_brush);
                }
                drop(inner);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn switcher_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let notes = parent_of(hwnd);
    if msg == WM_PAINT {
        paint_switcher(hwnd);
        return LRESULT(0);
    }
    if msg == WM_ERASEBKGND {
        return LRESULT(1);
    }
    if msg == WM_COMMAND {
        return wndproc(notes, msg, wparam, lparam);
    }
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x1B {
        close_switcher_of(notes);
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x0D {
        pick_switcher(notes);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn switcher_list_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let switcher = parent_of(hwnd);
    let notes = parent_of(switcher);
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x1B {
        close_switcher_of(notes);
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x0D {
        pick_switcher(notes);
        return LRESULT(0);
    }
    let prev = inner_from(notes).and_then(|i| (*i).list_prev);
    if let Some(prev) = prev {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe extern "system" fn edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x1B {
        let parent = parent_of(hwnd);
        escape(parent);
        return LRESULT(0);
    }
    let parent = parent_of(hwnd);
    let prev = inner_from(parent).and_then(|i| (*i).edit_prev);
    if let Some(prev) = prev {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe extern "system" fn switcher_edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let switcher = parent_of(hwnd);
    let notes = parent_of(switcher);
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x1B {
        close_switcher_of(notes);
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x0D {
        pick_switcher(notes);
        return LRESULT(0);
    }
    let prev = inner_from(notes).and_then(|i| (*i).switcher_prev);
    if let Some(prev) = prev {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn parent_of(hwnd: HWND) -> HWND {
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd).unwrap_or_default() }
}

fn notes_style() -> WINDOW_STYLE {
    WS_OVERLAPPEDWINDOW | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WINDOW_STYLE(0x02000000)
}

fn notes_ex() -> WINDOW_EX_STYLE {
    WINDOW_EX_STYLE::default()
}

fn cue_show_cmd(text_len: i32) -> windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD {
    if text_len == 0 {
        SW_SHOW
    } else {
        SW_HIDE
    }
}

fn switcher_size_px(dpi: u32) -> (i32, i32) {
    (
        dip_scalar_to_px(SWITCHER_WIDTH as f32, dpi),
        dip_scalar_to_px(SWITCHER_HEIGHT as f32, dpi),
    )
}

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

fn note_fg_colorref(appearance: u8) -> COLORREF {
    composite_colorref(text::note_text(appearance), note_bg_rgb(appearance))
}

fn note_bg_colorref(appearance: u8) -> COLORREF {
    let (r, g, b) = note_bg_rgb(appearance);
    COLORREF((b as u32) << 16 | (g as u32) << 8 | r as u32)
}

fn note_cue_colorref(appearance: u8) -> COLORREF {
    composite_colorref(text::tertiary_ink(appearance), note_bg_rgb(appearance))
}

unsafe fn sync_cue(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    if (*inner).cue.is_invalid() {
        return;
    }
    let len = GetWindowTextLengthW((*inner).edit);
    let _ = ShowWindow((*inner).cue, cue_show_cmd(len));
}

unsafe extern "system" fn cue_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCHITTEST {
        return LRESULT(-1);
    }
    let parent = parent_of(hwnd);
    let prev = inner_from(parent).and_then(|i| (*i).cue_prev);
    if let Some(prev) = prev {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe fn layout_switcher(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let dpi = GetDpiForWindow((*inner).switcher);
    let dpi = if dpi == 0 { GetDpiForWindow(hwnd) } else { dpi };
    let (w, h) = switcher_size_px(dpi);
    let pad = dip_scalar_to_px(8.0, dpi);
    let edit_h = dip_scalar_to_px(24.0, dpi);
    let gap = dip_scalar_to_px(8.0, dpi);
    let _ = MoveWindow(
        (*inner).switcher_edit,
        pad,
        pad,
        (w - pad * 2).max(20),
        edit_h,
        true,
    );
    let list_y = pad + edit_h + gap;
    let _ = MoveWindow(
        (*inner).switcher_list,
        pad,
        list_y,
        (w - pad * 2).max(20),
        (h - list_y - pad).max(20),
        true,
    );
}

fn empty_rect() -> DipRect {
    DipRect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    }
}

fn contains(rect: DipRect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
}

fn client_dip(hwnd: HWND, lparam: LPARAM) -> (f32, f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    let x = (lparam.0 as u32 & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    let y = ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    (x, y)
}

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

unsafe fn layout(hwnd: HWND) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let dpi = GetDpiForWindow(hwnd);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    let title_h = dip_scalar_to_px(theme::size::NOTE_TITLEBAR, dpi);
    let footer_h = dip_scalar_to_px(theme::size::NOTE_FOOTER_HEIGHT, dpi);
    let inset = dip_scalar_to_px(theme::size::NOTE_EDITOR_INSET, dpi);
    let top_inset = dip_scalar_to_px(theme::size::NOTE_EDITOR_TOP_INSET, dpi);
    if let Some(inner) = inner_from(hwnd) {
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
        sync_cue(hwnd);
    }
}

unsafe fn hit_titlebar(hwnd: HWND, lparam: LPARAM) {
    let (x, y) = client_dip(hwnd, lparam);
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    if contains((*inner).create_rect, x, y) {
        if let Some(core) = core_from_host((*inner).host) {
            (*core).notes_create();
        }
        return;
    }
    if contains((*inner).browse_rect, x, y) || contains((*inner).title_rect, x, y) {
        if let Some(core) = core_from_host((*inner).host) {
            (*core).notes_toggle_switcher();
        }
        return;
    }
    if contains((*inner).folder_rect, x, y) {
        if let Some(core) = core_from_host((*inner).host) {
            (*core).notes_open_folder();
        }
    }
}

fn window_title(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            "Notes".into()
        }
    }
}

fn paint_notes(hwnd: HWND) {
    unsafe {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return;
        }
        let Some(inner) = inner_from(hwnd) else {
            let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            return;
        };
        let title = window_title(hwnd);
        let count = GetWindowTextLengthW((*inner).edit) as usize;
        let footer = format!("{count}");
        if let Some(painter) = (*inner).painter.as_mut() {
            let mut create = empty_rect();
            let mut browse = empty_rect();
            let mut folder = empty_rect();
            let mut title_rect = empty_rect();
            let _ = painter.paint(hwnd, |target, fonts| {
                let size = target.GetSize();
                fill_squircle(
                    target,
                    DipRect {
                        x: 0.0,
                        y: 0.0,
                        w: size.width,
                        h: size.height,
                    },
                    theme::radius::PANEL,
                    theme::colors::scrim_rgba(0),
                )?;
                let bar_h = theme::size::NOTE_TITLEBAR;
                let inset = theme::size::NOTE_TITLE_INSET;
                title_rect = DipRect {
                    x: inset,
                    y: 0.0,
                    w: (size.width - inset * 2.0).max(40.0),
                    h: bar_h,
                };
                text::draw(
                    target,
                    &fonts.headline_center,
                    &title,
                    title_rect,
                    text::primary_ink(0),
                )?;
                let glyph = theme::size::NOTE_GLYPH;
                let cap_h = theme::size::BAR_BUTTON_HEIGHT;
                let slot = 28.0;
                let cap_w = slot * 3.0 + theme::spacing::XS * 2.0;
                let cap = DipRect {
                    x: size.width - theme::spacing::MD - cap_w,
                    y: (bar_h - cap_h) / 2.0,
                    w: cap_w,
                    h: cap_h,
                };
                fill_squircle(target, cap, cap.h / 2.0, text::control_surface(0))?;
                create = DipRect {
                    x: cap.x,
                    y: cap.y,
                    w: slot,
                    h: cap_h,
                };
                browse = DipRect {
                    x: cap.x + slot,
                    y: cap.y,
                    w: slot,
                    h: cap_h,
                };
                folder = DipRect {
                    x: cap.x + slot * 2.0,
                    y: cap.y,
                    w: slot,
                    h: cap_h,
                };
                let dwrite = &fonts.dwrite;
                for (rect, sf) in [
                    (create, "plus"),
                    (browse, "rectangle.stack"),
                    (folder, "folder"),
                ] {
                    crate::design_system::symbols::paint_fluent_in(
                        target,
                        dwrite,
                        sf,
                        DipRect {
                            x: rect.x + (rect.w - glyph) / 2.0,
                            y: rect.y + (rect.h - glyph) / 2.0,
                            w: glyph,
                            h: glyph,
                        },
                        text::secondary_ink(0),
                    )?;
                }
                let footer_h = theme::size::NOTE_FOOTER_HEIGHT;
                text::draw(
                    target,
                    &fonts.trailing,
                    &footer,
                    DipRect {
                        x: 0.0,
                        y: size.height - footer_h,
                        w: size.width,
                        h: footer_h,
                    },
                    text::tertiary_ink(0),
                )?;
                Ok(())
            });
            (*inner).create_rect = create;
            (*inner).browse_rect = browse;
            (*inner).folder_rect = folder;
            (*inner).title_rect = title_rect;
        }
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
    }
}

fn paint_switcher(hwnd: HWND) {
    unsafe {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return;
        }
        let notes = parent_of(hwnd);
        if let Some(inner) = inner_from(notes) {
            if let Some(painter) = (*inner).switcher_painter.as_mut() {
                let _ = painter.paint(hwnd, |target, _fonts| {
                    let size = target.GetSize();
                    fill_squircle(
                        target,
                        DipRect {
                            x: 0.0,
                            y: 0.0,
                            w: size.width,
                            h: size.height,
                        },
                        theme::radius::MENU_PANEL,
                        text::control_surface(0),
                    )?;
                    Ok(())
                });
            }
        }
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
    }
}

unsafe fn escape(hwnd: HWND) {
    if inner_from(hwnd)
        .map(|i| (*i).switcher_open)
        .unwrap_or(false)
    {
        close_switcher_of(hwnd);
        return;
    }
    if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
        (*core).notes_hide();
    }
}

unsafe fn open_switcher_of(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let dpi = GetDpiForWindow(hwnd);
    let (sw, sh) = switcher_size_px(dpi);
    let mut rc = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rc);
    let x = rc.left + ((rc.right - rc.left) - sw) / 2;
    let y = rc.top + dip_scalar_to_px(theme::size::NOTE_SWITCHER_DROP, dpi);
    let _ = SetWindowPos((*inner).switcher, None, x, y, sw, sh, SWP_NOZORDER);
    layout_switcher(hwnd);
    (*inner).switcher_open = true;
    let _ = ShowWindow((*inner).switcher, SW_SHOW);
    let _ = SetFocus((*inner).switcher_edit);
}

unsafe fn close_switcher_of(hwnd: HWND) {
    if let Some(inner) = inner_from(hwnd) {
        (*inner).switcher_open = false;
        let _ = ShowWindow((*inner).switcher, SW_HIDE);
        let _ = SetFocus((*inner).edit);
    }
}

unsafe fn pick_switcher(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let sel = SendMessageW((*inner).switcher_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    if sel < 0 {
        return;
    }
    if let Some(core) = core_from_host((*inner).host) {
        (*core).notes_pick_switcher(sel as usize);
    }
    close_switcher_of(hwnd);
}

fn edit_text(hwnd: HWND) -> String {
    unsafe {
        let Some(inner) = inner_from(hwnd) else {
            return String::new();
        };
        let len = GetWindowTextLengthW((*inner).edit) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW((*inner).edit, &mut buf);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            String::new()
        }
    }
}

fn switcher_query(hwnd: HWND) -> String {
    unsafe {
        let Some(inner) = inner_from(hwnd) else {
            return String::new();
        };
        let len = GetWindowTextLengthW((*inner).switcher_edit) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW((*inner).switcher_edit, &mut buf);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            String::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_titlebar_is_52() {
        assert_eq!(tinycast_pure::layout::notes::titlebar_height(), 52.0);
    }

    #[test]
    fn cue_hides_when_note_has_text() {
        assert_eq!(
            cue_show_cmd(0),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOW
        );
        assert_eq!(
            cue_show_cmd(4),
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE
        );
        let _ = ID_CUE;
    }

    #[test]
    fn charformat_applies_default_and_all() {
        assert!(CHARFORMAT_WPARAMS.contains(&SCF_DEFAULT));
        assert!(CHARFORMAT_WPARAMS.contains(&SCF_ALL));
    }

    #[test]
    fn note_fg_derives_from_note_text() {
        let (r, g, b, a) = crate::design_system::text::note_text(0);
        let bg = 0x1A as f32 / 255.0;
        let comp = |c: f32| ((c * a + bg * (1.0 - a)) * 255.0).round() as u32;
        let expected = COLORREF(comp(b) << 16 | comp(g) << 8 | comp(r));
        assert_eq!(note_fg_colorref(0), expected);
    }

    #[test]
    fn switcher_hwnd_converts_dip() {
        assert_eq!(switcher_size_px(96), (300, 240));
        assert_eq!(switcher_size_px(144), (450, 360));
    }

    #[test]
    fn notes_file_has_no_textout() {
        let src = include_str!("notes.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }

    #[test]
    fn switcher_is_300_by_240() {
        assert_eq!(SWITCHER_WIDTH, 300);
        assert_eq!(SWITCHER_HEIGHT, 240);
        assert_eq!(tinycast_pure::theme::size::NOTE_SWITCHER, (300.0, 240.0));
    }

    #[test]
    fn switcher_commands_reach_filter_and_pick() {
        assert_eq!(
            switcher_command_action(ID_SWITCHER_EDIT, EN_CHANGE),
            SwitcherAction::Filter
        );
        assert_eq!(
            switcher_command_action(ID_SWITCHER_LIST, LBN_SELCHANGE),
            SwitcherAction::Pick
        );
        assert_eq!(
            switcher_command_action(ID_SWITCHER_LIST, LBN_DBLCLK),
            SwitcherAction::Pick
        );
        assert_eq!(
            switcher_command_action(ID_CREATE, EN_CHANGE),
            SwitcherAction::None
        );
    }
}
