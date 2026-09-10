use std::collections::BTreeMap;
use std::path::Path;

use crate::hotkey::HotKeyBinding;

/// One binding per action key (`hotkey.toggleClipboard`, `hotkey.app.…`).
#[derive(Clone, Debug, Default)]
pub struct HotKeyStore {
    bindings: BTreeMap<String, HotKeyBinding>,
}

impl HotKeyStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let raw = match std::fs::read(path.as_ref()) {
            Ok(bytes) => serde_json::from_slice::<BTreeMap<String, HotKeyBinding>>(&bytes)
                .unwrap_or_default(),
            Err(_) => BTreeMap::new(),
        };
        let mut bindings = BTreeMap::new();
        for (key, binding) in raw {
            if !key.is_empty() {
                bindings.insert(key, binding);
            }
        }
        Self { bindings }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_vec_pretty(&self.bindings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, data)
    }

    pub fn get(&self, action: &str) -> Option<&HotKeyBinding> {
        self.bindings.get(action)
    }

    pub fn snapshot(&self) -> Vec<(String, HotKeyBinding)> {
        self.bindings
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn set(&mut self, action: String, binding: Option<HotKeyBinding>) {
        if action.is_empty() {
            return;
        }
        match binding {
            Some(value) => {
                self.bindings.insert(action, value);
            }
            None => {
                self.bindings.remove(&action);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HotKeyStore;
    use crate::hotkey::{HotKeyBinding, KeyShortcut, Modifiers};
    use std::path::PathBuf;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tc-hotkeys-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn combo() -> HotKeyBinding {
        HotKeyBinding::Combo(KeyShortcut {
            vk: 0x43,
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                win: false,
            },
        })
    }

    #[test]
    fn get_set_and_clear() {
        let mut s = HotKeyStore::default();
        assert_eq!(s.get("hotkey.toggleClipboard"), None);
        s.set("hotkey.toggleClipboard".into(), Some(combo()));
        assert_eq!(s.get("hotkey.toggleClipboard"), Some(&combo()));
        s.set("hotkey.toggleClipboard".into(), None);
        assert_eq!(s.get("hotkey.toggleClipboard"), None);
    }

    #[test]
    fn hotkeys_persist_across_load() {
        let path = temp_path("persist");
        let _ = std::fs::remove_file(&path);
        let mut s = HotKeyStore::default();
        s.set("hotkey.toggleClipboard".into(), Some(combo()));
        s.save(&path).unwrap();
        let loaded = HotKeyStore::load(&path);
        assert_eq!(loaded.get("hotkey.toggleClipboard"), Some(&combo()));
        let _ = std::fs::remove_file(&path);
    }
}
