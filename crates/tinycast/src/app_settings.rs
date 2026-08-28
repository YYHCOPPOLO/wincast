use std::path::{Path, PathBuf};

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
    #[serde(rename = "launchAtLogin")]
    pub launch_at_login: bool,
    #[serde(default = "default_search_scopes", rename = "launcherSearchScopes")]
    pub launcher_search_scopes: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            compact_mode: true,
            open_on_cursor_screen: true,
            appearance: Appearance::System,
            launch_at_login: false,
            launcher_search_scopes: default_search_scopes(),
        }
    }
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
