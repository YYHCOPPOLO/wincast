//! Settings recorder: not focusable; local capture while recordingAction is set.

use tinycast_pure::double_tap::DoubleTapDetector;
use tinycast_pure::hotkey::{
    capture_keydown, CaptureOutcome, DoubleTapModifier, HotKeyBinding, Modifiers,
};
use tinycast_pure::layout::settings::recorder_well;
use tinycast_pure::palette_placement::DipRect;

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

pub fn well_in_row(row: DipRect) -> DipRect {
    let well = recorder_well();
    DipRect {
        x: row.x + row.w - crate::design_system::settings::CARD_PAD - well.w,
        y: row.y + (row.h - well.h) / 2.0,
        w: well.w,
        h: well.h,
    }
}

pub fn well_caption(binding: Option<&str>, listening: bool) -> String {
    well_caption_lang(binding, listening, tinycast_pure::i18n::UiLang::En)
}

pub fn well_caption_lang(
    binding: Option<&str>,
    listening: bool,
    lang: tinycast_pure::i18n::UiLang,
) -> String {
    if listening {
        tinycast_pure::i18n::listening_label(lang).into()
    } else if let Some(text) = binding.filter(|s| !s.is_empty()) {
        text.to_string()
    } else {
        tinycast_pure::i18n::record_label(lang).into()
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

    #[test]
    fn well_caption_shows_record_listening_or_binding() {
        assert_eq!(well_caption(None, false), "Record");
        assert_eq!(well_caption(Some(""), false), "Record");
        assert_eq!(well_caption(None, true), "Listening…");
        assert_eq!(well_caption(Some("Ctrl+Space"), false), "Ctrl+Space");
        assert_eq!(well_caption(Some("Ctrl+Space"), true), "Listening…");
    }

    #[test]
    fn recorder_commits_double_tap_on_second_release() {
        let mut r = Recorder::new();
        r.begin("hotkey.togglePalette".into());
        let none = Modifiers::none();
        assert_eq!(r.on_keydown(0x11, none, 0), CaptureOutcome::Ignore);
        assert_eq!(r.on_keyup(0x11, 100), CaptureOutcome::Ignore);
        assert_eq!(r.on_keydown(0x11, none, 200), CaptureOutcome::Ignore);
        match r.on_keyup(0x11, 300) {
            CaptureOutcome::Commit(HotKeyBinding::DoubleTap(DoubleTapModifier::Control)) => {}
            other => panic!("{other:?}"),
        }
    }
}
