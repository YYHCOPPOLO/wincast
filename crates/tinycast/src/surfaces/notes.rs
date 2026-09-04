//! Titled Notes window: RichEdit source editor and a 300×240 switcher.

use std::path::PathBuf;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};

use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindow, LoadCursorW, MoveWindow,
    RegisterClassW, SendMessageW, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW,
    ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, GWLP_WNDPROC, IDC_ARROW,
    LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_NCCREATE, WM_NCDESTROY, WM_SIZE,
    WM_TIMER, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_TOOLWINDOW,
    WS_OVERLAPPEDWINDOW, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
};

use crate::app_core::AppCore;
use crate::platform::paths;

const CLASS: windows::core::PCWSTR = w!("TinycastNotes");
const SWITCHER_CLASS: windows::core::PCWSTR = w!("TinycastNoteSwitcher");
pub const SWITCHER_WIDTH: i32 = 300;
pub const SWITCHER_HEIGHT: i32 = 240;
const ID_EDIT: usize = 101;
const ID_CREATE: usize = 102;
const ID_BROWSE: usize = 103;
const ID_FOLDER: usize = 104;
const ID_SWITCHER_EDIT: usize = 201;
const ID_SWITCHER_LIST: usize = 202;
const SAVE_TIMER: usize = 1;
const SAVE_MS: u32 = 300;
const ES_MULTILINE: WINDOW_STYLE = WINDOW_STYLE(0x0004);
const ES_AUTOVSCROLL: WINDOW_STYLE = WINDOW_STYLE(0x0040);
const ES_WANTRETURN: WINDOW_STYLE = WINDOW_STYLE(0x1000);
const LBS_NOTIFY: WINDOW_STYLE = WINDOW_STYLE(0x0001);

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
}

impl NotesWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let _ = LoadLibraryW(w!("Msftedit.dll"));
            let hinstance = GetModuleHandleW(None)?;
            register(CLASS, Some(wndproc))?;
            register(SWITCHER_CLASS, Some(switcher_wndproc))?;
            let frame = load_frame();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS,
                w!("Notes"),
                WS_OVERLAPPEDWINDOW | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME,
                frame.0,
                frame.1,
                frame.2,
                frame.3,
                host,
                None,
                hinstance,
                Some(host.0 as *const core::ffi::c_void),
            )?;
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
                let _ = SendMessageW((*inner).switcher_list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
                for title in titles {
                    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = SendMessageW(
                        (*inner).switcher_list,
                        LB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(wide.as_ptr() as isize),
                    );
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
    proc: Option<
        unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    >,
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
                WS_EX_CLIENTEDGE,
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
                36,
                400,
                300,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_EDIT as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Create"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                8,
                6,
                72,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CREATE as *mut core::ffi::c_void),
                hinstance,
                None,
            );
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Browse"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                88,
                6,
                72,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_BROWSE as *mut core::ffi::c_void),
                hinstance,
                None,
            );
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Folder"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                168,
                6,
                72,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_FOLDER as *mut core::ffi::c_void),
                hinstance,
                None,
            );
            let switcher = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                SWITCHER_CLASS,
                w!(""),
                WS_POPUP | WS_BORDER,
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
            let inner = Box::new(Inner {
                host,
                edit,
                switcher,
                switcher_edit,
                switcher_list,
                switcher_open: false,
                edit_prev: None,
                switcher_prev: None,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
            if let Some(inner) = inner_from(hwnd) {
                let prev = SetWindowLongPtrW((*inner).edit, GWLP_WNDPROC, edit_subclass as usize as isize);
                (*inner).edit_prev = Some(std::mem::transmute(prev));
                let prev = SetWindowLongPtrW(
                    (*inner).switcher_edit,
                    GWLP_WNDPROC,
                    switcher_edit_subclass as usize as isize,
                );
                (*inner).switcher_prev = Some(std::mem::transmute(prev));
            }
            LRESULT(1)
        }
        WM_SIZE => {
            layout(hwnd);
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
                ID_EDIT if notify == 0x0300 => {
                    let _ = SetTimer(hwnd, SAVE_TIMER, SAVE_MS, None);
                }
                ID_SWITCHER_EDIT if notify == 0x0300 => {
                    if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                        (*core).notes_filter_switcher(&switcher_query(hwnd));
                    }
                }
                ID_SWITCHER_LIST if notify == 1 => {
                    pick_switcher(hwnd);
                }
                _ => {}
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
                drop(Box::from_raw(ptr as *mut Inner));
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
    if msg == WM_KEYDOWN && wparam.0 as u16 == 0x1B {
        let parent = parent_of(hwnd);
        close_switcher_of(parent);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
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
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd).unwrap_or_default()
    }
}

unsafe fn layout(hwnd: HWND) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    if let Some(inner) = inner_from(hwnd) {
        let _ = MoveWindow((*inner).edit, 8, 36, w - 16, h - 44, true);
    }
}

unsafe fn escape(hwnd: HWND) {
    if inner_from(hwnd).map(|i| (*i).switcher_open).unwrap_or(false) {
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
    let mut rc = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rc);
    let x = rc.left + ((rc.right - rc.left) - SWITCHER_WIDTH) / 2;
    let y = rc.top + 80;
    let _ = SetWindowPos(
        (*inner).switcher,
        None,
        x,
        y,
        SWITCHER_WIDTH,
        SWITCHER_HEIGHT,
        SWP_NOZORDER,
    );
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

fn load_frame() -> (i32, i32, i32, i32) {
    let default = (120, 120, 560, 420);
    let Ok(bytes) = std::fs::read(placement_path()) else {
        return default;
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return default;
    };
    (
        v["x"].as_i64().unwrap_or(120) as i32,
        v["y"].as_i64().unwrap_or(120) as i32,
        v["w"].as_i64().unwrap_or(560) as i32,
        v["h"].as_i64().unwrap_or(420) as i32,
    )
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
    fn switcher_is_300_by_240() {
        assert_eq!(SWITCHER_WIDTH, 300);
        assert_eq!(SWITCHER_HEIGHT, 240);
    }
}
