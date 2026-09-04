use std::path::{Path, PathBuf};

use tinycast_pure::feature_flags::FeatureFlags;

use crate::platform::launch_at_login;
use crate::platform::paths;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettings {
    #[serde(rename = "compactMode")]
    pub compact_mode: bool,
    #[serde(rename = "openOnCursorScreen")]
    pub open_on_cursor_screen: bool,
    #[serde(rename = "appearance")]
    pub appearance: Appearance,
    #[serde(default = "default_skin", rename = "emojiSkinTone")]
    pub emoji_skin_tone: String,
    #[serde(rename = "launchAtLogin")]
    pub launch_at_login: bool,
    #[serde(default = "default_hyper", rename = "hyperKeyPhysicalKey")]
    pub hyper_key: String,
    #[serde(default, rename = "hyperKeyIncludesShift")]
    pub hyper_includes_shift: bool,
    #[serde(default = "default_hyper_quick", rename = "hyperKeyQuickPress")]
    pub hyper_quick_press: String,
    #[serde(default = "default_search_scopes", rename = "launcherSearchScopes")]
    pub launcher_search_scopes: Vec<String>,
    #[serde(default, rename = "fileSearchEnabled")]
    pub file_search_enabled: bool,
    #[serde(default = "default_file_search_scopes", rename = "fileSearchScopes")]
    pub file_search_scopes: Vec<String>,
    #[serde(default, rename = "fileSearchIgnorePatterns")]
    pub file_search_ignore_patterns: Vec<String>,
    #[serde(default, rename = "notesEnabled")]
    pub notes_enabled: bool,
    #[serde(default, rename = "customCommandsEnabled")]
    pub custom_commands_enabled: bool,
    #[serde(default = "default_true", rename = "customCommandsShowInLauncher")]
    pub custom_commands_show_in_launcher: bool,
    #[serde(default, rename = "snippetsEnabled")]
    pub snippets_enabled: bool,
    #[serde(default = "default_true", rename = "snippetsShowInLauncher")]
    pub snippets_show_in_launcher: bool,
    #[serde(default, rename = "windowManagementEnabled")]
    pub window_management_enabled: bool,
    #[serde(default = "default_true", rename = "windowManagementShowInLauncher")]
    pub window_management_show_in_launcher: bool,
    #[serde(default, rename = "windowManagementGap")]
    pub window_gap: i32,
    #[serde(default, rename = "windowManagementCycleOnRepeat")]
    pub window_cycle_on_repeat: bool,
    #[serde(default, rename = "calendarEnabled")]
    pub calendar_enabled: bool,
    #[serde(default, rename = "aiEnabled")]
    pub ai_enabled: bool,
    #[serde(default, rename = "quickActionsEnabled")]
    pub quick_actions_enabled: bool,
    #[serde(default, rename = "extensionsEnabled")]
    pub extensions_enabled: bool,
    #[serde(default, rename = "quicklinksEnabled")]
    pub quicklinks_enabled: bool,
    #[serde(default = "default_true", rename = "quicklinksShowInLauncher")]
    pub quicklinks_show_in_launcher: bool,
    #[serde(default = "default_retention_days", rename = "clipboardRetentionDays")]
    pub clipboard_retention_days: i64,
    #[serde(default = "default_disabled_apps", rename = "clipboardDisabledApps")]
    pub clipboard_disabled_apps: Vec<String>,
    #[serde(default = "default_true", rename = "showFavoritesInCompactMode")]
    pub show_favorites_in_compact: bool,
    #[serde(default, rename = "paletteDraggable")]
    pub palette_draggable: bool,
    #[serde(default = "default_true", rename = "showInMenuBar")]
    pub show_in_menu_bar: bool,
    #[serde(default, rename = "popToRootTimeout")]
    pub pop_to_root_timeout: i64,
    #[serde(default, rename = "autoSwitchInputSource")]
    pub auto_switch_input_source: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            compact_mode: true,
            open_on_cursor_screen: true,
            appearance: Appearance::System,
            emoji_skin_tone: default_skin(),
            launch_at_login: false,
            hyper_key: default_hyper(),
            hyper_includes_shift: false,
            hyper_quick_press: default_hyper_quick(),
            launcher_search_scopes: default_search_scopes(),
            file_search_enabled: false,
            file_search_scopes: default_file_search_scopes(),
            file_search_ignore_patterns: Vec::new(),
            notes_enabled: false,
            custom_commands_enabled: false,
            custom_commands_show_in_launcher: true,
            snippets_enabled: false,
            snippets_show_in_launcher: true,
            window_management_enabled: false,
            window_management_show_in_launcher: true,
            window_gap: 0,
            window_cycle_on_repeat: false,
            calendar_enabled: false,
            ai_enabled: false,
            quick_actions_enabled: false,
            extensions_enabled: false,
            quicklinks_enabled: false,
            quicklinks_show_in_launcher: true,
            clipboard_retention_days: default_retention_days(),
            clipboard_disabled_apps: default_disabled_apps(),
            show_favorites_in_compact: true,
            palette_draggable: false,
            show_in_menu_bar: true,
            pop_to_root_timeout: 0,
            auto_switch_input_source: false,
        }
    }
}

impl AppSettings {
    pub fn feature_flags(&self) -> FeatureFlags {
        FeatureFlags {
            file_search_enabled: self.file_search_enabled,
            notes_enabled: self.notes_enabled,
            snippets_enabled: self.snippets_enabled,
            window_management_enabled: self.window_management_enabled,
            calendar_enabled: self.calendar_enabled,
            ai_enabled: self.ai_enabled,
            quick_actions_enabled: self.quick_actions_enabled,
            extensions_enabled: self.extensions_enabled,
            quicklinks_enabled: self.quicklinks_enabled,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_hyper() -> String {
    "none".into()
}

fn default_hyper_quick() -> String {
    "none".into()
}

fn default_skin() -> String {
    "none".into()
}

fn default_retention_days() -> i64 {
    90
}

fn default_disabled_apps() -> Vec<String> {
    crate::features::clipboard::service::manager::DEFAULT_DISABLED_APPS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn default_file_search_scopes() -> Vec<String> {
    vec!["~".into()]
}

fn default_search_scopes() -> Vec<String> {
    vec![
        r"%ProgramData%\Microsoft\Windows\Start Menu\Programs".into(),
        r"%APPDATA%\Microsoft\Windows\Start Menu\Programs".into(),
        r"%ProgramFiles%".into(),
        r"%ProgramFiles(x86)%".into(),
    ]
}

impl AppSettings {
    pub fn load() -> Self {
        Self::load_from(&settings_path())
    }

    #[allow(dead_code)]
    pub fn save(&self) -> std::io::Result<()> {
        self.write_json(&paths::roaming_dir(), &paths::local_dir())?;
        launch_at_login::apply(self.launch_at_login)
    }

    #[allow(dead_code)]
    fn write_json(&self, roaming: &Path, local: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(roaming)?;
        std::fs::create_dir_all(local)?;
        std::fs::write(
            roaming.join("settings.json"),
            serde_json::to_vec_pretty(self)?,
        )
    }

    fn load_from(path: &Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }
}

fn settings_path() -> PathBuf {
    paths::roaming_dir().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::app_settings_key::AppSettingsKey;

    #[test]
    fn app_settings_serde_names() {
        let s = AppSettings {
            compact_mode: true,
            open_on_cursor_screen: false,
            appearance: Appearance::Dark,
            launch_at_login: false,
            launcher_search_scopes: default_search_scopes(),
            ..Default::default()
        };
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["compactMode"], true);
        assert_eq!(j["openOnCursorScreen"], false);
        assert_eq!(j["appearance"], "dark");
        assert_eq!(j[AppSettingsKey::CompactMode.as_str()], true);
        assert_eq!(j[AppSettingsKey::OpenOnCursorScreen.as_str()], false);
        assert_eq!(j[AppSettingsKey::Appearance.as_str()], "dark");
        assert_eq!(j["launchAtLogin"], false);
        assert_eq!(
            j[AppSettingsKey::SearchScopes.as_str()],
            serde_json::json!(default_search_scopes())
        );
        assert_eq!(j[AppSettingsKey::FileSearchEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::AiEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::NotesEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::SnippetsEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::SnippetsShowInLauncher.as_str()], true);
        assert_eq!(j[AppSettingsKey::WindowManagementEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::CalendarEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::QuickActionsEnabled.as_str()], false);
        assert_eq!(j[AppSettingsKey::ExtensionsEnabled.as_str()], false);
        assert!(!s.feature_flags().file_search_enabled);
        assert!(!s.feature_flags().ai_enabled);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let s = AppSettings::load_from(Path::new("Z:\\tinycast-does-not-exist\\settings.json"));
        assert_eq!(s, AppSettings::default());
        assert!(s.compact_mode);
        assert!(s.open_on_cursor_screen);
        assert_eq!(s.appearance, Appearance::System);
        assert!(!s.launch_at_login);
        assert_eq!(s.launcher_search_scopes, default_search_scopes());
        assert!(!s.file_search_enabled);
        assert!(!s.notes_enabled);
        assert!(!s.snippets_enabled);
        assert!(s.snippets_show_in_launcher);
        assert!(!s.window_management_enabled);
        assert!(!s.calendar_enabled);
        assert!(!s.ai_enabled);
        assert!(!s.quick_actions_enabled);
        assert!(!s.extensions_enabled);
        assert!(!s.quicklinks_enabled);
        assert!(!s.custom_commands_enabled);
        assert!(s.custom_commands_show_in_launcher);
        assert!(s.quicklinks_show_in_launcher);
        assert_eq!(s.emoji_skin_tone, "none");
    }

    #[test]
    fn write_json_creates_roaming_and_local_dirs() {
        let root = std::env::temp_dir().join(format!(
            "tinycast-settings-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let roaming = root.join("roaming");
        let local = root.join("local");
        let s = AppSettings {
            compact_mode: true,
            open_on_cursor_screen: false,
            appearance: Appearance::Light,
            launch_at_login: false,
            launcher_search_scopes: default_search_scopes(),
            ..Default::default()
        };
        s.write_json(&roaming, &local).unwrap();
        assert!(local.is_dir());
        let path = roaming.join("settings.json");
        let loaded = AppSettings::load_from(&path);
        assert_eq!(loaded, s);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"openOnCursorScreen\": false"));
        assert!(text.contains("\"appearance\": \"light\""));
        let _ = std::fs::remove_dir_all(&root);
    }
}
