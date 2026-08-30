use tinycast_pure::hotkey::{default_toggle_palette, HotKeyBinding, Modifiers};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
    VK_SPACE,
};

/// `RegisterHotKey` id for the default palette toggle.
pub const TOGGLE_PALETTE_ID: i32 = 1;

pub fn register_toggle_palette(hwnd: HWND) -> windows::core::Result<()> {
    let HotKeyBinding::Combo(shortcut) = default_toggle_palette() else {
        return unsafe { RegisterHotKey(hwnd, TOGGLE_PALETTE_ID, MOD_ALT, VK_SPACE.0 as u32) };
    };
    unsafe {
        RegisterHotKey(
            hwnd,
            TOGGLE_PALETTE_ID,
            modifiers_to_win32(shortcut.modifiers),
            u32::from(shortcut.vk),
        )
    }
}

pub fn unregister_toggle_palette(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(hwnd, TOGGLE_PALETTE_ID);
    }
}

pub fn pause(hwnd: HWND) {
    unregister_toggle_palette(hwnd);
}

pub fn resume(hwnd: HWND) {
    let _ = register_toggle_palette(hwnd);
}

fn modifiers_to_win32(m: Modifiers) -> HOT_KEY_MODIFIERS {
    let mut flags = HOT_KEY_MODIFIERS(0);
    if m.alt {
        flags |= MOD_ALT;
    }
    if m.ctrl {
        flags |= MOD_CONTROL;
    }
    if m.shift {
        flags |= MOD_SHIFT;
    }
    if m.win {
        flags |= MOD_WIN;
    }
    flags
}
