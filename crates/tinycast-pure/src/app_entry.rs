use crate::hotkey::HotKeyBinding;
use crate::search_relevance::SearchFields;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AppKind {
    Application,
    SystemSettings,
    Quicklink,
    Snippet,
    SystemAction,
    WindowCommand,
    CustomCommand,
    Command,
}

impl AppKind {
    /// Exact section title or singular label (`Snippets` / `Snippet`). Prefixes do not match.
    pub fn named_by(query: &str) -> Option<AppKind> {
        match query {
            "Applications" | "Application" | "应用" => Some(AppKind::Application),
            "System Settings" | "System Setting" | "系统设置" => Some(AppKind::SystemSettings),
            "Quicklinks" | "Quicklink" | "快捷链接" => Some(AppKind::Quicklink),
            "Snippets" | "Snippet" | "片段" => Some(AppKind::Snippet),
            "System Actions" | "System Action" | "系统操作" => Some(AppKind::SystemAction),
            "Window Management" | "Window Command" | "窗口管理" => Some(AppKind::WindowCommand),
            "Custom Commands" | "Custom Command" | "自定义命令" => {
                Some(AppKind::CustomCommand)
            }
            "Commands" | "Command" | "命令" => Some(AppKind::Command),
            _ => None,
        }
    }

    pub fn section_title(self) -> &'static str {
        match self {
            AppKind::Application => "Applications",
            AppKind::SystemSettings => "System Settings",
            AppKind::Quicklink => "Quicklinks",
            AppKind::Snippet => "Snippets",
            AppKind::SystemAction => "System Actions",
            AppKind::WindowCommand => "Window Management",
            AppKind::CustomCommand => "Custom Commands",
            AppKind::Command => "Commands",
        }
    }

    pub fn kind_label(self) -> &'static str {
        match self {
            AppKind::Application => "Application",
            AppKind::SystemSettings => "System Settings",
            AppKind::Quicklink => "Quicklink",
            AppKind::Snippet => "Snippet",
            AppKind::SystemAction => "System Action",
            AppKind::WindowCommand => "Window Management",
            AppKind::CustomCommand => "Custom Command",
            AppKind::Command => "Command",
        }
    }

    pub fn open_verb(self) -> &'static str {
        match self {
            AppKind::Application => "Open Application",
            AppKind::SystemSettings => "Open System Setting",
            AppKind::Quicklink => "Open Quicklink",
            AppKind::Snippet => "Paste Snippet",
            AppKind::SystemAction => "Run System Action",
            AppKind::WindowCommand => "Move Window",
            AppKind::CustomCommand => "Run Custom Command",
            AppKind::Command => "Run Command",
        }
    }

    /// Windows wording of Show in Finder. Applications have a folder; commands do not.
    pub fn can_reveal_in_folder(self) -> bool {
        matches!(self, AppKind::Application)
    }
}

#[derive(Clone, Debug)]
pub struct AppEntry {
    pub id: String,
    pub kind: AppKind,
    pub name: String,
    pub fields: SearchFields,
    pub hotkey: Option<HotKeyBinding>,
}

#[cfg(test)]
mod tests {
    use super::AppKind;

    #[test]
    fn kind_named_by_is_exact() {
        assert_eq!(AppKind::named_by("Snippets"), Some(AppKind::Snippet));
        assert_eq!(AppKind::named_by("snip"), None);
    }

    #[test]
    fn kind_named_by_accepts_section_title_or_singular() {
        assert_eq!(
            AppKind::named_by("Applications"),
            Some(AppKind::Application)
        );
        assert_eq!(AppKind::named_by("Application"), Some(AppKind::Application));
        assert_eq!(
            AppKind::named_by("System Settings"),
            Some(AppKind::SystemSettings)
        );
        assert_eq!(
            AppKind::named_by("System Setting"),
            Some(AppKind::SystemSettings)
        );
        assert_eq!(AppKind::named_by("Quicklinks"), Some(AppKind::Quicklink));
        assert_eq!(AppKind::named_by("Quicklink"), Some(AppKind::Quicklink));
        assert_eq!(AppKind::named_by("Snippet"), Some(AppKind::Snippet));
        assert_eq!(
            AppKind::named_by("System Actions"),
            Some(AppKind::SystemAction)
        );
        assert_eq!(
            AppKind::named_by("System Action"),
            Some(AppKind::SystemAction)
        );
        assert_eq!(
            AppKind::named_by("Window Management"),
            Some(AppKind::WindowCommand)
        );
        assert_eq!(
            AppKind::named_by("Window Command"),
            Some(AppKind::WindowCommand)
        );
        assert_eq!(
            AppKind::named_by("Custom Commands"),
            Some(AppKind::CustomCommand)
        );
        assert_eq!(
            AppKind::named_by("Custom Command"),
            Some(AppKind::CustomCommand)
        );
        assert_eq!(AppKind::named_by("Commands"), Some(AppKind::Command));
        assert_eq!(AppKind::named_by("Command"), Some(AppKind::Command));
        assert_eq!(AppKind::named_by("Favorites"), None);
        assert_eq!(AppKind::named_by("snippets"), None);
        assert_eq!(AppKind::named_by("Window Commands"), None);
    }

    #[test]
    fn section_title_matches_named_by_labels() {
        assert_eq!(AppKind::Application.section_title(), "Applications");
        assert_eq!(AppKind::SystemSettings.section_title(), "System Settings");
        assert_eq!(AppKind::Command.section_title(), "Commands");
        assert_eq!(AppKind::Application.kind_label(), "Application");
        assert_eq!(AppKind::Command.kind_label(), "Command");
        assert_eq!(AppKind::Application.open_verb(), "Open Application");
        assert_eq!(AppKind::Command.open_verb(), "Run Command");
        assert!(AppKind::Application.can_reveal_in_folder());
        assert!(!AppKind::Command.can_reveal_in_folder());
        assert!(!AppKind::SystemSettings.can_reveal_in_folder());
        assert_eq!(
            AppKind::named_by(AppKind::Application.section_title()),
            Some(AppKind::Application)
        );
        assert_eq!(
            AppKind::named_by(AppKind::SystemSettings.section_title()),
            Some(AppKind::SystemSettings)
        );
    }
}
