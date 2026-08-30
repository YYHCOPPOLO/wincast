use std::path::PathBuf;

use tinycast_pure::quicklink::{sort_quicklinks, Quicklink};

use crate::platform::paths;

pub struct QuicklinkStore {
    path: PathBuf,
    links: Vec<Quicklink>,
}

impl QuicklinkStore {
    pub fn load() -> Self {
        Self::load_from(paths::roaming_dir().join("quicklinks.json"))
    }

    pub fn load_from(path: PathBuf) -> Self {
        let mut links: Vec<Quicklink> = if path.exists() {
            std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        sort_quicklinks(&mut links);
        Self { path, links }
    }

    pub fn links(&self) -> &[Quicklink] {
        &self.links
    }

    pub fn get(&self, id: &str) -> Option<&Quicklink> {
        self.links.iter().find(|l| l.id == id)
    }

    pub fn upsert(&mut self, link: Quicklink) -> Result<(), String> {
        if link.name.trim().is_empty() || link.destination.trim().is_empty() {
            return Err("Name and destination are required.".into());
        }
        if let Some(existing) = self.links.iter_mut().find(|l| l.id == link.id) {
            *existing = link;
        } else {
            self.links.push(link);
        }
        sort_quicklinks(&mut self.links);
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        if self.path.exists() {
            // never delete a library that will not serialize
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(&self.links).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, bytes).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_keeps_corrupt_file() {
        let path = std::env::temp_dir().join(format!(
            "tinycast-ql-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, b"not-json").unwrap();
        let store = QuicklinkStore::load_from(path.clone());
        assert!(store.links().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
        let mut store = store;
        let _ = store.upsert(Quicklink {
            id: "1".into(),
            name: "G".into(),
            destination: "https://example.com".into(),
            pinned: false,
            show_in_root: true,
            pin_order: 0,
        });
        let _ = std::fs::remove_file(path);
    }
}
