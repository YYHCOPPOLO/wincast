//! First-launch window: record `togglePalette` with the Settings recorder.

use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, SetBkMode, SetTextColor, TextOutW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW,
    IsWindow, LoadCursorW, RegisterClassW, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, GWLP_WNDPROC, IDC_ARROW, SW_HIDE, SW_SHOW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_KEYUP,
    WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU,
    WS_VISIBLE,
};

use crate::app_core::AppCore;
use crate::features::hotkeys::ui::recorder::Recorder;
use tinycast_pure::hotkey::{CaptureOutcome, HotKeyBinding, Modifiers};

const CLASS: windows::core::PCWSTR = w!("TinycastOnboarding");
const ID_CONTINUE: usize = 1;
const ID_RECORD: usize = 2;

pub struct OnboardingWindow {
    pub hwnd: HWND,
}

struct Inner {
    host: HWND,
    recorder: Recorder,
    record_btn: HWND,
    record_prev: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
}

impl OnboardingWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
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
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS,
                w!("Welcome to Tinycast"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                220,
                160,
                480,
                280,
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
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(self.hwnd).as_bool() }
    }
}

impl Drop for OnboardingWindow {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn current_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            win: GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0,
        }
    }
}

unsafe fn apply_capture(hwnd: HWND, outcome: CaptureOutcome) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    match outcome {
        CaptureOutcome::Ignore => {}
        CaptureOutcome::Cancel | CaptureOutcome::Clear => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).resume_global_hotkeys();
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                hwnd,
                None,
                windows::Win32::Foundation::FALSE,
            );
        }
        CaptureOutcome::Commit(binding) => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).set_hotkey("hotkey.togglePalette", Some(binding));
                (*core).resume_global_hotkeys();
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                hwnd,
                None,
                windows::Win32::Foundation::FALSE,
            );
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let record_btn = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Record shortcut"),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
                40,
                140,
                160,
                32,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_RECORD as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Continue"),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | 1),
                220,
                140,
                120,
                32,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CONTINUE as *mut core::ffi::c_void),
                hinstance,
                None,
            );
            let inner = Box::new(Inner {
                host: HWND(cs.lpCreateParams),
                recorder: Recorder::new(),
                record_btn,
                record_prev: None,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
            if let Some(inner) = inner_from(hwnd) {
                let prev = SetWindowLongPtrW(
                    (*inner).record_btn,
                    GWLP_WNDPROC,
                    record_subclass as usize as isize,
                );
                (*inner).record_prev = Some(std::mem::transmute(prev));
            }
            LRESULT(1)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x202020));
            let _ = GetClientRect(hwnd, &mut RECT::default());
            let recording = inner_from(hwnd)
                .map(|i| (*i).recorder.is_recording())
                .unwrap_or(false);
            let text = if recording {
                "Recording… press the palette shortcut."
            } else {
                "Welcome to Tinycast.\nRecord the palette shortcut, then continue."
            };
            let wide: Vec<u16> = text.encode_utf16().collect();
            TextOutW(hdc, 40, 40, &wide);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            if id == ID_RECORD {
                if let Some(inner) = inner_from(hwnd) {
                    (*inner).recorder.begin("hotkey.togglePalette".into());
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).pause_global_hotkeys();
                    }
                    let _ = SetFocus(hwnd);
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                        hwnd,
                        None,
                        windows::Win32::Foundation::FALSE,
                    );
                }
            }
            if id == ID_CONTINUE {
                if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                    (*core).finish_onboarding();
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if inner_from(hwnd)
                .map(|i| (*i).recorder.is_recording())
                .unwrap_or(false)
            {
                if let Some(inner) = inner_from(hwnd) {
                    let outcome =
                        (*inner)
                            .recorder
                            .on_keydown(wparam.0 as u16, current_modifiers(), now_ms());
                    apply_capture(hwnd, outcome);
                }
            }
            LRESULT(0)
        }
        WM_KEYUP => {
            if inner_from(hwnd)
                .map(|i| (*i).recorder.is_recording())
                .unwrap_or(false)
            {
                if let Some(inner) = inner_from(hwnd) {
                    let outcome = (*inner).recorder.on_keyup(wparam.0 as u16, now_ms());
                    apply_capture(hwnd, outcome);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                (*core).finish_onboarding();
            }
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

unsafe extern "system" fn record_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd).unwrap_or_default();
    if msg == WM_KEYDOWN || msg == WM_KEYUP {
        return wndproc(parent, msg, wparam, lparam);
    }
    let prev = inner_from(parent).and_then(|i| (*i).record_prev);
    if let Some(prev) = prev {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_default_summon_is_alt_space() {
        let b = tinycast_pure::hotkey::default_toggle_palette();
        match b {
            tinycast_pure::hotkey::HotKeyBinding::Combo(s) => {
                assert_eq!(s.vk, 0x20);
                assert!(s.modifiers.alt);
            }
            _ => panic!("expected combo"),
        }
    }

    #[test]
    fn recorder_is_used_for_onboarding_capture() {
        let mut r = Recorder::new();
        assert!(!r.is_recording());
        r.begin("hotkey.togglePalette".into());
        assert!(r.is_recording());
        let _ = now_ms();
        let _ = current_modifiers();
    }
}
