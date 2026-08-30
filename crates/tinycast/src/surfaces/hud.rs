//! Shared non-activating click-through message HUD.

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
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowTextW, IsWindow,
    KillTimer, LoadCursorW, RegisterClassW, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE,
    WM_DESTROY, WM_NCDESTROY, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::platform::screens::{dip_scalar_to_px, screens_px};

const CLASS: windows::core::PCWSTR = w!("TinycastHud");
const HIDE_TIMER: usize = 1;
const HIDE_MS: u32 = 1600;

pub struct MessageHud {
    hwnd: HWND,
}

impl MessageHud {
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
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                CLASS,
                w!(""),
                WS_POPUP,
                0,
                0,
                200,
                48,
                host,
                None,
                hinstance,
                None,
            )?;
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self, message: &str) {
        let mut wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = SetWindowTextW(self.hwnd, windows::core::PCWSTR(wide.as_mut_ptr()));
            position(self.hwnd);
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = KillTimer(self.hwnd, HIDE_TIMER);
            let _ = SetTimer(self.hwnd, HIDE_TIMER, HIDE_MS, None);
        }
    }
}

impl Drop for MessageHud {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

fn position(hwnd: HWND) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let w = dip_scalar_to_px(theme::size::HUD_WIDTH, dpi);
    let h = dip_scalar_to_px(48.0, dpi);
    let screens = screens_px();
    let (x, y) = if let Some(s) = screens.first() {
        let work = s.work;
        (
            work.left + (work.right - work.left - w) / 2,
            work.bottom - h - dip_scalar_to_px(theme::size::HUD_EDGE_OFFSET, dpi),
        )
    } else {
        (80, 80)
    };
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wparam.0 == HIDE_TIMER {
                let _ = KillTimer(hwnd, HIDE_TIMER);
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint(hwnd, hdc);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY | WM_NCDESTROY => DefWindowProcW(hwnd, msg, wparam, lparam),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let brush = CreateSolidBrush(COLORREF(0x00222222));
        FillRect(hdc, &rc, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(0x00FFFFFF));
        let mut text = [0u16; 256];
        let n = GetWindowTextW(hwnd, &mut text);
        if n > 0 {
            let x = rc.left + 12;
            let y = rc.top + 14;
            let _ = TextOutW(hdc, x, y, &text[..n as usize]);
        }
    }
}
