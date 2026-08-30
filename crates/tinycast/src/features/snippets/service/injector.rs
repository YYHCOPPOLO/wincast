//! Insert expanded snippet text into the previously focused window.

use std::time::Duration;

use unicode_segmentation::UnicodeSegmentation;
use windows::core::{Interface, BSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationValuePattern, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_LEFT,
};
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, SetForegroundWindow};

/// `dwExtraInfo` tag so the keyword hook can ignore Tinycast-generated key events.
pub const SYNTHETIC_EXTRA: usize = 0x5443_5354;

pub fn is_synthetic(extra: usize) -> bool {
    extra == SYNTHETIC_EXTRA
}

pub fn delete_chars(count: usize) {
    for _ in 0..count {
        send_vk(windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK);
    }
}

pub fn inject_into(previous: HWND, text: &str, cursor: Option<usize>) {
    let payload = text.to_string();
    let bits = previous.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-inject".into())
        .spawn(move || {
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            std::thread::sleep(Duration::from_millis(80));
            let hwnd = HWND(bits as *mut core::ffi::c_void);
            if !hwnd.is_invalid() {
                restore_foreground(hwnd);
                std::thread::sleep(Duration::from_millis(40));
            }
            if !uia_set_value_if_empty(&payload) {
                send_unicode(&payload);
            }
            if let Some(at) = cursor {
                let total = payload.graphemes(true).count();
                if at < total {
                    for _ in 0..(total - at) {
                        send_vk(VK_LEFT);
                    }
                }
            }
        });
}

fn restore_foreground(target: HWND) -> bool {
    unsafe {
        let _ = AllowSetForegroundWindow(u32::MAX);
        SetForegroundWindow(target).as_bool()
    }
}

fn uia_set_value_if_empty(text: &str) -> bool {
    unsafe {
        let automation: IUIAutomation =
            match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                Ok(a) => a,
                Err(_) => return false,
            };
        let Ok(element) = automation.GetFocusedElement() else {
            return false;
        };
        let Ok(unk) = element.GetCurrentPattern(UIA_ValuePatternId) else {
            return false;
        };
        let Ok(pattern) = unk.cast::<IUIAutomationValuePattern>() else {
            return false;
        };
        let current = pattern.CurrentValue().unwrap_or_default();
        if !current.is_empty() {
            return false;
        }
        pattern.SetValue(&BSTR::from(text)).is_ok()
    }
}

fn send_unicode(text: &str) {
    for unit in text.encode_utf16() {
        unicode_key(unit, false);
        unicode_key(unit, true);
    }
}

fn send_vk(vk: VIRTUAL_KEY) {
    key(vk, false);
    key(vk, true);
}

fn unicode_key(scan: u16, up: bool) {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let mut inputs = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SYNTHETIC_EXTRA,
            },
        },
    }];
    unsafe {
        let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) {
    let mut inputs = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: SYNTHETIC_EXTRA,
            },
        },
    }];
    unsafe {
        let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_tag_is_stable() {
        assert!(is_synthetic(SYNTHETIC_EXTRA));
        assert!(!is_synthetic(0));
    }
}
