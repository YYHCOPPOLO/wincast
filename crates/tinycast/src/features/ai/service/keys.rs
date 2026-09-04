//! API keys in DPAPI files keyed by connection UUID. Never logged.

use std::path::PathBuf;

use tinycast_pure::ai::Uuid;

use crate::platform::{dpapi, paths};

pub struct ApiKeyStore {
    dir: PathBuf,
}

impl ApiKeyStore {
    pub fn open() -> Self {
        Self {
            dir: paths::roaming_dir().join("ai-keys"),
        }
    }

    #[cfg(test)]
    pub fn open_in(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn get(&self, id: &Uuid) -> Option<String> {
        let bytes = std::fs::read(self.path(id)).ok()?;
        let plain = dpapi::unprotect(&bytes, id.as_str().as_bytes()).ok()?;
        String::from_utf8(plain).ok().filter(|s| !s.is_empty())
    }

    pub fn has(&self, id: &Uuid) -> bool {
        self.path(id).is_file()
    }

    pub fn set(&self, id: &Uuid, key: &str) -> Result<(), String> {
        let _ = std::fs::create_dir_all(&self.dir);
        let sealed = dpapi::protect(key.as_bytes(), id.as_str().as_bytes())?;
        std::fs::write(self.path(id), sealed).map_err(|_| "could not store API key".to_string())
    }

    pub fn remove(&self, id: &Uuid) {
        let _ = std::fs::remove_file(self.path(id));
    }

    fn path(&self, id: &Uuid) -> PathBuf {
        self.dir.join(format!("{}.dpapi", id.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_forgets_by_uuid() {
        let dir = std::env::temp_dir().join(format!(
            "tinycast-ai-keys-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ApiKeyStore::open_in(dir.clone());
        let id = Uuid::generate();
        store.set(&id, "sk-secret").unwrap();
        assert_eq!(store.get(&id).as_deref(), Some("sk-secret"));
        store.remove(&id);
        assert!(store.get(&id).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
