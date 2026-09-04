//! Quick Action result panel: 520 DIP wide, Replace / Copy / Esc.

use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor,
    TextOutW, HGDIOBJ, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetAncestor, GetClientRect, GetWindowLongPtrW, LoadCursorW,
    PostMessageW, RegisterClassW, SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_HREDRAW,
    CS_VREDRAW, GA_ROOTOWNER, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SW_HIDE,
    SW_SHOWNOACTIVATE, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCDESTROY, WM_PAINT,
    WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::platform::screens::{dip_scalar_to_px, screens_px};

pub const PANEL_WIDTH_DIP: f32 = 520.0;
const CLASS: windows::core::PCWSTR = w!("TinycastQuickAction");

pub struct ResultPanel {
    hwnd: HWND,
}

struct Inner {
    title: String,
    body: String,
}

impl ResultPanel {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            let atom = RegisterClassW(&class);
            if atom == 0 {
                let last = windows::Win32::Foundation::GetLastError();
                if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                    return Err(last.into());
                }
            }
            let inner = Box::new(Inner {
                title: String::new(),
                body: String::new(),
            });
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                CLASS,
                w!(""),
                WS_POPUP,
                0,
                0,
                520,
                240,
                host,
                None,
                hinstance,
                Some(Box::into_raw(inner) as *const core::ffi::c_void),
            )?;
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self, title: &str, body: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).title = title.to_string();
                (*inner).body = body.to_string();
            }
            let dpi = GetDpiForWindow(self.hwnd);
            let w = dip_scalar_to_px(PANEL_WIDTH_DIP, dpi);
            let h = dip_scalar_to_px(280.0, dpi);
            let screens = screens_px();
            let screen = screens
                .iter()
                .find(|s| s.origin_is_primary)
                .or_else(|| screens.first());
            let (x, y) = if let Some(s) = screen {
                let work_w = s.work.right - s.work.left;
                let work_h = s.work.bottom - s.work.top;
                (
                    s.work.left + (work_w - w) / 2,
                    s.work.top + (work_h - h) / 3,
                )
            } else {
                (80, 80)
            };
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE,
            );
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, false);
        }
    }

    pub fn set_body(&self, body: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).body = body.to_string();
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, false);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
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

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_NCCREATE => {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        WM_PAINT => {
            paint(hwnd);
            return LRESULT(0);
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            if vk == 0x1B {
                ShowWindow(hwnd, SW_HIDE);
            }
            if vk == 0x0D {
                let _ = PostMessageW(
                    GetAncestor(hwnd, GA_ROOTOWNER),
                    crate::platform::messages::WM_QA_APPLY,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            let _ = PostMessageW(
                GetAncestor(hwnd, GA_ROOTOWNER),
                crate::platform::messages::WM_QA_APPLY,
                WPARAM(0),
                LPARAM(0),
            );
            return LRESULT(0);
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            return LRESULT(0);
        }
        WM_DESTROY | WM_NCDESTROY => {}
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let bg = CreateSolidBrush(COLORREF(0x00222222));
        FillRect(hdc, &rc, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(0x00F0F0F0));
        if let Some(inner) = inner_from(hwnd) {
            let title: Vec<u16> = (*inner).title.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = TextOutW(hdc, 16, 12, &title);
            let body: Vec<u16> = (*inner).body.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = TextOutW(hdc, 16, 44, &body);
            let hint: Vec<u16> = "Enter Replace · Esc dismiss"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            SetTextColor(hdc, COLORREF(0x00AAAAAA));
            let _ = TextOutW(hdc, 16, rc.bottom - 28, &hint);
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_is_520_dip() {
        assert_eq!(PANEL_WIDTH_DIP, 520.0);
        let _ = theme::spacing::XL;
    }
}
