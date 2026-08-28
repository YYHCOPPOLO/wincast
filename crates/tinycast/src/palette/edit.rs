use tinycast_pure::theme;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, DeleteObject, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET,
    DEFAULT_PITCH, FW_NORMAL, HFONT, OUT_DEFAULT_PRECIS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_SETMARGINS};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::Ime::{
    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, GCS_COMPSTR,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL};
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindow,
    SendMessageW, SetWindowPos, SetWindowTextW, EC_LEFTMARGIN, EC_RIGHTMARGIN, ES_AUTOHSCROLL,
    ES_LEFT, GWLP_USERDATA, HWND_TOP, SWP_NOACTIVATE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CHAR,
    WM_ERASEBKGND, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_STARTCOMPOSITION, WM_KEYDOWN,
    WM_MOUSEWHEEL, WM_NCDESTROY, WM_SETFONT, WS_CHILD, WS_VISIBLE,
};

use crate::app_core::AppCore;
use crate::platform::screens::dip_scalar_to_px;

pub const SEARCH_FONT_DIP: f32 = 20.0;
pub const PLACEHOLDER_LAUNCHER: &str = "Search";

const EDIT_ID: usize = 100;
const SUBCLASS_ID: usize = 1;

pub struct SearchEdit {
    pub hwnd: HWND,
    font: HFONT,
    dpi: u32,
}

/// Search field in DIP: 20 inset, 44-tall header band.
pub fn search_field_dip() -> (f32, f32, f32, f32) {
    let x = theme::spacing::XXL;
    let y = theme::size::HEADER_PADDING;
    let w = theme::size::PANEL_WIDTH - theme::spacing::XXL * 2.0;
    let h = theme::size::HEADER_HEIGHT;
    (x, y, w, h)
}

impl SearchEdit {
    pub fn create(parent: HWND, host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let style = WS_CHILD
                | WS_VISIBLE
                | WINDOW_STYLE(ES_LEFT as u32)
                | WINDOW_STYLE(ES_AUTOHSCROLL as u32);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("EDIT"),
                w!(""),
                style,
                0,
                0,
                0,
                0,
                parent,
                windows::Win32::UI::WindowsAndMessaging::HMENU(EDIT_ID as *mut core::ffi::c_void),
                hinstance,
                None,
            )?;
            // Visual styles ignore WM_CTLCOLOREDIT; empty theme lets the parent
            // return a hollow brush so the D2D scrim/placeholder show through.
            let _ = windows::Win32::UI::Controls::SetWindowTheme(hwnd, w!(""), w!(""));
            let _ = SendMessageW(
                hwnd,
                EM_SETMARGINS,
                WPARAM((EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize),
                LPARAM(0),
            );
            // EM_SETCUEBANNER is forbidden: cue text jumps under IME composition.
            let _ = SetWindowSubclass(
                hwnd,
                SUBCLASSPROC::Some(edit_subclass),
                SUBCLASS_ID,
                host.0 as usize,
            );
            let mut edit = Self {
                hwnd,
                font: HFONT::default(),
                dpi: 0,
            };
            edit.layout(parent);
            Ok(edit)
        }
    }

    pub fn layout(&mut self, parent: HWND) {
        unsafe {
            let dpi = GetDpiForWindow(parent);
            if dpi != self.dpi {
                self.dpi = dpi;
                self.apply_font(dpi);
            }
            let (x, y, w, h) = search_field_dip();
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOP,
                dip_scalar_to_px(x, dpi),
                dip_scalar_to_px(y, dpi),
                dip_scalar_to_px(w, dpi),
                dip_scalar_to_px(h, dpi),
                SWP_NOACTIVATE,
            );
        }
    }

    pub fn focus(&self) {
        unsafe {
            let _ = SetFocus(self.hwnd);
        }
    }

    pub fn set_text(&self, text: &str) {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let _ = SetWindowTextW(self.hwnd, PCWSTR(wide.as_ptr()));
        }
    }

    fn apply_font(&mut self, dpi: u32) {
        let px = dip_scalar_to_px(SEARCH_FONT_DIP, dpi);
        let font = unsafe {
            CreateFontW(
                -px,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            )
        };
        if font.is_invalid() {
            return;
        }
        unsafe {
            let _ = SendMessageW(self.hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font);
            }
        }
        self.font = font;
    }
}

impl Drop for SearchEdit {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ =
                    RemoveWindowSubclass(self.hwnd, SUBCLASSPROC::Some(edit_subclass), SUBCLASS_ID);
            }
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font);
                self.font = HFONT::default();
            }
        }
        self.hwnd = HWND::default();
    }
}

pub fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        if n <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

fn core_from_host(host: HWND) -> Option<*mut AppCore> {
    let ptr = unsafe { GetWindowLongPtrW(host, GWLP_USERDATA) } as *mut AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

fn has_marked_text(hwnd: HWND) -> bool {
    let result = unsafe { SendMessageW(hwnd, EM_GETSEL, WPARAM(0), LPARAM(0)) };
    let start = result.0 as u32 & 0xffff;
    let end = (result.0 as u32 >> 16) & 0xffff;
    start != end
}

fn composition_bytes(hwnd: HWND) -> i32 {
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_invalid() {
            return 0;
        }
        let n = ImmGetCompositionStringW(himc, GCS_COMPSTR, None, 0);
        let _ = ImmReleaseContext(hwnd, himc);
        n
    }
}

fn composing_from_ime(hwnd: HWND) -> bool {
    composition_bytes(hwnd) > 0 || has_marked_text(hwnd)
}

fn ctrl_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 }
}

unsafe extern "system" fn edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let host = HWND(dwrefdata as *mut core::ffi::c_void);
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_KEYDOWN => {
            if let Some(core) = core_from_host(host) {
                if (*core).handle_key(wparam.0 as u16) {
                    return LRESULT(0);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_CHAR => {
            let code = wparam.0 as u32;
            if code == 13 || code == 10 || ctrl_down() {
                return LRESULT(0);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEWHEEL => {
            if let Some(core) = core_from_host(host) {
                let delta = ((wparam.0 as u32) >> 16) as i16;
                (*core).scroll_list(delta);
            }
            LRESULT(0)
        }
        WM_IME_STARTCOMPOSITION => {
            if let Some(core) = core_from_host(host) {
                (*core).set_composing(true);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_IME_COMPOSITION => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            if let Some(core) = core_from_host(host) {
                (*core).set_composing(composing_from_ime(hwnd));
            }
            result
        }
        WM_IME_ENDCOMPOSITION => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            if let Some(core) = core_from_host(host) {
                (*core).set_composing(false);
            }
            result
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(edit_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}
