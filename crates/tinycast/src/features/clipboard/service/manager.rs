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

use super::store::ClipboardStore;
use crate::app_settings::AppSettings;
use crate::platform::clipboard::{self, ImageBytes};
use crate::platform::messages::WM_CLIPBOARD_IMAGE;

pub const DEFAULT_DISABLED_APPS: &[&str] =
    &["KeePass", "1Password", "Bitwarden", "LastPass", "Dashlane"];

const MAX_TEXT: usize = 32_000;

struct PendingImage {
    dir: PathBuf,
    png: Vec<u8>,
}

static PENDING_IMAGES: Mutex<Vec<PendingImage>> = Mutex::new(Vec::new());

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
    capture_if_available(store, |store| capture_available(store, settings, hwnd));
}

fn capture_if_available(store: &mut ClipboardStore, capture: impl FnOnce(&mut ClipboardStore)) {
    if store.is_available() {
        capture(store);
    }
}

fn capture_available(store: &mut ClipboardStore, settings: &AppSettings, hwnd: HWND) {
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

pub fn install_pending_images(store: &mut ClipboardStore) -> bool {
    PENDING_IMAGES
        .lock()
        .map(|mut pending| install_images(store, &mut pending))
        .unwrap_or(false)
}

fn install_images(store: &mut ClipboardStore, pending: &mut Vec<PendingImage>) -> bool {
    if !store.is_available() {
        return false;
    }
    let dir = store.images_dir();
    let mut inserted = false;
    pending.retain(|image| {
        if image.dir != dir || !store.is_available() {
            return true;
        }
        if store.save_image(&image.png).is_some() {
            inserted = true;
            false
        } else {
            // Keep the encoded capture for a retry; no orphan PNG or silently
            // consumed queue entry after a failed database/file operation.
            true
        }
    });
    inserted
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
            if let Ok(mut pending) = PENDING_IMAGES.lock() {
                pending.push(PendingImage { dir, png });
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
    use super::super::store::new_id;
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("tinycast-clip-capture-{}", new_id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                if !std::thread::panicking() {
                    panic!("fixture cleanup failed: {error}");
                }
            }
        }
    }

    #[test]
    fn unavailable_store_never_enters_clipboard_capture() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("clipboard.sqlite3"), b"corrupt fixture").unwrap();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let mut entered = false;
        // Never touch the live clipboard, even for the failing baseline.
        capture_if_available(&mut store, |_| entered = true);
        assert!(
            !entered,
            "unavailable storage must stop before clipboard/UI access"
        );
        assert!(!fixture.0.join("images").exists());
    }

    #[test]
    fn unavailable_pending_images_are_retained_without_disk_writes() {
        let fixture = Fixture::new();
        let path = fixture.0.join("clipboard.sqlite3");
        std::fs::write(&path, b"corrupt fixture").unwrap();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let mut pending = vec![PendingImage {
            dir: store.images_dir(),
            png: b"pending image".to_vec(),
        }];
        assert!(!install_images(&mut store, &mut pending));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].png, b"pending image");
        assert!(!store.images_dir().exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt fixture");
    }

    #[test]
    fn failed_pending_insert_keeps_png_for_retry() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let conn = rusqlite::Connection::open(fixture.0.join("clipboard.sqlite3")).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON items BEGIN SELECT RAISE(FAIL, 'injected insert failure'); END;").unwrap();
        let mut pending = vec![PendingImage {
            dir: store.images_dir(),
            png: b"pending image".to_vec(),
        }];
        assert!(!install_images(&mut store, &mut pending));
        assert_eq!(pending.len(), 1);
        assert!(!store.is_available());
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 0);
        // Simulate another encoder completing after the first storage failure.
        pending.push(PendingImage {
            dir: store.images_dir(),
            png: b"later capture".to_vec(),
        });
        assert!(!install_images(&mut store, &mut pending));
        assert_eq!(pending.len(), 2);
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 0);
        conn.execute_batch("DROP TRIGGER reject_insert").unwrap();
        store.clear(); // successful explicit operation recovers transient failure
        assert!(install_images(&mut store, &mut pending));
        assert!(pending.is_empty());
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 2);
    }

    #[test]
    fn installs_only_images_for_the_matching_store() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let other_dir = fixture.0.join("other-store");
        let mut pending = vec![
            PendingImage {
                dir: other_dir.clone(),
                png: b"other image".to_vec(),
            },
            PendingImage {
                dir: store.images_dir(),
                png: b"owned image".to_vec(),
            },
        ];
        assert!(install_images(&mut store, &mut pending));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].dir, other_dir);
        assert!(!other_dir.exists());
        let rows = store.search("", super::super::store::ClipboardFilter::Images);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            std::fs::read(rows[0].image_path.as_ref().unwrap()).unwrap(),
            b"owned image"
        );
    }

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
