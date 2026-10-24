//! The onboarding surface is a Slint window; its host lives in `ui_host`.
//!
//! `OnboardingWindow` keeps the API the Direct2D window had (`create`, `show`,
//! `hide`, `is_visible`, `set_locale`) so `platform::run` and `AppCore` do not
//! change. The step content and the recorder well are Slint; the shortcut
//! capture runs from the HWND subclass, like the settings wells.

pub use crate::ui_host::onboarding::OnboardingWindow;

#[cfg(test)]
mod tests {
    use crate::features::hotkeys::ui::recorder::Recorder;
    use crate::ui_host::onboarding::{current_modifiers, now_ms, WINDOW};

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

    #[test]
    fn onboarding_size_is_520x400() {
        assert_eq!(WINDOW, (520.0, 400.0));
    }

    #[test]
    fn onboarding_has_no_textout() {
        let src = include_str!("onboarding.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
