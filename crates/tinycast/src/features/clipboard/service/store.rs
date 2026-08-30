//! SQLite clipboard history with pins and FTS5 trigram search.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
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
    pub id: i64,
    pub kind: ClipKind,
    pub text: Option<String>,
    pub image_path: Option<PathBuf>,
    pub pinned_at: Option<i64>,
}

impl ClipboardItem {
    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }
}

pub struct ClipboardStore {
    conn: Connection,
    dir: PathBuf,
    items: Vec<ClipboardItem>,
}

impl ClipboardStore {
    pub fn open(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join("images"));
        let db_path = dir.join("clipboard.sqlite3");
        let conn = match open_db(&db_path) {
            Ok(conn) => conn,
            Err(_) => {
                delete_db(&db_path);
                open_db(&db_path).expect("recreate clipboard db")
            }
        };
        let mut store = Self {
            conn,
            dir,
            items: Vec::new(),
        };
        store.reload();
        store
    }

    pub fn images_dir(&self) -> PathBuf {
        self.dir.join("images")
    }

    pub fn insert_text(&mut self, text: String) -> i64 {
        if let Some(first) = self.items.first() {
            if first.kind == ClipKind::Text && first.text.as_deref() == Some(text.as_str()) {
                return first.id;
            }
        }
        let now = unix_now();
        self.conn
            .execute(
                "INSERT INTO items(created_at, pinned_at, kind, text, image_path) VALUES (?1, NULL, 'text', ?2, NULL)",
                params![now, text],
            )
            .ok();
        let id = self.conn.last_insert_rowid();
        self.reload();
        id
    }

    pub fn insert_image(&mut self, png: PathBuf) -> i64 {
        let now = unix_now();
        let path = png.to_string_lossy();
        self.conn
            .execute(
                "INSERT INTO items(created_at, pinned_at, kind, text, image_path) VALUES (?1, NULL, 'image', NULL, ?2)",
                params![now, path.as_ref()],
            )
            .ok();
        let id = self.conn.last_insert_rowid();
        self.reload();
        id
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

    pub fn toggle_pin(&mut self, id: i64) {
        let Some(item) = self.find(id) else {
            return;
        };
        if item.is_pinned() {
            self.reinsert(item, unix_now(), None);
        } else {
            let stamp = next_pin_stamp(&self.items);
            let _ = self
                .conn
                .execute("UPDATE items SET pinned_at = ?1 WHERE id = ?2", params![stamp, id]);
            self.reload();
        }
    }

    pub fn promote(&mut self, id: i64) {
        let Some(item) = self.find(id) else {
            return;
        };
        if item.is_pinned() {
            return;
        }
        if self.items.first().map(|i| i.id) == Some(id) {
            return;
        }
        self.reinsert(item, unix_now(), None);
    }

    pub fn clear(&mut self) {
        let _ = self.conn.execute("DELETE FROM items", []);
        let images = self.images_dir();
        let _ = std::fs::remove_dir_all(&images);
        let _ = std::fs::create_dir_all(&images);
        self.items.clear();
    }

    pub fn prune_unpinned_older_than(&mut self, max_age_secs: i64) {
        if max_age_secs < 0 {
            return;
        }
        let cutoff = unix_now().saturating_sub(max_age_secs);
        let owned = self.images_dir();
        let stale: Vec<(i64, Option<String>)> = self
            .conn
            .prepare(
                "SELECT id, image_path FROM items WHERE pinned_at IS NULL AND created_at < ?1",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![cutoff], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
                })
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();
        let _ = self.conn.execute(
            "DELETE FROM items WHERE pinned_at IS NULL AND created_at < ?1",
            params![cutoff],
        );
        for (_, path) in stale {
            if let Some(path) = path {
                let p = PathBuf::from(path);
                if p.starts_with(&owned) {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        self.reload();
    }

    fn find(&self, id: i64) -> Option<ClipboardItem> {
        if let Some(item) = self.items.iter().find(|i| i.id == id).cloned() {
            return Some(item);
        }
        self.conn
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
        let _ = self.conn.execute_batch("BEGIN");
        let _ = self
            .conn
            .execute("DELETE FROM items WHERE id = ?1", params![item.id]);
        let kind = match item.kind {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
        };
        let path = item.image_path.as_ref().map(|p| p.to_string_lossy().into_owned());
        let _ = self.conn.execute(
            "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![item.id, created_at, pinned_at, kind, item.text, path],
        );
        let _ = self.conn.execute_batch("COMMIT");
        self.reload();
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
        let fts: Vec<ClipboardItem> = self
            .conn
            .prepare(
                "SELECT items.id, items.created_at, items.pinned_at, items.kind, items.text, items.image_path
                 FROM items
                 JOIN items_fts ON items.id = items_fts.rowid
                 WHERE items_fts MATCH ?1 AND items.pinned_at IS NULL
                 ORDER BY items.id DESC
                 LIMIT ?2",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![match_expr, FTS_LIMIT], row_item)
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_else(|| self.fallback_search(query));
        let pin_ids: std::collections::HashSet<i64> = pins.iter().map(|i| i.id).collect();
        pins.extend(fts.into_iter().filter(|i| !pin_ids.contains(&i.id)));
        pins
    }

    fn reload(&mut self) {
        let floor = self.window_floor();
        let mut items = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, created_at, pinned_at, kind, text, image_path FROM items
             WHERE pinned_at IS NOT NULL
                OR id >= ?1
             ORDER BY id DESC",
        ) {
            if let Ok(rows) = stmt.query_map(params![floor], row_item) {
                items.extend(rows.filter_map(|r| r.ok()));
            }
        }
        self.items = items;
    }

    fn window_floor(&self) -> i64 {
        self.conn
            .query_row(
                "SELECT id FROM items WHERE pinned_at IS NULL ORDER BY id DESC LIMIT 1 OFFSET ?1",
                params![MEMORY_WINDOW - 1],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0)
    }
}

fn apply_filter(rows: Vec<ClipboardItem>, filter: ClipboardFilter) -> Vec<ClipboardItem> {
    rows.into_iter()
        .filter(|item| match filter {
            ClipboardFilter::All => true,
            ClipboardFilter::Images => item.kind == ClipKind::Image,
            ClipboardFilter::Text => {
                item.kind == ClipKind::Text
                    && item
                        .text
                        .as_deref()
                        .map(text_form)
                        == Some(TextForm::Plain)
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
        kind,
        text: row.get(4)?,
        image_path: path.map(PathBuf::from),
        pinned_at: row.get(2)?,
    })
}

fn open_db(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS items(
          id INTEGER PRIMARY KEY,
          created_at INTEGER NOT NULL,
          pinned_at INTEGER,
          kind TEXT NOT NULL CHECK (kind IN ('text','image')),
          text TEXT,
          image_path TEXT
        );
        CREATE INDEX IF NOT EXISTS items_created_at ON items(created_at);
        CREATE INDEX IF NOT EXISTS items_pinned_at ON items(pinned_at) WHERE pinned_at IS NOT NULL;
        CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
          text, content='items', content_rowid='id', tokenize='trigram'
        );
        CREATE TRIGGER IF NOT EXISTS items_ai AFTER INSERT ON items BEGIN
          INSERT INTO items_fts(rowid, text) VALUES(new.id, new.text);
        END;
        CREATE TRIGGER IF NOT EXISTS items_ad AFTER DELETE ON items BEGIN
          INSERT INTO items_fts(items_fts, rowid, text) VALUES('delete', old.id, old.text);
        END;
        ",
    )?;
    Ok(conn)
}

fn delete_db(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
    let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    if let Some(s) = path.to_str() {
        let _ = std::fs::remove_file(format!("{s}-wal"));
        let _ = std::fs::remove_file(format!("{s}-shm"));
    }
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

    #[test]
    fn pins_lead_and_short_query_does_not_use_fts_requirement() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
        let a = s.insert_text("alpha".into());
        let _b = s.insert_text("beta".into());
        s.toggle_pin(a);
        let rows = s.search("", ClipboardFilter::All);
        assert_eq!(rows[0].id, a);
        let short = s.search("al", ClipboardFilter::All);
        assert_eq!(short[0].id, a);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn oldest_pin_stays_on_top() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
        let a = s.insert_text("oldest".into());
        let b = s.insert_text("middle".into());
        let c = s.insert_text("newest".into());
        s.toggle_pin(a);
        s.toggle_pin(b);
        s.toggle_pin(c);
        let rows = s.search("", ClipboardFilter::All);
        assert_eq!(
            rows.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![a, b, c]
        );
        s.prune_unpinned_older_than(-1);
        s.promote(c);
        let rows = s.search("", ClipboardFilter::All);
        assert_eq!(rows[0].id, a, "promote skips pinned rows");
        s.clear();
        assert!(s.search("", ClipboardFilter::All).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn type_filter_splits_text_and_links() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
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
        let _ = std::fs::remove_dir_all(dir);
    }
}
