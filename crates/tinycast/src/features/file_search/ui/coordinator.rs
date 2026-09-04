use tinycast_pure::file_search::FileSearchHit;
use tinycast_pure::palette_mode::PaletteMode;

use crate::features::launcher::ui::coordinator::{execute, show_in_folder, LaunchSpec};

pub fn guarded_show(enabled: bool) -> bool {
    enabled
}

pub fn should_leave_file_search(enabled: bool, mode: PaletteMode) -> bool {
    !enabled && mode == PaletteMode::FileSearch
}

pub fn open_spec(hit: &FileSearchHit) -> LaunchSpec {
    LaunchSpec::Path(hit.path.replace('/', "\\"))
}

pub fn open(hit: &FileSearchHit) -> windows::core::Result<()> {
    execute(&open_spec(hit))
}

pub fn reveal(hit: &FileSearchHit) -> windows::core::Result<()> {
    show_in_folder(&hit.path.replace('/', "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_is_gated_on_enablement() {
        assert!(!guarded_show(false));
        assert!(guarded_show(true));
        assert!(should_leave_file_search(false, PaletteMode::FileSearch));
        assert!(!should_leave_file_search(true, PaletteMode::FileSearch));
    }
}
