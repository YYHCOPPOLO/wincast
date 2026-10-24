use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, DrawTextW, GetDC, InvalidateRect, ReleaseDC, SelectObject, SetBkMode,
    SetTextColor, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HFONT, HGDIOBJ,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{EM_GETRECT, EM_GETSEL, EM_REPLACESEL, EM_SETMARGINS};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::Ime::{
    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, GCS_COMPSTR, GCS_RESULTSTR,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_CONTROL};
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindow,
    SendMessageW, SetWindowPos, SetWindowTextW, ShowWindow, EC_LEFTMARGIN, EC_RIGHTMARGIN,
    ES_AUTOHSCROLL, ES_LEFT, GWLP_USERDATA, HWND_TOP, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNA,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CHAR, WM_GETFONT, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
    WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEWHEEL,
    WM_NCDESTROY, WM_PAINT, WM_SETFONT, WS_CHILD, WS_VISIBLE,
};

use crate::app_core::AppCore;
use crate::platform::screens::dip_scalar_to_px;

pub const SEARCH_FONT_DIP: f32 = 20.0;
/// The native field is only the text line, centred inside the 40 DIP Slint
/// chrome. A full-height EDIT paints its text at the top instead of centring
/// it, which reads as "not vertically centred" next to the search glyph.
const SEARCH_EDIT_HEIGHT_DIP: f32 = 28.0;
/// Left/right text inset inside the native field (`EM_SETMARGINS`).
const SEARCH_TEXT_INSET_DIP: f32 = 8.0;

const EDIT_ID: usize = 100;
const SUBCLASS_ID: usize = 1;

pub struct SearchEdit {
    pub hwnd: HWND,
    font: HFONT,
    dpi: u32,
}

/// Search field in DIP, after the header icon slot.
#[cfg(test)]
pub fn search_field_dip() -> (f32, f32, f32, f32) {
    search_field_dip_with_trailing(0.0)
}

pub fn search_field_dip_with_trailing(trailing: f32) -> (f32, f32, f32, f32) {
    let r = tinycast_pure::layout::palette_chrome::search_field_rect(
        tinycast_pure::theme::size::PANEL_WIDTH,
        trailing,
    );
    (r.x, r.y, r.w, r.h)
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
            // Let WM_CTLCOLOREDIT supply our opaque light/dark field colors.
            let _ = windows::Win32::UI::Controls::SetWindowTheme(hwnd, w!(""), w!(""));
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
            edit.layout_with_trailing(parent, 0.0);
            Ok(edit)
        }
    }

    pub fn layout_with_trailing(&mut self, parent: HWND, trailing: f32) {
        unsafe {
            let dpi = GetDpiForWindow(parent);
            if dpi != self.dpi {
                self.dpi = dpi;
                self.apply_font(dpi);
            }
            let (x, y, w, h) = search_field_dip_with_trailing(trailing);
            // Keep the opaque child inside the rounded Slint field's border and
            // centre the text line in the 40 DIP chrome.
            let box_y = y + (h - tinycast_pure::theme::size::MAGPIE_SEARCH_HEIGHT) / 2.0;
            let edit_y = box_y
                + (tinycast_pure::theme::size::MAGPIE_SEARCH_HEIGHT - SEARCH_EDIT_HEIGHT_DIP) / 2.0;
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOP,
                dip_scalar_to_px(x, dpi),
                dip_scalar_to_px(edit_y, dpi),
                dip_scalar_to_px(w, dpi),
                dip_scalar_to_px(SEARCH_EDIT_HEIGHT_DIP, dpi),
                SWP_NOACTIVATE,
            );
            let margin = dip_scalar_to_px(SEARCH_TEXT_INSET_DIP, dpi) as isize;
            let _ = SendMessageW(
                self.hwnd,
                EM_SETMARGINS,
                WPARAM((EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize),
                LPARAM((margin & 0xffff) | ((margin & 0xffff) << 16)),
            );
        }
    }

    pub fn focus(&self) {
        unsafe {
            let _ = SetFocus(self.hwnd);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }

    pub fn set_text(&self, text: &str) {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let _ = SetWindowTextW(self.hwnd, PCWSTR(wide.as_ptr()));
        }
    }

    pub fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, true);
        }
    }

    fn apply_font(&mut self, dpi: u32) {
        let px = dip_scalar_to_px(SEARCH_FONT_DIP, dpi);
        let font = crate::design_system::fonts::create_gdi_ui_font(-px);
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

fn ime_string(hwnd: HWND, gcs: windows::Win32::UI::Input::Ime::IME_COMPOSITION_STRING) -> String {
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_invalid() {
            return String::new();
        }
        let n = ImmGetCompositionStringW(himc, gcs, None, 0);
        if n <= 0 {
            let _ = ImmReleaseContext(hwnd, himc);
            return String::new();
        }
        let mut buf = vec![0u16; (n as usize + 1) / 2 + 1];
        let got = ImmGetCompositionStringW(
            himc,
            gcs,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            (buf.len() * 2) as u32,
        );
        let _ = ImmReleaseContext(hwnd, himc);
        if got <= 0 {
            return String::new();
        }
        let chars = (got as usize) / 2;
        String::from_utf16_lossy(&buf[..chars.min(buf.len())])
    }
}

fn insert_committed(hwnd: HWND, text: &str) {
    if text.is_empty() {
        return;
    }
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    unsafe {
        let _ = SendMessageW(
            hwnd,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(wide.as_ptr() as isize),
        );
    }
}

fn composing_from_ime(hwnd: HWND) -> bool {
    composition_bytes(hwnd) > 0 || has_marked_text(hwnd)
}

fn ctrl_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 }
}

/// Draw the hint in the native child's backing store, using the EDIT's own
/// font and formatting rectangle so it shares the typed text's baseline.
/// EM_SETCUEBANNER is deliberately avoided: IME composition owns the cue state.
unsafe fn draw_placeholder(hwnd: HWND, host: HWND) {
    let Some(core) = core_from_host(host) else {
        return;
    };
    if GetWindowTextLengthW(hwnd) != 0
        || !tinycast_pure::palette_state::should_draw_placeholder(&(*core).palette)
    {
        return;
    }
    let mut text: Vec<u16> = (*core).search_placeholder().encode_utf16().collect();
    let hdc = GetDC(hwnd);
    if hdc.is_invalid() {
        return;
    }
    let mut rect = RECT::default();
    let _ = SendMessageW(
        hwnd,
        EM_GETRECT,
        WPARAM(0),
        LPARAM(&mut rect as *mut _ as isize),
    );
    let font = SendMessageW(hwnd, WM_GETFONT, WPARAM(0), LPARAM(0));
    let old_font = SelectObject(hdc, HGDIOBJ(font.0 as *mut core::ffi::c_void));
    let old_color = SetTextColor(
        hdc,
        crate::design_system::appearance::placeholder_colorref((*core).appearance_key()),
    );
    let old_mode = SetBkMode(hdc, TRANSPARENT);
    DrawTextW(
        hdc,
        &mut text,
        &mut rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
    );
    SetBkMode(
        hdc,
        windows::Win32::Graphics::Gdi::BACKGROUND_MODE(old_mode as u32),
    );
    SetTextColor(hdc, old_color);
    SelectObject(hdc, old_font);
    ReleaseDC(hwnd, hdc);
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
        WM_PAINT => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            draw_placeholder(hwnd, host);
            result
        }
        WM_KEYDOWN => {
            if let Some(core) = core_from_host(host) {
                let menu_open = (*core).menu_is_open();
                if (*core).handle_key(wparam.0 as u16) {
                    return LRESULT(0);
                }
                if menu_open {
                    return LRESULT(0);
                }
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_CHAR => {
            let code = wparam.0 as u32;
            let menu_open = core_from_host(host)
                .map(|core| (*core).menu_is_open())
                .unwrap_or(false);
            if menu_open || code == 13 || code == 10 || ctrl_down() {
                return LRESULT(0);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            if let Some(core) = core_from_host(host) {
                // A click in the search field is outside an open menu: close it
                // instead of leaving the menu up while the caret moves.
                if (*core).menu_is_open() {
                    (*core).close_menu();
                } else {
                    (*core).invalidate_palette();
                }
            }
            result
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
                if (*core).menu_is_open() {
                    return LRESULT(0);
                }
                (*core).set_composing(true);
            }
            LRESULT(0)
        }
        WM_IME_COMPOSITION => {
            if let Some(core) = core_from_host(host) {
                if (*core).menu_is_open() {
                    return LRESULT(0);
                }
                let bits = lparam.0 as u32;
                if bits & GCS_RESULTSTR.0 != 0 {
                    insert_committed(hwnd, &ime_string(hwnd, GCS_RESULTSTR));
                    (*core).set_composing(false);
                }
                if bits & GCS_COMPSTR.0 != 0 {
                    (*core).set_composing(composing_from_ime(hwnd));
                }
                (*core).invalidate_palette();
            }
            LRESULT(0)
        }
        WM_IME_ENDCOMPOSITION => {
            if let Some(core) = core_from_host(host) {
                (*core).set_composing(false);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(edit_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_field_matches_chrome_layout() {
        let (x, y, w, h) = search_field_dip();
        let r = tinycast_pure::layout::palette_chrome::search_field_rect(
            tinycast_pure::theme::size::PANEL_WIDTH,
            0.0,
        );
        assert!((x - r.x).abs() < 0.01);
        assert!((y - r.y).abs() < 0.01);
        assert!((h - r.h).abs() < 0.01);
        let _ = w;
    }

    #[test]
    fn search_edit_font_is_yahei_ui() {
        let src = include_str!("edit.rs");
        assert!(src.contains("create_gdi_ui_font"));
        let family = concat!("Microsoft ", "YaHei", " UI");
        assert_eq!(crate::design_system::fonts::gdi_font_family(), family);
    }
}
