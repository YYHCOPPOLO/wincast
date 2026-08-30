use tinycast_pure::app_entry::{AppEntry, AppKind};
use tinycast_pure::launcher_ranking::{should_record_ranking, LauncherRankingStore};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER};
use windows::Win32::UI::Shell::{
    IApplicationActivationManager, ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC,
    SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// What activating a row should do. View/AppCore execute HWND outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchSpec {
    Aumid(String),
    Path(String),
    Uri(String),
    Quit,
    OpenSettings,
    OpenAbout,
    OpenSupport,
    OpenCalculatorHistory,
    OpenClipboardHistory,
    ExpandSnippet(String),
    RunCustomCommand(String),
    OpenQuicklink(String),
    SearchQuicklinks,
    Noop,
}

pub fn launch_spec(entry: &AppEntry) -> LaunchSpec {
    match entry.kind {
        AppKind::Application => {
            if let Some(aumid) = entry
                .fields
                .bundle_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                LaunchSpec::Aumid(aumid.to_string())
            } else {
                let path = entry
                    .id
                    .strip_prefix("app:")
                    .unwrap_or(entry.id.as_str())
                    .trim()
                    .trim_matches('"');
                LaunchSpec::Path(path.to_string())
            }
        }
        AppKind::SystemSettings => {
            let uri = entry
                .fields
                .bundle_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    entry
                        .id
                        .strip_prefix("app:")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or_else(|| entry.id.clone());
            LaunchSpec::Uri(uri)
        }
        AppKind::Command => match entry.id.as_str() {
            "command:quit" => LaunchSpec::Quit,
            "command:settings" => LaunchSpec::OpenSettings,
            "command:about" => LaunchSpec::OpenAbout,
            "command:support" => LaunchSpec::OpenSupport,
            "command:calculator-history" => LaunchSpec::OpenCalculatorHistory,
            "command:clipboard-history" => LaunchSpec::OpenClipboardHistory,
            "command:search-quicklinks" => LaunchSpec::SearchQuicklinks,
            _ => LaunchSpec::Noop,
        },
        AppKind::Snippet => LaunchSpec::ExpandSnippet(entry.id.clone()),
        AppKind::CustomCommand => LaunchSpec::RunCustomCommand(entry.id.clone()),
        AppKind::Quicklink => LaunchSpec::OpenQuicklink(entry.id.clone()),
        _ => LaunchSpec::Noop,
    }
}

pub fn icon_source(entry: &AppEntry) -> Option<String> {
    match entry.kind {
        AppKind::Application => match launch_spec(entry) {
            LaunchSpec::Aumid(aumid) => Some(format!("shell:AppsFolder\\{aumid}")),
            LaunchSpec::Path(path) if !path.is_empty() => Some(path),
            _ => None,
        },
        AppKind::SystemSettings => {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
            Some(format!("{root}\\ImmersiveControlPanel\\SystemSettings.exe"))
        }
        _ => None,
    }
}

pub fn record_if_needed(
    ranking: &mut LauncherRankingStore,
    query: &str,
    entry_id: &str,
    now: i64,
    category_listing: bool,
) {
    if should_record_ranking(query, category_listing) {
        ranking.record(query, entry_id, now);
        let _ = ranking.save();
    }
}

pub fn copy_path_text(entry: &AppEntry) -> Option<String> {
    match launch_spec(entry) {
        LaunchSpec::Path(path) if !path.is_empty() => Some(path),
        LaunchSpec::Aumid(aumid) => Some(format!("shell:AppsFolder\\{aumid}")),
        LaunchSpec::Uri(uri) if !uri.is_empty() => Some(uri),
        _ => None,
    }
}

pub fn reveal_path(entry: &AppEntry) -> Option<String> {
    match launch_spec(entry) {
        LaunchSpec::Path(path) if !path.is_empty() => Some(path),
        LaunchSpec::Aumid(aumid) => Some(format!("shell:AppsFolder\\{aumid}")),
        _ => None,
    }
}

pub fn copy_text(text: &str) -> windows::core::Result<()> {
    crate::platform::clipboard::write_text_marked(text)
}

pub fn show_in_folder(path: &str) -> windows::core::Result<()> {
    let file: Vec<u16> = "explorer.exe"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let args: Vec<u16> = format!("/select,\"{path}\"")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_SHOWNORMAL.0 as i32,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }
}

pub fn execute(spec: &LaunchSpec) -> windows::core::Result<()> {
    match spec {
        LaunchSpec::Aumid(aumid) => activate_aumid(aumid)
            .or_else(|_| shell_open(&format!("shell:AppsFolder\\{aumid}"), HWND::default())),
        LaunchSpec::Path(path) => shell_open(path, HWND::default()),
        LaunchSpec::Uri(uri) => shell_open(uri, HWND::default()),
        LaunchSpec::Quit
        | LaunchSpec::OpenSettings
        | LaunchSpec::OpenAbout
        | LaunchSpec::OpenSupport
        | LaunchSpec::OpenCalculatorHistory
        | LaunchSpec::OpenClipboardHistory
        | LaunchSpec::ExpandSnippet(_)
        | LaunchSpec::RunCustomCommand(_)
        | LaunchSpec::OpenQuicklink(_)
        | LaunchSpec::SearchQuicklinks
        | LaunchSpec::Noop => Ok(()),
    }
}

fn activate_aumid(aumid: &str) -> windows::core::Result<()> {
    unsafe {
        let manager = CoCreateInstance::<_, IApplicationActivationManager>(
            &windows::Win32::UI::Shell::ApplicationActivationManager,
            None,
            CLSCTX_LOCAL_SERVER,
        )
        .or_else(|_| {
            CoCreateInstance::<_, IApplicationActivationManager>(
                &windows::Win32::UI::Shell::ApplicationActivationManager,
                None,
                CLSCTX_INPROC_SERVER,
            )
        })?;
        let _pid = manager.ActivateApplication(
            &HSTRING::from(aumid),
            &HSTRING::new(),
            windows::Win32::UI::Shell::ACTIVATEOPTIONS(0),
        )?;
        Ok(())
    }
}

fn shell_open(file: &str, hwnd: HWND) -> windows::core::Result<()> {
    let wide: Vec<u16> = file.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        hwnd,
        lpFile: PCWSTR(wide.as_ptr()),
        nShow: SW_SHOWNORMAL.0 as i32,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }
}

#[cfg(test)]
mod tests {
    use super::{
        copy_path_text, icon_source, launch_spec, record_if_needed, reveal_path, LaunchSpec,
    };
    use tinycast_pure::app_entry::{AppEntry, AppKind};
    use tinycast_pure::command_id::CommandID;
    use tinycast_pure::launcher_ranking::{should_record_ranking, LauncherRankingStore};
    use tinycast_pure::search_relevance::SearchFields;

    fn app(id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            kind: AppKind::Application,
            name: name.into(),
            fields: SearchFields {
                display_name: name.into(),
                ..Default::default()
            },
            hotkey: None,
        }
    }

    #[test]
    fn system_settings_launch_uses_ms_settings_uri() {
        let entry = AppEntry {
            id: "app:ms-settings:display".into(),
            kind: AppKind::SystemSettings,
            name: "Display".into(),
            fields: SearchFields {
                display_name: "Display".into(),
                bundle_id: Some("ms-settings:display".into()),
                ..Default::default()
            },
            hotkey: None,
        };
        assert_eq!(
            launch_spec(&entry),
            LaunchSpec::Uri("ms-settings:display".into())
        );
    }

    #[test]
    fn application_prefers_aumid_then_path_from_id() {
        let mut aumid = app("app:C:\\Windows\\notepad.exe", "Notepad");
        aumid.fields.bundle_id = Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".into());
        assert_eq!(
            launch_spec(&aumid),
            LaunchSpec::Aumid("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".into())
        );
        assert_eq!(
            icon_source(&aumid).as_deref(),
            Some(r"shell:AppsFolder\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App")
        );

        let path = app(r"app:C:\Windows\System32\notepad.exe", "Notepad");
        assert_eq!(
            launch_spec(&path),
            LaunchSpec::Path(r"C:\Windows\System32\notepad.exe".into())
        );
        assert_eq!(
            copy_path_text(&path).as_deref(),
            Some(r"C:\Windows\System32\notepad.exe")
        );
        assert_eq!(
            reveal_path(&path).as_deref(),
            Some(r"C:\Windows\System32\notepad.exe")
        );
        assert_eq!(
            copy_path_text(&aumid).as_deref(),
            Some(r"shell:AppsFolder\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App")
        );
        assert_eq!(
            reveal_path(&aumid).as_deref(),
            Some(r"shell:AppsFolder\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App")
        );
    }

    #[test]
    fn aumid_show_in_folder_uses_apps_folder_shell_path() {
        let mut aumid = app("app:C:\\Windows\\notepad.exe", "Notepad");
        aumid.fields.bundle_id = Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".into());
        let revealed = reveal_path(&aumid).expect("aumid apps are revealable");
        assert_eq!(
            revealed,
            r"shell:AppsFolder\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"
        );
        assert_eq!(copy_path_text(&aumid).as_deref(), Some(revealed.as_str()));
        assert_eq!(icon_source(&aumid).as_deref(), Some(revealed.as_str()));
        assert_eq!(reveal_path(&CommandID::Settings.as_entry()), None);
        let settings = AppEntry {
            id: "app:ms-settings:display".into(),
            kind: AppKind::SystemSettings,
            name: "Display".into(),
            fields: SearchFields {
                display_name: "Display".into(),
                bundle_id: Some("ms-settings:display".into()),
                ..Default::default()
            },
            hotkey: None,
        };
        assert_eq!(reveal_path(&settings), None);
    }

    #[test]
    fn commands_map_to_quit_settings_about_support_or_noop() {
        assert_eq!(launch_spec(&CommandID::Quit.as_entry()), LaunchSpec::Quit);
        assert_eq!(
            launch_spec(&CommandID::Settings.as_entry()),
            LaunchSpec::OpenSettings
        );
        assert_eq!(
            launch_spec(&CommandID::About.as_entry()),
            LaunchSpec::OpenAbout
        );
        assert_eq!(
            launch_spec(&CommandID::Support.as_entry()),
            LaunchSpec::OpenSupport
        );
        assert_eq!(launch_spec(&CommandID::AiChat.as_entry()), LaunchSpec::Noop);
        assert_eq!(
            launch_spec(&CommandID::ClipboardHistory.as_entry()),
            LaunchSpec::OpenClipboardHistory
        );
        assert_eq!(
            launch_spec(&CommandID::CalculatorHistory.as_entry()),
            LaunchSpec::OpenCalculatorHistory
        );
    }

    #[test]
    fn record_if_needed_follows_should_record_ranking() {
        let path = std::env::temp_dir().join(format!(
            "tc-launch-rank-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);
        let mut ranking = LauncherRankingStore::load(path.clone());
        record_if_needed(&mut ranking, "", "app:x", 1_000, false);
        assert_eq!(ranking.boost("w", "app:x", 1_000), 0);
        record_if_needed(&mut ranking, "Applications", "app:x", 1_000, true);
        assert_eq!(ranking.boost("applications", "app:x", 1_000), 0);
        assert!(!should_record_ranking("Applications", true));
        record_if_needed(&mut ranking, "not", "app:x", 1_000, false);
        assert!(ranking.boost("not", "app:x", 1_000) > 0);
        let _ = std::fs::remove_file(&path);
    }
}
