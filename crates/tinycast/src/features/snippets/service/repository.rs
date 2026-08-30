use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tinycast_pure::snippet::{
    parse_markdown, serialize, slug, SnippetSourceRevision, StoredSnippet,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::platform::messages::WM_SNIPPETS;
use crate::platform::paths;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnippetIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SnippetSnapshot {
    pub records: Vec<StoredSnippet>,
    pub issues: Vec<SnippetIssue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryError {
    Conflict {
        path: PathBuf,
        expected: SnippetSourceRevision,
        actual: Option<SnippetSourceRevision>,
    },
    FileNotFound(PathBuf),
    InvalidLocation(PathBuf),
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::Conflict { path, .. } => write!(
                f,
                "The snippet changed on disk. Reload it before saving or deleting. ({})",
                path.display()
            ),
            RepositoryError::FileNotFound(path) => {
                write!(
                    f,
                    "The snippet file no longer exists. ({})",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("snippet")
                )
            }
            RepositoryError::InvalidLocation(path) => {
                write!(
                    f,
                    "The snippet file is outside this Tinycast channel. ({})",
                    path.display()
                )
            }
            RepositoryError::Io { path, message } => {
                write!(f, "Could not access {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for RepositoryError {}

pub struct SnippetRepository {
    snippets_dir: PathBuf,
    stop: Arc<AtomicBool>,
    watch: Option<JoinHandle<()>>,
    generation: Arc<AtomicU64>,
}

impl SnippetRepository {
    pub fn new(snippets_dir: PathBuf) -> Self {
        Self {
            snippets_dir,
            stop: Arc::new(AtomicBool::new(true)),
            watch: None,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn in_roaming() -> Self {
        Self::new(paths::roaming_dir().join("Snippets"))
    }

    pub fn snippets_directory(&self) -> &Path {
        &self.snippets_dir
    }

    pub fn load(&self) -> Result<SnippetSnapshot, RepositoryError> {
        std::fs::create_dir_all(&self.snippets_dir).map_err(|e| io_err(&self.snippets_dir, e))?;
        let mut records = Vec::new();
        let mut issues = Vec::new();
        let entries = match std::fs::read_dir(&self.snippets_dir) {
            Ok(entries) => entries,
            Err(e) => return Err(io_err(&self.snippets_dir, e)),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if !is_md {
                continue;
            }
            let file_type = entry.file_type().ok();
            let is_file = file_type
                .map(|t| t.is_file() || t.is_symlink())
                .unwrap_or(false)
                && path.is_file();
            if !is_file {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => match parse_markdown(&path, &content) {
                    Ok(record) => records.push(record),
                    Err(err) => issues.push(SnippetIssue {
                        path,
                        message: err.to_string(),
                    }),
                },
                Err(err) => issues.push(SnippetIssue {
                    path,
                    message: err.to_string(),
                }),
            }
        }
        records.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        issues.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(SnippetSnapshot { records, issues })
    }

    pub fn create(
        &self,
        name: &str,
        keyword: Option<&str>,
        enabled: bool,
        show_confirmation: bool,
        body: &str,
    ) -> Result<StoredSnippet, RepositoryError> {
        std::fs::create_dir_all(&self.snippets_dir).map_err(|e| io_err(&self.snippets_dir, e))?;
        let draft = StoredSnippet {
            path: PathBuf::new(),
            name: name.to_string(),
            keyword: keyword.map(str::to_string),
            enabled,
            show_confirmation,
            body: body.to_string(),
            source_revision: SnippetSourceRevision::new(""),
        };
        let content = serialize(&draft);
        let mut suffix = 1;
        loop {
            let path = unique_path(&self.snippets_dir, name, suffix);
            match write_new(&path, content.as_bytes()) {
                Ok(()) => {
                    return Ok(StoredSnippet {
                        path,
                        name: draft.name,
                        keyword: draft.keyword,
                        enabled,
                        show_confirmation,
                        body: draft.body,
                        source_revision: SnippetSourceRevision::new(&content),
                    });
                }
                Err(_) if path.exists() => suffix += 1,
                Err(err) => return Err(io_err(&path, err)),
            }
        }
    }

    pub fn save(&self, record: &StoredSnippet) -> Result<StoredSnippet, RepositoryError> {
        let path = self.validated(&record.path)?;
        let actual = revision_at(&path)?;
        if actual != record.source_revision {
            return Err(RepositoryError::Conflict {
                path: path.clone(),
                expected: record.source_revision.clone(),
                actual: Some(actual),
            });
        }
        let content = serialize(record);
        atomic_write(&path, content.as_bytes()).map_err(|e| io_err(&path, e))?;
        Ok(StoredSnippet {
            path,
            name: record.name.clone(),
            keyword: record.keyword.clone(),
            enabled: record.enabled,
            show_confirmation: record.show_confirmation,
            body: record.body.clone(),
            source_revision: SnippetSourceRevision::new(&content),
        })
    }

    pub fn delete(&self, record: &StoredSnippet) -> Result<(), RepositoryError> {
        let path = self.validated(&record.path)?;
        let actual = revision_at(&path)?;
        if actual != record.source_revision {
            return Err(RepositoryError::Conflict {
                path: path.clone(),
                expected: record.source_revision.clone(),
                actual: Some(actual),
            });
        }
        std::fs::remove_file(&path).map_err(|e| io_err(&path, e))
    }

    pub fn start_watch(&mut self, host: HWND) {
        self.stop_watch();
        self.stop.store(false, Ordering::SeqCst);
        let dir = self.snippets_dir.clone();
        let stop = Arc::clone(&self.stop);
        let generation = Arc::clone(&self.generation);
        let host_bits = host.0 as isize;
        self.watch = std::thread::Builder::new()
            .name("tinycast-snippets".into())
            .spawn(move || {
                let mut last = fingerprint(&dir);
                while !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(400));
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let now = fingerprint(&dir);
                    if now != last {
                        last = now;
                        let gen = generation.fetch_add(1, Ordering::SeqCst) + 1;
                        let host = HWND(host_bits as *mut core::ffi::c_void);
                        let _ = unsafe {
                            PostMessageW(host, WM_SNIPPETS, WPARAM(gen as usize), LPARAM(0))
                        };
                    }
                }
            })
            .ok();
    }

    pub fn stop_watch(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.watch.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SnippetRepository {
    fn drop(&mut self) {
        self.stop_watch();
    }
}

fn unique_path(dir: &Path, name: &str, suffix: i32) -> PathBuf {
    let base = slug(name);
    let filename = if suffix == 1 {
        format!("{base}.md")
    } else {
        format!("{base}-{suffix}.md")
    };
    dir.join(filename)
}

fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!(
        "md.{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&tmp, bytes)?;
    match std::fs::hard_link(&tmp, path) {
        Ok(()) => {
            let _ = std::fs::remove_file(&tmp);
            Ok(())
        }
        Err(_) => {
            let _ = std::fs::remove_file(&tmp);
            if path.exists() {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "snippet exists",
                ))
            } else {
                std::fs::write(path, bytes)
            }
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_file_name(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("snippet")
    ));
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    }
}

fn revision_at(path: &Path) -> Result<SnippetSourceRevision, RepositoryError> {
    if !path.exists() {
        return Err(RepositoryError::FileNotFound(path.to_path_buf()));
    }
    let content = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    Ok(SnippetSourceRevision::new(&content))
}

fn fingerprint(dir: &Path) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let Ok(mut entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut names: Vec<_> = entries
        .by_ref()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for path in names {
        let meta = std::fs::metadata(&path);
        let stamp = meta
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        for b in path.to_string_lossy().as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash ^= stamp;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn io_err(path: &Path, err: std::io::Error) -> RepositoryError {
    RepositoryError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    }
}

impl SnippetRepository {
    fn validated(&self, path: &Path) -> Result<PathBuf, RepositoryError> {
        let parent = path.parent().ok_or_else(|| {
            RepositoryError::InvalidLocation(path.to_path_buf())
        })?;
        let same_dir = parent == self.snippets_dir
            || (parent.canonicalize().ok().as_deref()
                == self.snippets_dir.canonicalize().ok().as_deref());
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if !same_dir || !is_md {
            return Err(RepositoryError::InvalidLocation(path.to_path_buf()));
        }
        Ok(path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo() -> (PathBuf, SnippetRepository) {
        let dir = std::env::temp_dir().join(format!(
            "tinycast-snip-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        (dir.clone(), SnippetRepository::new(dir))
    }

    #[test]
    fn roaming_library_is_under_bundle_snippets() {
        let repo = SnippetRepository::in_roaming();
        assert_eq!(
            repo.snippets_directory().file_name().and_then(|n| n.to_str()),
            Some("Snippets")
        );
    }

    #[test]
    fn empty_library_on_first_load() {
        let (dir, repo) = temp_repo();
        let snap = repo.load().unwrap();
        assert!(snap.records.is_empty() && snap.issues.is_empty());
        assert_eq!(repo.snippets_directory(), dir.as_path());
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn create_save_delete_and_conflict() {
        let (dir, repo) = temp_repo();
        let first = repo
            .create("Same", None, true, false, "One")
            .unwrap();
        let second = repo
            .create("Same", None, true, false, "Two")
            .unwrap();
        assert_eq!(first.path.file_name().unwrap(), "same.md");
        assert_eq!(second.path.file_name().unwrap(), "same-2.md");

        let mut renamed = first.clone();
        renamed.name = "Renamed in Frontmatter".into();
        renamed.body = "Saved".into();
        let saved = repo.save(&renamed).unwrap();
        assert_eq!(saved.path, first.path);
        assert!(!dir.join("renamed-in-frontmatter.md").exists());

        std::fs::write(&saved.path, "External change").unwrap();
        let stale = repo.save(&saved);
        assert!(matches!(stale, Err(RepositoryError::Conflict { .. })));
        assert!(matches!(
            repo.delete(&saved),
            Err(RepositoryError::Conflict { .. })
        ));

        let loaded = repo.load().unwrap();
        assert_eq!(loaded.records.len(), 2);
        let current = loaded
            .records
            .iter()
            .find(|r| r.path == saved.path)
            .unwrap();
        repo.delete(current).unwrap();
        let after = repo.load().unwrap();
        assert_eq!(after.records.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_file_does_not_hide_valid() {
        let (dir, repo) = temp_repo();
        let _ = repo.load().unwrap();
        std::fs::write(
            dir.join("valid.md"),
            serialize(&StoredSnippet {
                path: dir.join("valid.md"),
                name: "Valid".into(),
                keyword: None,
                enabled: true,
                show_confirmation: false,
                body: "Body".into(),
                source_revision: SnippetSourceRevision::new(""),
            }),
        )
        .unwrap();
        std::fs::write(dir.join("invalid.md"), "---\nname: unquoted\n---\nBody").unwrap();
        let snap = repo.load().unwrap();
        assert_eq!(
            snap.records.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["Valid"]
        );
        assert_eq!(snap.issues.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn watch_start_stop_without_host() {
        let (dir, mut repo) = temp_repo();
        let _ = repo.load().unwrap();
        repo.start_watch(HWND::default());
        repo.stop_watch();
        let _ = std::fs::remove_dir_all(dir);
    }
}
