use unicode_normalization::UnicodeNormalization;

use crate::search_relevance::fuzzy::FuzzyMatch;

use super::ignore::IgnoreList;

pub const CANDIDATE_LIMIT: usize = 1_000;
pub const RESULT_LIMIT: usize = 200;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSearchHit {
    pub path: String,
    pub name: String,
    pub parent: String,
    pub is_directory: bool,
}

impl FileSearchHit {
    pub fn from_path(path: &str, is_directory: bool, home: &str) -> Self {
        let path = normalize_slashes(path);
        let name = filename(&path).to_string();
        let parent = abbreviate(parent_of(&path).as_str(), home);
        Self {
            path,
            name,
            parent,
            is_directory,
        }
    }

    pub fn id(&self) -> &str {
        &self.path
    }
}

pub fn tokens(q: &str) -> Vec<String> {
    q.split_whitespace().map(str::to_string).collect()
}

pub fn cap_candidates(n: usize) -> usize {
    n.min(CANDIDATE_LIMIT)
}

pub fn cap_rows(n: usize) -> usize {
    n.min(RESULT_LIMIT)
}

/// AND: every token must occur in the filename (case- and diacritic-insensitive).
pub fn matches_filename(filename: &str, query: &str) -> bool {
    let folded = fold(filename);
    tokens(query).iter().all(|term| folded.contains(&fold(term)))
}

/// Hidden path components and application-bundle contents are structural, not ignore patterns.
pub fn is_excluded_path(path: &str, ignore: &IgnoreList) -> bool {
    let structural = path_components(path).any(|component| {
        (component.starts_with('.') && component != "." && component != "..")
            || component.to_ascii_lowercase().ends_with(".app")
    });
    structural || ignore.drops(path)
}

pub fn rank(results: Vec<FileSearchHit>, query: &str, ignore: &IgnoreList) -> Vec<FileSearchHit> {
    let terms = tokens(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let whole = query.trim();
    let mut scored: Vec<(FileSearchHit, Option<i32>, i32)> = results
        .into_iter()
        .filter(|hit| !is_excluded_path(&hit.path, ignore))
        .map(|hit| {
            let full = FuzzyMatch::score(whole, &hit.name);
            let term_score: i32 = terms
                .iter()
                .filter_map(|term| FuzzyMatch::score(term, &hit.name))
                .sum();
            (hit, full, term_score)
        })
        .collect();
    scored.sort_by(|left, right| {
        match (left.1, right.1) {
            (Some(a), Some(b)) if a != b => return b.cmp(&a),
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        if left.2 != right.2 {
            return right.2.cmp(&left.2);
        }
        let name = left.0.name.to_lowercase().cmp(&right.0.name.to_lowercase());
        if name != std::cmp::Ordering::Equal {
            return name;
        }
        left.0.path.to_lowercase().cmp(&right.0.path.to_lowercase())
    });
    scored
        .into_iter()
        .take(RESULT_LIMIT)
        .map(|(hit, _, _)| hit)
        .collect()
}

pub fn like_clauses(query: &str) -> Option<String> {
    let terms = tokens(query);
    if terms.is_empty() {
        return None;
    }
    let clauses: Vec<String> = terms
        .iter()
        .map(|term| format!("System.FileName LIKE '%{}%'", escape_like(term)))
        .collect();
    Some(clauses.join(" AND "))
}

fn escape_like(term: &str) -> String {
    let mut out = String::new();
    for ch in term.chars() {
        match ch {
            '\'' => out.push_str("''"),
            '%' => out.push_str("[%]"),
            '_' => out.push_str("[_]"),
            '[' => out.push_str("[[]"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn fold(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

pub fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn path_components(path: &str) -> impl Iterator<Item = &str> {
    path.split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
}

pub fn filename(path: &str) -> &str {
    path_components(path).last().unwrap_or(path)
}

pub fn parent_of(path: &str) -> String {
    let normalized = normalize_slashes(path);
    match normalized.rfind('/') {
        Some(0) => "/".into(),
        Some(i) => normalized[..i].to_string(),
        None => String::new(),
    }
}

pub fn abbreviate(path: &str, home: &str) -> String {
    let path = normalize_slashes(path);
    let home = normalize_slashes(home).trim_end_matches('/').to_string();
    if path == home {
        return "~".into();
    }
    if let Some(rest) = path.strip_prefix(&(home.clone() + "/")) {
        format!("~/{rest}")
    } else {
        path
    }
}

pub fn expand_scope(scope: &str, home: &str) -> String {
    let home = normalize_slashes(home).trim_end_matches('/').to_string();
    if scope == "~" {
        return home;
    }
    if let Some(rest) = scope.strip_prefix("~/") {
        return format!("{home}/{rest}");
    }
    if let Some(rest) = scope.strip_prefix("~\\") {
        return format!("{home}/{rest}");
    }
    normalize_slashes(scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_search::IgnoreList;

    #[test]
    fn caps_and_and_tokens() {
        assert_eq!(cap_candidates(5000), 1000);
        assert_eq!(tokens("annual report"), vec!["annual", "report"]);
    }

    #[test]
    fn tokens_split_whitespace_and_empty_does_no_work() {
        assert_eq!(tokens("  annual\treport  "), vec!["annual", "report"]);
        assert!(tokens(" \n ").is_empty());
        assert!(like_clauses(" \n ").is_none());
        assert_eq!(cap_rows(500), 200);
    }

    #[test]
    fn and_tokens_require_every_term() {
        assert!(matches_filename("Résumé Final.pdf", "resume final"));
        assert!(!matches_filename("Annual Notes.pdf", "annual report"));
    }

    #[test]
    fn hidden_and_app_bundle_are_structural() {
        let ignore = IgnoreList::defaults();
        assert!(is_excluded_path("C:/src/.cache/x", &ignore));
        assert!(is_excluded_path("C:/Apps/Local.app/Contents/Info.plist", &ignore));
        assert!(!is_excluded_path("C:/src/foo/report.pdf", &ignore));
    }
}
