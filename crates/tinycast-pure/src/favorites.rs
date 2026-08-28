use std::path::Path;

/// Ordered favorite ids. Index 0 is Ctrl+1 … index 8 is Ctrl+9, index 9 is Ctrl+0.
#[derive(Clone, Debug, Default)]
pub struct FavoritesStore {
    pub ids: Vec<String>,
}

impl FavoritesStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let ids = match std::fs::read(path.as_ref()) {
            Ok(bytes) => serde_json::from_slice::<Vec<String>>(&bytes).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        Self {
            ids: sanitize_ids(ids),
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_vec_pretty(&self.ids)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, data)
    }

    /// `1..=9` map to the first nine ids; `0` is the tenth. Other digits have no slot.
    pub fn slot(&self, n: u8) -> Option<&str> {
        let i = match n {
            1..=9 => (n - 1) as usize,
            0 => 9,
            _ => return None,
        };
        self.ids.get(i).map(String::as_str)
    }

    pub fn toggle(&mut self, id: String) {
        if id.is_empty() {
            return;
        }
        if let Some(index) = self.ids.iter().position(|k| k == &id) {
            self.ids.remove(index);
        } else {
            self.ids.push(id);
        }
    }
}

fn sanitize_ids(ids: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for id in ids {
        if id.is_empty() || out.iter().any(|k| k == &id) {
            continue;
        }
        out.push(id);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::FavoritesStore;
    use std::path::PathBuf;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tc-fav-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn favorite_slots_map_ctrl_digits() {
        let mut f = FavoritesStore::default();
        for i in 1..=11 {
            f.toggle(format!("app:{i}"));
        }
        assert_eq!(f.slot(1), Some("app:1"));
        assert_eq!(f.slot(9), Some("app:9"));
        assert_eq!(f.slot(0), Some("app:10"));
        assert_eq!(f.slot(11), None);
        assert_eq!(f.ids.len(), 11);
    }

    #[test]
    fn toggle_appends_then_removes_without_reordering_others() {
        let mut f = FavoritesStore::default();
        f.toggle("app:a".into());
        f.toggle("app:b".into());
        f.toggle("app:c".into());
        assert_eq!(f.ids, ["app:a", "app:b", "app:c"]);
        f.toggle("app:b".into());
        assert_eq!(f.ids, ["app:a", "app:c"]);
        assert_eq!(f.slot(1), Some("app:a"));
        assert_eq!(f.slot(2), Some("app:c"));
        assert_eq!(f.slot(3), None);
        assert_eq!(f.slot(0), None);
    }

    #[test]
    fn empty_slot_and_empty_id_are_noop() {
        let mut f = FavoritesStore::default();
        f.toggle(String::new());
        assert!(f.ids.is_empty());
        assert_eq!(f.slot(1), None);
        assert_eq!(f.slot(0), None);
    }

    #[test]
    fn favorites_persist_across_load() {
        let path = temp_path("persist");
        let _ = std::fs::remove_file(&path);
        let mut f = FavoritesStore::default();
        f.toggle("app:a".into());
        f.toggle("app:b".into());
        f.save(&path).unwrap();
        let loaded = FavoritesStore::load(&path);
        assert_eq!(loaded.ids, ["app:a", "app:b"]);
        assert_eq!(loaded.slot(1), Some("app:a"));
        assert_eq!(loaded.slot(2), Some("app:b"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let f = FavoritesStore::load(PathBuf::from("Z:\\tinycast-does-not-exist\\favorites.json"));
        assert!(f.ids.is_empty());
    }
}
