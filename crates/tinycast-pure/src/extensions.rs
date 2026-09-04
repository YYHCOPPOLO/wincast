use crate::app_entry::AppEntry;

/// Shown on the Extensions settings pane and returned when enable is refused.
pub const RUNTIME_ABSENT: &str = "The extension runtime is not in this version.";

/// Consent to run third-party JavaScript. v1 ships no engine, so this never succeeds.
pub fn try_set_extensions_enabled(_on: bool) -> Result<(), &'static str> {
    Err(RUNTIME_ABSENT)
}

/// Extension commands never reach launcher search while the runtime is absent.
pub fn launcher_entries(_enabled: bool, _show_in_launcher: bool) -> Vec<AppEntry> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette_mode::PaletteMode;
    use crate::palette_tab::{tab_from, TabHop};

    #[test]
    fn extensions_cannot_be_enabled() {
        assert!(try_set_extensions_enabled(true).is_err());
        assert_eq!(
            try_set_extensions_enabled(true).unwrap_err(),
            RUNTIME_ABSENT
        );
        assert!(try_set_extensions_enabled(false).is_err());
    }

    #[test]
    fn extension_command_mode_is_never_the_tab_ring() {
        assert_ne!(
            tab_from(PaletteMode::Launcher, true, false),
            TabHop::StayForArguments
        );
        // ExtensionCommand is not a Tab hop
        match tab_from(PaletteMode::ExtensionCommand, false, false) {
            TabHop::Launcher => {}
            other => panic!("{other:?}"),
        }
        assert_eq!(
            tab_from(PaletteMode::ExtensionCommand, true, true),
            TabHop::Launcher
        );
    }

    #[test]
    fn extensions_publish_no_launcher_rows() {
        assert!(launcher_entries(true, true).is_empty());
        assert!(launcher_entries(false, true).is_empty());
    }
}
