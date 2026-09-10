//! Clipboard capture via AddClipboardFormatListener.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

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

struct ImageCapture {
    dir: PathBuf,
    generation: Arc<()>,
}

struct PendingImage {
    capture: ImageCapture,
    png: Vec<u8>,
}

#[derive(Default)]
struct ImageQueue {
    pending: Vec<PendingImage>,
    generations: HashMap<PathBuf, Arc<()>>,
}

impl ImageQueue {
    fn begin_capture(&mut self, dir: PathBuf) -> ImageCapture {
        let generation = self.generations.entry(dir.clone()).or_default().clone();
        ImageCapture { dir, generation }
    }

    fn complete(&mut self, capture: ImageCapture, png: Vec<u8>) -> bool {
        if !self
            .generations
            .get(&capture.dir)
            .is_some_and(|generation| Arc::ptr_eq(generation, &capture.generation))
        {
            return false;
        }
        self.pending.push(PendingImage { capture, png });
        true
    }

    fn clear_history(&mut self, store: &mut ClipboardStore) -> bool {
        if !store.clear() {
            return false;
        }
        let dir = store.images_dir();
        // An outstanding worker owns the old identity, so it cannot become valid
        // again when a later capture creates a new generation for this directory.
        self.generations.remove(&dir);
        self.pending.retain(|image| image.capture.dir != dir);
        true
    }

    fn install(&mut self, store: &mut ClipboardStore) -> bool {
        if !store.is_available() {
            return false;
        }
        let dir = store.images_dir();
        let mut inserted = false;
        self.pending.retain(|image| {
            if image.capture.dir != dir || !store.is_available() {
                return true;
            }
            if store.save_image(&image.png).is_some() {
                inserted = true;
                false
            } else {
                // Ordinary failures retain the capture for retry. Only a committed
                // explicit clear invalidates its generation and discards it.
                true
            }
        });
        inserted
    }
}

static PENDING_IMAGES: LazyLock<Mutex<ImageQueue>> =
    LazyLock::new(|| Mutex::new(ImageQueue::default()));

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
    // Stamp before clipboard reads/encoding, never when the worker completes.
    let Some(capture) = PENDING_IMAGES
        .lock()
        .ok()
        .map(|mut queue| queue.begin_capture(store.images_dir()))
    else {
        return;
    };
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
    queue_image(hwnd, bytes, capture);
}

pub fn install_pending_images(store: &mut ClipboardStore) -> bool {
    PENDING_IMAGES
        .lock()
        .map(|mut queue| queue.install(store))
        .unwrap_or(false)
}

pub fn clear_history(store: &mut ClipboardStore) -> bool {
    // Serialize the committed clear and invalidation with worker completion.
    PENDING_IMAGES
        .lock()
        .map(|mut queue| queue.clear_history(store))
        .unwrap_or(false)
}

pub fn is_disabled_app(stem: &str, disabled: &[String]) -> bool {
    disabled.iter().any(|name| name.eq_ignore_ascii_case(stem))
}

fn queue_image(hwnd: HWND, bytes: ImageBytes, capture: ImageCapture) {
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
            let queued = PENDING_IMAGES
                .lock()
                .map(|mut queue| queue.complete(capture, png))
                .unwrap_or(false);
            if !queued {
                return;
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
    use super::super::store::{new_id, ClipboardFilter};
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

    fn enqueue_image(queue: &mut ImageQueue, dir: PathBuf, png: &[u8]) {
        let capture = queue.begin_capture(dir);
        queue.complete(capture, png.to_vec());
    }

    fn saved_images(store: &ClipboardStore) -> Vec<Vec<u8>> {
        let mut images: Vec<_> = store
            .search("", ClipboardFilter::Images)
            .into_iter()
            .map(|item| std::fs::read(item.image_path.unwrap()).unwrap())
            .collect();
        images.sort();
        images
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
        let mut queue = ImageQueue::default();
        enqueue_image(&mut queue, store.images_dir(), b"pending image");
        assert!(!queue.install(&mut store));
        assert!(!queue.clear_history(&mut store));
        assert_eq!(queue.pending.len(), 1);
        assert_eq!(queue.pending[0].png, b"pending image");
        assert!(!store.images_dir().exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt fixture");
    }

    #[test]
    fn failed_pending_insert_keeps_png_for_retry() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let conn = rusqlite::Connection::open(fixture.0.join("clipboard.sqlite3")).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON items BEGIN SELECT RAISE(FAIL, 'injected insert failure'); END;").unwrap();
        let mut queue = ImageQueue::default();
        enqueue_image(&mut queue, store.images_dir(), b"pending image");
        let late = queue.begin_capture(store.images_dir());
        assert!(!queue.install(&mut store));
        assert_eq!(queue.pending.len(), 1);
        assert!(!store.is_available());
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 0);
        // Simulate another encoder completing after the first storage failure.
        assert!(queue.complete(late, b"later capture".to_vec()));
        assert!(!queue.install(&mut store));
        assert_eq!(queue.pending.len(), 2);
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 0);
        conn.execute_batch("DROP TRIGGER reject_insert").unwrap();
        // Recover without a clear: ordinary failures must still be retryable.
        store.prune_unpinned_older_than(i64::MAX);
        assert!(queue.install(&mut store));
        assert!(queue.pending.is_empty());
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 2);
    }

    #[test]
    fn successful_clear_discards_failed_capture_before_next_image() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let conn = rusqlite::Connection::open(fixture.0.join("clipboard.sqlite3")).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON items BEGIN SELECT RAISE(FAIL, 'injected insert failure'); END;").unwrap();
        let mut queue = ImageQueue::default();
        enqueue_image(&mut queue, store.images_dir(), b"pre-clear failed capture");
        assert!(!queue.install(&mut store));
        assert!(!store.is_available());
        conn.execute_batch("DROP TRIGGER reject_insert").unwrap();

        assert!(queue.clear_history(&mut store));
        assert!(store.is_available());
        assert!(queue.pending.is_empty());
        enqueue_image(&mut queue, store.images_dir(), b"post-clear capture");
        assert!(queue.install(&mut store));
        assert_eq!(saved_images(&store), vec![b"post-clear capture".to_vec()]);
        assert!(queue.pending.is_empty());
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 1);
    }

    #[test]
    fn successful_clear_discards_late_worker_but_accepts_post_clear_capture() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        // The worker starts before clear, but its simulated encoding finishes later.
        let mut queue = ImageQueue::default();
        let late = queue.begin_capture(store.images_dir());
        // Even a successful clear of an empty database is a boundary.
        assert!(queue.clear_history(&mut store));
        enqueue_image(&mut queue, store.images_dir(), b"post-clear capture");
        assert!(!queue.complete(late, b"late pre-clear capture".to_vec()));
        assert!(queue.install(&mut store));
        assert_eq!(saved_images(&store), vec![b"post-clear capture".to_vec()]);
        assert!(queue.pending.is_empty());
    }

    #[test]
    fn successful_clear_invalidates_only_its_store() {
        let first_fixture = Fixture::new();
        let second_fixture = Fixture::new();
        let mut first = ClipboardStore::open(first_fixture.0.clone());
        let mut second = ClipboardStore::open(second_fixture.0.clone());
        let mut queue = ImageQueue::default();
        let late_second = queue.begin_capture(second.images_dir());
        enqueue_image(&mut queue, first.images_dir(), b"cleared capture");
        enqueue_image(&mut queue, second.images_dir(), b"other queued capture");
        assert!(queue.clear_history(&mut first));
        enqueue_image(&mut queue, first.images_dir(), b"new first capture");
        assert!(queue.complete(late_second, b"other late capture".to_vec()));
        assert!(queue.install(&mut first));
        assert!(queue.install(&mut second));
        assert_eq!(saved_images(&first), vec![b"new first capture".to_vec()]);
        assert_eq!(
            saved_images(&second),
            vec![
                b"other late capture".to_vec(),
                b"other queued capture".to_vec()
            ]
        );
        assert!(queue.pending.is_empty());
    }

    #[test]
    fn failed_clear_preserves_rows_files_and_pending_captures() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        store.save_image(b"existing image").unwrap();
        let before = store.search("", ClipboardFilter::All);
        let mut queue = ImageQueue::default();
        let late = queue.begin_capture(store.images_dir());
        enqueue_image(&mut queue, store.images_dir(), b"queued capture");
        let conn = rusqlite::Connection::open(fixture.0.join("clipboard.sqlite3")).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON items BEGIN SELECT RAISE(FAIL, 'injected clear failure'); END;").unwrap();

        assert!(!queue.clear_history(&mut store));
        assert!(!store.is_available());
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_eq!(saved_images(&store), vec![b"existing image".to_vec()]);
        assert!(queue.complete(late, b"late capture".to_vec()));
        assert!(!queue.install(&mut store));
        assert_eq!(queue.pending.len(), 2);
        conn.execute_batch("DROP TRIGGER reject_delete").unwrap();
        store.prune_unpinned_older_than(i64::MAX);
        assert!(queue.install(&mut store));
        assert_eq!(
            saved_images(&store),
            vec![
                b"existing image".to_vec(),
                b"late capture".to_vec(),
                b"queued capture".to_vec()
            ]
        );
        assert!(queue.pending.is_empty());
    }

    #[test]
    fn app_core_clear_invalidates_pending_captures() {
        let fixture = Fixture::new();
        let mut core = crate::app_core::AppCore::new();
        core.clipboard = ClipboardStore::open(fixture.0.clone());
        struct PendingCleanup(PathBuf);
        impl Drop for PendingCleanup {
            fn drop(&mut self) {
                if let Ok(mut queue) = PENDING_IMAGES.lock() {
                    queue.pending.retain(|image| image.capture.dir != self.0);
                    queue.generations.remove(&self.0);
                }
            }
        }
        let _cleanup = PendingCleanup(core.clipboard.images_dir());
        let late = {
            let mut queue = PENDING_IMAGES.lock().unwrap();
            enqueue_image(
                &mut queue,
                core.clipboard.images_dir(),
                b"pre-clear capture",
            );
            queue.begin_capture(core.clipboard.images_dir())
        };
        core.clear_clipboard_history();
        let queued_late = {
            let mut queue = PENDING_IMAGES.lock().unwrap();
            enqueue_image(
                &mut queue,
                core.clipboard.images_dir(),
                b"post-clear capture",
            );
            queue.complete(late, b"late pre-clear capture".to_vec())
        };
        assert!(!queued_late);
        core.install_clipboard_images();
        assert_eq!(
            saved_images(&core.clipboard),
            vec![b"post-clear capture".to_vec()]
        );
    }

    #[test]
    fn installs_only_images_for_the_matching_store() {
        let fixture = Fixture::new();
        let mut store = ClipboardStore::open(fixture.0.clone());
        let other_dir = fixture.0.join("other-store");
        let mut queue = ImageQueue::default();
        enqueue_image(&mut queue, other_dir.clone(), b"other image");
        enqueue_image(&mut queue, store.images_dir(), b"owned image");
        assert!(queue.install(&mut store));
        assert_eq!(queue.pending.len(), 1);
        assert_eq!(queue.pending[0].capture.dir, other_dir);
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
