use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app_entry::AppKind;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct VisibilityData {
    #[serde(default, rename = "hiddenItems")]
    hidden_items: Vec<String>,
    #[serde(default, rename = "disabledKinds")]
    disabled_kinds: Vec<String>,
}

/// Kind master switch plus per-id row hide. Kind off also blocks that kind's hotkeys.
#[derive(Clone, Debug, Default)]
pub struct VisibilityStore {
    disabled_kinds: HashSet<AppKind>,
    hidden_ids: HashSet<String>,
}

impl VisibilityStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let data = match std::fs::read(path.as_ref()) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => VisibilityData::default(),
        };
        Self::from_data(data)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_vec_pretty(&self.to_data())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, data)
    }

    pub fn is_kind_enabled(&self, k: AppKind) -> bool {
        !self.disabled_kinds.contains(&k)
    }

    pub fn set_kind_enabled(&mut self, k: AppKind, on: bool) {
        if on {
            self.disabled_kinds.remove(&k);
        } else {
            self.disabled_kinds.insert(k);
        }
    }

    pub fn is_item_visible(&self, id: &str) -> bool {
        !self.hidden_ids.contains(id)
    }

    pub fn hide_item(&mut self, id: &str) {
        self.set_item_visible(id, false);
    }

    pub fn set_item_visible(&mut self, id: &str, visible: bool) {
        if id.is_empty() {
            return;
        }
        if visible {
            self.hidden_ids.remove(id);
        } else {
            self.hidden_ids.insert(id.to_string());
        }
    }

    pub fn allows_hotkey(&self, kind: AppKind) -> bool {
        self.is_kind_enabled(kind)
    }

    fn from_data(data: VisibilityData) -> Self {
        let mut hidden_ids = HashSet::new();
        for id in data.hidden_items {
            if !id.is_empty() {
                hidden_ids.insert(id);
            }
        }
        let mut disabled_kinds = HashSet::new();
        for raw in data.disabled_kinds {
            if let Some(kind) = kind_from_key(&raw) {
                disabled_kinds.insert(kind);
            }
        }
        Self {
            disabled_kinds,
            hidden_ids,
        }
    }

    fn to_data(&self) -> VisibilityData {
        let mut hidden_items: Vec<String> = self.hidden_ids.iter().cloned().collect();
        hidden_items.sort();
        let mut disabled_kinds: Vec<String> = self
            .disabled_kinds
            .iter()
            .copied()
            .map(kind_key)
            .map(str::to_string)
            .collect();
        disabled_kinds.sort();
        VisibilityData {
            hidden_items,
            disabled_kinds,
        }
    }
}

fn kind_key(kind: AppKind) -> &'static str {
    match kind {
        AppKind::Application => "application",
        AppKind::SystemSettings => "systemSettings",
        AppKind::Quicklink => "quicklink",
        AppKind::Snippet => "snippet",
        AppKind::SystemAction => "systemAction",
        AppKind::WindowCommand => "windowCommand",
        AppKind::CustomCommand => "customCommand",
        AppKind::Command => "command",
    }
}

fn kind_from_key(raw: &str) -> Option<AppKind> {
    match raw {
        "application" => Some(AppKind::Application),
        "systemSettings" => Some(AppKind::SystemSettings),
        "quicklink" => Some(AppKind::Quicklink),
        "snippet" => Some(AppKind::Snippet),
        "systemAction" => Some(AppKind::SystemAction),
        "windowCommand" => Some(AppKind::WindowCommand),
        "customCommand" => Some(AppKind::CustomCommand),
        "command" => Some(AppKind::Command),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{kind_from_key, kind_key, VisibilityStore};
    use crate::app_entry::AppKind;
    use std::path::PathBuf;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tc-vis-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn kind_switch_blocks_hotkeys_item_hide_does_not() {
        let mut v = VisibilityStore::default();
        v.set_kind_enabled(AppKind::Application, false);
        assert!(!v.allows_hotkey(AppKind::Application));
        v.set_kind_enabled(AppKind::Application, true);
        v.hide_item("app:x");
        assert!(v.allows_hotkey(AppKind::Application));
        assert!(!v.is_item_visible("app:x"));
    }

    #[test]
    fn kinds_default_enabled_and_items_visible() {
        let v = VisibilityStore::default();
        for kind in [
            AppKind::Application,
            AppKind::SystemSettings,
            AppKind::Quicklink,
            AppKind::Snippet,
            AppKind::SystemAction,
            AppKind::WindowCommand,
            AppKind::CustomCommand,
            AppKind::Command,
        ] {
            assert!(v.is_kind_enabled(kind));
            assert!(v.allows_hotkey(kind));
        }
        assert!(v.is_item_visible("app:x"));
    }

    #[test]
    fn kind_off_does_not_hide_the_item_itself() {
        let mut v = VisibilityStore::default();
        v.set_kind_enabled(AppKind::Command, false);
        assert!(!v.is_kind_enabled(AppKind::Command));
        assert!(!v.allows_hotkey(AppKind::Command));
        assert!(v.is_item_visible("command:quit"));
        assert!(v.allows_hotkey(AppKind::Application));
    }

    #[test]
    fn set_item_visible_unhides() {
        let mut v = VisibilityStore::default();
        v.hide_item("app:x");
        assert!(!v.is_item_visible("app:x"));
        v.set_item_visible("app:x", true);
        assert!(v.is_item_visible("app:x"));
    }

    #[test]
    fn visibility_persists_across_load() {
        let path = temp_path("persist");
        let _ = std::fs::remove_file(&path);
        let mut v = VisibilityStore::default();
        v.set_kind_enabled(AppKind::Application, false);
        v.hide_item("app:x");
        v.save(&path).unwrap();
        let loaded = VisibilityStore::load(&path);
        assert!(!loaded.is_kind_enabled(AppKind::Application));
        assert!(!loaded.allows_hotkey(AppKind::Application));
        assert!(!loaded.is_item_visible("app:x"));
        assert!(loaded.is_kind_enabled(AppKind::Command));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn visibility_json_uses_camel_case_keys() {
        let path = temp_path("keys");
        let _ = std::fs::remove_file(&path);
        let mut v = VisibilityStore::default();
        v.set_kind_enabled(AppKind::Application, false);
        v.hide_item("app:x");
        v.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"hiddenItems\""));
        assert!(text.contains("\"disabledKinds\""));
        assert!(text.contains("\"application\""));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_is_defaults() {
        let v = VisibilityStore::load(PathBuf::from(
            "Z:\\tinycast-does-not-exist\\visibility.json",
        ));
        assert!(v.is_kind_enabled(AppKind::Application));
        assert!(v.is_item_visible("app:x"));
    }

    #[test]
    fn kind_keys_match_v0102_raw_values() {
        assert_eq!(kind_key(AppKind::Application), "application");
        assert_eq!(kind_key(AppKind::SystemSettings), "systemSettings");
        assert_eq!(kind_from_key("windowCommand"), Some(AppKind::WindowCommand));
        assert_eq!(kind_from_key("favorite"), None);
    }
}
