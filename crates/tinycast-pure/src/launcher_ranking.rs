use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

const FREQUENCY_SCALE: f64 = 600.0;
const FREQUENCY_CAP: f64 = 3_000.0;
const RECENCY_WEIGHT: f64 = 1_500.0;
const RECENCY_TAU_DAYS: f64 = 14.0;
const SECONDS_PER_DAY: f64 = 86_400.0;
const RECORD_CAP: usize = 1_000;
const PREFIX_LIMIT: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RankingRecord {
    #[serde(rename = "itemKey")]
    item_key: String,
    query: String,
    count: u32,
    #[serde(rename = "lastUsed")]
    last_used: i64,
}

/// On-device frecency table keyed by normalized query prefix × entry id.
pub struct LauncherRankingStore {
    path: PathBuf,
    /// query → entry_id → record
    records: HashMap<String, HashMap<String, RankingRecord>>,
}

impl LauncherRankingStore {
    pub const MAXIMUM_BOOST: i32 = 4_500;

    pub fn load(path: PathBuf) -> Self {
        Self {
            records: load_records(&path),
            path,
        }
    }

    pub fn record(&mut self, query: &str, entry_id: &str, now: i64) {
        let query = normalize(query);
        if query.is_empty() || entry_id.is_empty() {
            return;
        }
        for prefix in prefixes(&query) {
            let by_item = self.records.entry(prefix.clone()).or_default();
            if let Some(slot) = by_item.get_mut(entry_id) {
                slot.count = slot.count.saturating_add(1);
                slot.last_used = now;
            } else {
                by_item.insert(
                    entry_id.to_string(),
                    RankingRecord {
                        item_key: entry_id.to_string(),
                        query: prefix,
                        count: 1,
                        last_used: now,
                    },
                );
            }
        }
        self.evict_if_needed();
    }

    pub fn boost(&self, query: &str, entry_id: &str, now: i64) -> i32 {
        let query = normalize(query);
        if query.is_empty() || entry_id.is_empty() {
            return 0;
        }
        self.records
            .get(&query)
            .and_then(|by_item| by_item.get(entry_id))
            .map(|record| score(record, now))
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn reset_all(&mut self) {
        self.records.clear();
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut records = flatten(&self.records);
        records.sort_by(|a, b| a.query.cmp(&b.query).then(a.item_key.cmp(&b.item_key)));
        let data = serde_json::to_vec(&records)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.path, data)
    }

    fn evict_if_needed(&mut self) {
        let n = self.records.values().map(|m| m.len()).sum::<usize>();
        if n <= RECORD_CAP {
            return;
        }
        let mut all = flatten(&self.records);
        all.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_used.cmp(&a.last_used)));
        all.truncate(RECORD_CAP);
        self.records = rebuild(all);
    }
}

/// Empty-query favorite slots and category listings must not train frecency.
pub fn should_record_ranking(query: &str, category_listing: bool) -> bool {
    !query.is_empty() && !category_listing
}

fn score(record: &RankingRecord, now: i64) -> i32 {
    let age_secs = now.saturating_sub(record.last_used).max(0) as f64;
    let age_in_days = age_secs / SECONDS_PER_DAY;
    let frequency = FREQUENCY_CAP.min((f64::from(record.count) + 1.0).log2() * FREQUENCY_SCALE);
    let recency = RECENCY_WEIGHT * (-age_in_days / RECENCY_TAU_DAYS).exp();
    let blended = (frequency + recency).round() as i32;
    blended.clamp(0, LauncherRankingStore::MAXIMUM_BOOST)
}

fn normalize(query: &str) -> String {
    query
        .trim()
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

fn prefixes(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut acc = String::new();
    for ch in query.chars().take(PREFIX_LIMIT) {
        acc.push(ch);
        out.push(acc.clone());
    }
    out
}

fn flatten(records: &HashMap<String, HashMap<String, RankingRecord>>) -> Vec<RankingRecord> {
    records
        .values()
        .flat_map(|by_item| by_item.values().cloned())
        .collect()
}

fn rebuild(records: Vec<RankingRecord>) -> HashMap<String, HashMap<String, RankingRecord>> {
    let mut map = HashMap::new();
    for rec in records {
        map.entry(rec.query.clone())
            .or_insert_with(HashMap::new)
            .insert(rec.item_key.clone(), rec);
    }
    map
}

fn load_records(path: &Path) -> HashMap<String, HashMap<String, RankingRecord>> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(_) => return HashMap::new(),
    };
    let decoded: Vec<RankingRecord> = match serde_json::from_slice(&data) {
        Ok(decoded) => decoded,
        Err(_) => return HashMap::new(),
    };
    rebuild(
        decoded
            .into_iter()
            .filter(|r| !r.item_key.is_empty() && !r.query.is_empty() && r.count > 0)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize, should_record_ranking, LauncherRankingStore};
    use std::path::PathBuf;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tc-rank-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn load_temp(label: &str) -> (LauncherRankingStore, PathBuf) {
        let path = temp_path(label);
        let _ = std::fs::remove_file(&path);
        (LauncherRankingStore::load(path.clone()), path)
    }

    #[test]
    fn empty_query_does_not_record_ranking_on_activate_policy() {
        assert!(!should_record_ranking("", false));
        assert!(should_record_ranking("not", false));
        assert!(!should_record_ranking("Applications", true));
    }

    #[test]
    fn prefixes_are_recorded_and_boost_is_capped() {
        let dir = std::env::temp_dir().join("tc-rank-test.json");
        let _ = std::fs::remove_file(&dir);
        let mut s = LauncherRankingStore::load(dir);
        s.record("wha", "app:whatsapp", 1_000);
        assert!(s.boost("w", "app:whatsapp", 1_000) > 0);
        assert!(s.boost("wha", "app:whatsapp", 1_000) <= LauncherRankingStore::MAXIMUM_BOOST);
    }

    #[test]
    fn query_key_normalizes_trim_case_and_diacritics() {
        assert_eq!(normalize(" wha \n"), "wha");
        assert_eq!(normalize("WhA"), "wha");
        assert_eq!(normalize("Café"), "cafe");
        assert_eq!(normalize("I"), "i");
    }

    #[test]
    fn frecency_curve_matches_golden_values() {
        let (mut s, path) = load_temp("golden");
        let t0: i64 = 2_000_000_000;
        let whats_app = "net.whatsapp.WhatsApp";
        let wick = "com.example.wick";

        assert_eq!(s.boost("w", whats_app, t0), 0);
        s.record("Wha", whats_app, t0);
        assert_eq!(s.boost("w", whats_app, t0), 2_100);
        assert!(s.boost("WHA", whats_app, t0) > 0);
        assert_eq!(s.boost("wa", whats_app, t0), 0);

        for _ in 0..9 {
            s.record("Wha", whats_app, t0);
        }
        assert_eq!(s.boost("w", whats_app, t0), 3_576);
        assert_eq!(s.boost("w", whats_app, t0 + 14 * 86_400), 2_627);
        assert_eq!(s.boost("w", whats_app, t0 + 365 * 86_400), 2_076);

        for _ in 0..200 {
            s.record("w", wick, t0);
        }
        assert_eq!(s.boost("w", wick, t0), 4_500);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frequency_rises_and_recency_decays() {
        let (mut s, path) = load_temp("decay");
        let t0: i64 = 2_000_000_000;
        s.record("Wha", "app:whatsapp", t0);
        let same_day = s.boost("w", "app:whatsapp", t0);
        s.record("Wha", "app:whatsapp", t0);
        assert!(s.boost("w", "app:whatsapp", t0) > same_day);
        assert!(s.boost("w", "app:whatsapp", t0 + 60 * 86_400) < same_day);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_query_record_is_a_noop() {
        let (mut s, path) = load_temp("empty");
        s.record("", "app:whatsapp", 1_000);
        s.record("   ", "app:whatsapp", 1_000);
        assert_eq!(s.boost("w", "app:whatsapp", 1_000), 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reset_all_clears_learned_ranking() {
        let (mut s, path) = load_temp("reset");
        assert!(s.is_empty());
        s.record("wha", "app:whatsapp", 1_000);
        assert!(!s.is_empty());
        s.reset_all();
        assert!(s.is_empty());
        assert_eq!(s.boost("w", "app:whatsapp", 1_000), 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn records_persist_across_load() {
        let (mut s, path) = load_temp("persist");
        s.record("wha", "app:whatsapp", 1_000);
        let stored = s.boost("w", "app:whatsapp", 1_000);
        s.save().unwrap();
        let reloaded = LauncherRankingStore::load(path.clone());
        assert_eq!(reloaded.boost("w", "app:whatsapp", 1_000), stored);
        assert_eq!(reloaded.boost("wh", "app:whatsapp", 1_000), stored);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn learned_query_is_recalled_through_normalized_key() {
        let (mut s, path) = load_temp("cafe");
        s.record(" Café ", "com.example.cafe", 1_000);
        assert!(s.boost("cafe", "com.example.cafe", 1_000) > 0);
        let _ = std::fs::remove_file(path);
    }
}
