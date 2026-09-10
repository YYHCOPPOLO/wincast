use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tinycast_pure::note::{filter_filenames, unique_untitled_name, Note, NoteSummary};

use crate::platform::paths;

#[derive(Debug)]
pub enum NotesError {
    InvalidLocation,
    Io(String),
}

impl std::fmt::Display for NotesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotesError::InvalidLocation => {
                write!(f, "The note file is outside this Tinycast channel.")
            }
            NotesError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

pub struct NotesStore {
    dir: PathBuf,
    summaries: Vec<NoteSummary>,
    active: Option<Note>,
    dirty: bool,
}

impl NotesStore {
    pub fn in_roaming() -> Self {
        Self::new(paths::roaming_dir().join("Notes"))
    }

    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            summaries: Vec::new(),
            active: None,
            dirty: false,
        }
    }

    pub fn notes_directory(&self) -> &Path {
        &self.dir
    }

    pub fn summaries(&self) -> &[NoteSummary] {
        &self.summaries
    }

    pub fn active(&self) -> Option<&Note> {
        self.active.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_body(&mut self, body: String) {
        if let Some(note) = &mut self.active {
            if note.body != body {
                note.body = body;
                self.dirty = true;
            }
        }
    }

    pub fn reload(&mut self) -> Result<(), NotesError> {
        self.ensure_dir()?;
        let preferred = self.active.as_ref().map(|n| n.id());
        self.summaries = self.list()?;
        if self.active.is_some() {
            return Ok(());
        }
        if let Some(id) = preferred {
            if self.summaries.iter().any(|s| s.id == id) {
                self.active = Some(self.load_id(&id)?);
                return Ok(());
            }
        }
        if let Some(first) = self.summaries.first() {
            self.active = Some(self.load_id(&first.id)?);
        }
        Ok(())
    }

    pub fn show_last(&mut self) -> Result<(), NotesError> {
        self.flush()?;
        self.reload()?;
        if self.active.is_none() {
            if let Some(first) = self.summaries.first().cloned() {
                self.active = Some(self.load_id(&first.id)?);
            }
        }
        Ok(())
    }

    pub fn create(&mut self) -> Result<Note, NotesError> {
        self.flush()?;
        self.ensure_dir()?;
        self.summaries = self.list()?;
        let occupied: Vec<String> = self.summaries.iter().map(|s| s.id.clone()).collect();
        let name = unique_untitled_name(&occupied);
        let path = self.validated_child(&name)?;
        fs::write(&path, "").map_err(|e| NotesError::Io(e.to_string()))?;
        let note = Note {
            path: PathBuf::from(&name),
            body: String::new(),
        };
        self.active = Some(note.clone());
        self.dirty = false;
        self.summaries = self.list()?;
        Ok(note)
    }

    pub fn select(&mut self, id: &str) -> Result<(), NotesError> {
        self.flush()?;
        self.active = Some(self.load_id(id)?);
        self.dirty = false;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Vec<NoteSummary> {
        filter_filenames(&self.summaries, query)
    }

    pub fn flush(&mut self) -> Result<(), NotesError> {
        if !self.dirty {
            return Ok(());
        }
        let Some(note) = &self.active else {
            return Ok(());
        };
        let path = self.validated_child(&note.id())?;
        fs::write(&path, note.body.as_bytes()).map_err(|e| NotesError::Io(e.to_string()))?;
        self.dirty = false;
        self.summaries = self.list()?;
        Ok(())
    }

    fn load_id(&self, id: &str) -> Result<Note, NotesError> {
        let path = self.validated_child(id)?;
        let body = fs::read_to_string(&path).map_err(|e| NotesError::Io(e.to_string()))?;
        Ok(Note {
            path: PathBuf::from(id),
            body,
        })
    }

    fn list(&self) -> Result<Vec<NoteSummary>, NotesError> {
        self.ensure_dir()?;
        let mut out = Vec::new();
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) => return Err(NotesError::Io(e.to_string())),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .eq_ignore_ascii_case("md")
                && path.is_file()
            {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.starts_with('.') {
                    continue;
                }
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let title = Path::new(&name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name.clone());
                out.push(NoteSummary {
                    id: name,
                    title,
                    modified_at: modified,
                });
            }
        }
        out.sort_by(|a, b| {
            b.modified_at
                .cmp(&a.modified_at)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(out)
    }

    fn ensure_dir(&self) -> Result<(), NotesError> {
        fs::create_dir_all(&self.dir).map_err(|e| NotesError::Io(e.to_string()))
    }

    fn validated_child(&self, id: &str) -> Result<PathBuf, NotesError> {
        if id.is_empty()
            || id.contains(['/', '\\', '\0'])
            || id == "."
            || id == ".."
            || id.starts_with('.')
            || !id.to_lowercase().ends_with(".md")
        {
            return Err(NotesError::InvalidLocation);
        }
        let path = self.dir.join(id);
        if path.parent() != Some(self.dir.as_path()) {
            return Err(NotesError::InvalidLocation);
        }
        Ok(path)
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (NotesStore, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "tinycast-notes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        (NotesStore::new(root.clone()), root)
    }

    #[test]
    fn create_and_search_by_filename() {
        let (mut store, root) = temp_store();
        store.create().unwrap();
        store.create().unwrap();
        store.reload().unwrap();
        assert_eq!(store.summaries().len(), 2);
        let hits = store.search("Untitled");
        assert_eq!(hits.len(), 2);
        let hits = store.search("Untitled 2");
        assert_eq!(hits.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn identity_is_relative_filename() {
        let (mut store, root) = temp_store();
        let note = store.create().unwrap();
        assert_eq!(note.id(), "Untitled.md");
        store.set_body("# hi".into());
        store.flush().unwrap();
        let text = fs::read_to_string(root.join("Untitled.md")).unwrap();
        assert_eq!(text, "# hi");
        let _ = fs::remove_dir_all(&root);
    }
}
