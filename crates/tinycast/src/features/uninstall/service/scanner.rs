use std::fs;
use std::path::{Path, PathBuf};

use tinycast_pure::uninstall::plan::{UninstallCandidate, UninstallIdentity, RUNNING_BUNDLE_ID};
use tinycast_pure::uninstall::rules::{folded, is_acceptable_candidate};

#[derive(Clone, Debug)]
pub struct UninstallTarget {
    pub bundle_id: Option<String>,
    pub display_name: String,
    pub install_path: Option<String>,
}

pub fn leftover_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["APPDATA", "LOCALAPPDATA", "ProgramData"] {
        if let Ok(p) = std::env::var(var) {
            roots.push(PathBuf::from(p));
        }
    }
    // Home itself is never a root.
    roots
}

pub fn discover(target: &UninstallTarget, running_id: &str) -> Option<Vec<UninstallCandidate>> {
    let bundle = target
        .bundle_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let identity = if bundle.is_empty() {
        None
    } else {
        UninstallIdentity::make(bundle, running_id)
    };
    if !bundle.is_empty() && identity.is_none() {
        return None;
    }
    let mut out = Vec::new();
    if let Some(path) = &target.install_path {
        if Path::new(path).exists() {
            let locked = !is_parent_writable(path);
            out.push(UninstallCandidate {
                id: path.clone(),
                name: target.display_name.clone(),
                path: path.clone(),
                locked,
                is_bundle: true,
                size: None,
            });
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let bundle_path = target.install_path.clone().unwrap_or_default();
    let display = folded(&target.display_name);
    for root in leftover_roots() {
        let root_s = root.to_string_lossy().replace('\\', "/");
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path().to_string_lossy().replace('\\', "/");
            if !is_acceptable_candidate(&path, &root_s, &home, &bundle_path) {
                continue;
            }
            let matches = identity
                .as_ref()
                .is_some_and(|id| id.matches_bundle(&name))
                || (!display.is_empty()
                    && display.len() >= 3
                    && folded(&name) == display);
            if !matches {
                continue;
            }
            let locked = !is_parent_writable(&path);
            out.push(UninstallCandidate {
                id: path.clone(),
                name,
                path,
                locked,
                is_bundle: false,
                size: None,
            });
        }
    }
    Some(out)
}

pub fn running_id() -> &'static str {
    RUNNING_BUNDLE_ID
}

fn is_parent_writable(path: &str) -> bool {
    Path::new(path)
        .parent()
        .map(|p| {
            let probe = p.join(".tinycast-uninstall-probe");
            match fs::write(&probe, b"") {
                Ok(()) => {
                    let _ = fs::remove_file(&probe);
                    true
                }
                Err(_) => false,
            }
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_not_a_root() {
        let roots = leftover_roots();
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        assert!(!roots.iter().any(|r| r == Path::new(&home)));
    }

    #[test]
    fn refuses_running_tinycast() {
        let target = UninstallTarget {
            bundle_id: Some(RUNNING_BUNDLE_ID.into()),
            display_name: "Tinycast".into(),
            install_path: None,
        };
        assert!(discover(&target, RUNNING_BUNDLE_ID).is_none());
    }
}
