use crate::palette_mode::PaletteMode;

#[derive(Debug, PartialEq)]
pub enum TabHop {
    Clipboard,
    Launcher,
    Ai,
    StayForArguments,
}

pub fn tab_from(mode: PaletteMode, ai_enabled: bool, row_has_arguments: bool) -> TabHop {
    if row_has_arguments {
        return TabHop::StayForArguments;
    }
    match mode {
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
        assert_eq!(tab_from(PaletteMode::Launcher, false, false), TabHop::Clipboard);
        assert_eq!(tab_from(PaletteMode::Clipboard, false, false), TabHop::Launcher);
    }

    #[test]
    fn tab_ring_with_ai() {
        assert_eq!(tab_from(PaletteMode::Launcher, true, false), TabHop::Ai);
        assert_eq!(tab_from(PaletteMode::Ai, true, false), TabHop::Clipboard);
        assert_eq!(tab_from(PaletteMode::Clipboard, true, false), TabHop::Launcher);
    }

    #[test]
    fn arguments_row_stays() {
        assert_eq!(
            tab_from(PaletteMode::Launcher, false, true),
            TabHop::StayForArguments
        );
    }
}
