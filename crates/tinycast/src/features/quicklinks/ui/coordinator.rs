use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use tinycast_pure::quicklink::{detect_kind, DestinationKind, Quicklink};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn open_destination(expanded: &str) -> Result<(), String> {
    let mut target = expanded.trim().to_string();
    if detect_kind(&target) == Some(DestinationKind::Web) && !target.contains(':') {
        target = format!("https://{target}");
    }
    if detect_kind(&target) == Some(DestinationKind::Path) {
        let path = expand_tilde(&target);
        if !PathBuf::from(&path).exists() {
            return Err(format!("The path no longer exists. ({path})"));
        }
        return shell_open(&path);
    }
    shell_open(&target)
}

fn expand_tilde(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        format!("{home}\\{rest}")
    } else if let Some(rest) = raw.strip_prefix("file://") {
        rest.trim_start_matches('/').replace('/', "\\")
    } else {
        raw.to_string()
    }
}

fn shell_open(target: &str) -> Result<(), String> {
    let file: Vec<u16> = std::ffi::OsStr::new(target)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0 as i32,
        hwnd: HWND::default(),
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn uses_selection_token(link: &Quicklink) -> bool {
    tinycast_pure::template::uses_selection(&link.destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expands_against_home() {
        std::env::set_var("USERPROFILE", r"C:\Users\test");
        assert_eq!(expand_tilde("~/Notes/a.md"), r"C:\Users\test\Notes/a.md");
    }
}
