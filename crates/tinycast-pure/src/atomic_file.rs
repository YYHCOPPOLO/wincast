//! Crash-safe file replacement for settings and user data.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Write `bytes` to `path` so readers only ever see the old or the new
/// contents: write a sibling temp file, flush it to disk, then rename it over
/// `path` (`std::fs::rename` replaces an existing file on Windows too). A
/// crash or kill mid-write leaves at most a stray `.<name>.tmp`, never a
/// truncated `path`. The parent directory must already exist.
pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let path = path.as_ref();
    let tmp = temp_path(path);
    let result = write_synced(&tmp, bytes.as_ref()).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// `.<file name>.tmp` next to `path`, so the rename never crosses volumes.
pub fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    path.with_file_name(format!(".{name}.tmp"))
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tinycast-atomic-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_new_file() {
        let dir = temp_dir("new");
        let path = dir.join("settings.json");
        write(&path, b"{}").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"{}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replaces_existing_file_and_leaves_no_temp() {
        let dir = temp_dir("replace");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"old contents that are longer").unwrap();
        write(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!temp_path(&path).exists());
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(names.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overwrites_stale_temp_from_a_crashed_write() {
        let dir = temp_dir("stale");
        let path = dir.join("note.md");
        std::fs::write(temp_path(&path), b"half-written garbage").unwrap();
        write(&path, b"body").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"body");
        assert!(!temp_path(&path).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failure_keeps_old_file_and_cleans_temp() {
        let dir = temp_dir("fail");
        // Renaming a file over a non-empty directory fails on every platform.
        let path = dir.join("taken");
        std::fs::create_dir_all(path.join("child")).unwrap();
        assert!(write(&path, b"x").is_err());
        assert!(path.join("child").is_dir());
        assert!(!temp_path(&path).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_parent_is_an_error() {
        let dir = temp_dir("parent");
        let path = dir.join("missing").join("settings.json");
        assert!(write(&path, b"{}").is_err());
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn temp_lives_next_to_target() {
        let path = Path::new("dir").join("Untitled.md");
        assert_eq!(temp_path(&path), Path::new("dir").join(".Untitled.md.tmp"));
    }
}
