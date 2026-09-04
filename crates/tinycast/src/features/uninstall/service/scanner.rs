use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tinycast_pure::uninstall::plan::{UninstallCandidate, UninstallIdentity, RUNNING_BUNDLE_ID};
use tinycast_pure::uninstall::rules::{
    display_name_is_shared, folded, is_acceptable_candidate, is_generic_display_name,
};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::platform::messages::WM_UNINSTALL_SIZE;

use super::runner::native_windows_path;

#[derive(Clone, Debug)]
pub struct UninstallTarget {
    pub bundle_id: Option<String>,
    pub display_name: String,
    pub install_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InstalledPeer {
    pub bundle_id: String,
    pub display_name: String,
}

static SIZES: Mutex<Option<Vec<(String, u64)>>> = Mutex::new(None);

pub fn leftover_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["APPDATA", "LOCALAPPDATA", "ProgramData"] {
        if let Ok(p) = std::env::var(var) {
            roots.push(PathBuf::from(p));
        }
    }
    roots.extend(arp_install_roots());
    roots.extend(appx_install_roots());
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        roots.retain(|r| r != &home);
    }
    roots
}

pub fn discover(
    target: &UninstallTarget,
    running_id: &str,
    peers: &[InstalledPeer],
) -> Option<Vec<UninstallCandidate>> {
    let bundle = target
        .bundle_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let other_ids: Vec<&str> = peers
        .iter()
        .map(|p| p.bundle_id.as_str())
        .filter(|s| !s.is_empty() && folded(s) != folded(bundle))
        .collect();
    let identity = if bundle.is_empty() {
        None
    } else {
        UninstallIdentity::make_with_others(bundle, running_id, &other_ids)
    };
    if !bundle.is_empty() && identity.is_none() {
        return None;
    }
    let peer_names: Vec<String> = peers.iter().map(|p| p.display_name.clone()).collect();
    let mut out = Vec::new();
    if let Some(path) = &target.install_path {
        let native = native_windows_path(path);
        if Path::new(&native).exists() {
            let locked = !is_parent_writable(&native);
            out.push(UninstallCandidate {
                id: native.clone(),
                name: target.display_name.clone(),
                path: native,
                locked,
                is_bundle: true,
                size: None,
            });
        }
    }
    for extra in matching_install_locations(target, peers) {
        if out.iter().any(|c| folded(&c.path) == folded(&extra)) {
            continue;
        }
        if Path::new(&extra).exists() {
            out.push(UninstallCandidate {
                id: extra.clone(),
                name: target.display_name.clone(),
                path: extra.clone(),
                locked: !is_parent_writable(&extra),
                is_bundle: true,
                size: None,
            });
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let bundle_path = target
        .install_path
        .as_deref()
        .map(native_windows_path)
        .unwrap_or_default();
    let display = folded(&target.display_name);
    let allow_display = !display.is_empty()
        && display.len() >= 3
        && !is_generic_display_name(&target.display_name)
        && !display_name_is_shared(&target.display_name, &peer_names);
    for root in leftover_roots() {
        let root_s = root.to_string_lossy().replace('\\', "/");
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = native_windows_path(&entry.path().to_string_lossy());
            if !is_acceptable_candidate(&path, &root_s, &home, &bundle_path) {
                continue;
            }
            let matches = identity
                .as_ref()
                .is_some_and(|id| id.matches_bundle(&name))
                || (allow_display && folded(&name) == display);
            if !matches {
                continue;
            }
            if out.iter().any(|c| folded(&c.path) == folded(&path)) {
                continue;
            }
            out.push(UninstallCandidate {
                id: path.clone(),
                name,
                path: path.clone(),
                locked: !is_parent_writable(&path),
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

pub fn measure_path(path: &str) -> u64 {
    let path = Path::new(path);
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    let mut n = 0u32;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            n += 1;
            if n > 8_000 {
                return total;
            }
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

pub fn begin_measure(paths: Vec<String>, host: HWND) {
    if host.is_invalid() || paths.is_empty() {
        return;
    }
    let bits = host.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-uninstall-size".into())
        .spawn(move || {
            let host = HWND(bits as *mut core::ffi::c_void);
            let mut out = Vec::new();
            for p in paths {
                out.push((p.clone(), measure_path(&p)));
            }
            if let Ok(mut g) = SIZES.lock() {
                *g = Some(out);
            }
            unsafe {
                let _ = PostMessageW(host, WM_UNINSTALL_SIZE, WPARAM(0), LPARAM(0));
            }
        });
}

pub fn take_sizes() -> Option<Vec<(String, u64)>> {
    SIZES.lock().ok().and_then(|mut g| g.take())
}

fn matching_install_locations(target: &UninstallTarget, peers: &[InstalledPeer]) -> Vec<String> {
    let want_name = folded(&target.display_name);
    let want_id = target
        .bundle_id
        .as_deref()
        .map(folded)
        .unwrap_or_default();
    let mut out = Vec::new();
    for row in arp_rows() {
        if (!want_id.is_empty() && folded(&row.0) == want_id)
            || (!want_name.is_empty() && folded(&row.1) == want_name)
        {
            if !row.2.is_empty() {
                out.push(native_windows_path(&row.2));
            }
        }
    }
    for row in appx_rows() {
        if (!want_id.is_empty() && (folded(&row.0) == want_id || folded(&row.1) == want_id))
            || (!want_name.is_empty() && folded(&row.2) == want_name)
        {
            if !row.3.is_empty() {
                out.push(native_windows_path(&row.3));
            }
        }
    }
    let _ = peers;
    out
}

fn arp_install_roots() -> Vec<PathBuf> {
    arp_rows()
        .into_iter()
        .filter_map(|r| {
            if r.2.is_empty() {
                None
            } else {
                Path::new(&r.2).parent().map(|p| p.to_path_buf())
            }
        })
        .collect()
}

fn appx_install_roots() -> Vec<PathBuf> {
    appx_rows()
        .into_iter()
        .filter_map(|r| {
            if r.3.is_empty() {
                None
            } else {
                Path::new(&r.3).parent().map(|p| p.to_path_buf())
            }
        })
        .collect()
}

/// (product, display, install_location)
fn arp_rows() -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    const KEYS: &[(HKEY, &str)] = &[
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (root, path) in KEYS {
        rows.extend(enum_uninstall(*root, path));
    }
    rows
}

fn enum_uninstall(root: HKEY, path: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut key = HKEY::default();
    unsafe {
        if RegOpenKeyExW(root, PCWSTR(wide.as_ptr()), 0, KEY_READ, &mut key).is_err() {
            return out;
        }
        let mut index = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut name_len = name.len() as u32;
            if RegEnumKeyExW(key, index, windows::core::PWSTR(name.as_mut_ptr()), &mut name_len, None, windows::core::PWSTR::null(), None, None).is_err() {
                break;
            }
            index += 1;
            let sub_name = String::from_utf16_lossy(&name[..name_len as usize]);
            let mut sub = HKEY::default();
            let sub_wide: Vec<u16> = sub_name.encode_utf16().chain(std::iter::once(0)).collect();
            if RegOpenKeyExW(key, PCWSTR(sub_wide.as_ptr()), 0, KEY_READ, &mut sub).is_err() {
                continue;
            }
            let display = reg_sz(sub, "DisplayName").unwrap_or_default();
            let location = reg_sz(sub, "InstallLocation").unwrap_or_default();
            let _ = RegCloseKey(sub);
            out.push((sub_name, display, location.trim().trim_matches('"').to_string()));
        }
        let _ = RegCloseKey(key);
    }
    out
}

fn reg_sz(key: HKEY, name: &str) -> Option<String> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut ty = REG_VALUE_TYPE::default();
    let mut size = 0u32;
    unsafe {
        let _ = RegQueryValueExW(key, PCWSTR(wide.as_ptr()), None, Some(&mut ty), None, Some(&mut size));
        if ty != REG_SZ || size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if RegQueryValueExW(
            key,
            PCWSTR(wide.as_ptr()),
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
        .is_err()
        {
            return None;
        }
        let u16s = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|c| *c != 0)
            .collect::<Vec<_>>();
        Some(String::from_utf16_lossy(&u16s))
    }
}

/// (family, name, display, path)
fn appx_rows() -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    let Ok(pm) = windows::Management::Deployment::PackageManager::new() else {
        return out;
    };
    let Ok(packages) = pm.FindPackages() else {
        return out;
    };
    let Ok(iter) = packages.First() else {
        return out;
    };
    while iter.HasCurrent().unwrap_or(false) {
        let Ok(pkg) = iter.Current() else {
            let _ = iter.MoveNext();
            continue;
        };
        let family = pkg
            .Id()
            .ok()
            .and_then(|id| id.FamilyName().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let name = pkg
            .Id()
            .ok()
            .and_then(|id| id.Name().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let display = pkg.DisplayName().map(|s| s.to_string()).unwrap_or_default();
        let path = pkg
            .InstalledPath()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let _ = Interface::vtable(&pkg);
        out.push((family, name, display, path));
        let _ = iter.MoveNext();
    }
    out
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
        assert!(discover(&target, RUNNING_BUNDLE_ID, &[]).is_none());
    }

    #[test]
    fn sibling_peers_are_passed_to_identity() {
        let target = UninstallTarget {
            bundle_id: Some("com.foo.bar".into()),
            display_name: "Foo".into(),
            install_path: None,
        };
        let peers = vec![
            InstalledPeer {
                bundle_id: "com.foo.bar".into(),
                display_name: "Foo".into(),
            },
            InstalledPeer {
                bundle_id: "com.foo.bar.beta".into(),
                display_name: "Foo Beta".into(),
            },
        ];
        let found = discover(&target, "com.other.app", &peers).unwrap();
        assert!(!found.iter().any(|c| folded(&c.name) == "com.foo.bar.beta"));
    }

    #[test]
    fn measure_counts_file_bytes() {
        let dir = std::env::temp_dir().join(format!(
            "tinycast-un-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"hello").unwrap();
        assert_eq!(measure_path(&dir.to_string_lossy()), 5);
        let _ = fs::remove_dir_all(&dir);
    }
}
