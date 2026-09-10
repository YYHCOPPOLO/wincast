//! SQLite clipboard history with pins and FTS5 trigram search.
//!
//! Stable `id` is a unique text UUID. Recency is implicit AUTOINCREMENT `rowid`.
//! `promote` / unpin delete+insert without binding the old rowid so the row leads.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use tinycast_pure::clipboard_text::{text_form, TextForm};

const MEMORY_WINDOW: i64 = 1000;
const FTS_LIMIT: i64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipKind {
    Text,
    Image,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardFilter {
    All,
    Text,
    Images,
    Links,
    Emails,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardItem {
    pub id: String,
    pub kind: ClipKind,
    pub text: Option<String>,
    pub image_path: Option<PathBuf>,
    pub created_at: i64,
    pub pinned_at: Option<i64>,
}

impl ClipboardItem {
    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }
}

pub struct ClipboardStore {
    conn: Option<Connection>,
    dir: PathBuf,
    items: Vec<ClipboardItem>,
    max_age_secs: i64,
    last_error: Option<String>,
}

impl ClipboardStore {
    pub fn open(dir: PathBuf) -> Self {
        let mut store = Self {
            conn: None,
            dir,
            items: Vec::new(),
            max_age_secs: -1,
            last_error: None,
        };
        let opened = (|| -> Result<_, String> {
            if store.dir.components().any(|c| c == Component::ParentDir) {
                return Err("Invalid clipboard storage directory".into());
            }
            std::fs::create_dir_all(&store.dir).map_err(|e| e.to_string())?;
            checked_directory(&store.dir)?;
            let dir = std::fs::canonicalize(&store.dir).map_err(|e| e.to_string())?;
            let images = dir.join("images");
            match std::fs::symlink_metadata(&images) {
                Ok(_) => checked_directory(&images)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
            let conn = open_db(&dir.join("clipboard.sqlite3"))?;
            let items = load_items(&conn).map_err(|e| e.to_string())?;
            // Do not even create an image directory when the existing database
            // cannot be opened safely. There is deliberately no recovery delete.
            std::fs::create_dir_all(&images).map_err(|e| e.to_string())?;
            checked_directory(&images)?;
            conn.set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                false,
            )
            .map_err(|e| e.to_string())?;
            Ok((conn, dir, items))
        })();
        match opened {
            Ok((conn, dir, items)) => {
                store.conn = Some(conn);
                store.dir = dir;
                store.items = items;
            }
            Err(error) => store.last_error = Some(error),
        }
        store
    }

    pub fn is_available(&self) -> bool {
        self.conn.is_some() && self.last_error.is_none()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn images_dir(&self) -> PathBuf {
        self.dir.join("images")
    }

    pub fn set_max_age_secs(&mut self, max_age_secs: i64) {
        self.max_age_secs = max_age_secs;
    }

    pub fn insert_text(&mut self, text: String) -> Option<String> {
        if !self.is_available() {
            return None;
        }
        if let Some(first) = self.items.first() {
            if first.kind == ClipKind::Text && first.text.as_deref() == Some(text.as_str()) {
                return Some(first.id.clone());
            }
        }
        let now = unix_now();
        let id = new_id();
        self.transact(|tx| {
            expect_one(tx.execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, NULL, 'text', ?3, NULL)",
                params![id, now, text],
            )?)
        })?;
        self.after_insert();
        Some(id)
    }

    pub fn insert_image(&mut self, png: PathBuf) -> Option<String> {
        if !self.is_available() {
            return None;
        }
        let now = unix_now();
        let id = new_id();
        let path = png.to_string_lossy();
        self.transact(|tx| {
            expect_one(tx.execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, NULL, 'image', NULL, ?3)",
                params![id, now, path.as_ref()],
            )?)
        })?;
        self.after_insert();
        Some(id)
    }

    /// Install an encoded capture only while storage is healthy. Workers keep
    /// PNG bytes in memory, so an in-flight encoder cannot write after failure.
    pub fn save_image(&mut self, png: &[u8]) -> Option<String> {
        if !self.is_available() {
            return None;
        }
        let dir = match self.checked_images_dir() {
            Ok(dir) => dir,
            Err(error) => {
                self.last_error = Some(error);
                return None;
            }
        };
        let path = dir.join(format!("{}.png", new_id()));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) => {
                self.last_error = Some(error.to_string());
                return None;
            }
        };
        let written = file.write_all(png).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = written {
            self.last_error = Some(error.to_string());
            let _ = std::fs::remove_file(&path); // exclusively created by this call
            return None;
        }
        let id = self.insert_image(path.clone());
        if id.is_none() {
            let _ = std::fs::remove_file(path); // failed transaction owns no row
        }
        id
    }

    pub fn pinned_item(
        &self,
        index: usize,
        query: &str,
        filter: ClipboardFilter,
    ) -> Option<ClipboardItem> {
        self.search(query, filter)
            .into_iter()
            .filter(|i| i.is_pinned())
            .nth(index)
    }

    pub fn row_index(&self, id: &str, query: &str, filter: ClipboardFilter) -> Option<usize> {
        self.search(query, filter)
            .iter()
            .position(|item| item.id == id)
    }

    pub fn recent_text(&self, limit: usize) -> Vec<String> {
        self.items
            .iter()
            .filter_map(|item| item.text.clone())
            .take(limit)
            .collect()
    }

    pub fn search(&self, query: &str, filter: ClipboardFilter) -> Vec<ClipboardItem> {
        let q = query.trim();
        let rows = if q.is_empty() {
            self.ordered()
        } else if q.chars().count() < 3 {
            self.fallback_search(q)
        } else {
            self.fts_search(q)
        };
        apply_filter(rows, filter)
    }

    pub fn toggle_pin(&mut self, id: &str) {
        let Some(item) = self.find(id) else {
            return;
        };
        if item.is_pinned() {
            let created_at = unix_now().max(item.created_at.saturating_add(1));
            self.reinsert(item, created_at, None);
        } else {
            let stamp = next_pin_stamp(&self.items);
            self.transact(|tx| {
                expect_one(tx.execute(
                    "UPDATE items SET pinned_at = ?1 WHERE id = ?2",
                    params![stamp, id],
                )?)
            });
        }
    }

    pub fn promote(&mut self, id: &str) {
        let Some(item) = self.find(id) else {
            return;
        };
        if item.is_pinned() {
            return;
        }
        if self.items.first().map(|i| i.id.as_str()) == Some(id) {
            return;
        }
        let created_at = unix_now().max(item.created_at.saturating_add(1));
        self.reinsert(item, created_at, None);
    }

    pub fn clear(&mut self) {
        self.delete_images_after("DELETE FROM items RETURNING image_path", []);
    }

    pub fn prune_unpinned_older_than(&mut self, max_age_secs: i64) {
        if max_age_secs < 0 {
            return;
        }
        let cutoff = unix_now().saturating_sub(max_age_secs);
        self.delete_images_after(
            "DELETE FROM items WHERE pinned_at IS NULL AND created_at < ?1 RETURNING image_path",
            params![cutoff],
        );
    }

    fn delete_images_after(&mut self, sql: &str, params: impl rusqlite::Params) {
        let deleted = self.transact(|tx| {
            let paths = tx
                .prepare(sql)?
                .query_map(params, |row| row.get::<_, Option<String>>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            // Include pins and rows outside the in-memory window. A shared file
            // still belongs to a surviving row even if another row was deleted.
            let referenced = tx
                .prepare("SELECT image_path FROM items WHERE image_path IS NOT NULL")?
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok((paths, referenced))
        });
        if let Some((paths, referenced)) = deleted {
            self.remove_unreferenced_images(paths.into_iter().flatten(), referenced);
        }
    }

    fn checked_images_dir(&self) -> Result<PathBuf, String> {
        checked_directory(&self.dir)?;
        let images = self.images_dir();
        checked_directory(&images)?;
        let canonical = std::fs::canonicalize(&images).map_err(|e| e.to_string())?;
        if canonical != images {
            return Err("Clipboard image directory changed".into());
        }
        Ok(images)
    }

    fn remove_unreferenced_images(
        &self,
        deleted: impl Iterator<Item = String>,
        referenced: Vec<String>,
    ) {
        let Ok(root) = self.checked_images_dir() else {
            return;
        };
        let references: Vec<PathBuf> = referenced.into_iter().map(PathBuf::from).collect();
        let canonical_references: HashSet<PathBuf> = references
            .iter()
            .filter_map(|path| std::fs::canonicalize(path).ok())
            .collect();
        for path in deleted.map(PathBuf::from) {
            if !path.is_absolute()
                || path.components().any(|c| c == Component::ParentDir)
                || references.contains(&path)
            {
                continue;
            }
            let Some(parent) = path.parent() else {
                continue;
            };
            if checked_directory(parent).is_err()
                || std::fs::canonicalize(parent).ok().as_ref() != Some(&root)
            {
                continue;
            }
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_file() || is_reparse(&metadata) {
                continue;
            }
            let Ok(canonical) = std::fs::canonicalize(&path) else {
                continue;
            };
            if canonical.parent() == Some(root.as_path())
                && !canonical_references.contains(&canonical)
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    fn after_insert(&mut self) {
        if self.max_age_secs >= 0 {
            self.prune_unpinned_older_than(self.max_age_secs);
        }
    }

    fn find(&self, id: &str) -> Option<ClipboardItem> {
        if let Some(item) = self.items.iter().find(|i| i.id == id).cloned() {
            return Some(item);
        }
        self.conn
            .as_ref()?
            .query_row(
                "SELECT id, created_at, pinned_at, kind, text, image_path FROM items WHERE id = ?1",
                params![id],
                row_item,
            )
            .optional()
            .ok()
            .flatten()
    }

    fn reinsert(&mut self, item: ClipboardItem, created_at: i64, pinned_at: Option<i64>) {
        let kind = match item.kind {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
        };
        let path = item
            .image_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        self.transact(|tx| {
            expect_one(tx.execute("DELETE FROM items WHERE id = ?1", params![item.id])?)?;
            expect_one(tx.execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![item.id, created_at, pinned_at, kind, item.text, path],
            )?)
        });
    }

    fn transact<T>(
        &mut self,
        operation: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<T>,
    ) -> Option<T> {
        let conn = self.conn.as_mut()?;
        let result = (|| {
            let tx = conn.transaction()?;
            let value = operation(&tx)?;
            let items = load_items(&tx)?;
            tx.commit()?;
            Ok::<_, rusqlite::Error>((value, items))
        })();
        match result {
            Ok((value, items)) => {
                self.items = items;
                self.last_error = None;
                Some(value)
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
                None
            }
        }
    }

    fn ordered(&self) -> Vec<ClipboardItem> {
        let mut pins: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|i| i.is_pinned())
            .cloned()
            .collect();
        pins.sort_by_key(|i| i.pinned_at.unwrap_or(i64::MAX));
        let rest: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|i| !i.is_pinned())
            .cloned()
            .collect();
        pins.extend(rest);
        pins
    }

    fn fallback_search(&self, query: &str) -> Vec<ClipboardItem> {
        let needle = query.to_lowercase();
        let matches = |item: &ClipboardItem| {
            item.text
                .as_deref()
                .is_some_and(|t| t.to_lowercase().contains(&needle))
        };
        let mut pins: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|i| i.is_pinned() && matches(i))
            .cloned()
            .collect();
        pins.sort_by_key(|i| i.pinned_at.unwrap_or(i64::MAX));
        let rest: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|i| !i.is_pinned() && matches(i))
            .cloned()
            .collect();
        pins.extend(rest);
        pins
    }

    fn fts_search(&self, query: &str) -> Vec<ClipboardItem> {
        let mut pins: Vec<ClipboardItem> = self
            .items
            .iter()
            .filter(|i| i.is_pinned() && text_matches(i, query))
            .cloned()
            .collect();
        pins.sort_by_key(|i| i.pinned_at.unwrap_or(i64::MAX));
        let match_expr = format!("\"{}\"", query.replace('"', "\"\""));
        let fts = self.conn.as_ref().and_then(|conn| {
            conn.prepare(
                "SELECT items.id, items.created_at, items.pinned_at, items.kind, items.text, items.image_path
                 FROM items
                 JOIN items_fts ON items.rowid = items_fts.rowid
                 WHERE items_fts MATCH ?1 AND items.pinned_at IS NULL
                 ORDER BY items.rowid DESC
                 LIMIT ?2",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![match_expr, FTS_LIMIT], row_item)
                    .ok()
                    .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>().ok())
            })
        })
            .unwrap_or_else(|| self.fallback_search(query));
        let pin_ids: std::collections::HashSet<String> =
            pins.iter().map(|i| i.id.clone()).collect();
        pins.extend(fts.into_iter().filter(|i| !pin_ids.contains(&i.id)));
        pins
    }

    #[cfg(test)]
    fn reload(&mut self) {
        let Some(conn) = &self.conn else {
            return;
        };
        match load_items(conn) {
            Ok(items) => self.items = items,
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }
}

fn load_items(conn: &Connection) -> rusqlite::Result<Vec<ClipboardItem>> {
    let floor = conn
        .query_row(
            "SELECT rowid FROM items WHERE pinned_at IS NULL ORDER BY rowid DESC LIMIT 1 OFFSET ?1",
            params![MEMORY_WINDOW - 1],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    // Two indexed branches, not `pinned_at IS NOT NULL OR rowid >= ?`.
    conn.prepare(
        "SELECT id, created_at, pinned_at, kind, text, image_path FROM (
           SELECT rowid AS rid, * FROM items WHERE rowid >= ?1
           UNION ALL
           SELECT rowid AS rid, * FROM items WHERE pinned_at IS NOT NULL AND rowid < ?1
         ) ORDER BY rid DESC",
    )?
    .query_map(params![floor], row_item)?
    .collect()
}

fn expect_one(changed: usize) -> rusqlite::Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(rusqlite::Error::StatementChangedRows(changed))
    }
}

fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT (includes junctions)
}

fn checked_directory(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || is_reparse(&metadata) {
        return Err(format!("Invalid clipboard directory: {}", path.display()));
    }
    Ok(())
}

fn apply_filter(rows: Vec<ClipboardItem>, filter: ClipboardFilter) -> Vec<ClipboardItem> {
    rows.into_iter()
        .filter(|item| match filter {
            ClipboardFilter::All => true,
            ClipboardFilter::Images => item.kind == ClipKind::Image,
            ClipboardFilter::Text => {
                item.kind == ClipKind::Text
                    && item.text.as_deref().map(text_form) == Some(TextForm::Plain)
            }
            ClipboardFilter::Links => {
                item.kind == ClipKind::Text
                    && item.text.as_deref().map(text_form) == Some(TextForm::Link)
            }
            ClipboardFilter::Emails => {
                item.kind == ClipKind::Text
                    && item.text.as_deref().map(text_form) == Some(TextForm::Email)
            }
        })
        .collect()
}

fn text_matches(item: &ClipboardItem, query: &str) -> bool {
    let needle = query.to_lowercase();
    item.text
        .as_deref()
        .is_some_and(|t| t.to_lowercase().contains(&needle))
}

fn row_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardItem> {
    let kind: String = row.get(3)?;
    let kind = if kind == "image" {
        ClipKind::Image
    } else {
        ClipKind::Text
    };
    let path: Option<String> = row.get(5)?;
    Ok(ClipboardItem {
        id: row.get(0)?,
        created_at: row.get(1)?,
        pinned_at: row.get(2)?,
        kind,
        text: row.get(4)?,
        image_path: path.map(PathBuf::from),
    })
}

fn validate_schema(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("Cannot read clipboard database version: {e:?}"))?;
    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='items'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let lower = sql.to_lowercase();
    let columns = conn
        .prepare("PRAGMA table_info(items)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let expected = [
        ("rowid", "INTEGER", 1),
        ("id", "TEXT", 0),
        ("created_at", "INTEGER", 0),
        ("pinned_at", "INTEGER", 0),
        ("kind", "TEXT", 0),
        ("text", "TEXT", 0),
        ("image_path", "TEXT", 0),
    ];
    if version != 0
        || !lower.contains("autoincrement")
        || columns.len() != expected.len()
        || columns
            .iter()
            .zip(expected)
            .any(|((name, kind, pk), (n, k, p))| {
                name != n || !kind.eq_ignore_ascii_case(k) || *pk != p
            })
    {
        return Err("Unsupported clipboard database schema (original preserved)".into());
    }
    let objects: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE (type='table' AND name='items_fts') OR (type='trigger' AND name IN ('items_ai', 'items_ad'))",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if objects != 3 {
        return Err("Incomplete clipboard database schema (original preserved)".into());
    }
    let check: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        return Err(check);
    }
    load_items(conn).map_err(|e| e.to_string())?;
    Ok(())
}

fn open_db(path: &Path) -> Result<Connection, String> {
    let existing = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_reparse(&metadata) => true,
        Ok(_) => return Err("Invalid clipboard database path".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.to_string()),
    };
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        match std::fs::symlink_metadata(Path::new(&sidecar)) {
            Ok(metadata) => {
                if !existing || suffix == "-journal" || !metadata.is_file() || is_reparse(&metadata)
                {
                    // In particular, never let opening a database recover a hot
                    // rollback journal before its schema has been validated.
                    return Err(
                        "Clipboard database has unsupported sidecars (originals preserved)".into(),
                    );
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if existing {
        // Windows SQLite cannot take the exclusive lock on a READ_ONLY handle.
        // A query-only preflight uses private WAL indexing, never checkpoints on
        // close, and refuses rollback journals above, preserving unknown bytes.
        // A concurrently locked database leaves clipboard history unavailable.
        let probe = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| e.to_string())?;
        probe
            .set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                true,
            )
            .map_err(|e| e.to_string())?;
        probe
            .busy_timeout(Duration::from_millis(100))
            .map_err(|e| e.to_string())?;
        probe
            .execute_batch("PRAGMA query_only=ON; PRAGMA locking_mode=EXCLUSIVE;")
            .map_err(|e| e.to_string())?;
        validate_schema(&probe)?;
        drop(probe);
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| e.to_string())?;
        conn.set_db_config(
            rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
            true,
        )
        .map_err(|e| e.to_string())?;
        conn.busy_timeout(Duration::from_millis(100))
            .map_err(|e| e.to_string())?;
        // No CREATE, journal-mode changes, or migrations on an existing file.
        validate_schema(&conn)?;
        return Ok(conn);
    }
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.set_db_config(
        rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
        true,
    )
    .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_millis(100))
        .map_err(|e| e.to_string())?;
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS items(
          rowid INTEGER PRIMARY KEY AUTOINCREMENT,
          id TEXT NOT NULL UNIQUE,
          created_at INTEGER NOT NULL,
          pinned_at INTEGER,
          kind TEXT NOT NULL CHECK (kind IN ('text','image')),
          text TEXT,
          image_path TEXT
        );
        CREATE INDEX IF NOT EXISTS items_created_at ON items(created_at);
        CREATE INDEX IF NOT EXISTS items_pinned_at ON items(pinned_at) WHERE pinned_at IS NOT NULL;
        CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
          text, content='items', content_rowid='rowid', tokenize='trigram'
        );
        CREATE TRIGGER IF NOT EXISTS items_ai AFTER INSERT ON items BEGIN
          INSERT INTO items_fts(rowid, text) VALUES(new.rowid, new.text);
        END;
        CREATE TRIGGER IF NOT EXISTS items_ad AFTER DELETE ON items BEGIN
          INSERT INTO items_fts(items_fts, rowid, text) VALUES('delete', old.rowid, old.text);
        END;
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn next_pin_stamp(items: &[ClipboardItem]) -> i64 {
    let now = unix_now();
    let last = items.iter().filter_map(|i| i.pinned_at).max().unwrap_or(0);
    now.max(last + 1)
}

pub(crate) fn new_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    format!("{nanos:016x}-{pid:08x}-{seq:08x}")
}

#[cfg(test)]
fn temp_dir() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tinycast-clip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&root);
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            Self(temp_dir())
        }

        fn open(&self) -> ClipboardStore {
            ClipboardStore::open(self.0.clone())
        }

        fn image(&self, name: &str) -> PathBuf {
            let path = self.0.join("images").join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"fixture image").unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // The fixture is declared before every connection, so all SQLite
            // handles are closed before this owned, uniquely named tree is removed.
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                if !std::thread::panicking() {
                    panic!("could not clean fixture {}: {error}", self.0.display());
                }
            }
        }
    }

    fn connection(store: &ClipboardStore) -> &Connection {
        store
            .conn
            .as_ref()
            .expect("fixture database should be available")
    }

    fn fail_deletes(store: &ClipboardStore) {
        connection(store)
            .execute_batch(
                "
            CREATE TABLE attempted_deletes(value INTEGER);
            CREATE TRIGGER reject_delete BEFORE DELETE ON items BEGIN
                INSERT INTO attempted_deletes VALUES(1);
                SELECT RAISE(FAIL, 'injected delete failure');
            END;
        ",
            )
            .unwrap();
    }

    fn assert_delete_rolled_back(store: &ClipboardStore) {
        let attempts: i64 = connection(store)
            .query_row("SELECT COUNT(*) FROM attempted_deletes", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(attempts, 0, "trigger side effects must roll back too");
    }

    #[test]
    fn corrupt_open_preserves_database_sidecars_and_images() {
        let fixture = Fixture::new();
        let image = fixture.image("keep.png");
        let files = [
            "clipboard.sqlite3",
            "clipboard.sqlite3-wal",
            "clipboard.sqlite3-shm",
        ];
        for file in files {
            std::fs::write(
                fixture.0.join(file),
                format!("original corrupt fixture {file}"),
            )
            .unwrap();
        }
        let mut store = fixture.open();
        assert!(!store.is_available());
        assert!(store.last_error().is_some());
        store.clear();
        drop(store);
        for file in files {
            assert_eq!(
                std::fs::read(fixture.0.join(file)).unwrap(),
                format!("original corrupt fixture {file}").as_bytes()
            );
        }
        assert!(image.exists());
    }

    #[test]
    fn incompatible_schema_is_preserved_without_recreation() {
        let fixture = Fixture::new();
        let path = fixture.0.join("clipboard.sqlite3");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE items(id INTEGER PRIMARY KEY, payload TEXT); INSERT INTO items VALUES(1, 'keep');").unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let store = fixture.open();
        assert!(!store.is_available());
        drop(store);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn path_as_file_is_unavailable_instead_of_panicking() {
        let fixture = Fixture::new();
        let path = fixture.0.join("not-a-directory");
        std::fs::write(&path, b"keep").unwrap();
        let result = std::panic::catch_unwind(|| ClipboardStore::open(path.clone()));
        assert!(result.is_ok(), "open failures must not panic");
        assert!(!result.as_ref().unwrap().is_available());
        drop(result);
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }

    #[test]
    fn locked_open_preserves_database_and_does_not_panic() {
        let fixture = Fixture::new();
        {
            let mut store = fixture.open();
            store.insert_text("keep under lock".into());
        }
        let path = fixture.0.join("clipboard.sqlite3");
        let lock = Connection::open(&path).unwrap();
        lock.execute_batch("PRAGMA locking_mode = EXCLUSIVE; BEGIN EXCLUSIVE;")
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        let result = std::panic::catch_unwind(|| fixture.open());
        assert!(result.is_ok(), "locked databases must not be recreated");
        assert!(!result.as_ref().unwrap().is_available());
        drop(result);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        lock.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn failed_reinsert_preserves_row_order_pin_and_fts() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let id = store.insert_text("original alpha".into()).unwrap();
        store.toggle_pin(&id);
        store.insert_text("second beta".into());
        let before = store.search("", ClipboardFilter::All);
        connection(&store).execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON items BEGIN SELECT RAISE(FAIL, 'injected insert failure'); END;").unwrap();
        store.toggle_pin(&id);
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_eq!(
            store.search("alpha", ClipboardFilter::All),
            vec![before[0].clone()]
        );
        store.reload();
        assert_eq!(store.search("", ClipboardFilter::All), before);
    }

    #[test]
    fn failed_reinsert_delete_rolls_back_trigger_side_effects() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let id = store.insert_text("original".into()).unwrap();
        store.insert_text("newer".into());
        let before = store.search("", ClipboardFilter::All);
        fail_deletes(&store);
        store.promote(&id);
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_delete_rolled_back(&store);
    }

    #[test]
    fn failed_clear_retains_rows_cache_and_images() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let image = fixture.image("keep.png");
        store.insert_image(image.clone());
        let before = store.search("", ClipboardFilter::All);
        fail_deletes(&store);
        store.clear();
        assert!(image.exists());
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_delete_rolled_back(&store);
    }

    #[test]
    fn failed_prune_retains_rows_cache_and_images() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let image = fixture.image("keep.png");
        store.insert_image(image.clone());
        connection(&store)
            .execute("UPDATE items SET created_at = 0", [])
            .unwrap();
        store.reload();
        let before = store.search("", ClipboardFilter::All);
        fail_deletes(&store);
        store.prune_unpinned_older_than(1);
        assert!(image.exists());
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_delete_rolled_back(&store);
    }

    #[test]
    fn successful_prune_only_removes_unreferenced_owned_image_children() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let stale = fixture.image("stale.png");
        let pinned = fixture.image("pinned.png");
        let untracked = fixture.image("untracked.png");
        let outside = fixture.0.join("outside.png");
        let traversal_target = fixture.0.join("traversal.png");
        std::fs::write(&outside, b"outside").unwrap();
        std::fs::write(&traversal_target, b"outside").unwrap();
        store.insert_image(stale.clone());
        let pin_id = store.insert_image(pinned.clone()).unwrap();
        store.toggle_pin(&pin_id);
        store.insert_image(pinned.clone()); // the stale row must not delete a pin's image
        store.insert_image(outside.clone());
        store.insert_image(store.images_dir().join("..").join("traversal.png"));
        connection(&store)
            .execute("UPDATE items SET created_at = 0", [])
            .unwrap();
        store.reload();
        store.prune_unpinned_older_than(1);
        assert!(!stale.exists());
        for path in [pinned, untracked, outside, traversal_target] {
            assert!(path.exists(), "must retain {}", path.display());
        }
        let rows = store.search("", ClipboardFilter::All);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, pin_id);
    }

    #[test]
    fn successful_clear_leaves_untracked_and_outside_files_alone() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let owned = fixture.image("owned.png");
        let untracked = fixture.image("untracked.png");
        let outside = fixture.0.join("outside.png");
        std::fs::write(&outside, b"outside").unwrap();
        store.insert_image(owned.clone());
        store.insert_image(outside.clone());
        store.clear();
        assert!(!owned.exists());
        assert!(untracked.exists());
        assert!(outside.exists());
        assert!(store.search("", ClipboardFilter::All).is_empty());
    }

    #[test]
    fn healthy_database_reopens_with_pins_and_fts_intact() {
        let fixture = Fixture::new();
        let before = {
            let mut store = fixture.open();
            let id = store.insert_text("persisted alpha".into()).unwrap();
            store.toggle_pin(&id);
            store.insert_text("persisted beta".into()).unwrap();
            store.search("", ClipboardFilter::All)
        };
        let store = fixture.open();
        assert!(store.is_available(), "{:?}", store.last_error());
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_eq!(
            store.search("beta", ClipboardFilter::All),
            vec![before[1].clone()]
        );
    }

    #[test]
    fn healthy_database_reopens_with_uncheckpointed_wal_content() {
        let fixture = Fixture::new();
        let before = {
            let mut store = fixture.open();
            connection(&store)
                .set_db_config(
                    rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                    true,
                )
                .unwrap();
            store.insert_text("persisted only in WAL".into()).unwrap();
            store.search("", ClipboardFilter::All)
        };
        assert!(fixture.0.join("clipboard.sqlite3-wal").exists());
        let mut store = fixture.open();
        assert!(store.is_available(), "{:?}", store.last_error());
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_eq!(store.search("WAL", ClipboardFilter::All), before);
        assert!(store.insert_text("new capture".into()).is_some());
    }

    #[test]
    fn existing_rollback_journal_is_preserved_without_recovery() {
        let fixture = Fixture::new();
        drop(fixture.open());
        let path = fixture.0.join("clipboard.sqlite3");
        let journal = fixture.0.join("clipboard.sqlite3-journal");
        std::fs::write(&journal, b"original journal fixture").unwrap();
        let before = std::fs::read(&path).unwrap();
        let store = fixture.open();
        assert!(!store.is_available());
        drop(store);
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert_eq!(std::fs::read(journal).unwrap(), b"original journal fixture");
    }

    #[test]
    fn unsupported_version_in_wal_preserves_all_database_bytes() {
        let fixture = Fixture::new();
        drop(fixture.open());
        let path = fixture.0.join("clipboard.sqlite3");
        {
            let conn = Connection::open(&path).unwrap();
            conn.set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                true,
            )
            .unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let paths: Vec<_> = std::fs::read_dir(&fixture.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .collect();
        assert!(paths.iter().any(|p| p.to_string_lossy().ends_with("-wal")));
        let before: Vec<_> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
        let store = fixture.open();
        assert!(!store.is_available());
        assert!(
            store
                .last_error()
                .unwrap()
                .contains("Unsupported clipboard database schema"),
            "{:?}",
            store.last_error()
        );
        drop(store);
        for (path, original) in paths.iter().zip(before) {
            assert!(
                std::fs::read(path).unwrap() == original,
                "changed {}",
                path.display()
            );
        }
    }

    #[test]
    fn incomplete_schema_and_orphaned_sidecars_are_not_initialized() {
        let fixture = Fixture::new();
        let sidecar = fixture.0.join("clipboard.sqlite3-wal");
        std::fs::write(&sidecar, b"orphaned data").unwrap();
        let mut store = fixture.open();
        assert!(!store.is_available());
        assert!(store.insert_text("ignored".into()).is_none());
        assert!(store.save_image(b"ignored png").is_none());
        store.clear();
        store.prune_unpinned_older_than(0);
        assert_eq!(std::fs::read(&sidecar).unwrap(), b"orphaned data");
        assert!(!fixture.0.join("clipboard.sqlite3").exists());
        assert!(!fixture.0.join("images").exists());
        drop(store);
        std::fs::remove_file(sidecar).unwrap(); // owned fixture, not a recovery path
        {
            let conn = Connection::open(fixture.0.join("clipboard.sqlite3")).unwrap();
            conn.execute_batch(
                "CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('keep');",
            )
            .unwrap();
        }
        let before = std::fs::read(fixture.0.join("clipboard.sqlite3")).unwrap();
        let store = fixture.open();
        assert!(!store.is_available());
        drop(store);
        assert!(std::fs::read(fixture.0.join("clipboard.sqlite3")).unwrap() == before);
    }

    #[test]
    fn failed_reload_retains_complete_cache() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        store.insert_text("valid cached text".into()).unwrap();
        let before = store.search("", ClipboardFilter::All);
        connection(&store)
            .execute("UPDATE items SET created_at = 'invalid integer'", [])
            .unwrap();
        store.reload();
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert!(store.last_error().is_some());
        assert!(!store.is_available());
        assert_eq!(store.search("cached", ClipboardFilter::All), before);
    }

    #[test]
    fn failed_image_insert_rolls_back_side_effects_and_removes_only_its_new_file() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        store.insert_text("existing".into()).unwrap();
        let keep = fixture.image("untracked.png");
        let before = store.search("", ClipboardFilter::All);
        connection(&store)
            .execute_batch(
                "CREATE TABLE attempts(value INTEGER);
            CREATE TRIGGER reject_insert BEFORE INSERT ON items BEGIN
              INSERT INTO attempts VALUES(1); SELECT RAISE(FAIL, 'injected insert failure'); END;",
            )
            .unwrap();
        assert!(store.save_image(b"encoded fixture png").is_none());
        assert_eq!(store.search("", ClipboardFilter::All), before);
        assert_eq!(std::fs::read_dir(store.images_dir()).unwrap().count(), 1);
        assert!(keep.exists());
        let attempts: i64 = connection(&store)
            .query_row("SELECT COUNT(*) FROM attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(attempts, 0);
        assert!(!store.is_available());
    }

    #[test]
    fn failed_pin_update_rolls_back_and_keeps_cache() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let id = store.insert_text("existing".into()).unwrap();
        let before = store.search("", ClipboardFilter::All);
        connection(&store)
            .execute_batch(
                "CREATE TABLE attempts(value INTEGER);
            CREATE TRIGGER reject_update BEFORE UPDATE ON items BEGIN
              INSERT INTO attempts VALUES(1); SELECT RAISE(FAIL, 'injected update failure'); END;",
            )
            .unwrap();
        store.toggle_pin(&id);
        assert_eq!(store.search("", ClipboardFilter::All), before);
        let attempts: i64 = connection(&store)
            .query_row("SELECT COUNT(*) FROM attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(attempts, 0);
    }

    #[test]
    fn commit_failure_keeps_rows_cache_and_images() {
        for prune in [false, true] {
            let fixture = Fixture::new();
            let mut store = fixture.open();
            let image = fixture.image("retained.png");
            store.insert_image(image.clone()).unwrap();
            connection(&store).execute_batch("UPDATE items SET created_at=0; PRAGMA foreign_keys=ON;
                CREATE TABLE parent(id INTEGER PRIMARY KEY);
                CREATE TABLE child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);
                CREATE TRIGGER reject_commit BEFORE DELETE ON items BEGIN INSERT INTO child VALUES(1); END;").unwrap();
            store.reload();
            let before = store.search("", ClipboardFilter::All);
            if prune {
                store.prune_unpinned_older_than(1);
            } else {
                store.clear();
            }
            assert!(!store.is_available());
            assert_eq!(store.search("", ClipboardFilter::All), before);
            assert!(image.exists());
            store.reload();
            assert_eq!(store.search("", ClipboardFilter::All), before);
            let children: i64 = connection(&store)
                .query_row("SELECT COUNT(*) FROM child", [], |r| r.get(0))
                .unwrap();
            assert_eq!(children, 0);
        }
    }

    #[test]
    fn ignored_deletion_does_not_delete_its_image() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let image = fixture.image("keep.png");
        let id = store.insert_image(image.clone()).unwrap();
        connection(&store).execute_batch("CREATE TRIGGER ignore_delete BEFORE DELETE ON items BEGIN SELECT RAISE(IGNORE); END;").unwrap();
        store.clear();
        assert!(image.exists());
        assert_eq!(store.search("", ClipboardFilter::All)[0].id, id);
    }

    #[test]
    fn invalid_image_directory_is_never_recreated_or_removed() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        store.insert_text("existing".into()).unwrap();
        let images = store.images_dir();
        std::fs::remove_dir(&images).unwrap(); // empty, owned fixture only
        std::fs::write(&images, b"not an image directory").unwrap();
        assert!(store.save_image(b"image").is_none());
        store.clear();
        assert_eq!(std::fs::read(&images).unwrap(), b"not an image directory");
        drop(store);
        let store = fixture.open();
        assert!(!store.is_available());
        assert_eq!(std::fs::read(&images).unwrap(), b"not an image directory");
    }

    fn texts(store: &ClipboardStore) -> Vec<String> {
        store
            .search("", ClipboardFilter::All)
            .into_iter()
            .filter_map(|i| i.text)
            .collect()
    }

    fn item<'a>(store: &'a ClipboardStore, text: &str) -> ClipboardItem {
        store
            .search("", ClipboardFilter::All)
            .into_iter()
            .find(|i| i.text.as_deref() == Some(text))
            .unwrap_or_else(|| panic!("missing {text}"))
    }

    fn seed_texts(store: &mut ClipboardStore, count: usize, prefix: &str) -> Vec<String> {
        let now = unix_now();
        let mut ids = Vec::with_capacity(count);
        if let Ok(tx) = connection(store).unchecked_transaction() {
            for i in 0..count {
                let id = new_id();
                let text = format!("{prefix}{i}");
                tx.execute(
                    "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                     VALUES (?1, ?2, NULL, 'text', ?3, NULL)",
                    params![id, now, text],
                )
                .unwrap();
                ids.push(id);
            }
            tx.commit().unwrap();
        }
        store.reload();
        ids
    }

    #[test]
    fn pins_lead_and_short_query_does_not_use_fts_requirement() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        let a = s.insert_text("alpha".into()).unwrap();
        let _b = s.insert_text("beta".into());
        s.toggle_pin(&a);
        let rows = s.search("", ClipboardFilter::All);
        assert_eq!(rows[0].id, a);
        let short = s.search("al", ClipboardFilter::All);
        assert_eq!(short[0].id, a);
    }

    #[test]
    fn oldest_pin_stays_on_top() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        let a = s.insert_text("oldest".into()).unwrap();
        let b = s.insert_text("middle".into()).unwrap();
        let c = s.insert_text("newest".into()).unwrap();
        s.toggle_pin(&a);
        s.toggle_pin(&b);
        s.toggle_pin(&c);
        let rows = s.search("", ClipboardFilter::All);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec![a.as_str(), b.as_str(), c.as_str()]
        );
        s.prune_unpinned_older_than(-1);
        s.promote(&c);
        let rows = s.search("", ClipboardFilter::All);
        assert_eq!(rows[0].id, a, "promote skips pinned rows");
        s.clear();
        assert!(s.search("", ClipboardFilter::All).is_empty());
    }

    #[test]
    fn type_filter_splits_text_and_links() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        s.insert_text("just some prose".into());
        s.insert_text("https://example.com".into());
        s.insert_text("hi@example.com".into());
        assert_eq!(s.search("", ClipboardFilter::Text).len(), 1);
        assert_eq!(s.search("", ClipboardFilter::Links).len(), 1);
        assert_eq!(s.search("", ClipboardFilter::Emails).len(), 1);
        assert!(s.search("", ClipboardFilter::Images).is_empty());
        let png = s.images_dir().join("t.png");
        let _ = std::fs::write(&png, b"png");
        s.insert_image(png);
        assert_eq!(s.search("", ClipboardFilter::Images).len(), 1);
    }

    #[test]
    fn unpin_rejoins_as_newest() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        s.insert_text("a".into());
        s.insert_text("b".into());
        s.insert_text("c".into());
        let before = item(&s, "a").created_at;

        s.toggle_pin(&item(&s, "a").id);
        s.toggle_pin(&item(&s, "a").id);

        assert_eq!(texts(&s), vec!["a", "c", "b"]);
        let a = item(&s, "a");
        assert!(!a.is_pinned(), "pin stamp cleared");
        assert!(a.created_at > before, "unpin re-recencies the row");
    }

    #[test]
    fn unpinned_promote_heads_unpinned_block() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        s.insert_text("one".into());
        s.insert_text("two".into());
        s.toggle_pin(&item(&s, "one").id);
        s.toggle_pin(&item(&s, "two").id);
        let stamp = item(&s, "one").created_at;

        s.promote(&item(&s, "one").id);
        assert_eq!(texts(&s), vec!["one", "two"]);
        assert_eq!(item(&s, "one").created_at, stamp);

        s.insert_text("three".into());
        s.insert_text("four".into());
        s.promote(&item(&s, "three").id);
        assert_eq!(texts(&s), vec!["one", "two", "three", "four"]);
        let unpinned: Vec<_> = s
            .search("", ClipboardFilter::All)
            .into_iter()
            .filter(|i| !i.is_pinned())
            .filter_map(|i| i.text)
            .collect();
        assert_eq!(unpinned, vec!["three", "four"]);
    }

    #[test]
    fn fts_trigram_matches_three_char_prefix() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        s.insert_text("alpha".into());
        s.insert_text("beta".into());
        let rows = s.search("alp", ClipboardFilter::All);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text.as_deref(), Some("alpha"));

        // Evict alpha from the 1000-unpinned window so the short-query fallback cannot see it.
        let alpha = item(&s, "alpha");
        seed_texts(&mut s, MEMORY_WINDOW as usize, "zzz");
        let window = s.search("", ClipboardFilter::All);
        assert!(
            window.iter().all(|i| i.id != alpha.id),
            "alpha is below the memory window"
        );
        let fts = s.search("alp", ClipboardFilter::All);
        assert_eq!(fts.iter().filter(|i| i.id == alpha.id).count(), 1);
    }

    #[test]
    fn promote_and_unpin_below_window_floor_lead_unpinned_block() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        let oldest = s.insert_text("ancient".into()).unwrap();
        seed_texts(&mut s, MEMORY_WINDOW as usize, "n");
        assert!(
            s.search("", ClipboardFilter::All)
                .iter()
                .all(|i| i.id != oldest),
            "oldest is below the unpinned floor"
        );

        s.promote(&oldest);
        let unpinned: Vec<_> = s
            .search("", ClipboardFilter::All)
            .into_iter()
            .filter(|i| !i.is_pinned())
            .collect();
        assert_eq!(unpinned[0].id, oldest);
        assert_eq!(unpinned[0].text.as_deref(), Some("ancient"));

        let buried = s.insert_text("buried-pin".into()).unwrap();
        seed_texts(&mut s, MEMORY_WINDOW as usize, "m");
        s.toggle_pin(&buried);
        assert!(s.find(&buried).unwrap().is_pinned());
        s.toggle_pin(&buried);
        let unpinned: Vec<_> = s
            .search("", ClipboardFilter::All)
            .into_iter()
            .filter(|i| !i.is_pinned())
            .collect();
        assert_eq!(unpinned[0].id, buried);
        assert!(!unpinned[0].is_pinned());
    }

    #[test]
    fn insert_prunes_unpinned_but_not_pins() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        s.set_max_age_secs(86_400);
        let old = unix_now() - 2 * 86_400;
        let pin_id = new_id();
        let loose_id = new_id();
        connection(&s)
            .execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, ?3, 'text', 'ancient-pinned', NULL)",
                params![pin_id, old, old],
            )
            .unwrap();
        connection(&s)
            .execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, NULL, 'text', 'ancient-loose', NULL)",
                params![loose_id, old],
            )
            .unwrap();
        s.reload();
        s.insert_text("fresh".into());
        let found: Vec<_> = texts(&s);
        assert!(found.contains(&"ancient-pinned".to_string()));
        assert!(found.contains(&"fresh".to_string()));
        assert!(!found.contains(&"ancient-loose".to_string()));
    }

    #[test]
    fn pinning_moves_row_index_into_pin_block() {
        let fixture = Fixture::new();
        let mut s = fixture.open();
        s.insert_text("a".into());
        s.insert_text("b".into());
        s.insert_text("c".into());
        let a = item(&s, "a");
        assert_eq!(s.row_index(&a.id, "", ClipboardFilter::All), Some(2));
        s.toggle_pin(&a.id);
        assert_eq!(s.row_index(&a.id, "", ClipboardFilter::All), Some(0));
    }
}
