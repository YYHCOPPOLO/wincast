//! SQLite clipboard history with pins and FTS5 trigram search.
//!
//! Stable `id` is a unique text UUID. Recency is implicit AUTOINCREMENT `rowid`.
//! `promote` / unpin delete+insert without binding the old rowid so the row leads.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
    conn: Connection,
    dir: PathBuf,
    items: Vec<ClipboardItem>,
    max_age_secs: i64,
}

impl ClipboardStore {
    pub fn open(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join("images"));
        let db_path = dir.join("clipboard.sqlite3");
        let conn = match try_open(&db_path) {
            Some(conn) => conn,
            None => {
                delete_db(&db_path);
                open_db(&db_path).expect("recreate clipboard db")
            }
        };
        let mut store = Self {
            conn,
            dir,
            items: Vec::new(),
            max_age_secs: -1,
        };
        store.reload();
        store
    }

    pub fn images_dir(&self) -> PathBuf {
        self.dir.join("images")
    }

    pub fn set_max_age_secs(&mut self, max_age_secs: i64) {
        self.max_age_secs = max_age_secs;
    }

    pub fn insert_text(&mut self, text: String) -> String {
        if let Some(first) = self.items.first() {
            if first.kind == ClipKind::Text && first.text.as_deref() == Some(text.as_str()) {
                return first.id.clone();
            }
        }
        let now = unix_now();
        let id = new_id();
        self.conn
            .execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, NULL, 'text', ?3, NULL)",
                params![id, now, text],
            )
            .ok();
        self.after_insert();
        id
    }

    pub fn insert_image(&mut self, png: PathBuf) -> String {
        let now = unix_now();
        let id = new_id();
        let path = png.to_string_lossy();
        self.conn
            .execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, NULL, 'image', NULL, ?3)",
                params![id, now, path.as_ref()],
            )
            .ok();
        self.after_insert();
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
            let _ = self.conn.execute(
                "UPDATE items SET pinned_at = ?1 WHERE id = ?2",
                params![stamp, id],
            );
            self.reload();
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
        let stale: Vec<(String, Option<String>)> = self
            .conn
            .prepare("SELECT id, image_path FROM items WHERE pinned_at IS NULL AND created_at < ?1")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![cutoff], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
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

    fn after_insert(&mut self) {
        if self.max_age_secs >= 0 {
            self.prune_unpinned_older_than(self.max_age_secs);
        } else {
            self.reload();
        }
    }

    fn find(&self, id: &str) -> Option<ClipboardItem> {
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
        let kind = match item.kind {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
        };
        let path = item
            .image_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        if let Ok(tx) = self.conn.unchecked_transaction() {
            let _ = tx.execute("DELETE FROM items WHERE id = ?1", params![item.id]);
            let _ = tx.execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![item.id, created_at, pinned_at, kind, item.text, path],
            );
            let _ = tx.commit();
        }
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
                 JOIN items_fts ON items.rowid = items_fts.rowid
                 WHERE items_fts MATCH ?1 AND items.pinned_at IS NULL
                 ORDER BY items.rowid DESC
                 LIMIT ?2",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![match_expr, FTS_LIMIT], row_item)
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_else(|| self.fallback_search(query));
        let pin_ids: std::collections::HashSet<String> =
            pins.iter().map(|i| i.id.clone()).collect();
        pins.extend(fts.into_iter().filter(|i| !pin_ids.contains(&i.id)));
        pins
    }

    fn reload(&mut self) {
        let floor = self.window_floor();
        let mut items = Vec::new();
        // Two indexed branches, not `pinned_at IS NOT NULL OR rowid >= ?`.
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, created_at, pinned_at, kind, text, image_path FROM (
               SELECT rowid AS rid, * FROM items WHERE rowid >= ?1
               UNION ALL
               SELECT rowid AS rid, * FROM items WHERE pinned_at IS NOT NULL AND rowid < ?1
             ) ORDER BY rid DESC",
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
                "SELECT rowid FROM items WHERE pinned_at IS NULL ORDER BY rowid DESC LIMIT 1 OFFSET ?1",
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

fn try_open(path: &Path) -> Option<Connection> {
    let conn = open_db(path).ok()?;
    if schema_ok(&conn) {
        Some(conn)
    } else {
        None
    }
}

fn schema_ok(conn: &Connection) -> bool {
    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='items'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();
    if sql.is_empty() {
        return true;
    }
    let lower = sql.to_lowercase();
    lower.contains("id text") && lower.contains("autoincrement")
}

fn open_db(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
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
        if let Ok(tx) = store.conn.unchecked_transaction() {
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
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
        let a = s.insert_text("alpha".into());
        let _b = s.insert_text("beta".into());
        s.toggle_pin(&a);
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

    #[test]
    fn unpin_rejoins_as_newest() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
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
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unpinned_promote_heads_unpinned_block() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
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
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fts_trigram_matches_three_char_prefix() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
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
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn promote_and_unpin_below_window_floor_lead_unpinned_block() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
        let oldest = s.insert_text("ancient".into());
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

        let buried = s.insert_text("buried-pin".into());
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
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn insert_prunes_unpinned_but_not_pins() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
        s.set_max_age_secs(86_400);
        let old = unix_now() - 2 * 86_400;
        let pin_id = new_id();
        let loose_id = new_id();
        s.conn
            .execute(
                "INSERT INTO items(id, created_at, pinned_at, kind, text, image_path)
                 VALUES (?1, ?2, ?3, 'text', 'ancient-pinned', NULL)",
                params![pin_id, old, old],
            )
            .unwrap();
        s.conn
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
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pinning_moves_row_index_into_pin_block() {
        let dir = temp_dir();
        let mut s = ClipboardStore::open(dir.clone());
        s.insert_text("a".into());
        s.insert_text("b".into());
        s.insert_text("c".into());
        let a = item(&s, "a");
        assert_eq!(s.row_index(&a.id, "", ClipboardFilter::All), Some(2));
        s.toggle_pin(&a.id);
        assert_eq!(s.row_index(&a.id, "", ClipboardFilter::All), Some(0));
        let _ = std::fs::remove_dir_all(dir);
    }
}
