//! Combo RegisterHotKey (MOD_NOREPEAT), conflict lookup, dispatch.

use std::sync::Mutex;

use tinycast_pure::hotkey::{registered_combos, HotKeyBinding, KeyShortcut, Modifiers};
use tinycast_pure::hotkey_store::HotKeyStore;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::platform::messages::WM_HOTKEY_ACTION;

static PENDING: Mutex<Option<String>> = Mutex::new(None);
static MAP: Mutex<Vec<(i32, String, KeyShortcut)>> = Mutex::new(Vec::new());
static HOST: Mutex<isize> = Mutex::new(0);
static PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_host(host: HWND) {
    if let Ok(mut slot) = HOST.lock() {
        *slot = host.0 as isize;
    }
}

pub fn pause(hwnd: HWND) {
    // Hyper first: the LL hook runs on another thread and must pass through
    // while the recorder is capturing.
    crate::features::hotkeys::service::hyper::set_paused(true);
    PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
    unregister_all(hwnd);
    crate::features::hotkeys::service::double_tap_monitor::sync(&[]);
}

pub fn resume(hwnd: HWND, store: &HotKeyStore) {
    PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
    crate::features::hotkeys::service::hyper::set_paused(false);
    sync(hwnd, store);
}

pub fn sync(hwnd: HWND, store: &HotKeyStore) {
    unregister_all(hwnd);
    if PAUSED.load(std::sync::atomic::Ordering::SeqCst) {
        crate::features::hotkeys::service::double_tap_monitor::sync(&[]);
        return;
    }
    let snapshot = store_bindings(store);
    crate::features::hotkeys::service::double_tap_monitor::sync(&snapshot);
    let mut next_id = 1i32;
    let mut map = Vec::new();
    for (action, shortcut) in registered_combos(&snapshot) {
        if register(hwnd, next_id, shortcut) {
            map.push((next_id, action, shortcut));
            next_id += 1;
        }
    }
    if let Ok(mut slot) = MAP.lock() {
        *slot = map;
    }
}

pub fn on_hotkey_id(id: i32) {
    let action = MAP.lock().ok().and_then(|m| {
        m.iter()
            .find(|(i, _, _)| *i == id)
            .map(|(_, a, _)| a.clone())
    });
    if let Some(action) = action {
        dispatch_action(&action);
    }
}

pub fn has_combo(shortcut: KeyShortcut) -> bool {
    MAP.lock()
        .ok()
        .map(|m| m.iter().any(|(_, _, s)| *s == shortcut))
        .unwrap_or(false)
}

pub fn dispatch_combo(shortcut: KeyShortcut) -> bool {
    let action = MAP.lock().ok().and_then(|m| {
        m.iter()
            .find(|(_, _, s)| *s == shortcut)
            .map(|(_, a, _)| a.clone())
    });
    if let Some(action) = action {
        dispatch_action(&action);
        true
    } else {
        false
    }
}

pub fn dispatch_action(action: &str) {
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(action.to_string());
    }
    let host = HOST.lock().ok().map(|g| *g).unwrap_or(0);
    let hwnd = HWND(host as *mut core::ffi::c_void);
    if !hwnd.is_invalid() {
        unsafe {
            let _ = PostMessageW(hwnd, WM_HOTKEY_ACTION, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn take_pending() -> Option<String> {
    PENDING.lock().ok()?.take()
}

pub fn conflict_owner(
    store: &HotKeyStore,
    binding: &HotKeyBinding,
    excluding: &str,
) -> Option<String> {
    store_bindings(store)
        .into_iter()
        .find(|(action, b)| action != excluding && b == binding)
        .map(|(action, _)| action)
}

fn store_bindings(store: &HotKeyStore) -> Vec<(String, HotKeyBinding)> {
    store.snapshot()
}

fn register(hwnd: HWND, id: i32, shortcut: KeyShortcut) -> bool {
    unsafe {
        RegisterHotKey(
            hwnd,
            id,
            modifiers_to_win32(shortcut.modifiers) | MOD_NOREPEAT,
            u32::from(shortcut.vk),
        )
        .is_ok()
    }
}

fn unregister_all(hwnd: HWND) {
    let ids: Vec<i32> = MAP
        .lock()
        .ok()
        .map(|m| m.iter().map(|(id, _, _)| *id).collect())
        .unwrap_or_default();
    for id in ids {
        unsafe {
            let _ = UnregisterHotKey(hwnd, id);
        }
    }
    if let Ok(mut slot) = MAP.lock() {
        slot.clear();
    }
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

#[cfg(test)]
mod tests {
    use super::conflict_owner;
    use tinycast_pure::hotkey::{HotKeyBinding, KeyShortcut, Modifiers};
    use tinycast_pure::hotkey_store::HotKeyStore;

    #[test]
    fn conflict_detects_same_combo() {
        let mut store = HotKeyStore::default();
        let combo = HotKeyBinding::Combo(KeyShortcut {
            vk: 0x43,
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                win: false,
            },
        });
        store.set("hotkey.togglePalette".into(), Some(combo.clone()));
        assert_eq!(
            conflict_owner(&store, &combo, "other"),
            Some("hotkey.togglePalette".into())
        );
        assert_eq!(conflict_owner(&store, &combo, "hotkey.togglePalette"), None);
    }

    #[test]
    fn clearing_or_double_tapping_toggle_palette_does_not_keep_alt_space() {
        use tinycast_pure::hotkey::{default_toggle_palette, registered_combos, DoubleTapModifier};
        let empty = HotKeyStore::default();
        assert!(registered_combos(&empty.snapshot()).is_empty());
        let mut store = HotKeyStore::default();
        store.set(
            "hotkey.togglePalette".into(),
            Some(HotKeyBinding::DoubleTap(DoubleTapModifier::Control)),
        );
        assert!(registered_combos(&store.snapshot())
            .iter()
            .all(|(a, _)| a != "hotkey.togglePalette"));
        store.set(
            "hotkey.togglePalette".into(),
            Some(default_toggle_palette()),
        );
        assert_eq!(registered_combos(&store.snapshot()).len(), 1);
    }
}
