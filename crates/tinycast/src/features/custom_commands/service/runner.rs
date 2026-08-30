use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Mutex;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE,
    PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::platform::messages::WM_CUSTOM_COMMAND_FAILED;

static PENDING_ERROR: Mutex<Option<String>> = Mutex::new(None);

pub fn take_pending_error() -> Option<String> {
    PENDING_ERROR.lock().ok()?.take()
}

/// Always `NULL` so `CreateProcessW` searches `PATH` (e.g. `notepad.exe`).
pub fn application_name_is_null() -> bool {
    true
}

pub fn format_command_line(command: &str) -> String {
    let t = command.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.starts_with('"') {
        return t.to_string();
    }
    let mut chars = t.chars();
    let first = chars.next().unwrap_or(' ');
    if first == '"' {
        return t.to_string();
    }
    if let Some(space) = t.find(char::is_whitespace) {
        let argv0 = &t[..space];
        let rest = t[space..].trim_start();
        if argv0.contains('\\') || argv0.contains('/') {
            if argv0.contains(' ') {
                if rest.is_empty() {
                    format!("\"{argv0}\"")
                } else {
                    format!("\"{argv0}\" {rest}")
                }
            } else if rest.is_empty() {
                t.to_string()
            } else {
                t.to_string()
            }
        } else {
            t.to_string()
        }
    } else {
        t.to_string()
    }
}

pub fn start(command: &str, host: HWND) {
    let command = command.to_string();
    let bits = host.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-cmd".into())
        .spawn(move || {
            let result = run_blocking(&command);
            if let Err(err) = result {
                if let Ok(mut slot) = PENDING_ERROR.lock() {
                    *slot = Some(err);
                }
                let hwnd = HWND(bits as *mut core::ffi::c_void);
                if !hwnd.is_invalid() {
                    unsafe {
                        let _ = PostMessageW(hwnd, WM_CUSTOM_COMMAND_FAILED, WPARAM(0), LPARAM(0));
                    }
                }
            }
        });
}

pub fn run_blocking(command: &str) -> Result<u32, String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("Enter a command to run.".into());
    }
    let formatted = format_command_line(command);
    let mut cmdline: Vec<u16> = formatted.encode_utf16().chain(std::iter::once(0)).collect();
    let dir = std::env::var_os("USERPROFILE").map(|p| wide_os(PathBuf::from(p)));
    let dir_ptr = dir
        .as_ref()
        .map(|d| windows::core::PCWSTR(d.as_ptr()))
        .unwrap_or(windows::core::PCWSTR::null());
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            None,
            PWSTR(cmdline.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            dir_ptr,
            &si,
            &mut pi,
        )
        .map_err(|e| e.to_string())?;
        let _ = CloseHandle(pi.hThread);
        let _ = WaitForSingleObject(pi.hProcess, INFINITE);
        let mut code = 0u32;
        let _ = GetExitCodeProcess(pi.hProcess, &mut code);
        let _ = CloseHandle(pi.hProcess);
        if code == 0 {
            Ok(0)
        } else {
            Err(format!("Command exited with status {code}"))
        }
    }
}

fn wide_os(path: PathBuf) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_process_uses_null_application_name() {
        assert!(application_name_is_null());
    }

    #[test]
    fn command_line_leaves_path_lookup_names_unquoted() {
        assert_eq!(format_command_line("notepad.exe"), "notepad.exe");
        assert_eq!(
            format_command_line("notepad.exe file.txt"),
            "notepad.exe file.txt"
        );
        assert_eq!(
            format_command_line(r#""C:\Program Files\app.exe" -a"#),
            r#""C:\Program Files\app.exe" -a"#
        );
    }
}
