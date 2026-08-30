use std::path::PathBuf;

use tinycast_pure::custom_command::{sort_by_name, CustomCommand};

use crate::platform::paths;

pub struct CustomCommandStore {
    path: PathBuf,
    commands: Vec<CustomCommand>,
}

impl CustomCommandStore {
    pub fn load() -> Self {
        Self::load_from(paths::roaming_dir().join("custom-commands.json"))
    }

    pub fn load_from(path: PathBuf) -> Self {
        let mut commands: Vec<CustomCommand> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        sort_by_name(&mut commands);
        Self { path, commands }
    }

    pub fn commands(&self) -> &[CustomCommand] {
        &self.commands
    }

    pub fn get(&self, id: &str) -> Option<&CustomCommand> {
        self.commands.iter().find(|c| c.id.eq_ignore_ascii_case(id))
    }

    pub fn upsert(&mut self, command: CustomCommand) -> Result<(), String> {
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
        self.commands.retain(|c| c.id != id);
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
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

    #[test]
    fn store_sorts_and_rejects_duplicate_names() {
        let path = std::env::temp_dir().join(format!(
            "tinycast-cc-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
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
}
