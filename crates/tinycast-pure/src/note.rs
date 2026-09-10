use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub path: PathBuf,
    pub body: String,
}

impl Note {
    /// Identity is the relative filename. A rename therefore returns a new identity.
    pub fn id(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    pub fn title(&self) -> String {
        Path::new(&self.id())
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.id())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub modified_at: i64,
}

/// Empty query lists metadata only. A nonempty query filters filenames (and titles).
pub fn filter_filenames(summaries: &[NoteSummary], query: &str) -> Vec<NoteSummary> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    if terms.is_empty() {
        return summaries.to_vec();
    }
    summaries
        .iter()
        .filter(|s| {
            let hay = format!("{} {}", s.title, s.id).to_lowercase();
            terms.iter().all(|t| hay.contains(t))
        })
        .cloned()
        .take(200)
        .collect()
}

pub fn unique_untitled_name(occupied: &[String]) -> String {
    unique_name("Untitled", occupied)
}

pub fn unique_name(base: &str, occupied: &[String]) -> String {
    let folded: Vec<String> = occupied.iter().map(|s| s.to_lowercase()).collect();
    let mut suffix = 1u32;
    loop {
        let candidate = if suffix == 1 {
            format!("{base}.md")
        } else {
            format!("{base} {suffix}.md")
        };
        if !folded.iter().any(|s| s == &candidate.to_lowercase()) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_identity_is_path() {
        let n = Note {
            path: PathBuf::from("a.md"),
            body: "x".into(),
        };
        assert_eq!(n.id(), "a.md");
    }

    #[test]
    fn search_filters_filenames() {
        let notes = vec![
            NoteSummary {
                id: "alpha.md".into(),
                title: "alpha".into(),
                modified_at: 2,
            },
            NoteSummary {
                id: "beta.md".into(),
                title: "beta".into(),
                modified_at: 1,
            },
        ];
        let hits = filter_filenames(&notes, "alp");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "alpha.md");
        assert_eq!(filter_filenames(&notes, "").len(), 2);
    }

    #[test]
    fn untitled_skips_occupied() {
        assert_eq!(unique_untitled_name(&[]), "Untitled.md");
        assert_eq!(
            unique_untitled_name(&["Untitled.md".into()]),
            "Untitled 2.md"
        );
    }
}
