pub mod dialog;
mod hud;
mod notes;
mod onboarding;
mod settings;
mod stub;
mod support;
pub mod updates;
pub use hud::MessageHud;
pub use notes::NotesWindow;
pub use onboarding::OnboardingWindow;
pub use settings::SettingsWindow;
pub use stub::StubWindow;
pub use support::{AboutWindow, SupportWindow, CHECKOUT};

#[cfg(test)]
mod i18n_scan {
    #[test]
    fn user_surfaces_do_not_embed_oracle_english() {
        let files = [
            include_str!("../platform/tray.rs"),
            include_str!("onboarding.rs"),
            include_str!("../features/settings/panes/general.rs"),
            include_str!("../palette/menu.rs"),
            include_str!("../app_core.rs"),
            include_str!("dialog.rs"),
            include_str!("support.rs"),
            include_str!("../features/quick_actions/ui/result.rs"),
            include_str!("../features/calendar/ui/preview.rs"),
            include_str!("../features/calendar/ui/coordinator.rs"),
            include_str!("../features/system_actions/ui/coordinator.rs"),
            include_str!("../features/uninstall/ui/coordinator.rs"),
            include_str!("../features/custom_commands/ui/coordinator.rs"),
        ];
        for src in files {
            let impl_src = src.split("#[cfg(test)]").next().unwrap_or(src);
            assert!(!impl_src.contains("w!(\"Settings\")"), "embedded Settings");
            assert!(
                !impl_src.contains("Welcome to Tinycast"),
                "embedded Welcome to Tinycast"
            );
            assert!(
                !impl_src.contains("Show in menu bar"),
                "embedded Show in menu bar"
            );
            assert!(
                !impl_src.contains("label: \"About Tinycast\""),
                "embedded About Tinycast menu"
            );
            assert!(
                !impl_src.contains("label: \"Open\".into()"),
                "embedded Open action"
            );
            assert!(
                !impl_src.contains("\"Actions\""),
                "embedded Actions footer"
            );
            assert!(
                !impl_src.contains("w!(\"Remind me later\")"),
                "embedded Remind me later"
            );
            assert!(
                !impl_src.contains("w!(\"Camera preview\")"),
                "embedded Camera preview"
            );
            assert!(
                !impl_src.contains("\"Camera is ready. Join or cancel.\""),
                "embedded camera ready copy"
            );
        }
    }

    #[test]
    fn placeholder_and_edit_share_search_field_dip() {
        let d2d = include_str!("../palette/d2d.rs");
        let edit = include_str!("../palette/edit.rs");
        assert!(d2d.contains("search_field_dip_with_trailing"));
        assert!(edit.contains("search_field_dip_with_trailing"));
    }
}
