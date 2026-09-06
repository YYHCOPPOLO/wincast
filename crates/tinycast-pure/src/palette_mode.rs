#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteMode {
    Launcher,
    Clipboard,
    CalculatorHistory,
    Emoji,
    FileSearch,
    Schedule,
    Uninstall,
    Quicklinks,
    QuicklinkArguments,
    Ai,
    AiHistory,
    ExtensionCommand,
}

impl PaletteMode {
    pub fn placeholder(self) -> &'static str {
        match self {
            PaletteMode::Launcher => "Search for apps and commands…",
            PaletteMode::Clipboard => "Type to filter entries…",
            PaletteMode::Ai => "Ask anything…",
            PaletteMode::AiHistory => "Search chats…",
            PaletteMode::CalculatorHistory => {
                "Do math, convert units, or search your past calculations…"
            }
            PaletteMode::Emoji => "Search emoji and symbols…",
            PaletteMode::FileSearch => "Search files and folders…",
            PaletteMode::Schedule => "Search today and tomorrow…",
            PaletteMode::Uninstall => "Filter files and folders by name…",
            PaletteMode::Quicklinks => "Search quicklinks…",
            PaletteMode::QuicklinkArguments => "Enter a value…",
            PaletteMode::ExtensionCommand => "Search…",
        }
    }

    pub fn header_symbol(self) -> &'static str {
        match self {
            PaletteMode::Launcher => "magnifyingglass",
            PaletteMode::Clipboard => "doc.on.doc",
            PaletteMode::Ai => "sparkles",
            PaletteMode::AiHistory => "clock.arrow.circlepath",
            PaletteMode::CalculatorHistory => "plus.forwardslash.minus",
            PaletteMode::Emoji => "face.smiling",
            PaletteMode::FileSearch => "doc.text.magnifyingglass",
            PaletteMode::Schedule => "calendar",
            PaletteMode::Uninstall => "trash",
            PaletteMode::Quicklinks | PaletteMode::QuicklinkArguments => "link",
            PaletteMode::ExtensionCommand => "puzzlepiece.extension",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_placeholder_is_v0102() {
        assert_eq!(
            PaletteMode::Launcher.placeholder(),
            "Search for apps and commands…"
        );
        assert_eq!(PaletteMode::Ai.placeholder(), "Ask anything…");
        assert_eq!(PaletteMode::Launcher.header_symbol(), "magnifyingglass");
    }
}
