use super::ignore::IgnoreList;
use super::query::{abbreviate, expand_scope, filename, normalize_slashes};

/// Resolved answer to "what does this scope list mean".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSearchPolicy {
    pub home: String,
    pub direct_roots: Vec<String>,
    pub includes_home: bool,
    pub ignore: IgnoreList,
}

impl FileSearchPolicy {
    pub fn new(scopes: &[String], ignore_patterns: &[String], home: &str) -> Self {
        let home = normalize_slashes(home).trim_end_matches('/').to_string();
        let roots = unique_roots(scopes, &home);
        let direct_roots: Vec<String> = roots.iter().filter(|p| *p != &home).cloned().collect();
        let includes_home = roots.len() != direct_roots.len();
        Self {
            home,
            direct_roots,
            includes_home,
            ignore: IgnoreList::with_user(ignore_patterns),
        }
    }

    pub fn default_scopes() -> Vec<String> {
        vec!["~".into()]
    }
}

pub fn unique_roots(scopes: &[String], home: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for scope in scopes {
        let expanded = expand_scope(scope, home);
        if seen.insert(expanded.clone()) {
            out.push(expanded);
        }
    }
    out
}

pub fn normalize_scopes(scopes: &[String], home: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    unique_roots(scopes, home)
        .into_iter()
        .map(|p| abbreviate(&p, home))
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// Home expands into visible children; `AppData` is never a scope Tinycast picks by itself.
pub fn keep_home_child(name: &str, hidden: bool, is_application: bool) -> bool {
    if hidden || is_application {
        return false;
    }
    !filename(name).eq_ignore_ascii_case("AppData")
        && !filename(name).eq_ignore_ascii_case("Library")
        && !filename(name).eq_ignore_ascii_case("Application Data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scope_list_searches_nothing() {
        let p = FileSearchPolicy::new(&[], &[], "C:/Users/test");
        assert!(p.direct_roots.is_empty());
        assert!(!p.includes_home);
    }

    #[test]
    fn tilde_means_home_and_appdata_is_not_auto_picked() {
        let p = FileSearchPolicy::new(&["~".into()], &[], "C:/Users/test");
        assert!(p.includes_home);
        assert!(p.direct_roots.is_empty());
        assert!(!keep_home_child("AppData", false, false));
        assert!(keep_home_child("Documents", false, false));
    }
}
