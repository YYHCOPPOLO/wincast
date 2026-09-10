//! LL consumer: double-tap modifiers. Installed only while a DoubleTap binding exists.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tinycast_pure::double_tap::DoubleTapDetector;
use tinycast_pure::hotkey::{DoubleTapModifier, HotKeyBinding};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::platform::keyboard_ll;

struct State {
    detector: DoubleTapDetector,
    bindings: Vec<(String, DoubleTapModifier)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn sync(bindings: &[(String, HotKeyBinding)]) {
    let taps: Vec<(String, DoubleTapModifier)> = bindings
        .iter()
        .filter_map(|(action, b)| match b {
            HotKeyBinding::DoubleTap(m) => Some((action.clone(), *m)),
            _ => None,
        })
        .collect();
    if taps.is_empty() {
        if let Ok(mut slot) = STATE.lock() {
            *slot = None;
        }
        keyboard_ll::release(keyboard_ll::DOUBLE_TAP);
        return;
    }
    if let Ok(mut slot) = STATE.lock() {
        *slot = Some(State {
            detector: DoubleTapDetector::new(),
            bindings: taps,
        });
    }
    keyboard_ll::retain(keyboard_ll::DOUBLE_TAP);
}

pub fn on_ll(wparam: windows::Win32::Foundation::WPARAM, info: &KBDLLHOOKSTRUCT) {
    let msg = wparam.0 as u32;
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
    if !down && !up {
        return;
    }
    let Some(modifier) = modifier_of(info.vkCode as u16) else {
        if let Ok(mut slot) = STATE.lock() {
            if let Some(state) = slot.as_mut() {
                state.detector.reset();
            }
        }
        return;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let fired = {
        let Ok(mut slot) = STATE.lock() else {
            return;
        };
        let Some(state) = slot.as_mut() else {
            return;
        };
        if !state.detector.on_flags(modifier, down, now) {
            return;
        }
        state
            .bindings
            .iter()
            .find(|(_, m)| *m == modifier)
            .map(|(a, _)| a.clone())
    };
    if let Some(action) = fired {
        crate::features::hotkeys::service::center::dispatch_action(&action);
    }
}

fn modifier_of(vk: u16) -> Option<DoubleTapModifier> {
    match vk {
        x if x == VK_CONTROL.0 || x == VK_LCONTROL.0 || x == VK_RCONTROL.0 => {
            Some(DoubleTapModifier::Control)
        }
        x if x == VK_MENU.0 || x == VK_LMENU.0 || x == VK_RMENU.0 => {
            Some(DoubleTapModifier::Option)
        }
        x if x == VK_SHIFT.0 || x == VK_LSHIFT.0 || x == VK_RSHIFT.0 => {
            Some(DoubleTapModifier::Shift)
        }
        x if x == VK_LWIN.0 || x == VK_RWIN.0 => Some(DoubleTapModifier::Command),
        _ => None,
    }
}
