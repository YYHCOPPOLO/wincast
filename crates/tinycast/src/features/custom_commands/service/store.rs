use std::path::PathBuf;

use tinycast_pure::custom_command::{sort_by_name, CustomCommand};

use crate::platform::paths;

pub const STORAGE_UNAVAILABLE: &str = "The library is unavailable.";

pub struct CustomCommandStore {
    path: PathBuf,
    commands: Vec<CustomCommand>,
    available: bool,
}

impl CustomCommandStore {
    pub fn load() -> Self {
        Self::load_from(paths::roaming_dir().join("custom-commands.json"))
    }

    pub fn load_from(path: PathBuf) -> Self {
        if !path.exists() {
            return Self {
                path,
                commands: Vec::new(),
                available: true,
            };
        }
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<CustomCommand>>(&bytes) {
                Ok(mut commands) => {
                    sort_by_name(&mut commands);
                    Self {
                        path,
                        commands,
                        available: true,
                    }
                }
                Err(_) => Self {
                    path,
                    commands: Vec::new(),
                    available: false,
                },
            },
            Err(_) => Self {
                path,
                commands: Vec::new(),
                available: false,
            },
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn commands(&self) -> &[CustomCommand] {
        &self.commands
    }

    pub fn get(&self, id: &str) -> Option<&CustomCommand> {
        self.commands.iter().find(|c| c.id.eq_ignore_ascii_case(id))
    }

    pub fn upsert(&mut self, command: CustomCommand) -> Result<(), String> {
        self.ensure_available()?;
        let name = command.name.trim().to_string();
        let cmdline = command.command.trim().to_string();
        if name.is_empty() {
            return Err("Enter a name for the command.".into());
        }
        if cmdline.is_empty() {
            return Err("Enter a command to run.".into());
        }
        if self
            .commands
            .iter()
            .any(|c| c.id != command.id && c.name.eq_ignore_ascii_case(&name))
        {
            return Err("A custom command with this name already exists.".into());
        }
        let mut command = command;
        command.name = name;
        command.command = cmdline;
        if let Some(existing) = self.commands.iter_mut().find(|c| c.id == command.id) {
            *existing = command;
        } else {
            self.commands.push(command);
        }
        sort_by_name(&mut self.commands);
        self.persist()
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        self.ensure_available()?;
        self.commands.retain(|c| c.id != id);
        self.persist()
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
        let bytes = serde_json::to_vec_pretty(&self.commands).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, bytes).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tinycast-cc-{tag}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn store_sorts_and_rejects_duplicate_names() {
        let path = temp_path("sort");
        let _ = std::fs::remove_file(&path);
        let mut store = CustomCommandStore::load_from(path.clone());
        store
            .upsert(CustomCommand {
                id: "2".into(),
                name: "B".into(),
                command: "echo b".into(),
                confirm: false,
            })
            .unwrap();
        store
            .upsert(CustomCommand {
                id: "1".into(),
                name: "A".into(),
                command: "echo a".into(),
                confirm: true,
            })
            .unwrap();
        assert_eq!(
            store.commands().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["A", "B"]
        );
        store.remove("2").unwrap();
        assert_eq!(store.commands().len(), 1);
        let err = store.upsert(CustomCommand {
            id: "3".into(),
            name: "a".into(),
            command: "echo".into(),
            confirm: false,
        });
        assert!(err.is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn corrupt_library_is_unavailable_and_not_overwritten() {
        let path = temp_path("corrupt");
        std::fs::write(&path, b"{not-json").unwrap();
        let mut store = CustomCommandStore::load_from(path.clone());
        assert!(!store.is_available());
        assert!(store.commands().is_empty());
        let err = store
            .upsert(CustomCommand {
                id: "1".into(),
                name: "A".into(),
                command: "echo".into(),
                confirm: false,
            })
            .unwrap_err();
        assert_eq!(err, STORAGE_UNAVAILABLE);
        assert_eq!(std::fs::read(&path).unwrap(), b"{not-json");
        assert_eq!(store.remove("1").unwrap_err(), STORAGE_UNAVAILABLE);
        assert_eq!(std::fs::read(&path).unwrap(), b"{not-json");
        let _ = std::fs::remove_file(path);
    }
}
