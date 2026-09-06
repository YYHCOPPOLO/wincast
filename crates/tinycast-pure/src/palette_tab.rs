use crate::palette_mode::PaletteMode;

#[derive(Debug, PartialEq)]
pub enum TabHop {
    Clipboard,
    Launcher,
    Ai,
    StayForArguments,
}

pub fn tab_opens_ai_chat(
    mode: PaletteMode,
    ai_enabled: bool,
    expanded: bool,
    row_has_arguments: bool,
) -> bool {
    expanded && tab_from(mode, ai_enabled, row_has_arguments) == TabHop::Ai
}

pub fn tab_from(mode: PaletteMode, ai_enabled: bool, row_has_arguments: bool) -> TabHop {
    match mode {
        PaletteMode::ExtensionCommand => TabHop::Launcher,
        _ if row_has_arguments => TabHop::StayForArguments,
        PaletteMode::Launcher => {
            if ai_enabled {
                TabHop::Ai
            } else {
                TabHop::Clipboard
            }
        }
        PaletteMode::Ai | PaletteMode::AiHistory => {
            if ai_enabled {
                TabHop::Clipboard
            } else {
                TabHop::Launcher
            }
        }
        PaletteMode::Clipboard => TabHop::Launcher,
        _ => TabHop::Launcher,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_ring_without_ai() {
        assert_eq!(
            tab_from(PaletteMode::Launcher, false, false),
            TabHop::Clipboard
        );
        assert_eq!(
            tab_from(PaletteMode::Clipboard, false, false),
            TabHop::Launcher
        );
    }

    #[test]
    fn tab_ring_with_ai() {
        assert_eq!(tab_from(PaletteMode::Launcher, true, false), TabHop::Ai);
        assert_eq!(tab_from(PaletteMode::Ai, true, false), TabHop::Clipboard);
        assert_eq!(
            tab_from(PaletteMode::Clipboard, true, false),
            TabHop::Launcher
        );
    }

    #[test]
    fn arguments_row_stays() {
        assert_eq!(
            tab_from(PaletteMode::Launcher, false, true),
            TabHop::StayForArguments
        );
    }

    #[test]
    fn compact_never_advertises_tab() {
        assert!(!tab_opens_ai_chat(PaletteMode::Launcher, true, false, false));
        assert!(tab_opens_ai_chat(PaletteMode::Launcher, true, true, false));
        assert!(!tab_opens_ai_chat(PaletteMode::Launcher, false, true, false));
    }

    #[test]
    fn extension_command_mode_is_never_the_tab_ring() {
        assert_eq!(
            tab_from(PaletteMode::ExtensionCommand, false, false),
            TabHop::Launcher
        );
        assert_eq!(
            tab_from(PaletteMode::ExtensionCommand, true, true),
            TabHop::Launcher
        );
    }
}
