//! Paste into the previously focused window via Ctrl+V.

use std::time::Duration;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

use super::clipboard;

const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);

pub fn paste_text(text: &str, previous: HWND) {
    let _ = clipboard::write_text_marked(text);
    paste_into(previous);
}

pub fn paste_into(previous: HWND) {
    if previous.is_invalid() {
        send_ctrl_v();
        return;
    }
    let bits = previous.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-paste".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            let hwnd = HWND(bits as *mut core::ffi::c_void);
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
            std::thread::sleep(Duration::from_millis(40));
            send_ctrl_v();
        });
}

fn send_ctrl_v() {
    unsafe {
        let mut inputs = [
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ];
        let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
