use crate::app_entry::{AppEntry, AppKind};
use crate::search_relevance::SearchFields;

pub const ENTRY_PREFIX: &str = "custom-command:";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CustomCommand {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub confirm: bool,
}

impl CustomCommand {
    pub fn entry_id(&self) -> String {
        format!("{ENTRY_PREFIX}{}", self.id.to_lowercase())
    }

    pub fn id_from_entry(entry_id: &str) -> Option<&str> {
        entry_id.strip_prefix(ENTRY_PREFIX)
    }

    pub fn as_entry(&self) -> AppEntry {
        AppEntry {
            id: self.entry_id(),
            kind: AppKind::CustomCommand,
            name: self.name.clone(),
            fields: SearchFields {
                display_name: self.name.clone(),
                ..Default::default()
            },
            hotkey: None,
        }
    }
}

pub fn sort_by_name(commands: &mut [CustomCommand]) {
    commands.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub fn uses_cmd_shell(command: &str) -> bool {
    let trimmed = command.trim_start();
    let head = trimmed.split_whitespace().next().unwrap_or("");
    let file = std::path::Path::new(head)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(head);
    file.eq_ignore_ascii_case("cmd.exe") || file.eq_ignore_ascii_case("cmd")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str) -> CustomCommand {
        CustomCommand {
            id: name.to_string(),
            name: name.to_string(),
            command: "true".into(),
            confirm: false,
        }
    }

    #[test]
    fn custom_commands_sort_by_name() {
        let mut v = vec![cmd("b"), cmd("a")];
        v.sort_by(|x, y| x.name.cmp(&y.name));
        assert_eq!(v[0].name, "a");
    }

    #[test]
    fn sort_by_name_helper_is_case_insensitive() {
        let mut v = vec![cmd("B"), cmd("a")];
        sort_by_name(&mut v);
        assert_eq!(v[0].name, "a");
    }

    #[test]
    fn cmd_exe_is_detected_as_shell() {
        assert!(uses_cmd_shell(r"cmd.exe /c echo hi"));
        assert!(uses_cmd_shell("CMD /c dir"));
        assert!(!uses_cmd_shell(r"C:\Windows\System32\notepad.exe"));
    }

    #[test]
    fn entry_id_uses_prefix() {
        let c = cmd("sleep");
        assert_eq!(c.entry_id(), "custom-command:sleep");
        assert_eq!(CustomCommand::id_from_entry(&c.entry_id()), Some("sleep"));
        assert_eq!(c.as_entry().kind, AppKind::CustomCommand);
    }
}
