use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use tinycast_pure::custom_command::uses_cmd_shell;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE,
    PROCESS_INFORMATION, STARTUPINFOW,
};

pub fn run(command: &str) -> Result<u32, String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("Enter a command to run.".into());
    }
    let mut cmdline: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
    let app_wide = if uses_cmd_shell(command) {
        None
    } else {
        first_token(command).map(|exe| wide(&exe))
    };
    let dir = std::env::var_os("USERPROFILE").map(|p| wide_os(PathBuf::from(p)));
    let app_ptr = app_wide
        .as_ref()
        .map(|a| PCWSTR(a.as_ptr()))
        .unwrap_or(PCWSTR::null());
    let dir_ptr = dir
        .as_ref()
        .map(|d| PCWSTR(d.as_ptr()))
        .unwrap_or(PCWSTR::null());
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            app_ptr,
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

fn first_token(command: &str) -> Option<String> {
    let t = command.trim();
    if let Some(rest) = t.strip_prefix('"') {
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        t.split_whitespace().next().map(str::to_string)
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
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
    fn first_token_handles_quotes() {
        assert_eq!(
            first_token(r#""C:\Program Files\app.exe" -a"#).as_deref(),
            Some(r"C:\Program Files\app.exe")
        );
        assert_eq!(
            first_token("notepad.exe file.txt").as_deref(),
            Some("notepad.exe")
        );
    }
}
