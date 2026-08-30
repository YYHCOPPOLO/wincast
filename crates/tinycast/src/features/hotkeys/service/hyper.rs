//! Caps Lock Scan Code Map + right-side Hyper intercept.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tinycast_pure::hotkey::{KeyShortcut, Modifiers};
use windows::core::w;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_BINARY,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_CAPITAL, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::platform::keyboard_ll;

pub const SCAN_HYPER: u32 = 0x6A;
pub const SCAN_CAPS: u32 = 0x3A;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HyperKey {
    None,
    CapsLock,
    RightControl,
    RightShift,
    RightOption,
    RightCommand,
}

impl HyperKey {
    pub fn from_raw(raw: &str) -> Self {
        match raw {
            "capsLock" => HyperKey::CapsLock,
            "rightControl" => HyperKey::RightControl,
            "rightShift" => HyperKey::RightShift,
            "rightOption" => HyperKey::RightOption,
            "rightCommand" => HyperKey::RightCommand,
            _ => HyperKey::None,
        }
    }

    pub fn as_raw(self) -> &'static str {
        match self {
            HyperKey::None => "none",
            HyperKey::CapsLock => "capsLock",
            HyperKey::RightControl => "rightControl",
            HyperKey::RightShift => "rightShift",
            HyperKey::RightOption => "rightOption",
            HyperKey::RightCommand => "rightCommand",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            HyperKey::None => "None",
            HyperKey::CapsLock => "Caps Lock (⇪)",
            HyperKey::RightControl => "Right Control",
            HyperKey::RightShift => "Right Shift",
            HyperKey::RightOption => "Right Alt",
            HyperKey::RightCommand => "Right Win",
        }
    }
}

static KEY: AtomicU8 = AtomicU8::new(0);
static INCLUDE_SHIFT: AtomicBool = AtomicBool::new(false);
static HELD: AtomicBool = AtomicBool::new(false);
static USED: AtomicBool = AtomicBool::new(false);

pub fn configure(key: HyperKey, include_shift: bool) {
    let prev = HyperKey::from_raw(match KEY.load(Ordering::SeqCst) {
        1 => "capsLock",
        2 => "rightControl",
        3 => "rightShift",
        4 => "rightOption",
        5 => "rightCommand",
        _ => "none",
    });
    KEY.store(
        match key {
            HyperKey::None => 0,
            HyperKey::CapsLock => 1,
            HyperKey::RightControl => 2,
            HyperKey::RightShift => 3,
            HyperKey::RightOption => 4,
            HyperKey::RightCommand => 5,
        },
        Ordering::SeqCst,
    );
    INCLUDE_SHIFT.store(include_shift, Ordering::SeqCst);
    HELD.store(false, Ordering::SeqCst);
    if key == HyperKey::CapsLock {
        set_scancode_map(true);
    } else if prev == HyperKey::CapsLock {
        set_scancode_map(false);
    }
    if key == HyperKey::None {
        keyboard_ll::release(keyboard_ll::HYPER);
    } else {
        keyboard_ll::retain(keyboard_ll::HYPER);
    }
}

pub fn shutdown() {
    HELD.store(false, Ordering::SeqCst);
    set_scancode_map(false);
    keyboard_ll::release(keyboard_ll::HYPER);
}

pub fn chord() -> Modifiers {
    Modifiers::hyper_chord(INCLUDE_SHIFT.load(Ordering::SeqCst))
}

pub fn on_ll(wparam: windows::Win32::Foundation::WPARAM, info: &KBDLLHOOKSTRUCT) -> bool {
    let key = KEY.load(Ordering::SeqCst);
    if key == 0 {
        return false;
    }
    let msg = wparam.0 as u32;
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
    if is_hyper_key(key, info) {
        if down {
            HELD.store(true, Ordering::SeqCst);
            USED.store(false, Ordering::SeqCst);
        } else if up {
            HELD.store(false, Ordering::SeqCst);
        }
        return true;
    }
    if HELD.load(Ordering::SeqCst) && down {
        USED.store(true, Ordering::SeqCst);
        let mods = chord();
        crate::features::hotkeys::service::center::dispatch_combo(KeyShortcut {
            vk: info.vkCode as u16,
            modifiers: mods,
        });
        return true;
    }
    false
}

fn is_hyper_key(key: u8, info: &KBDLLHOOKSTRUCT) -> bool {
    match key {
        1 => info.scanCode == SCAN_HYPER || info.vkCode == VK_CAPITAL.0 as u32,
        2 => info.vkCode == VK_RCONTROL.0 as u32,
        3 => info.vkCode == VK_RSHIFT.0 as u32,
        4 => info.vkCode == VK_RMENU.0 as u32,
        5 => info.vkCode == VK_RWIN.0 as u32,
        _ => false,
    }
}

fn set_scancode_map(enable: bool) {
    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layout"),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return;
        }
        if enable {
            // header, flags, count=2 (1 mapping + terminator), map 0x3A -> 0x6A, terminator
            let data: [u8; 24] = [
                0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0x6A, 0, 0x3A, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ];
            let _ = RegSetValueExW(hkey, w!("Scancode Map"), 0, REG_BINARY, Some(&data));
        } else {
            let _ = RegDeleteValueW(hkey, w!("Scancode Map"));
        }
        let _ = RegCloseKey(hkey);
    }
}

#[cfg(test)]
mod tests {
    use super::HyperKey;
    use tinycast_pure::hotkey::{KeyShortcut, Modifiers};

    #[test]
    fn caps_lock_raw_value_is_stable() {
        assert_eq!(HyperKey::CapsLock.as_raw(), "capsLock");
        assert_eq!(HyperKey::from_raw("none"), HyperKey::None);
        let combo = KeyShortcut {
            vk: 0x47,
            modifiers: Modifiers::hyper_chord(false),
        };
        assert!(combo.collapsed_label(Some(Modifiers::hyper_chord(false))).starts_with('✦'));
    }
}
