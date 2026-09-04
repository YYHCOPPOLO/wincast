//! Support and About windows. Every `showSupport` route lands here so the reminder anchor moves once.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, SetBkMode, SetTextColor, TextOutW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW, IsWindow,
    LoadCursorW, RegisterClassW, SetWindowLongPtrW, SetWindowTextW, ShowWindow, CREATESTRUCTW,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_LBUTTONDOWN, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
};

use crate::app_core::AppCore;
use crate::features::launcher::ui::coordinator::{execute, LaunchSpec};

pub const CHECKOUT: &str =
    "https://buy.polar.sh/polar_cl_NDVFC20DKQpLcNawsh97QzbARBXD3WNn8v35R0mbJmT";

const SUPPORT_CLASS: windows::core::PCWSTR = w!("TinycastSupport");
const ABOUT_CLASS: windows::core::PCWSTR = w!("TinycastAbout");
const ID_SUPPORT: usize = 1;
const ID_REMINDERS: usize = 2;

pub struct SupportWindow {
    pub hwnd: HWND,
}

pub struct AboutWindow {
    pub hwnd: HWND,
}

struct Inner {
    host: HWND,
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    Support,
    About,
}

impl SupportWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        Ok(Self {
            hwnd: create(host, Kind::Support)?,
        })
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);
        }
    }
}

impl AboutWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        Ok(Self {
            hwnd: create(host, Kind::About)?,
        })
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);
        }
    }
}

impl Drop for SupportWindow {
    fn drop(&mut self) {
        destroy(self.hwnd);
        self.hwnd = HWND::default();
    }
}

impl Drop for AboutWindow {
    fn drop(&mut self) {
        destroy(self.hwnd);
        self.hwnd = HWND::default();
    }
}

fn destroy(hwnd: HWND) {
    unsafe {
        if IsWindow(hwnd).as_bool() {
            let _ = DestroyWindow(hwnd);
        }
    }
}

fn create(host: HWND, kind: Kind) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = match kind {
            Kind::Support => SUPPORT_CLASS,
            Kind::About => ABOUT_CLASS,
        };
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
        let title = match kind {
            Kind::Support => w!("Support Tinycast"),
            Kind::About => w!("About Tinycast"),
        };
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            title,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            240,
            180,
            460,
            320,
            host,
            None,
            hinstance,
            Some(host.0 as *const core::ffi::c_void),
        )?;
        if let Some(inner) = inner_from(hwnd) {
            (*inner).kind = kind;
        }
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Support Tinycast"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | 1),
            40,
            200,
            200,
            36,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_SUPPORT as *mut core::ffi::c_void),
            hinstance,
            None,
        );
        Ok(hwnd)
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
            let inner = Box::new(Inner {
                host: HWND(cs.lpCreateParams),
                kind: Kind::Support,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
            LRESULT(1)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x202020));
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let kind = inner_from(hwnd).map(|i| (*i).kind).unwrap_or(Kind::Support);
            let text = match kind {
                Kind::Support => "Tinycast stays independent if people who use it chip in.\nSecure checkout on Polar.",
                Kind::About => "Tinycast for Windows\nA small launcher. Support keeps it independent.",
            };
            let wide: Vec<u16> = text.encode_utf16().collect();
            TextOutW(hdc, 40, 40, &wide);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            if id == ID_SUPPORT {
                let _ = execute(&LaunchSpec::Uri(CHECKOUT.into()));
                if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                    (*core).support_mark_asked();
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => LRESULT(0),
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
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

#[cfg(test)]
mod tests {
    use super::CHECKOUT;

    #[test]
    fn checkout_is_the_one_polar_link() {
        assert!(CHECKOUT.contains("polar.sh"));
    }
}
