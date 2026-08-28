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
            "Applications" | "Application" => Some(AppKind::Application),
            "System Settings" | "System Setting" => Some(AppKind::SystemSettings),
            "Quicklinks" | "Quicklink" => Some(AppKind::Quicklink),
            "Snippets" | "Snippet" => Some(AppKind::Snippet),
            "System Actions" | "System Action" => Some(AppKind::SystemAction),
            "Window Management" | "Window Command" => Some(AppKind::WindowCommand),
            "Custom Commands" | "Custom Command" => Some(AppKind::CustomCommand),
            "Commands" | "Command" => Some(AppKind::Command),
            _ => None,
        }
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
}
