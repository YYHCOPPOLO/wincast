use std::collections::BTreeMap;
use std::path::Path;

/// One user-chosen alias per entry id (`AppEntry.id` / preference key).
#[derive(Clone, Debug, Default)]
pub struct AliasStore {
    aliases: BTreeMap<String, String>,
}

impl AliasStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let raw = match std::fs::read(path.as_ref()) {
            Ok(bytes) => {
                serde_json::from_slice::<BTreeMap<String, String>>(&bytes).unwrap_or_default()
            }
            Err(_) => BTreeMap::new(),
        };
        let mut aliases = BTreeMap::new();
        for (id, alias) in raw {
            if id.is_empty() || alias.trim().is_empty() {
                continue;
            }
            aliases.insert(id, alias);
        }
        Self { aliases }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_vec_pretty(&self.aliases)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, data)
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.aliases.get(id).map(String::as_str)
    }

    pub fn set(&mut self, id: String, alias: Option<String>) {
        if id.is_empty() {
            return;
        }
        match alias {
            Some(value) if !value.trim().is_empty() => {
                self.aliases.insert(id, value);
            }
            _ => {
                self.aliases.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AliasStore;
    use std::path::PathBuf;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tc-alias-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn alias_get_set_and_clear() {
        let mut a = AliasStore::default();
        assert_eq!(a.get("app:x"), None);
        a.set("app:x".into(), Some("iterm".into()));
        assert_eq!(a.get("app:x"), Some("iterm"));
        a.set("app:x".into(), None);
        assert_eq!(a.get("app:x"), None);
    }

    #[test]
    fn blank_alias_means_none_but_typed_spaces_are_kept() {
        let mut a = AliasStore::default();
        a.set("app:x".into(), Some("   ".into()));
        assert_eq!(a.get("app:x"), None);
        a.set("app:x".into(), Some(" i term ".into()));
        assert_eq!(a.get("app:x"), Some(" i term "));
        a.set("app:x".into(), Some(String::new()));
        assert_eq!(a.get("app:x"), None);
    }

    #[test]
    fn aliases_persist_across_load() {
        let path = temp_path("persist");
        let _ = std::fs::remove_file(&path);
        let mut a = AliasStore::default();
        a.set("app:x".into(), Some("iterm".into()));
        a.set("command:quit".into(), Some("bye".into()));
        a.save(&path).unwrap();
        let loaded = AliasStore::load(&path);
        assert_eq!(loaded.get("app:x"), Some("iterm"));
        assert_eq!(loaded.get("command:quit"), Some("bye"));
        assert_eq!(loaded.get("app:y"), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let a = AliasStore::load(PathBuf::from("Z:\\tinycast-does-not-exist\\aliases.json"));
        assert_eq!(a.get("app:x"), None);
    }
}
