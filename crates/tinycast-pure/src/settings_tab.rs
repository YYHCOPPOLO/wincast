#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SettingsTab {
    General,
    Applications,
    SystemSettings,
    SystemActions,
    Commands,
    Quicklinks,
    Ai,
    QuickActions,
    FileSearch,
    Notes,
    Snippets,
    WindowManagement,
    Clipboard,
    Emoji,
    Calendar,
    Extensions,
    Permissions,
    Backup,
    About,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SettingsSection {
    General,
    Launcher,
    Features,
    Advanced,
}

impl SettingsSection {
    pub fn all() -> [SettingsSection; 4] {
        [
            SettingsSection::General,
            SettingsSection::Launcher,
            SettingsSection::Features,
            SettingsSection::Advanced,
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            SettingsSection::General => "General",
            SettingsSection::Launcher => "Launcher",
            SettingsSection::Features => "Features",
            SettingsSection::Advanced => "Advanced",
        }
    }

    pub fn tabs(self) -> &'static [SettingsTab] {
        match self {
            SettingsSection::General => &[SettingsTab::General, SettingsTab::Permissions],
            SettingsSection::Launcher => &[
                SettingsTab::Applications,
                SettingsTab::SystemSettings,
                SettingsTab::SystemActions,
                SettingsTab::Commands,
                SettingsTab::Quicklinks,
            ],
            SettingsSection::Features => &[
                SettingsTab::Ai,
                SettingsTab::QuickActions,
                SettingsTab::FileSearch,
                SettingsTab::Notes,
                SettingsTab::Snippets,
                SettingsTab::WindowManagement,
                SettingsTab::Clipboard,
                SettingsTab::Emoji,
                SettingsTab::Calendar,
                SettingsTab::Extensions,
            ],
            SettingsSection::Advanced => &[SettingsTab::Backup, SettingsTab::About],
        }
    }
}

impl SettingsTab {
    pub fn title(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Applications => "Applications",
            SettingsTab::SystemSettings => "System Settings",
            SettingsTab::SystemActions => "System Actions",
            SettingsTab::Commands => "Commands",
            SettingsTab::Quicklinks => "Quicklinks",
            SettingsTab::Ai => "AI",
            SettingsTab::QuickActions => "Quick Actions",
            SettingsTab::FileSearch => "File Search",
            SettingsTab::Notes => "Notes",
            SettingsTab::Snippets => "Snippets",
            SettingsTab::WindowManagement => "Window Management",
            SettingsTab::Clipboard => "Clipboard",
            SettingsTab::Emoji => "Emoji & Symbols",
            SettingsTab::Calendar => "Calendar",
            SettingsTab::Extensions => "Extensions",
            SettingsTab::Permissions => "Permissions",
            SettingsTab::Backup => "Backup",
            SettingsTab::About => "About",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_sidebar_order() {
        use SettingsSection::*;
        assert_eq!(
            SettingsSection::all(),
            [General, Launcher, Features, Advanced]
        );
        assert_eq!(
            General.tabs(),
            &[SettingsTab::General, SettingsTab::Permissions]
        );
        assert_eq!(
            Launcher.tabs(),
            &[
                SettingsTab::Applications,
                SettingsTab::SystemSettings,
                SettingsTab::SystemActions,
                SettingsTab::Commands,
                SettingsTab::Quicklinks,
            ]
        );
        assert_eq!(
            Features.tabs(),
            &[
                SettingsTab::Ai,
                SettingsTab::QuickActions,
                SettingsTab::FileSearch,
                SettingsTab::Notes,
                SettingsTab::Snippets,
                SettingsTab::WindowManagement,
                SettingsTab::Clipboard,
                SettingsTab::Emoji,
                SettingsTab::Calendar,
                SettingsTab::Extensions,
            ]
        );
        assert_eq!(Advanced.tabs(), &[SettingsTab::Backup, SettingsTab::About]);
    }

    #[test]
    fn default_settings_tab_is_general() {
        assert_eq!(SettingsTab::General.title(), "General");
        assert_eq!(SettingsSection::Features.tabs()[0].title(), "AI");
    }

    #[test]
    fn settings_tab_titles_match_v0102() {
        assert_eq!(SettingsTab::General.title(), "General");
        assert_eq!(SettingsTab::Applications.title(), "Applications");
        assert_eq!(SettingsTab::SystemSettings.title(), "System Settings");
        assert_eq!(SettingsTab::SystemActions.title(), "System Actions");
        assert_eq!(SettingsTab::Commands.title(), "Commands");
        assert_eq!(SettingsTab::Quicklinks.title(), "Quicklinks");
        assert_eq!(SettingsTab::Ai.title(), "AI");
        assert_eq!(SettingsTab::QuickActions.title(), "Quick Actions");
        assert_eq!(SettingsTab::FileSearch.title(), "File Search");
        assert_eq!(SettingsTab::Notes.title(), "Notes");
        assert_eq!(SettingsTab::Snippets.title(), "Snippets");
        assert_eq!(SettingsTab::WindowManagement.title(), "Window Management");
        assert_eq!(SettingsTab::Clipboard.title(), "Clipboard");
        assert_eq!(SettingsTab::Emoji.title(), "Emoji & Symbols");
        assert_eq!(SettingsTab::Calendar.title(), "Calendar");
        assert_eq!(SettingsTab::Extensions.title(), "Extensions");
        assert_eq!(SettingsTab::Permissions.title(), "Permissions");
        assert_eq!(SettingsTab::Backup.title(), "Backup");
        assert_eq!(SettingsTab::About.title(), "About");
        assert_eq!(SettingsSection::General.title(), "General");
        assert_eq!(SettingsSection::Launcher.title(), "Launcher");
        assert_eq!(SettingsSection::Features.title(), "Features");
        assert_eq!(SettingsSection::Advanced.title(), "Advanced");
        assert_eq!(SettingsSection::Features.tabs()[0].title(), "AI");
    }
}
