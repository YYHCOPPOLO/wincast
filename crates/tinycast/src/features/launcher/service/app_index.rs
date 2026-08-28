use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tinycast_pure::app_entry::{AppEntry, AppKind};
use tinycast_pure::search_relevance::SearchFields;
use tinycast_pure::search_scopes::{identity_key, push_alternate, system_settings_entries};
use windows::core::{w, Interface, BSTR, GUID, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, WPARAM};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, WIN32_FIND_DATAW,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    STGM_READ,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, RRF_RT_REG_EXPAND_SZ,
    RRF_RT_REG_SZ,
};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IPropertyStore, SHGetPropertyStoreFromParsingName, GPS_DEFAULT, PROPERTYKEY,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use super::search_scopes::{collect_leaf_paths, expand_env, SearchScopes};
use crate::platform::messages::WM_APP_INDEX;

const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};

const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

#[derive(Clone, Debug)]
pub(crate) struct ResolvedApp {
    pub name: String,
    pub aumid: Option<String>,
    pub target: Option<String>,
    pub executable_name: Option<String>,
    pub alternate_names: Vec<String>,
}

pub struct AppIndex {
    host: HWND,
    generation: u64,
    pending: Arc<Mutex<Option<(u64, Vec<AppEntry>)>>>,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            host: HWND::default(),
            generation: 0,
            pending: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_host(&mut self, host: HWND) {
        self.host = host;
    }

    pub fn start(&mut self) {
        let generation = self.begin_scan();
        if self.host.is_invalid() {
            return;
        }
        let host_bits = self.host.0 as isize;
        let pending = Arc::clone(&self.pending);
        let _ = std::thread::Builder::new()
            .name("tinycast-app-index".into())
            .spawn(move || {
                let entries = scan_catalog();
                publish_pending(&pending, generation, entries);
                let host = HWND(host_bits as *mut core::ffi::c_void);
                let _ = unsafe {
                    PostMessageW(host, WM_APP_INDEX, WPARAM(generation as usize), LPARAM(0))
                };
            });
    }

    pub fn begin_scan(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn publish(&self, generation: u64, entries: Vec<AppEntry>) {
        publish_pending(&self.pending, generation, entries);
    }

    pub fn take_latest(&self) -> Option<Vec<AppEntry>> {
        let mut slot = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let (generation, entries) = slot.take()?;
        if generation != self.generation {
            return None;
        }
        Some(entries)
    }
}

fn publish_pending(
    pending: &Mutex<Option<(u64, Vec<AppEntry>)>>,
    generation: u64,
    entries: Vec<AppEntry>,
) {
    let mut slot = pending.lock().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_some_and(|(g, _)| *g > generation) {
        return;
    }
    *slot = Some((generation, entries));
}

pub(crate) fn scan_catalog() -> Vec<AppEntry> {
    ensure_com();
    let mut catalog = Catalog::default();
    for root in SearchScopes::load().expanded() {
        for leaf in collect_leaf_paths(&root) {
            if let Some(app) = resolve_leaf(&leaf) {
                catalog.push(app);
            }
        }
    }
    scan_appx(&mut catalog);
    scan_app_paths(&mut catalog);
    catalog.finish()
}

pub(crate) fn fold_apps(apps: Vec<ResolvedApp>) -> Vec<AppEntry> {
    let mut catalog = Catalog::default();
    for app in apps {
        catalog.push(app);
    }
    catalog.finish()
}

#[derive(Default)]
struct Catalog {
    entries: Vec<AppEntry>,
    keys: HashMap<String, usize>,
}

impl Catalog {
    fn push(&mut self, app: ResolvedApp) {
        let key = identity_key(app.aumid.as_deref(), app.target.as_deref());
        if !key.is_empty() {
            if let Some(&idx) = self.keys.get(&key) {
                let existing = &mut self.entries[idx];
                push_alternate(
                    &mut existing.fields.alternate_names,
                    &existing.name,
                    &app.name,
                );
                for alt in app.alternate_names {
                    push_alternate(&mut existing.fields.alternate_names, &existing.name, &alt);
                }
                return;
            }
        }
        let entry = application_entry(&app);
        if !key.is_empty() {
            self.keys.insert(key, self.entries.len());
        }
        self.entries.push(entry);
    }

    fn finish(mut self) -> Vec<AppEntry> {
        self.entries.extend(system_settings_entries());
        self.entries
    }
}

fn application_entry(app: &ResolvedApp) -> AppEntry {
    let rest = app
        .aumid
        .as_deref()
        .or(app.target.as_deref())
        .unwrap_or(app.name.as_str());
    let mut alternate_names = Vec::new();
    for alt in &app.alternate_names {
        push_alternate(&mut alternate_names, &app.name, alt);
    }
    AppEntry {
        id: format!("app:{rest}"),
        kind: AppKind::Application,
        name: app.name.clone(),
        fields: SearchFields {
            display_name: app.name.clone(),
            alternate_names,
            bundle_id: app.aumid.clone(),
            executable_name: app.executable_name.clone(),
            ..Default::default()
        },
        hotkey: None,
    }
}

fn resolve_leaf(path: &Path) -> Option<ResolvedApp> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "lnk" => resolve_lnk(path),
        "exe" => resolve_exe(path),
        _ => None,
    }
}

fn resolve_exe(path: &Path) -> Option<ResolvedApp> {
    if !path.is_file() {
        return None;
    }
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let exe_name = path.file_name()?.to_string_lossy().into_owned();
    let target = canonicalize_text(path);
    let desc = file_description(path);
    let name = pick_display_name(&stem, desc.as_deref());
    let mut alternate_names = Vec::new();
    if let Some(desc) = desc {
        push_alternate(&mut alternate_names, &name, &desc);
    }
    Some(ResolvedApp {
        name,
        aumid: None,
        target: Some(target),
        executable_name: Some(exe_name),
        alternate_names,
    })
}

fn resolve_lnk(path: &Path) -> Option<ResolvedApp> {
    let name = path.file_stem()?.to_string_lossy().into_owned();
    if name.is_empty() {
        return None;
    }
    let aumid = shortcut_aumid(path);
    let (target, arguments) = shortcut_target(path);
    if aumid.is_none() && target.as_ref().is_none_or(|t| Path::new(t).is_dir()) {
        return None;
    }
    let mut alternate_names = Vec::new();
    if let Some(args) = arguments {
        push_alternate(&mut alternate_names, &name, &args);
    }
    let executable_name = target.as_ref().and_then(|t| {
        Path::new(t)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    });
    if let Some(target) = &target {
        if let Some(desc) = file_description(Path::new(target)) {
            push_alternate(&mut alternate_names, &name, &desc);
        }
    }
    Some(ResolvedApp {
        name,
        aumid,
        target,
        executable_name,
        alternate_names,
    })
}

fn pick_display_name(stem: &str, desc: Option<&str>) -> String {
    match desc.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d)
            if tinycast_pure::search_scopes::keep_alternate(stem, d)
                || d.eq_ignore_ascii_case(stem) =>
        {
            d.to_string()
        }
        _ => stem.to_string(),
    }
}

fn shortcut_target(path: &Path) -> (Option<String>, Option<String>) {
    let wide = to_wide(path.as_os_str());
    unsafe {
        let Ok(link) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER)
        else {
            return (None, None);
        };
        let Ok(persist) = Interface::cast::<IPersistFile>(&link) else {
            return (None, None);
        };
        if persist.Load(PCWSTR(wide.as_ptr()), STGM_READ).is_err() {
            return (None, None);
        }
        let mut buf = [0u16; 1024];
        let mut fd = WIN32_FIND_DATAW::default();
        let target = match link.GetPath(&mut buf, &mut fd, SLGP_RAWPATH.0 as u32) {
            Ok(()) => {
                nonempty_wide(&buf).map(|raw| canonicalize_text(Path::new(&expand_env(&raw))))
            }
            Err(_) => None,
        };
        let mut args = [0u16; 1024];
        let arguments = match link.GetArguments(&mut args) {
            Ok(()) => nonempty_wide(&args),
            Err(_) => None,
        };
        (target, arguments)
    }
}

fn shortcut_aumid(path: &Path) -> Option<String> {
    let wide = to_wide(path.as_os_str());
    unsafe {
        let store = SHGetPropertyStoreFromParsingName::<_, _, IPropertyStore>(
            PCWSTR(wide.as_ptr()),
            None,
            GPS_DEFAULT,
        )
        .ok()?;
        let value = store.GetValue(&PKEY_APPUSERMODEL_ID).ok()?;
        let text = BSTR::try_from(&value).ok()?.to_string();
        let text = text.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
}

fn scan_appx(catalog: &mut Catalog) {
    use windows::Management::Deployment::{PackageManager, PackageTypes};

    let Ok(pm) = PackageManager::new() else {
        return;
    };
    let packages = pm
        .FindPackagesByUserSecurityIdWithPackageTypes(&HSTRING::new(), PackageTypes::Main)
        .or_else(|_| pm.FindPackagesWithPackageTypes(PackageTypes::Main));
    let Ok(packages) = packages else {
        return;
    };
    for package in packages {
        if package.IsFramework().unwrap_or(true) || package.IsResourcePackage().unwrap_or(true) {
            continue;
        }
        let pkg_name = package.DisplayName().ok().map(|s| s.to_string());
        let Ok(entries) = package.GetAppListEntries() else {
            continue;
        };
        for entry in entries {
            let Ok(aumid) = entry.AppUserModelId() else {
                continue;
            };
            let aumid = aumid.to_string();
            if aumid.is_empty() {
                continue;
            }
            let name = entry
                .DisplayInfo()
                .ok()
                .and_then(|info| info.DisplayName().ok())
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty())
                .or_else(|| pkg_name.clone())
                .unwrap_or_else(|| aumid.clone());
            let mut alternate_names = Vec::new();
            if let Some(pkg) = &pkg_name {
                push_alternate(&mut alternate_names, &name, pkg);
            }
            catalog.push(ResolvedApp {
                name,
                aumid: Some(aumid),
                target: None,
                executable_name: None,
                alternate_names,
            });
        }
    }
}

fn scan_app_paths(catalog: &mut Catalog) {
    scan_app_paths_hive(catalog, HKEY_CURRENT_USER, KEY_READ);
    scan_app_paths_hive(catalog, HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY);
    scan_app_paths_hive(catalog, HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY);
}

fn scan_app_paths_hive(
    catalog: &mut Catalog,
    hive: HKEY,
    sam: windows::Win32::System::Registry::REG_SAM_FLAGS,
) {
    let subkey = to_wide(APP_PATHS);
    let mut key = HKEY::default();
    let status = unsafe { RegOpenKeyExW(hive, PCWSTR(subkey.as_ptr()), 0, sam, &mut key) };
    if status != ERROR_SUCCESS {
        return;
    }
    let mut index = 0u32;
    loop {
        let mut name_buf = [0u16; 256];
        let mut name_len = name_buf.len() as u32;
        let err = unsafe {
            RegEnumKeyExW(
                key,
                index,
                PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                PWSTR::null(),
                None,
                None,
            )
        };
        if err != ERROR_SUCCESS {
            break;
        }
        index += 1;
        let name = match nonempty_wide(&name_buf[..name_len as usize]) {
            Some(n) => n,
            None => continue,
        };
        let Some(raw) = app_paths_value(key, &name) else {
            continue;
        };
        let path = expand_env(&raw);
        if !path.is_file() {
            continue;
        }
        if let Some(mut app) = resolve_exe(&path) {
            if app.executable_name.is_none() {
                app.executable_name = Some(name);
            }
            catalog.push(app);
        }
    }
    unsafe {
        let _ = RegCloseKey(key);
    }
}

fn app_paths_value(key: HKEY, subkey: &str) -> Option<String> {
    let sub = to_wide(subkey);
    let mut buf = [0u16; 1024];
    let mut bytes = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            key,
            PCWSTR(sub.as_ptr()),
            PCWSTR::null(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    nonempty_wide(&buf)
}

fn file_description(path: &Path) -> Option<String> {
    let wide = to_wide(path.as_os_str());
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()), 0, size, block.as_mut_ptr() as *mut _).ok()?;
        let mut trans_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut trans_len = 0u32;
        let translated = VerQueryValueW(
            block.as_ptr() as *const _,
            w!("\\VarFileInfo\\Translation"),
            &mut trans_ptr,
            &mut trans_len,
        );
        let (lang, cp) = if translated.0 != 0 && !trans_ptr.is_null() && trans_len >= 4 {
            let p = trans_ptr as *const u16;
            (*p, *p.add(1))
        } else {
            (0x0409, 0x04b0)
        };
        let query = format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription");
        let qwide = to_wide(&query);
        let mut value_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len = 0u32;
        let ok = VerQueryValueW(
            block.as_ptr() as *const _,
            PCWSTR(qwide.as_ptr()),
            &mut value_ptr,
            &mut value_len,
        );
        if ok.0 == 0 || value_ptr.is_null() || value_len < 2 {
            return None;
        }
        let chars = std::slice::from_raw_parts(value_ptr as *const u16, (value_len as usize) / 2);
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        let text = String::from_utf16_lossy(&chars[..end]);
        let text = text.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
}

fn canonicalize_text(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string()
}

fn ensure_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}

fn to_wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn nonempty_wide(buf: &[u16]) -> Option<String> {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let text = String::from_utf16_lossy(&buf[..end]);
    let text = text.trim().trim_matches('"');
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, aumid: Option<&str>, target: Option<&str>) -> ResolvedApp {
        ResolvedApp {
            name: name.into(),
            aumid: aumid.map(str::to_string),
            target: target.map(str::to_string),
            executable_name: target.and_then(|t| {
                Path::new(t)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            }),
            alternate_names: Vec::new(),
        }
    }

    #[test]
    fn overlapping_scans_keep_latest() {
        let mut index = AppIndex::new();
        let first = index.begin_scan();
        let second = index.begin_scan();
        index.publish(
            first,
            vec![application_entry(&app("Old", None, Some(r"C:\old.exe")))],
        );
        index.publish(
            second,
            vec![application_entry(&app("New", None, Some(r"C:\new.exe")))],
        );
        let latest = index.take_latest().expect("latest");
        assert_eq!(latest[0].name, "New");
        assert!(index.take_latest().is_none());
    }

    #[test]
    fn stale_scan_is_dropped() {
        let mut index = AppIndex::new();
        let first = index.begin_scan();
        let _second = index.begin_scan();
        index.publish(
            first,
            vec![application_entry(&app("Old", None, Some(r"C:\old.exe")))],
        );
        assert!(index.take_latest().is_none());
    }

    #[test]
    fn fold_dedups_by_aumid_and_path() {
        let entries = fold_apps(vec![
            app(
                "Notepad",
                Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"),
                None,
            ),
            ResolvedApp {
                name: "Windows Notepad".into(),
                aumid: Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".into()),
                target: None,
                executable_name: None,
                alternate_names: vec!["app".into(), "Notepad".into()],
            },
            app("Notepad", None, Some(r"C:\Windows\System32\notepad.exe")),
            app("Notepad", None, Some(r"c:/windows/system32/notepad.exe")),
        ]);
        let apps: Vec<_> = entries
            .iter()
            .filter(|e| e.kind == AppKind::Application)
            .collect();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].id, "app:Microsoft.WindowsNotepad_8wekyb3d8bbwe!App");
        assert!(apps[0]
            .fields
            .alternate_names
            .iter()
            .any(|n| n == "Windows Notepad"));
        assert!(!apps[0]
            .fields
            .alternate_names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("app")));
        assert!(entries
            .iter()
            .any(|e| e.kind == AppKind::SystemSettings && e.name == "Display"));
        assert!(entries.iter().any(|e| e.id == "app:ms-settings:display"));
    }

    #[test]
    fn application_id_uses_app_prefix() {
        let entry = application_entry(&app("Calc", None, Some(r"C:\Windows\System32\calc.exe")));
        assert!(entry.id.starts_with("app:"));
        assert_eq!(entry.kind, AppKind::Application);
        assert_eq!(entry.fields.executable_name.as_deref(), Some("calc.exe"));
    }

    #[test]
    fn scan_catalog_includes_settings_and_an_application() {
        let entries = std::thread::spawn(scan_catalog)
            .join()
            .expect("scan thread");
        assert!(entries
            .iter()
            .any(|e| e.kind == AppKind::SystemSettings && e.name == "Display"));
        assert!(entries.iter().any(|e| e.kind == AppKind::Application));
    }
}
