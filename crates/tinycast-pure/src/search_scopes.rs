#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEnt {
    pub name: String,
    pub is_dir: bool,
    pub nested: Vec<DirEnt>,
}

/// One-level listing: files (and `.lnk`) at the root, plus files (and `.lnk`)
/// in an immediate child folder. `.lnk` is always a leaf.
pub fn visible_leaves(root_children: &[DirEnt]) -> Vec<String> {
    let mut out = Vec::new();
    for child in root_children {
        if child.name.is_empty() || child.name == "." || child.name == ".." {
            continue;
        }
        if is_lnk_leaf(&child.name) || !child.is_dir {
            out.push(child.name.clone());
            continue;
        }
        for nested in &child.nested {
            if nested.name.is_empty() || nested.name == "." || nested.name == ".." {
                continue;
            }
            if is_lnk_leaf(&nested.name) || !nested.is_dir {
                out.push(format!("{}/{}", child.name, nested.name));
            }
        }
    }
    out
}

fn is_lnk_leaf(name: &str) -> bool {
    name.len() >= 4 && name.as_bytes()[name.len() - 4..].eq_ignore_ascii_case(b".lnk")
}

pub fn keep_alternate(display_name: &str, candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return false;
    }
    if candidate.eq_ignore_ascii_case(display_name.trim()) {
        return false;
    }
    !matches!(
        candidate.to_ascii_lowercase().as_str(),
        "app" | "application" | "generic" | "unknown"
    )
}

pub fn push_alternate(names: &mut Vec<String>, display_name: &str, candidate: &str) {
    let candidate = candidate.trim();
    if !keep_alternate(display_name, candidate) {
        return;
    }
    if names.iter().any(|n| n.eq_ignore_ascii_case(candidate)) {
        return;
    }
    names.push(candidate.to_string());
}

/// Dedup key: AUMID wins; otherwise a slash-normalized lowercase target path.
pub fn identity_key(aumid: Option<&str>, resolved_target: Option<&str>) -> String {
    if let Some(id) = aumid.map(str::trim).filter(|s| !s.is_empty()) {
        return format!("aumid:{}", id.to_ascii_lowercase());
    }
    if let Some(path) = resolved_target.map(str::trim).filter(|s| !s.is_empty()) {
        return format!("path:{}", normalize_target(path));
    }
    String::new()
}

fn normalize_target(path: &str) -> String {
    path.trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemSettingsPane {
    pub name: &'static str,
    pub uri: &'static str,
}

pub const SYSTEM_SETTINGS: &[SystemSettingsPane] = &[
    SystemSettingsPane {
        name: "Display",
        uri: "ms-settings:display",
    },
    SystemSettingsPane {
        name: "Bluetooth",
        uri: "ms-settings:bluetooth",
    },
    SystemSettingsPane {
        name: "Apps",
        uri: "ms-settings:appsfeatures",
    },
    SystemSettingsPane {
        name: "Power",
        uri: "ms-settings:powersleep",
    },
    SystemSettingsPane {
        name: "Time",
        uri: "ms-settings:dateandtime",
    },
    SystemSettingsPane {
        name: "Update",
        uri: "ms-settings:windowsupdate",
    },
    SystemSettingsPane {
        name: "Personalization",
        uri: "ms-settings:personalization",
    },
    SystemSettingsPane {
        name: "Privacy",
        uri: "ms-settings:privacy",
    },
    SystemSettingsPane {
        name: "Network",
        uri: "ms-settings:network",
    },
    SystemSettingsPane {
        name: "Gaming",
        uri: "ms-settings:gaming-gamebar",
    },
    SystemSettingsPane {
        name: "Accessibility",
        uri: "ms-settings:easeofaccess",
    },
    SystemSettingsPane {
        name: "Accounts",
        uri: "ms-settings:accounts",
    },
    SystemSettingsPane {
        name: "System",
        uri: "ms-settings:about",
    },
];

pub fn system_settings_entries() -> Vec<crate::app_entry::AppEntry> {
    use crate::app_entry::{AppEntry, AppKind};
    use crate::search_relevance::SearchFields;

    SYSTEM_SETTINGS
        .iter()
        .map(|pane| {
            let name = pane.name.to_string();
            AppEntry {
                id: format!("app:{}", pane.uri),
                kind: AppKind::SystemSettings,
                name: name.clone(),
                fields: SearchFields {
                    display_name: name,
                    bundle_id: Some(pane.uri.to_string()),
                    ..Default::default()
                },
                hotkey: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        identity_key, keep_alternate, system_settings_entries, visible_leaves, DirEnt,
        SYSTEM_SETTINGS,
    };
    use crate::app_entry::AppKind;

    #[test]
    fn walk_is_one_level_and_treats_lnk_as_leaf() {
        // temp dir: a.lnk, Vendor/b.lnk, Vendor/Nested/c.lnk
        let root = [
            DirEnt {
                name: "a.lnk".into(),
                is_dir: false,
                nested: vec![],
            },
            DirEnt {
                name: "Vendor".into(),
                is_dir: true,
                nested: vec![
                    DirEnt {
                        name: "b.lnk".into(),
                        is_dir: false,
                        nested: vec![],
                    },
                    DirEnt {
                        name: "Nested".into(),
                        is_dir: true,
                        nested: vec![DirEnt {
                            name: "c.lnk".into(),
                            is_dir: false,
                            nested: vec![],
                        }],
                    },
                ],
            },
        ];
        let found = visible_leaves(&root);
        assert!(found.iter().any(|p| p.ends_with("a.lnk")));
        assert!(found.iter().any(|p| p.ends_with("b.lnk")));
        assert!(!found.iter().any(|p| p.ends_with("c.lnk")));
    }

    #[test]
    fn lnk_directory_is_not_descended() {
        let root = [DirEnt {
            name: "folder.lnk".into(),
            is_dir: true,
            nested: vec![DirEnt {
                name: "hidden.lnk".into(),
                is_dir: false,
                nested: vec![],
            }],
        }];
        let found = visible_leaves(&root);
        assert_eq!(found, ["folder.lnk"]);
    }

    #[test]
    fn keep_alternate_drops_display_name_and_junk() {
        assert!(keep_alternate("Notepad", "Windows Notepad"));
        assert!(!keep_alternate("Notepad", "Notepad"));
        assert!(!keep_alternate("Notepad", "notepad"));
        assert!(!keep_alternate("Code", "app"));
        assert!(!keep_alternate("Code", "Application"));
        assert!(!keep_alternate("Code", "  "));
        assert!(!keep_alternate("Code", "generic"));
        assert!(!keep_alternate("Code", "unknown"));
        assert!(keep_alternate("Code", "-n"));
    }

    #[test]
    fn system_settings_catalog_covers_required_panes() {
        let names: Vec<&str> = SYSTEM_SETTINGS.iter().map(|p| p.name).collect();
        for required in [
            "Display",
            "Bluetooth",
            "Apps",
            "Power",
            "Time",
            "Update",
            "Personalization",
            "Privacy",
            "Network",
            "Gaming",
            "Accessibility",
            "Accounts",
            "System",
        ] {
            assert!(names.contains(&required), "missing {required}");
        }
        assert!(SYSTEM_SETTINGS
            .iter()
            .all(|p| p.uri.starts_with("ms-settings:")));
        let entries = system_settings_entries();
        assert_eq!(entries.len(), SYSTEM_SETTINGS.len());
        let display = entries
            .iter()
            .find(|e| e.name == "Display")
            .expect("Display");
        assert_eq!(display.kind, AppKind::SystemSettings);
        assert_eq!(display.id, "app:ms-settings:display");
        assert_eq!(
            display.fields.bundle_id.as_deref(),
            Some("ms-settings:display")
        );
        assert_eq!(display.fields.display_name, "Display");
    }

    #[test]
    fn identity_key_prefers_aumid_then_normalized_path() {
        assert_eq!(
            identity_key(Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"), None),
            "aumid:microsoft.windowsnotepad_8wekyb3d8bbwe!app"
        );
        assert_eq!(
            identity_key(None, Some(r#"C:\Windows\System32\notepad.exe"#)),
            identity_key(None, Some(r#"c:/windows/system32/notepad.exe"#))
        );
        assert_ne!(
            identity_key(Some("Foo!App"), Some(r"C:\Foo.exe")),
            identity_key(None, Some(r"C:\Foo.exe"))
        );
    }
}
