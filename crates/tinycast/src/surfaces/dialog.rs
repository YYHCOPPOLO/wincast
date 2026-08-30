//! Tinycast-owned Continue/Cancel dialog. One at a time; Escape cancels.

use std::sync::atomic::{AtomicBool, Ordering};

use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor,
    TextOutW, HGDIOBJ, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, IsWindow, LoadCursorW, PeekMessageW, RegisterClassW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOP, IDC_ARROW, MSG, PM_REMOVE, SWP_NOACTIVATE,
    SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_EX_DLGMODALFRAME, WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::platform::screens::dip_scalar_to_px;

const CLASS: windows::core::PCWSTR = w!("TinycastDialog");
const ID_CONTINUE: usize = 1;
const ID_CANCEL: usize = 2;

static DIALOG_UP: AtomicBool = AtomicBool::new(false);
static LAST_VOLUME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static VOLUME_ACCEPTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmPrompt {
    pub title: String,
    pub message: String,
    pub accept: String,
    pub cancel: String,
}

pub fn begin() -> bool {
    DIALOG_UP
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn end() {
    DIALOG_UP.store(false, Ordering::SeqCst);
}

pub fn is_up() -> bool {
    DIALOG_UP.load(Ordering::SeqCst)
}

/// Continue / Cancel. Escape and clicking outside the card cancel.
pub fn confirm(prompt: &ConfirmPrompt) -> bool {
    run(prompt, true)
}

/// Set Volume slider: ←/→ walk the 5% grid, Enter accepts, Escape cancels.
pub fn pick_volume(current: f32) -> Option<f32> {
    if !begin() {
        return None;
    }
    VOLUME_ACCEPTED.store(false, Ordering::SeqCst);
    LAST_VOLUME.store(current.to_bits(), Ordering::SeqCst);
    let prompt = ConfirmPrompt {
        title: "Set Volume".into(),
        message: "Choose the output volume. Use Left and Right to adjust.".into(),
        accept: "Set Volume".into(),
        cancel: "Cancel".into(),
    };
    let accepted = run_volume(&prompt, current);
    end();
    if accepted {
        Some(f32::from_bits(LAST_VOLUME.load(Ordering::SeqCst)))
    } else {
        None
    }
}

/// Single Continue button for failure reports.
pub fn alert(title: &str, message: &str) {
    let prompt = ConfirmPrompt {
        title: title.to_string(),
        message: message.to_string(),
        accept: "Continue".into(),
        cancel: String::new(),
    };
    if !begin() {
        return;
    }
    let _ = run(&prompt, false);
    end();
}

fn run(prompt: &ConfirmPrompt, has_cancel: bool) -> bool {
    let hwnd = match create_window(prompt, has_cancel, None) {
        Ok(h) => h,
        Err(_) => return false,
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
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_PAINT || msg.message == 0x0012 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        take_result()
    }
}

fn run_volume(prompt: &ConfirmPrompt, current: f32) -> bool {
    let hwnd = match create_window(prompt, true, Some(current)) {
        Ok(h) => h,
        Err(_) => return false,
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
                if vk == 0x25 || vk == 0x27 {
                    nudge_volume(hwnd, vk == 0x27);
                    continue;
                }
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_PAINT || msg.message == 0x0012 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        take_result()
    }
}

struct Inner {
    prompt: ConfirmPrompt,
    has_cancel: bool,
    result: bool,
    continue_rect: RECT,
    cancel_rect: RECT,
    volume: Option<f32>,
}

fn create_window(
    prompt: &ConfirmPrompt,
    has_cancel: bool,
    volume: Option<f32>,
) -> windows::core::Result<HWND> {
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
            prompt: prompt.clone(),
            has_cancel,
            result: false,
            continue_rect: RECT::default(),
            cancel_rect: RECT::default(),
            volume,
        });
        let ptr = Box::into_raw(inner);
        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_DLGMODALFRAME,
            CLASS,
            w!("Tinycast"),
            WS_POPUP | WINDOW_STYLE(0x00C00000), // WS_CAPTION
            0,
            0,
            420,
            200,
            HWND::default(),
            None,
            hinstance,
            Some(ptr as *const core::ffi::c_void),
        ) {
            Ok(h) => h,
            Err(err) => {
                drop(Box::from_raw(ptr));
                return Err(err);
            }
        };
        position(hwnd);
        Ok(hwnd)
    }
}

fn position(hwnd: HWND) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let w = dip_scalar_to_px(theme::size::DIALOG_WIDTH, dpi);
    let h = dip_scalar_to_px(200.0, dpi);
    let screens = crate::platform::screens::screens_px();
    let screen = screens
        .iter()
        .find(|s| s.origin_is_primary)
        .or(screens.first());
    let (x, y) = if let Some(s) = screen {
        let sw = s.work.right - s.work.left;
        let sh = s.work.bottom - s.work.top;
        (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 3)
    } else {
        (100, 100)
    };
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOACTIVATE);
    }
}

fn nudge_volume(hwnd: HWND, up: bool) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        if let Some(level) = (*ptr).volume {
            let next = tinycast_pure::volume::volume_step(level, up);
            (*ptr).volume = Some(next);
            LAST_VOLUME.store(next.to_bits(), Ordering::SeqCst);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, windows::Win32::Foundation::FALSE);
        }
    }
}

fn set_result(hwnd: HWND, value: bool) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if !ptr.is_null() {
            (*ptr).result = value;
            LAST_RESULT.store(if value { 1 } else { 0 }, Ordering::SeqCst);
            if let Some(level) = (*ptr).volume {
                LAST_VOLUME.store(level.to_bits(), Ordering::SeqCst);
            }
        }
    }
}

static LAST_RESULT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn take_result() -> bool {
    LAST_RESULT.swap(0, Ordering::SeqCst) == 1
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        0x0081 => {
            // WM_NCCREATE
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
            hit(hwnd, lparam);
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
            } else if vk == 0x25 || vk == 0x27 {
                nudge_volume(hwnd, vk == 0x27);
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
                LAST_RESULT.store(if (*ptr).result { 1 } else { 0 }, Ordering::SeqCst);
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return;
        }
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let bg = CreateSolidBrush(COLORREF(0x00222222));
        let _ = FillRect(hdc, &rc, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            let _ = EndPaint(hwnd, &ps);
            return;
        }
        let inner = &mut *ptr;
        let dpi = GetDpiForWindow(hwnd);
        let pad = dip_scalar_to_px(theme::spacing::XXL, dpi);
        let btn_h = dip_scalar_to_px(28.0, dpi);
        let btn_w = dip_scalar_to_px(120.0, dpi);
        let cancel_w = dip_scalar_to_px(80.0, dpi);
        let btn_y = rc.bottom - pad - btn_h;
        inner.continue_rect = RECT {
            left: rc.right - pad - btn_w,
            top: btn_y,
            right: rc.right - pad,
            bottom: btn_y + btn_h,
        };
        if inner.has_cancel {
            inner.cancel_rect = RECT {
                left: inner.continue_rect.left - dip_scalar_to_px(theme::spacing::SM, dpi) - cancel_w,
                top: btn_y,
                right: inner.continue_rect.left - dip_scalar_to_px(theme::spacing::SM, dpi),
                bottom: btn_y + btn_h,
            };
        }
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(0x00FFFFFF));
        let mut title: Vec<u16> = inner.prompt.title.encode_utf16().collect();
        title.push(0);
        let _ = TextOutW(hdc, pad, pad, &title);
        SetTextColor(hdc, COLORREF(0x00CCCCCC));
        let mut msg: Vec<u16> = inner.prompt.message.encode_utf16().collect();
        msg.push(0);
        let _ = TextOutW(hdc, pad, pad + dip_scalar_to_px(28.0, dpi), &msg);
        if let Some(level) = inner.volume {
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
            let readout = tinycast_pure::volume::percentage(level);
            let mut vol: Vec<u16> = readout.encode_utf16().collect();
            vol.push(0);
            let _ = TextOutW(hdc, pad, pad + dip_scalar_to_px(56.0, dpi), &vol);
        }
        fill_button(hdc, inner.continue_rect, true);
        SetTextColor(hdc, COLORREF(0x00FFFFFF));
        let mut acc: Vec<u16> = inner.prompt.accept.encode_utf16().collect();
        acc.push(0);
        let _ = TextOutW(
            hdc,
            inner.continue_rect.left + 12,
            inner.continue_rect.top + 6,
            &acc,
        );
        if inner.has_cancel && !inner.prompt.cancel.is_empty() {
            fill_button(hdc, inner.cancel_rect, false);
            let mut can: Vec<u16> = inner.prompt.cancel.encode_utf16().collect();
            can.push(0);
            let _ = TextOutW(
                hdc,
                inner.cancel_rect.left + 12,
                inner.cancel_rect.top + 6,
                &can,
            );
        }
        let _ = EndPaint(hwnd, &ps);
        let _ = ID_CONTINUE;
        let _ = ID_CANCEL;
        let _ = SW_HIDE;
        let _ = WINDOW_EX_STYLE::default();
    }
}

unsafe fn fill_button(hdc: windows::Win32::Graphics::Gdi::HDC, rect: RECT, accent: bool) {
    let color = if accent {
        COLORREF(0x00E08A32)
    } else {
        COLORREF(0x00444444)
    };
    let br = CreateSolidBrush(color);
    let _ = FillRect(hdc, &rect, br);
    let _ = DeleteObject(HGDIOBJ(br.0));
}

fn hit(hwnd: HWND, lparam: LPARAM) {
    let x = (lparam.0 as u32 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as i32;
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        let inner = &*ptr;
        if pt_in(inner.continue_rect, x, y) {
            set_result(hwnd, true);
            let _ = DestroyWindow(hwnd);
            return;
        }
        if inner.has_cancel && pt_in(inner.cancel_rect, x, y) {
            set_result(hwnd, false);
            let _ = DestroyWindow(hwnd);
            return;
        }
    }
}

fn pt_in(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_dialog_is_refused_while_one_is_up() {
        assert!(begin());
        assert!(is_up());
        assert!(!begin());
        end();
        assert!(!is_up());
        assert!(begin());
        end();
    }
}
