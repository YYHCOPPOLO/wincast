//! The read-only ECDICT database behind the dictionary panel.
//!
//! The database is built once by `tools/build-dict.ps1` and only ever read
//! here: no schema changes, no writes, and no fallback download. When the file
//! is missing or unusable the store is `None` and the palette keeps its plain
//! Tab ring.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use tinycast_pure::dictionary::{search_key, DictionaryEntry};

use crate::platform::paths;

const COLUMNS: &str = "word, phonetic, definition, translation, pos, collins, oxford, tag, bnc, frq, exchange, detail, audio";

/// How ties inside one `sw` key break: learner dictionaries first, then the
/// more frequent word. A zero rank means "unranked", so it sorts last instead
/// of first.
const ORDER: &str =
    "ORDER BY oxford DESC, collins DESC, (frq <= 0), frq ASC, (bnc <= 0), bnc ASC, length(word)";

pub struct DictionaryStore {
    connection: Connection,
}

impl DictionaryStore {
    /// `%APPDATA%\com.tinycast.win\Dictionary\ecdict.sqlite`, or `None` when it
    /// is absent, not an ECDICT build, or held open for writing elsewhere.
    pub fn open_default() -> Option<Self> {
        Self::open(&paths::dictionary_dir().join("ecdict.sqlite")).ok()
    }

    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let connection =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.execute_batch("PRAGMA query_only=ON;")?;
        // A file that opens but has no `words` table is not a dictionary.
        connection.prepare("SELECT 1 FROM words LIMIT 1")?;
        Ok(Self { connection })
    }

    /// Exact headword first (ECDICT's `word` is `COLLATE NOCASE`), then the
    /// folded key: `Gave up!` still lands on `give up`.
    pub fn lookup(&self, query: &str) -> Option<DictionaryEntry> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        let sql = format!("SELECT {COLUMNS} FROM words WHERE word = ?1 COLLATE NOCASE LIMIT 1");
        if let Some(entry) = self.first(&sql, query) {
            return Some(entry);
        }
        let key = search_key(query);
        if key.is_empty() {
            return None;
        }
        let sql = format!("SELECT {COLUMNS} FROM words WHERE sw = ?1 {ORDER} LIMIT 1");
        self.first(&sql, &key)
    }

    /// Prefix neighbours for a miss, best first: the same ranking as `lookup`,
    /// so suggestions lead with the word a learner would have meant. A query
    /// past the last matching word (`zzzz`) backs off one character at a time
    /// until the prefix range is non-empty.
    pub fn suggest(&self, query: &str, limit: usize) -> Vec<String> {
        let query = query.trim();
        let mut prefix = search_key(query);
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        loop {
            let words = self.prefix_words(&prefix, query, limit);
            if !words.is_empty() || prefix.len() == 1 {
                return words;
            }
            prefix.pop();
            if prefix.is_empty() {
                return Vec::new();
            }
        }
    }

    fn prefix_words(&self, prefix: &str, query: &str, limit: usize) -> Vec<String> {
        // `sw` holds ASCII only, so the highest code point closes the range.
        let end = format!("{prefix}\u{10FFFF}");
        let sql = format!(
            "SELECT word FROM words WHERE sw >= ?1 AND sw < ?2 AND word <> ?3 COLLATE NOCASE {ORDER} LIMIT ?4"
        );
        let mut words = Vec::new();
        let Ok(mut statement) = self.connection.prepare_cached(&sql) else {
            return words;
        };
        let Ok(mut rows) = statement.query(rusqlite::params![prefix, end, query, limit]) else {
            return words;
        };
        while let Ok(Some(row)) = rows.next() {
            match row.get::<_, String>(0) {
                Ok(word) => words.push(word),
                Err(_) => break,
            }
        }
        words
    }

    fn first(&self, sql: &str, param: &str) -> Option<DictionaryEntry> {
        let mut statement = self.connection.prepare_cached(sql).ok()?;
        let mut rows = statement.query([param]).ok()?;
        let row = rows.next().ok()??;
        Some(DictionaryEntry {
            word: row.get(0).ok()?,
            phonetic: row.get(1).ok()?,
            definition: row.get(2).ok()?,
            translation: row.get(3).ok()?,
            pos: row.get(4).ok()?,
            collins: row.get(5).ok()?,
            oxford: row.get(6).ok()?,
            tag: row.get(7).ok()?,
            bnc: row.get(8).ok()?,
            frq: row.get(9).ok()?,
            exchange: row.get(10).ok()?,
            detail: row.get(11).ok()?,
            audio: row.get(12).ok()?,
        })
    }
}
