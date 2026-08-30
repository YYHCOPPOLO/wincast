//! Clipboard capture via AddClipboardFormatListener.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::DataExchange::AddClipboardFormatListener;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, PostMessageW,
};

use super::store::{new_id, ClipboardStore};
use crate::app_settings::AppSettings;
use crate::platform::clipboard::{self, ImageBytes};
use crate::platform::messages::WM_CLIPBOARD_IMAGE;

pub const DEFAULT_DISABLED_APPS: &[&str] =
    &["KeePass", "1Password", "Bitwarden", "LastPass", "Dashlane"];

const MAX_TEXT: usize = 32_000;

static PENDING_IMAGES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub fn listen(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    clipboard::set_owner(hwnd);
    unsafe {
        let _ = AddClipboardFormatListener(hwnd);
    }
}

pub fn capture(store: &mut ClipboardStore, settings: &AppSettings, hwnd: HWND) {
    if clipboard::has_internal_marker() {
        return;
    }
    if source_is_disabled(settings) {
        return;
    }
    if let Some(text) = clipboard::read_unicode_text() {
        let text = text.trim_end_matches('\u{0}').to_string();
        if !text.is_empty() && text.len() <= MAX_TEXT {
            store.insert_text(text);
            return;
        }
    }
    let Some(bytes) = clipboard::read_image_bytes() else {
        return;
    };
    queue_image(hwnd, bytes, store.images_dir());
}

pub fn take_pending_images() -> Vec<PathBuf> {
    PENDING_IMAGES
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}

pub fn is_disabled_app(stem: &str, disabled: &[String]) -> bool {
    disabled.iter().any(|name| name.eq_ignore_ascii_case(stem))
}

fn queue_image(hwnd: HWND, bytes: ImageBytes, dir: PathBuf) {
    if hwnd.is_invalid() {
        return;
    }
    let bits = hwnd.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-clip-img".into())
        .spawn(move || {
            let Some(png) = clipboard::encode_image_png(bytes) else {
                return;
            };
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(format!("{}.png", new_id()));
            if std::fs::write(&path, png).is_err() {
                return;
            }
            if let Ok(mut pending) = PENDING_IMAGES.lock() {
                pending.push(path);
            }
            let hwnd = HWND(bits as *mut core::ffi::c_void);
            unsafe {
                let _ = PostMessageW(hwnd, WM_CLIPBOARD_IMAGE, WPARAM(0), LPARAM(0));
            }
        });
}

fn source_is_disabled(settings: &AppSettings) -> bool {
    let stem = foreground_exe_stem().unwrap_or_default();
    if stem.is_empty() {
        return false;
    }
    is_disabled_app(&stem, &settings.clipboard_disabled_apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_password_managers_are_disabled() {
        let disabled: Vec<String> = DEFAULT_DISABLED_APPS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(is_disabled_app("KeePass", &disabled));
        assert!(is_disabled_app("bitwarden", &disabled));
        assert!(!is_disabled_app("notepad", &disabled));
    }
}

fn foreground_exe_stem() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            Default::default(),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(process);
        if ok.is_err() || len == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }
}
