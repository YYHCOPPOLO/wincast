//! Caps Lock Scan Code Map + right-side Hyper intercept.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};

use tinycast_pure::hotkey::{KeyShortcut, Modifiers};
use windows::core::w;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_BINARY, REG_VALUE_TYPE,
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
static PAUSED: AtomicBool = AtomicBool::new(false);
static LAST_VK: AtomicU16 = AtomicU16::new(0);

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
    LAST_VK.store(0, Ordering::SeqCst);
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

pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
    HELD.store(false, Ordering::SeqCst);
    LAST_VK.store(0, Ordering::SeqCst);
}

pub fn shutdown() {
    HELD.store(false, Ordering::SeqCst);
    LAST_VK.store(0, Ordering::SeqCst);
    set_scancode_map(false);
    keyboard_ll::release(keyboard_ll::HYPER);
}

pub fn chord() -> Modifiers {
    Modifiers::hyper_chord(INCLUDE_SHIFT.load(Ordering::SeqCst))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HyperLl {
    Pass,
    Eat,
    Dispatch,
}

/// Eat the Hyper key itself and bound Hyper+key; unbound follow-up keys pass through.
fn hyper_ll_action(
    paused: bool,
    key_configured: bool,
    is_hyper_key: bool,
    down: bool,
    held: bool,
    has_combo: bool,
    last_vk: u16,
    vk: u16,
) -> HyperLl {
    if paused || !key_configured {
        return HyperLl::Pass;
    }
    if is_hyper_key {
        return HyperLl::Eat;
    }
    if held && down {
        if !has_combo {
            return HyperLl::Pass;
        }
        if last_vk == vk {
            return HyperLl::Eat;
        }
        return HyperLl::Dispatch;
    }
    HyperLl::Pass
}

pub fn on_ll(wparam: windows::Win32::Foundation::WPARAM, info: &KBDLLHOOKSTRUCT) -> bool {
    let paused = PAUSED.load(Ordering::SeqCst);
    let key = KEY.load(Ordering::SeqCst);
    let msg = wparam.0 as u32;
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
    let is_hyper = is_hyper_key(key, info);
    let vk = info.vkCode as u16;
    let held = HELD.load(Ordering::SeqCst);
    let last = LAST_VK.load(Ordering::SeqCst);
    let has = if !paused && key != 0 && held && down && !is_hyper {
        crate::features::hotkeys::service::center::has_combo(KeyShortcut {
            vk,
            modifiers: chord(),
        })
    } else {
        false
    };
    match hyper_ll_action(paused, key != 0, is_hyper, down, held, has, last, vk) {
        HyperLl::Pass => {
            if held && up && last == vk {
                LAST_VK.store(0, Ordering::SeqCst);
            }
            false
        }
        HyperLl::Eat => {
            if is_hyper {
                if down {
                    HELD.store(true, Ordering::SeqCst);
                    LAST_VK.store(0, Ordering::SeqCst);
                } else if up {
                    HELD.store(false, Ordering::SeqCst);
                    LAST_VK.store(0, Ordering::SeqCst);
                }
            }
            true
        }
        HyperLl::Dispatch => {
            LAST_VK.store(vk, Ordering::SeqCst);
            crate::features::hotkeys::service::center::dispatch_combo(KeyShortcut {
                vk,
                modifiers: chord(),
            });
            true
        }
    }
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

fn parse_scancode_map(data: &[u8]) -> Option<Vec<(u16, u16)>> {
    if data.len() < 12 {
        return None;
    }
    let count = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
    if data.len() < 12 + count * 4 {
        return None;
    }
    let mut maps = Vec::new();
    for i in 0..count {
        let off = 12 + i * 4;
        let dest = u16::from_le_bytes(data[off..off + 2].try_into().ok()?);
        let src = u16::from_le_bytes(data[off + 2..off + 4].try_into().ok()?);
        if dest == 0 && src == 0 {
            continue;
        }
        maps.push((dest, src));
    }
    Some(maps)
}

fn encode_scancode_map(maps: &[(u16, u16)]) -> Vec<u8> {
    let mut out = vec![0u8; 12];
    let count = (maps.len() + 1) as u32;
    out[8..12].copy_from_slice(&count.to_le_bytes());
    for (dest, src) in maps {
        out.extend_from_slice(&dest.to_le_bytes());
        out.extend_from_slice(&src.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0, 0, 0]);
    out
}

/// Enable inserts 0x3A→0x6A; disable removes only that source. `None` means delete the value.
pub fn patch_scancode_map(existing: Option<&[u8]>, enable: bool) -> Option<Vec<u8>> {
    let mut maps = existing.and_then(parse_scancode_map).unwrap_or_default();
    maps.retain(|(_, src)| *src != SCAN_CAPS as u16);
    if enable {
        maps.push((SCAN_HYPER as u16, SCAN_CAPS as u16));
    }
    if maps.is_empty() {
        None
    } else {
        Some(encode_scancode_map(&maps))
    }
}

fn set_scancode_map(enable: bool) {
    unsafe {
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layout"),
            0,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return;
        }
        let mut buf = [0u8; 256];
        let mut size = buf.len() as u32;
        let mut kind = REG_VALUE_TYPE(0);
        let query = RegQueryValueExW(
            hkey,
            w!("Scancode Map"),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        );
        let existing = if query == ERROR_SUCCESS && kind == REG_BINARY && size as usize <= buf.len()
        {
            Some(&buf[..size as usize])
        } else {
            None
        };
        match patch_scancode_map(existing, enable) {
            Some(data) => {
                let _ = RegSetValueExW(hkey, w!("Scancode Map"), 0, REG_BINARY, Some(&data));
            }
            None => {
                let _ = RegDeleteValueW(hkey, w!("Scancode Map"));
            }
        }
        let _ = RegCloseKey(hkey);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::hotkey::{KeyShortcut, Modifiers};

    #[test]
    fn caps_lock_raw_value_is_stable() {
        assert_eq!(HyperKey::CapsLock.as_raw(), "capsLock");
        assert_eq!(HyperKey::from_raw("none"), HyperKey::None);
        let combo = KeyShortcut {
            vk: 0x47,
            modifiers: Modifiers::hyper_chord(false),
        };
        assert!(combo
            .collapsed_label(Some(Modifiers::hyper_chord(false)))
            .starts_with('✦'));
    }

    #[test]
    fn patch_scancode_map_preserves_other_entries() {
        let existing = encode_scancode_map(&[(0x3B, 0x02)]);
        let enabled = patch_scancode_map(Some(&existing), true).unwrap();
        let parsed = parse_scancode_map(&enabled).unwrap();
        assert!(parsed.contains(&(SCAN_HYPER as u16, SCAN_CAPS as u16)));
        assert!(parsed.contains(&(0x3B, 0x02)));
        let disabled = patch_scancode_map(Some(&enabled), false).unwrap();
        let parsed = parse_scancode_map(&disabled).unwrap();
        assert!(!parsed.iter().any(|(_, src)| *src == SCAN_CAPS as u16));
        assert!(parsed.contains(&(0x3B, 0x02)));
        assert!(patch_scancode_map(None, false).is_none());
    }

    #[test]
    fn paused_or_unbound_hyper_follow_up_is_not_eaten() {
        assert_eq!(
            hyper_ll_action(true, true, true, true, false, false, 0, 0x14),
            HyperLl::Pass
        );
        assert_eq!(
            hyper_ll_action(false, true, false, true, true, false, 0, 0x47),
            HyperLl::Pass
        );
        assert_eq!(
            hyper_ll_action(false, true, false, true, true, true, 0, 0x47),
            HyperLl::Dispatch
        );
        assert_eq!(
            hyper_ll_action(false, true, false, true, true, true, 0x47, 0x47),
            HyperLl::Eat
        );
        assert_eq!(
            hyper_ll_action(false, true, true, true, false, false, 0, 0x14),
            HyperLl::Eat
        );
    }
}
