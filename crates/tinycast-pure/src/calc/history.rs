//! Capped newest-first calculator history. Persistence is JSON beside other roaming stores.

use std::path::Path;

const CAP: usize = 200;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalcHistoryEntry {
    pub expression: String,
    pub result: String,
}

#[derive(Clone, Debug, Default)]
pub struct CalculatorHistoryStore {
    /// Newest-first result strings. The Task 3 unit test indexes this field.
    pub items: Vec<String>,
    entries: Vec<CalcHistoryEntry>,
}

impl CalculatorHistoryStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let Ok(bytes) = std::fs::read(path.as_ref()) else {
            return Self::default();
        };
        let entries: Vec<CalcHistoryEntry> = serde_json::from_slice(&bytes).unwrap_or_default();
        Self::from_entries(entries)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_vec_pretty(&self.entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, data)
    }

    pub fn push(&mut self, item: String) {
        self.record(item.clone(), item);
    }

    pub fn record(&mut self, expression: String, result: String) {
        if self
            .entries
            .first()
            .is_some_and(|e| e.expression == expression && e.result == result)
        {
            return;
        }
        self.entries.insert(
            0,
            CalcHistoryEntry {
                expression,
                result: result.clone(),
            },
        );
        self.items.insert(0, result);
        if self.entries.len() > CAP {
            self.entries.truncate(CAP);
            self.items.truncate(CAP);
        }
    }

    pub fn entries(&self) -> &[CalcHistoryEntry] {
        &self.entries
    }

    pub fn search(&self, query: &str) -> Vec<CalcHistoryEntry> {
        let q = query.trim();
        if q.is_empty() {
            return self.entries.clone();
        }
        let needle = q.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.expression.to_lowercase().contains(&needle)
                    || e.result.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect()
    }

    fn from_entries(entries: Vec<CalcHistoryEntry>) -> Self {
        let items = entries.iter().map(|e| e.result.clone()).collect();
        Self { items, entries }
    }
}

/// Copy payload for a history row. Strip thousands grouping only when the
/// stored result is a grouped number (`1,024`), not dates (`Friday, 14 August`).
pub fn history_copy_payload(result: &str) -> String {
    if is_grouped_number(result) {
        result.replace(',', "")
    } else {
        result.to_string()
    }
}

fn is_grouped_number(s: &str) -> bool {
    let t = s.trim();
    t.contains(',') && t.replace(',', "").parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_caps_and_newest_first() {
        let mut h = CalculatorHistoryStore::default();
        for i in 0..20 {
            h.push(format!("{i}"));
        }
        assert_eq!(h.items[0], "19");
    }

    #[test]
    fn history_drops_oldest_past_cap() {
        let mut h = CalculatorHistoryStore::default();
        for i in 0..=CAP {
            h.push(format!("{i}"));
        }
        assert_eq!(h.items.len(), CAP);
        assert_eq!(h.items[0], CAP.to_string());
        assert_eq!(h.items.last().unwrap(), "1");
    }

    #[test]
    fn history_copy_strips_grouping_only_on_numeric_copy_text() {
        assert_eq!(history_copy_payload("1,024"), "1024");
        assert_eq!(history_copy_payload("-1,024.5"), "-1024.5");
        assert_eq!(
            history_copy_payload("Friday, 14 August"),
            "Friday, 14 August"
        );
        assert_eq!(
            history_copy_payload("Friday, 24 July at 1:48 AM"),
            "Friday, 24 July at 1:48 AM"
        );
    }
}
