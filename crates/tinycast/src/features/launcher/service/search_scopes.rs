use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use tinycast_pure::search_scopes::{visible_leaves, DirEnt};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_PublicDesktop, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
};

use crate::app_settings::AppSettings;

pub struct SearchScopes {
    pub roots: Vec<String>,
}

impl SearchScopes {
    pub fn defaults() -> Self {
        Self {
            roots: AppSettings::default().launcher_search_scopes,
        }
    }

    pub fn load() -> Self {
        let roots = AppSettings::load().launcher_search_scopes;
        if roots.is_empty() {
            Self::defaults()
        } else {
            Self { roots }
        }
    }

    pub fn expanded(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for path in self
            .roots
            .iter()
            .map(|root| expand_env(root))
            .chain(desktop_roots())
        {
            if !path.is_dir() {
                continue;
            }
            if !seen.insert(path_key(&path)) {
                continue;
            }
            out.push(path);
        }
        out
    }
}

/// User and Public Desktop, including OneDrive-redirected Desktop.
pub fn desktop_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for path in [
        known_folder(&FOLDERID_Desktop),
        known_folder(&FOLDERID_PublicDesktop),
        Some(expand_env(r"%USERPROFILE%\Desktop")),
        Some(expand_env(r"%PUBLIC%\Desktop")),
    ]
    .into_iter()
    .flatten()
    {
        if !path.is_dir() {
            continue;
        }
        if !seen.insert(path_key(&path)) {
            continue;
        }
        out.push(path);
    }
    out
}

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let pwstr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, HANDLE::default()).ok()?;
        if pwstr.is_null() {
            return None;
        }
        let path = pwstr.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(pwstr.0.cast()));
        path
    }
}

fn path_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

pub fn collect_leaf_paths(root: &Path) -> Vec<PathBuf> {
    visible_leaves(&read_dirents(root, true))
        .into_iter()
        .map(|rel| {
            let mut path = root.to_path_buf();
            for part in rel.split('/') {
                path.push(part);
            }
            path
        })
        .collect()
}

pub fn expand_env(value: &str) -> PathBuf {
    let src = to_wide(value);
    unsafe {
        let needed = ExpandEnvironmentStringsW(PCWSTR(src.as_ptr()), None);
        if needed == 0 {
            return PathBuf::from(value);
        }
        let mut buf = vec![0u16; needed as usize];
        let written = ExpandEnvironmentStringsW(PCWSTR(src.as_ptr()), Some(&mut buf));
        if written == 0 {
            return PathBuf::from(value);
        }
        let len = (written as usize).saturating_sub(1).min(buf.len());
        PathBuf::from(OsString::from_wide(&buf[..len]))
    }
}

fn read_dirents(root: &Path, descend: bool) -> Vec<DirEnt> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.eq_ignore_ascii_case("desktop.ini") || name == "." || name == ".." {
            continue;
        }
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let nested = if descend && is_dir && !is_lnk_name(&name) {
            read_dirents(&ent.path(), false)
        } else {
            Vec::new()
        };
        out.push(DirEnt {
            name,
            is_dir,
            nested,
        });
    }
    out
}

fn is_lnk_name(name: &str) -> bool {
    name.len() >= 4 && name.as_bytes()[name.len() - 4..].eq_ignore_ascii_case(b".lnk")
}

fn to_wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::app_settings_key::AppSettingsKey;

    fn unique_temp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "tinycast-scopes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn default_scopes_include_start_menu_program_files_and_desktop() {
        let scopes = SearchScopes::defaults();
        assert_eq!(
            scopes.roots,
            [
                r"%ProgramData%\Microsoft\Windows\Start Menu\Programs",
                r"%APPDATA%\Microsoft\Windows\Start Menu\Programs",
                r"%ProgramFiles%",
                r"%ProgramFiles(x86)%",
                r"%USERPROFILE%\Desktop",
                r"%PUBLIC%\Desktop",
            ]
        );
        assert_eq!(
            AppSettingsKey::SearchScopes.as_str(),
            "launcherSearchScopes"
        );
    }

    #[test]
    fn expanded_always_includes_desktop() {
        let scopes = SearchScopes {
            roots: vec![r"%ProgramFiles%".into()],
        };
        let expanded = scopes.expanded();
        assert!(
            expanded.iter().any(|p| is_desktop_dir(p)),
            "expected a Desktop folder in {expanded:?}"
        );
        let desktops = desktop_roots();
        assert!(
            !desktops.is_empty(),
            "this machine should have a Desktop directory"
        );
        for desktop in &desktops {
            assert!(
                expanded.iter().any(|p| path_key(p) == path_key(desktop)),
                "missing {desktop:?} in {expanded:?}"
            );
        }
    }

    fn is_desktop_dir(path: &Path) -> bool {
        path.file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case("Desktop"))
            .unwrap_or(false)
    }

    #[test]
    fn expand_env_resolves_programdata() {
        let path = expand_env(r"%ProgramData%\Microsoft\Windows\Start Menu\Programs");
        let text = path.to_string_lossy();
        assert!(!text.contains('%'));
        assert!(text.ends_with(r"Microsoft\Windows\Start Menu\Programs"));
    }

    #[test]
    fn collect_leaf_paths_is_one_level_and_treats_lnk_as_leaf() {
        let root = unique_temp();
        std::fs::create_dir_all(root.join("Vendor").join("Nested")).unwrap();
        std::fs::write(root.join("a.lnk"), []).unwrap();
        std::fs::write(root.join("Vendor").join("b.lnk"), []).unwrap();
        std::fs::write(root.join("Vendor").join("Nested").join("c.lnk"), []).unwrap();
        std::fs::write(root.join("Vendor").join("tool.exe"), []).unwrap();
        let found = collect_leaf_paths(&root);
        let names: Vec<String> = found
            .iter()
            .filter_map(|p| p.file_name()?.to_str().map(|s| s.to_string()))
            .collect();
        assert!(names.iter().any(|n| n == "a.lnk"));
        assert!(names.iter().any(|n| n == "b.lnk"));
        assert!(names.iter().any(|n| n == "tool.exe"));
        assert!(!names.iter().any(|n| n == "c.lnk"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
