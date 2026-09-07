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
        ];
        for src in files {
            let impl_src = src.split("#[cfg(test)]").next().unwrap_or(src);
            assert!(!impl_src.contains("w!(\"Settings\")"));
            assert!(!impl_src.contains("Welcome to Tinycast"));
            assert!(!impl_src.contains("Show in menu bar"));
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
