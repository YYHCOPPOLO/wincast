//! Builds the read-only ECDICT database the dictionary panel queries.
//!
//! `tools/build-dict.ps1` drives it, so this example owns only the file format:
//! the CSV is streamed once, rows land in a single transaction, and the index is
//! created after the load instead of being maintained row by row.
//!
//! ```text
//! cargo run -p tinycast --example build_ecdict -- --input ecdict.csv --output ecdict.sqlite
//! ```

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::Connection;
use tinycast_pure::dictionary::{search_key, SCHEMA};

/// ECDICT column order: `word,phonetic,definition,translation,pos,collins,oxford,tag,bnc,frq,exchange,detail,audio`.
const INSERT: &str = "\
INSERT OR REPLACE INTO words
  (word, sw, phonetic, definition, translation, pos, collins, oxford, tag, bnc, frq, exchange, detail, audio)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";

const USAGE: &str =
    "usage: cargo run -p tinycast --example build_ecdict -- --input <ecdict.csv> --output <ecdict.sqlite>";

fn main() -> Result<(), Box<dyn Error>> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--input" | "-i" => input = args.next().map(PathBuf::from),
            "--output" | "-o" => output = args.next().map(PathBuf::from),
            other => return Err(format!("unknown argument: {other}\n{USAGE}").into()),
        }
    }
    let Some(input) = input else {
        return Err(USAGE.into());
    };
    let Some(output) = output else {
        return Err(USAGE.into());
    };

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    // The partial file is what a crashed build leaves behind; it is never the
    // database the app opens, so a stale one is simply replaced.
    let part = part_path(&output);
    let _ = fs::remove_file(&part);

    let started = Instant::now();
    let mut connection = Connection::open(&part)?;
    // A one-shot bulk load: no journal to keep, no per-row fsync.
    connection.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA page_size=8192;")?;
    apply_schema(&connection, false)?;

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(&input)?;
    let mut inserted = 0u64;
    {
        let transaction = connection.transaction()?;
        {
            let mut statement = transaction.prepare(INSERT)?;
            for record in reader.records() {
                let record = record?;
                let word = record.get(0).unwrap_or("").trim();
                // A blank headword cannot be looked up; everything else stays,
                // including zero-frequency phrases.
                if word.is_empty() {
                    continue;
                }
                statement.execute(rusqlite::params![
                    word,
                    search_key(word),
                    record.get(1).unwrap_or("").trim(),
                    record.get(2).unwrap_or("").trim(),
                    record.get(3).unwrap_or("").trim(),
                    record.get(4).unwrap_or("").trim(),
                    number(record.get(5)),
                    number(record.get(6)),
                    record.get(7).unwrap_or("").trim(),
                    number(record.get(8)),
                    number(record.get(9)),
                    record.get(10).unwrap_or("").trim(),
                    record.get(11).unwrap_or("").trim(),
                    record.get(12).unwrap_or("").trim(),
                ])?;
                inserted += 1;
            }
        }
        transaction.commit()?;
    }
    apply_schema(&connection, true)?;
    connection.execute_batch("VACUUM;")?;
    drop(connection);

    // Rename last, so the app either sees the old database or the new one,
    // never a half-written file.
    fs::remove_file(&output).ok();
    fs::rename(&part, &output)?;

    let size = fs::metadata(&output)?.len();
    println!(
        "wrote {inserted} entries to {} ({:.1} MB) in {:.1?}",
        output.display(),
        size as f64 / (1024.0 * 1024.0),
        started.elapsed()
    );
    Ok(())
}

/// Applies every statement in the shared `SCHEMA`. With `index = false` the
/// `CREATE INDEX` statement is skipped and `apply_schema(&connection, true)`
/// builds it once after the load.
fn apply_schema(connection: &Connection, index: bool) -> rusqlite::Result<()> {
    for statement in SCHEMA.split(';') {
        let statement = statement.trim();
        if statement.is_empty() || statement.starts_with("CREATE INDEX") != index {
            continue;
        }
        connection.execute_batch(statement)?;
    }
    Ok(())
}

fn part_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

fn number(raw: Option<&str>) -> i64 {
    raw.and_then(|value| value.trim().parse().ok()).unwrap_or(0)
}
