//! Settings recorder: not focusable; local capture while recordingAction is set.

use tinycast_pure::double_tap::DoubleTapDetector;
use tinycast_pure::hotkey::{
    capture_keydown, CaptureOutcome, DoubleTapModifier, HotKeyBinding, Modifiers,
};

pub struct Recorder {
    pub action: Option<String>,
    detector: DoubleTapDetector,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            action: None,
            detector: DoubleTapDetector::new(),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.action.is_some()
    }

    pub fn begin(&mut self, action: String) {
        self.action = Some(action);
        self.detector.reset();
    }

    pub fn cancel(&mut self) {
        self.action = None;
        self.detector.reset();
    }

    pub fn on_keydown(&mut self, vk: u16, modifiers: Modifiers, now_ms: u64) -> CaptureOutcome {
        if self.action.is_none() {
            return CaptureOutcome::Ignore;
        }
        if let Some(m) = double_tap_mod(vk) {
            if self.detector.on_flags(m, true, now_ms) {
                return CaptureOutcome::Commit(HotKeyBinding::DoubleTap(m));
            }
            return CaptureOutcome::Ignore;
        }
        capture_keydown(vk, modifiers)
    }

    pub fn on_keyup(&mut self, vk: u16, now_ms: u64) -> CaptureOutcome {
        if self.action.is_none() {
            return CaptureOutcome::Ignore;
        }
        if let Some(m) = double_tap_mod(vk) {
            if self.detector.on_flags(m, false, now_ms) {
                return CaptureOutcome::Commit(HotKeyBinding::DoubleTap(m));
            }
        }
        CaptureOutcome::Ignore
    }
}

fn double_tap_mod(vk: u16) -> Option<DoubleTapModifier> {
    match vk {
        0x11 | 0xA2 | 0xA3 => Some(DoubleTapModifier::Control),
        0x12 | 0xA4 | 0xA5 => Some(DoubleTapModifier::Option),
        0x10 | 0xA0 | 0xA1 => Some(DoubleTapModifier::Shift),
        0x5B | 0x5C => Some(DoubleTapModifier::Command),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_is_not_focusable_state() {
        let mut r = Recorder::new();
        assert!(!r.is_recording());
        r.begin("hotkey.togglePalette".into());
        assert!(r.is_recording());
        r.cancel();
        assert!(!r.is_recording());
    }
}
