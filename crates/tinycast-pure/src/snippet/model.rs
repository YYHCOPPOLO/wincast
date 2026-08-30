use std::path::{Path, PathBuf};

use crate::template::TemplateSnippet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnippetSourceRevision {
    value: String,
}

impl SnippetSourceRevision {
    pub fn new(content: &str) -> Self {
        let mut hash: u64 = 14_695_981_039_346_656_037;
        let mut byte_count: u64 = 0;
        for byte in content.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(1_099_511_628_211);
            byte_count += 1;
        }
        Self {
            value: format!("{byte_count}:{hash:x}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSnippet {
    pub path: PathBuf,
    pub name: String,
    pub keyword: Option<String>,
    pub enabled: bool,
    pub show_confirmation: bool,
    pub body: String,
    pub source_revision: SnippetSourceRevision,
}

impl StoredSnippet {
    pub fn identity(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    pub fn to_template(&self) -> TemplateSnippet {
        TemplateSnippet {
            id: self.identity(),
            name: self.name.clone(),
            keyword: self.keyword.clone(),
            enabled: self.enabled,
            body: self.body.clone(),
        }
    }
}

pub fn default_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("snippet");
    posix_capitalized(&stem.replace('-', " "))
}

pub fn slug(name: &str) -> String {
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                current.push(c);
            }
        } else if !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.is_empty() {
        "snippet".into()
    } else {
        parts.join("-")
    }
}

fn posix_capitalized(s: &str) -> String {
    let mut out = String::new();
    let mut new_word = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            out.push(ch);
            new_word = true;
        } else if new_word {
            for c in ch.to_uppercase() {
                out.push(c);
            }
            new_word = false;
        } else {
            for c in ch.to_lowercase() {
                out.push(c);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_is_deterministic_and_content_sensitive() {
        assert_eq!(
            SnippetSourceRevision::new("same"),
            SnippetSourceRevision::new("same")
        );
        assert_ne!(
            SnippetSourceRevision::new("same"),
            SnippetSourceRevision::new("same\n")
        );
    }

    #[test]
    fn filename_fallback_is_posix_capitalized() {
        assert_eq!(
            default_name(Path::new("/tmp/body-only-name.md")),
            "Body Only Name"
        );
        assert_eq!(slug("Meeting Notes"), "meeting-notes");
        assert_eq!(slug("!!!"), "snippet");
    }
}
