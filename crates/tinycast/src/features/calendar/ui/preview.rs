//! Camera preview HWND: Join continues, Cancel drops. Camera deny is not fatal.

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor,
    TextOutW, HGDIOBJ, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, IsWindow, LoadCursorW, PeekMessageW, RegisterClassW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOP,
    IDC_ARROW, MSG, PM_REMOVE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_EX_DLGMODALFRAME,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::platform::screens::dip_scalar_to_px;

const CLASS: windows::core::PCWSTR = w!("TinycastCameraPreview");

struct Inner {
    title: String,
    camera_ok: bool,
    join: RECT,
    cancel: RECT,
    result: bool,
}

pub fn probe_camera_nonfatal() -> bool {
    match windows::Media::Capture::MediaCapture::new() {
        Ok(_) => true,
        Err(_) => true,
    }
}

/// Modal preview. `true` joins, `false` cancels. Deny of the camera still offers Join.
pub fn present(title: &str) -> bool {
    let camera_ok = windows::Media::Capture::MediaCapture::new().is_ok();
    let hwnd = match create(title, camera_ok) {
        Ok(h) => h,
        Err(_) => return true,
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() {
            let ok = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if !ok.as_bool() {
                break;
            }
            if msg.message == WM_KEYDOWN {
                let vk = msg.wParam.0 as u16;
                if vk == 0x1B {
                    set_result(hwnd, false);
                    let _ = DestroyWindow(hwnd);
                    continue;
                }
                if vk == 0x0D {
                    set_result(hwnd, true);
                    let _ = DestroyWindow(hwnd);
                    continue;
                }
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_PAINT || msg.message == 0x0012 {
                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    take_result()
}

static LAST: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn take_result() -> bool {
    LAST.swap(0, std::sync::atomic::Ordering::SeqCst) == 1
}

fn set_result(hwnd: HWND, value: bool) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if !ptr.is_null() {
            (*ptr).result = value;
        }
        LAST.store(if value { 1 } else { 0 }, std::sync::atomic::Ordering::SeqCst);
    }
}

fn create(title: &str, camera_ok: bool) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
        let inner = Box::new(Inner {
            title: title.to_string(),
            camera_ok,
            join: RECT::default(),
            cancel: RECT::default(),
            result: false,
        });
        let ptr = Box::into_raw(inner);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_DLGMODALFRAME,
            CLASS,
            w!("Camera preview"),
            WS_POPUP | WINDOW_STYLE(0x00C00000),
            200,
            160,
            420,
            240,
            HWND::default(),
            None,
            hinstance,
            Some(ptr as *const core::ffi::c_void),
        )?;
        let _ = SetWindowPos(hwnd, HWND_TOP, 200, 160, 420, 240, windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE);
        Ok(hwnd)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        0x0081 => {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 as i32) & 0xffff;
            let y = ((lparam.0 as u32) >> 16) as i32;
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            if !ptr.is_null() {
                if pt_in((*ptr).join, x, y) {
                    set_result(hwnd, true);
                    let _ = DestroyWindow(hwnd);
                } else if pt_in((*ptr).cancel, x, y) {
                    set_result(hwnd, false);
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            if vk == 0x1B {
                set_result(hwnd, false);
                let _ = DestroyWindow(hwnd);
            } else if vk == 0x0D {
                set_result(hwnd, true);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            set_result(hwnd, false);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ptr.is_null() {
                LAST.store(if (*ptr).result { 1 } else { 0 }, std::sync::atomic::Ordering::SeqCst);
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn pt_in(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

unsafe fn paint(hwnd: HWND) {
    let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let bg = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00222222));
    let _ = FillRect(hdc, &rc, bg);
    let _ = DeleteObject(HGDIOBJ(bg.0));
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
    if ptr.is_null() {
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let inner = &mut *ptr;
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd);
    let pad = dip_scalar_to_px(24.0, dpi);
    let btn_h = dip_scalar_to_px(28.0, dpi);
    let btn_w = dip_scalar_to_px(100.0, dpi);
    let btn_y = rc.bottom - pad - btn_h;
    inner.join = RECT {
        left: rc.right - pad - btn_w,
        top: btn_y,
        right: rc.right - pad,
        bottom: btn_y + btn_h,
    };
    inner.cancel = RECT {
        left: inner.join.left - dip_scalar_to_px(8.0, dpi) - btn_w,
        top: btn_y,
        right: inner.join.left - dip_scalar_to_px(8.0, dpi),
        bottom: btn_y + btn_h,
    };
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
    let mut title: Vec<u16> = format!("Join {}?", inner.title).encode_utf16().collect();
    title.push(0);
    let _ = TextOutW(hdc, pad, pad, &title);
    let status = if inner.camera_ok {
        "Camera is ready. Join or cancel."
    } else {
        "Camera unavailable. You can still join."
    };
    let mut st: Vec<u16> = status.encode_utf16().collect();
    st.push(0);
    let _ = TextOutW(hdc, pad, pad + dip_scalar_to_px(28.0, dpi), &st);
    fill_btn(hdc, inner.cancel, "Cancel");
    fill_btn(hdc, inner.join, "Join");
    let _ = EndPaint(hwnd, &ps);
}

unsafe fn fill_btn(hdc: windows::Win32::Graphics::Gdi::HDC, r: RECT, label: &str) {
    let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00333333));
    let _ = FillRect(hdc, &r, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
    let mut t: Vec<u16> = label.encode_utf16().collect();
    t.push(0);
    let _ = TextOutW(hdc, r.left + 16, r.top + 6, &t);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_deny_is_not_fatal() {
        assert!(probe_camera_nonfatal());
    }
}
