use crate::app_entry::{AppEntry, AppKind};
use crate::feature_flags::FeatureFlags;
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

    /// Commands settings lists the full catalog, including feature-gated rows.
    pub fn settings_pane_ids() -> &'static [CommandID] {
        Self::ALL
    }

    pub fn from_raw(raw: &str) -> Option<CommandID> {
        Self::ALL.iter().copied().find(|id| id.raw() == raw)
    }

    pub fn listed_in_commands_settings(self) -> bool {
        true
    }

    pub fn shows_in_launcher(self, flags: FeatureFlags) -> bool {
        match self {
            CommandID::SearchFiles => flags.file_search_enabled,
            CommandID::ShowNotes | CommandID::CreateNote | CommandID::SearchNotes => {
                flags.notes_enabled
            }
            CommandID::JoinNextMeeting
            | CommandID::CopyMeetingLink
            | CommandID::MySchedule
            | CommandID::OpenInCalendar
            | CommandID::CreateEvent => flags.calendar_enabled,
            CommandID::AiChat => flags.ai_enabled,
            CommandID::FixGrammar
            | CommandID::Rewrite
            | CommandID::Translate
            | CommandID::Summarize => flags.quick_actions_enabled,
            CommandID::CreateQuicklink
            | CommandID::SearchQuicklinks
            | CommandID::ImportQuicklinks
            | CommandID::ExportQuicklinks => flags.quicklinks_enabled,
            _ => true,
        }
    }

    /// `UserDefaults` key in v0.10.2; `None` means the Commands row has no recorder.
    pub fn hotkey_defaults_key(self) -> Option<&'static str> {
        match self {
            CommandID::SearchFiles => Some("hotkey.searchFiles"),
            CommandID::ClipboardHistory => Some("hotkey.toggleClipboard"),
            CommandID::SearchEmoji => Some("hotkey.toggleEmoji"),
            CommandID::ShowNotes => Some("hotkey.showNotes"),
            CommandID::CreateNote => Some("hotkey.createNote"),
            CommandID::SearchNotes => Some("hotkey.searchNotes"),
            CommandID::JoinNextMeeting => Some("hotkey.joinNextMeeting"),
            CommandID::MySchedule => Some("hotkey.mySchedule"),
            CommandID::CreateEvent => Some("hotkey.createEvent"),
            CommandID::AiChat => Some("hotkey.aiChat"),
            CommandID::FixGrammar => Some("hotkey.quickAction.fixGrammar"),
            CommandID::Rewrite => Some("hotkey.quickAction.rewrite"),
            CommandID::Translate => Some("hotkey.quickAction.translate"),
            CommandID::Summarize => Some("hotkey.quickAction.summarize"),
            _ => None,
        }
    }

    pub fn from_hotkey_key(key: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|id| id.hotkey_defaults_key() == Some(key))
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

    pub fn as_entry_for(self, lang: crate::i18n::UiLang) -> AppEntry {
        let display = crate::i18n::command_title(self, lang).to_string();
        let other = crate::i18n::command_title(self, lang.other()).to_string();
        AppEntry {
            id: self.raw().to_string(),
            kind: AppKind::Command,
            name: display.clone(),
            fields: SearchFields {
                display_name: display,
                alternate_names: vec![other],
                ..Default::default()
            },
            hotkey: None,
        }
    }

    pub fn as_entry(self) -> AppEntry {
        self.as_entry_for(crate::i18n::UiLang::En)
    }
}

#[cfg(test)]
mod tests {
    use super::CommandID;
    use crate::app_entry::AppKind;

    #[test]
    fn four_quick_actions_map_to_command_ids() {
        assert_eq!(
            CommandID::from_quick(crate::ai::quick_action::QuickAction::FixGrammar),
            CommandID::FixGrammar
        );
        assert_eq!(crate::ai::quick_action::QuickAction::all().len(), 4);
    }

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

    #[test]
    fn commands_pane_lists_every_command_id() {
        // v0.10.2 ships 29 CommandIDs; do not invent extras to satisfy a looser bound.
        assert_eq!(CommandID::all().len(), 29);
        assert_eq!(
            CommandID::from_hotkey_key("hotkey.toggleEmoji"),
            Some(CommandID::SearchEmoji)
        );
        assert_eq!(
            CommandID::from_hotkey_key("hotkey.toggleClipboard"),
            Some(CommandID::ClipboardHistory)
        );
        assert_eq!(CommandID::from_hotkey_key("hotkey.togglePalette"), None);
        for id in CommandID::all() {
            if let Some(key) = id.hotkey_defaults_key() {
                assert_eq!(CommandID::from_hotkey_key(key), Some(*id), "{key}");
            }
        }
        let listed = CommandID::settings_pane_ids();
        assert_eq!(listed.len(), CommandID::all().len());
        assert_eq!(listed, CommandID::all());
        for id in CommandID::all() {
            assert!(
                id.listed_in_commands_settings(),
                "{} must stay listed when its feature is off",
                id.raw()
            );
        }
    }

    #[test]
    fn feature_off_commands_stay_out_of_the_launcher() {
        let flags = crate::feature_flags::FeatureFlags::default();
        assert!(!flags.file_search_enabled);
        assert!(!flags.notes_enabled);
        assert!(!flags.snippets_enabled);
        assert!(!flags.window_management_enabled);
        assert!(!flags.calendar_enabled);
        assert!(!flags.ai_enabled);
        assert!(!flags.quick_actions_enabled);
        assert!(!flags.extensions_enabled);
        assert!(!flags.quicklinks_enabled);

        for always in [
            CommandID::Settings,
            CommandID::About,
            CommandID::Support,
            CommandID::Quit,
            CommandID::ClipboardHistory,
            CommandID::CalculatorHistory,
            CommandID::SearchEmoji,
        ] {
            assert!(always.shows_in_launcher(flags), "{}", always.raw());
        }
        assert!(!CommandID::AiChat.shows_in_launcher(flags));
        assert!(!CommandID::SearchFiles.shows_in_launcher(flags));
        assert!(!CommandID::ShowNotes.shows_in_launcher(flags));
        assert!(!CommandID::FixGrammar.shows_in_launcher(flags));
        assert!(!CommandID::JoinNextMeeting.shows_in_launcher(flags));
        assert!(!CommandID::CreateQuicklink.shows_in_launcher(flags));

        let on = crate::feature_flags::FeatureFlags {
            file_search_enabled: true,
            ..Default::default()
        };
        assert!(CommandID::SearchFiles.shows_in_launcher(on));
        assert!(!CommandID::AiChat.shows_in_launcher(on));
    }
}
