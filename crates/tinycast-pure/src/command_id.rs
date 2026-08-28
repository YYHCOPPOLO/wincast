use crate::app_entry::{AppEntry, AppKind};
use crate::search_relevance::SearchFields;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CommandID {
    AiChat,
    FixGrammar,
    Rewrite,
    Translate,
    Summarize,
    CalculatorHistory,
    ClipboardHistory,
    SearchEmoji,
    SearchFiles,
    JoinNextMeeting,
    CopyMeetingLink,
    MySchedule,
    OpenInCalendar,
    CreateEvent,
    ShowNotes,
    CreateNote,
    SearchNotes,
    CreateQuicklink,
    SearchQuicklinks,
    ImportQuicklinks,
    ExportQuicklinks,
    ExportSettings,
    ImportSettings,
    ImportFromRaycast,
    CheckForUpdates,
    Settings,
    About,
    Support,
    Quit,
}

impl CommandID {
    pub const ALL: &'static [CommandID] = &[
        CommandID::AiChat,
        CommandID::FixGrammar,
        CommandID::Rewrite,
        CommandID::Translate,
        CommandID::Summarize,
        CommandID::CalculatorHistory,
        CommandID::ClipboardHistory,
        CommandID::SearchEmoji,
        CommandID::SearchFiles,
        CommandID::JoinNextMeeting,
        CommandID::CopyMeetingLink,
        CommandID::MySchedule,
        CommandID::OpenInCalendar,
        CommandID::CreateEvent,
        CommandID::ShowNotes,
        CommandID::CreateNote,
        CommandID::SearchNotes,
        CommandID::CreateQuicklink,
        CommandID::SearchQuicklinks,
        CommandID::ImportQuicklinks,
        CommandID::ExportQuicklinks,
        CommandID::ExportSettings,
        CommandID::ImportSettings,
        CommandID::ImportFromRaycast,
        CommandID::CheckForUpdates,
        CommandID::Settings,
        CommandID::About,
        CommandID::Support,
        CommandID::Quit,
    ];

    pub fn all() -> &'static [CommandID] {
        Self::ALL
    }

    pub fn raw(self) -> &'static str {
        match self {
            CommandID::AiChat => "command:ai-chat",
            CommandID::FixGrammar => "command:fix-grammar",
            CommandID::Rewrite => "command:rewrite",
            CommandID::Translate => "command:translate",
            CommandID::Summarize => "command:summarize",
            CommandID::CalculatorHistory => "command:calculator-history",
            CommandID::ClipboardHistory => "command:clipboard-history",
            CommandID::SearchEmoji => "command:search-emoji",
            CommandID::SearchFiles => "command:search-files",
            CommandID::JoinNextMeeting => "command:join-next-meeting",
            CommandID::CopyMeetingLink => "command:copy-meeting-link",
            CommandID::MySchedule => "command:my-schedule",
            CommandID::OpenInCalendar => "command:open-in-calendar",
            CommandID::CreateEvent => "command:create-event",
            CommandID::ShowNotes => "command:show-notes",
            CommandID::CreateNote => "command:create-note",
            CommandID::SearchNotes => "command:search-notes",
            CommandID::CreateQuicklink => "command:create-quicklink",
            CommandID::SearchQuicklinks => "command:search-quicklinks",
            CommandID::ImportQuicklinks => "command:import-quicklinks",
            CommandID::ExportQuicklinks => "command:export-quicklinks",
            CommandID::ExportSettings => "command:export-settings",
            CommandID::ImportSettings => "command:import-settings",
            CommandID::ImportFromRaycast => "command:import-from-raycast",
            CommandID::CheckForUpdates => "command:check-for-updates",
            CommandID::Settings => "command:settings",
            CommandID::About => "command:about",
            CommandID::Support => "command:support",
            CommandID::Quit => "command:quit",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CommandID::AiChat => "AI Chat",
            CommandID::FixGrammar => "Fix Grammar",
            CommandID::Rewrite => "Rewrite",
            CommandID::Translate => "Translate",
            CommandID::Summarize => "Summarize",
            CommandID::CalculatorHistory => "Calculator History",
            CommandID::ClipboardHistory => "Clipboard History",
            CommandID::SearchEmoji => "Search Emoji & Symbols",
            CommandID::SearchFiles => "Search Files",
            CommandID::JoinNextMeeting => "Join Next Meeting",
            CommandID::CopyMeetingLink => "Copy Meeting Link",
            CommandID::MySchedule => "My Schedule",
            CommandID::OpenInCalendar => "Open in Calendar",
            CommandID::CreateEvent => "Create Event",
            CommandID::ShowNotes => "Show Notes",
            CommandID::CreateNote => "Create Note",
            CommandID::SearchNotes => "Search Notes",
            CommandID::CreateQuicklink => "Create Quicklink",
            CommandID::SearchQuicklinks => "Search Quicklinks",
            CommandID::ImportQuicklinks => "Import Quicklinks",
            CommandID::ExportQuicklinks => "Export Quicklinks",
            CommandID::ExportSettings => "Export Settings",
            CommandID::ImportSettings => "Import Settings",
            CommandID::ImportFromRaycast => "Import from Raycast",
            CommandID::CheckForUpdates => "Check for Updates",
            CommandID::Settings => "Settings",
            CommandID::About => "About Tinycast",
            CommandID::Support => "Support Tinycast",
            CommandID::Quit => "Quit Tinycast",
        }
    }

    pub fn as_entry(self) -> AppEntry {
        let name = self.name().to_string();
        AppEntry {
            id: self.raw().to_string(),
            kind: AppKind::Command,
            name: name.clone(),
            fields: SearchFields {
                display_name: name,
                ..Default::default()
            },
            hotkey: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommandID;
    use crate::app_entry::AppKind;

    #[test]
    fn command_ids_match_upstream_names() {
        assert_eq!(CommandID::AiChat.raw(), "command:ai-chat");
        assert_eq!(CommandID::AiChat.name(), "AI Chat");
        assert_eq!(CommandID::SearchEmoji.name(), "Search Emoji & Symbols");
        assert_eq!(CommandID::Quit.name(), "Quit Tinycast");
    }

    #[test]
    fn command_catalog_matches_v0102() {
        const EXPECTED: &[(&str, &str)] = &[
            ("command:ai-chat", "AI Chat"),
            ("command:fix-grammar", "Fix Grammar"),
            ("command:rewrite", "Rewrite"),
            ("command:translate", "Translate"),
            ("command:summarize", "Summarize"),
            ("command:calculator-history", "Calculator History"),
            ("command:clipboard-history", "Clipboard History"),
            ("command:search-emoji", "Search Emoji & Symbols"),
            ("command:search-files", "Search Files"),
            ("command:join-next-meeting", "Join Next Meeting"),
            ("command:copy-meeting-link", "Copy Meeting Link"),
            ("command:my-schedule", "My Schedule"),
            ("command:open-in-calendar", "Open in Calendar"),
            ("command:create-event", "Create Event"),
            ("command:show-notes", "Show Notes"),
            ("command:create-note", "Create Note"),
            ("command:search-notes", "Search Notes"),
            ("command:create-quicklink", "Create Quicklink"),
            ("command:search-quicklinks", "Search Quicklinks"),
            ("command:import-quicklinks", "Import Quicklinks"),
            ("command:export-quicklinks", "Export Quicklinks"),
            ("command:export-settings", "Export Settings"),
            ("command:import-settings", "Import Settings"),
            ("command:import-from-raycast", "Import from Raycast"),
            ("command:check-for-updates", "Check for Updates"),
            ("command:settings", "Settings"),
            ("command:about", "About Tinycast"),
            ("command:support", "Support Tinycast"),
            ("command:quit", "Quit Tinycast"),
        ];
        let actual: Vec<(&str, &str)> = CommandID::all()
            .iter()
            .copied()
            .map(|id| (id.raw(), id.name()))
            .collect();
        assert_eq!(actual, EXPECTED);
    }

    #[test]
    fn command_as_entry_uses_command_kind_and_raw_id() {
        let entry = CommandID::AiChat.as_entry();
        assert_eq!(entry.id, "command:ai-chat");
        assert_eq!(entry.kind, AppKind::Command);
        assert_eq!(entry.name, "AI Chat");
        assert_eq!(entry.fields.display_name, "AI Chat");
        assert!(entry.hotkey.is_none());
        assert!(entry.fields.user_alias.is_none());
        assert!(entry.fields.bundle_id.is_none());
    }
}
