use std::path::PathBuf;

use tinycast_pure::quicklink::{sort_quicklinks, Quicklink};

use crate::platform::paths;

pub const STORAGE_UNAVAILABLE: &str = "The library is unavailable.";

pub struct QuicklinkStore {
    path: PathBuf,
    links: Vec<Quicklink>,
    available: bool,
}

impl QuicklinkStore {
    pub fn load() -> Self {
        Self::load_from(paths::roaming_dir().join("quicklinks.json"))
    }

    pub fn load_from(path: PathBuf) -> Self {
        if !path.exists() {
            return Self {
                path,
                links: Vec::new(),
                available: true,
            };
        }
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<Quicklink>>(&bytes) {
                Ok(mut links) => {
                    sort_quicklinks(&mut links);
                    Self {
                        path,
                        links,
                        available: true,
                    }
                }
                Err(_) => Self {
                    path,
                    links: Vec::new(),
                    available: false,
                },
            },
            Err(_) => Self {
                path,
                links: Vec::new(),
                available: false,
            },
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn links(&self) -> &[Quicklink] {
        &self.links
    }

    pub fn get(&self, id: &str) -> Option<&Quicklink> {
        self.links.iter().find(|l| l.id == id)
    }

    pub fn upsert(&mut self, link: Quicklink) -> Result<(), String> {
        self.ensure_available()?;
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

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        self.ensure_available()?;
        self.links.retain(|l| l.id != id);
        self.persist()
    }

    pub fn import_from_bytes(&mut self, bytes: &[u8]) -> Result<usize, String> {
        self.ensure_available()?;
        let incoming: Vec<Quicklink> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let mut n = 0usize;
        for link in incoming {
            if link.name.trim().is_empty() || link.destination.trim().is_empty() {
                continue;
            }
            if let Some(existing) = self.links.iter_mut().find(|l| l.id == link.id) {
                *existing = link;
            } else {
                self.links.push(link);
            }
            n += 1;
        }
        sort_quicklinks(&mut self.links);
        self.persist()?;
        Ok(n)
    }

    pub fn export_bytes(&self) -> Result<Vec<u8>, String> {
        self.ensure_available()?;
        serde_json::to_vec_pretty(&self.links).map_err(|e| e.to_string())
    }

    fn ensure_available(&self) -> Result<(), String> {
        if self.available {
            Ok(())
        } else {
            Err(STORAGE_UNAVAILABLE.into())
        }
    }

    fn persist(&self) -> Result<(), String> {
        self.ensure_available()?;
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

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tinycast-ql-{tag}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn store_keeps_corrupt_file() {
        let path = temp_path("corrupt");
        std::fs::write(&path, b"not-json").unwrap();
        let store = QuicklinkStore::load_from(path.clone());
        assert!(store.links().is_empty());
        assert!(!store.is_available());
        assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
        let mut store = store;
        let err = store
            .upsert(Quicklink {
                id: "1".into(),
                name: "G".into(),
                destination: "https://example.com".into(),
                pinned: false,
                show_in_root: true,
                pin_order: 0,
            })
            .unwrap_err();
        assert_eq!(err, STORAGE_UNAVAILABLE);
        assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
        let err = store.remove("1").unwrap_err();
        assert_eq!(err, STORAGE_UNAVAILABLE);
        assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_file_is_available_empty() {
        let path = temp_path("missing");
        let _ = std::fs::remove_file(&path);
        let mut store = QuicklinkStore::load_from(path.clone());
        assert!(store.is_available());
        store
            .upsert(Quicklink {
                id: "1".into(),
                name: "G".into(),
                destination: "https://example.com".into(),
                pinned: false,
                show_in_root: true,
                pin_order: 0,
            })
            .unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(path);
    }
}
