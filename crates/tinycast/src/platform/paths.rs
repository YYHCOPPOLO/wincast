use std::path::PathBuf;

const BUNDLE_ID: &str = "com.tinycast.win";

pub fn roaming_dir() -> PathBuf {
    known_dir("APPDATA")
}

pub fn local_dir() -> PathBuf {
    known_dir("LOCALAPPDATA")
}

#[cfg(not(test))]
fn known_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(BUNDLE_ID)
}

#[cfg(test)]
fn known_dir(var: &str) -> PathBuf {
    TEST_DATA_DIR.with(|dir| dir.0.join(var).join(BUNDLE_ID))
}

#[cfg(test)]
std::thread_local! {
    // Libtest runs each test on its own thread. Keep paths stable until the
    // thread exits, after ordinary test locals (including SQLite handles) drop.
    static TEST_DATA_DIR: TestDataDir = TestDataDir::new();
}

#[cfg(test)]
struct TestDataDir(PathBuf);

#[cfg(test)]
impl TestDataDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
        let temp = std::env::temp_dir().canonicalize().expect("resolve test temp directory");
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        loop {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let root = temp.join(format!("tinycast-test-data-{}-{stamp}-{sequence}", std::process::id()));
            // Never adopt an existing directory, even after a process ID reuse.
            match std::fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned test data directory {}: {error}", root.display()),
            }
        }
    }
}

#[cfg(test)]
impl Drop for TestDataDir {
    fn drop(&mut self) {
        // The root is an absolute, uniquely created child of canonical temp.
        // Refuse a retargeted root; never remove its parent or sibling fixtures.
        match self.0.canonicalize() {
            Ok(resolved) if resolved == self.0 => {
                if let Err(error) = std::fs::remove_dir_all(&resolved) {
                    eprintln!("could not clean test data directory {}: {error}", resolved.display());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            result => eprintln!("refusing cleanup of changed test root {}: {result:?}", self.0.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn environment_dir(var: &str) -> PathBuf {
        std::env::var_os(var).map(PathBuf::from).unwrap_or_default().join(BUNDLE_ID)
    }

    fn isolated_dirs() -> (PathBuf, PathBuf, PathBuf) {
        let roaming = roaming_dir();
        let local = local_dir();
        // Check isolation before any test writes, even during the RED run.
        assert_ne!(roaming, environment_dir("APPDATA"));
        assert_ne!(local, environment_dir("LOCALAPPDATA"));
        let root = roaming.parent().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(local.parent().and_then(Path::parent), Some(root.as_path()));
        assert!(root.is_absolute());
        assert!(root.file_name().unwrap().to_str().unwrap().starts_with("tinycast-test-data-"));
        assert!(root.is_dir());
        (root, roaming, local)
    }

    #[test]
    fn test_dirs_are_automatically_isolated_from_environment_appdata() {
        isolated_dirs();
    }

    #[test]
    fn test_dirs_are_stable_and_separate_without_changing_environment() {
        let vars = ["APPDATA", "LOCALAPPDATA", "TEMP", "TMP", "USERPROFILE"];
        let before: Vec<_> = vars.iter().map(std::env::var_os).collect();
        let roaming = roaming_dir();
        let local = local_dir();
        assert_eq!(roaming, roaming_dir());
        assert_eq!(local, local_dir());
        assert_ne!(roaming, local);
        assert!(roaming.is_absolute());
        assert!(local.is_absolute());
        let after: Vec<_> = vars.iter().map(std::env::var_os).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn test_threads_get_distinct_data_dirs() {
        let parent = (roaming_dir(), local_dir());
        let first = std::thread::spawn(|| (roaming_dir(), local_dir()));
        let second = std::thread::spawn(|| (roaming_dir(), local_dir()));
        let first = first.join().unwrap();
        let second = second.join().unwrap();
        assert_ne!(parent.0, first.0);
        assert_ne!(parent.1, first.1);
        assert_ne!(first.0, second.0);
        assert_ne!(first.1, second.1);
    }

    #[test]
    fn test_thread_cleanup_follows_store_drop_and_preserves_other_roots() {
        use crate::features::ai::service::history::ChatHistoryStore;
        use crate::features::clipboard::service::store::ClipboardStore;
        use tinycast_pure::ai::{ChatMessage, ChatSession};

        let (parent_root, roaming, local) = isolated_dirs();
        for dir in [&roaming, &local] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("keep.txt"), b"another thread's fixture").unwrap();
        }
        let child_root = std::thread::spawn(|| {
            let (root, roaming, local) = isolated_dirs();
            let mut clipboard = ClipboardStore::open(local.clone());
            assert!(clipboard.insert_text("owned clipboard fixture".into()).is_some());
            let mut chat = ChatHistoryStore::in_roaming();
            let mut session = ChatSession::new(1);
            session.append(ChatMessage::user("owned chat fixture", 1));
            chat.save(&session);
            assert_eq!(chat.conversations().len(), 1);
            assert!(local.join("clipboard.sqlite3").is_file());
            assert!(roaming.join("ai-chats.sqlite3").is_file());
            // Both SQLite handles remain in scope until this thread returns.
            root
        }).join().unwrap();
        assert!(!child_root.try_exists().unwrap());
        assert!(parent_root.is_dir());
        for dir in [roaming, local] {
            assert_eq!(std::fs::read(dir.join("keep.txt")).unwrap(), b"another thread's fixture");
        }
    }

    #[test]
    fn test_thread_cleanup_also_runs_after_a_test_panics() {
        use crate::features::clipboard::service::store::ClipboardStore;

        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (root, _, local) = isolated_dirs();
            tx.send(root).unwrap();
            let mut store = ClipboardStore::open(local);
            assert!(store.insert_text("owned panic fixture".into()).is_some());
            panic!("intentional fixture-unwind test");
        });
        let panic = worker.join().expect_err("worker should unwind");
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"intentional fixture-unwind test"));
        let root = rx.recv().expect("worker must have created an isolated root");
        assert!(!root.try_exists().unwrap());
    }

    #[test]
    fn dirs_use_bundle_id() {
        assert_eq!(
            roaming_dir().file_name().and_then(|n| n.to_str()),
            Some("com.tinycast.win")
        );
        assert_eq!(
            local_dir().file_name().and_then(|n| n.to_str()),
            Some("com.tinycast.win")
        );
    }
}
