use std::collections::HashSet;

use super::query::path_components;

/// gitignore-flavoured excludes: a bare pattern matches any path component, one with `/` the whole path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IgnoreList {
    literal_names: HashSet<String>,
    name_globs: Vec<String>,
    path_globs: Vec<String>,
}

impl IgnoreList {
    /// Compiled in rather than stored, so changing the shipped rules reaches installs that already ran.
    pub const DEFAULTS: &'static [&'static str] = &[
        "node_modules",
        "DerivedData",
        "build",
        "dist",
        "target",
        "Pods",
    ];

    pub fn defaults() -> Self {
        Self::from_patterns(Self::DEFAULTS.iter().copied())
    }

    pub fn with_user(user: &[String]) -> Self {
        Self::from_patterns(
            Self::DEFAULTS
                .iter()
                .copied()
                .chain(user.iter().map(String::as_str)),
        )
    }

    pub fn from_patterns<'a, I: IntoIterator<Item = &'a str>>(patterns: I) -> Self {
        let mut literal_names = HashSet::new();
        let mut name_globs = Vec::new();
        let mut path_globs = Vec::new();
        for pattern in patterns {
            let trimmed = pattern.trim();
            if trimmed.is_empty() || trimmed.contains('\0') {
                continue;
            }
            if trimmed.contains('/') || trimmed.contains('\\') {
                path_globs.push(trimmed.replace('\\', "/"));
            } else if trimmed.chars().any(is_meta) {
                name_globs.push(trimmed.to_string());
            } else {
                literal_names.insert(trimmed.to_lowercase());
            }
        }
        Self {
            literal_names,
            name_globs,
            path_globs,
        }
    }

    pub fn drops(&self, path: &str) -> bool {
        for component in path_components(path) {
            if self.literal_names.contains(&component.to_lowercase()) {
                return true;
            }
            if self
                .name_globs
                .iter()
                .any(|glob| glob_match(glob, component))
            {
                return true;
            }
        }
        let normalized = path.replace('\\', "/");
        self.path_globs
            .iter()
            .any(|glob| glob_match(glob, &normalized))
    }

    /// Name globs Windows Search can evaluate itself (`*` only).
    pub fn search_name_exclusions(&self) -> Vec<&str> {
        self.name_globs
            .iter()
            .filter(|g| !g.contains(['?', '[', '"', '\\']))
            .map(String::as_str)
            .collect()
    }
}

fn is_meta(ch: char) -> bool {
    ch == '*' || ch == '?' || ch == '['
}

/// Case-insensitive glob. `*` spans `/` (FNM_PATHNAME off), so `**/tmp/**` works.
pub fn glob_match(pattern: &str, candidate: &str) -> bool {
    glob_rec(
        &pattern.chars().collect::<Vec<_>>(),
        &candidate.chars().collect::<Vec<_>>(),
    )
}

fn glob_rec(pat: &[char], text: &[char]) -> bool {
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star_p: Option<usize> = None;
    let mut star_t = 0usize;
    while ti < text.len() {
        if pi < pat.len() && pat[pi] == '[' {
            if let Some((ok, next)) = match_class(pat, pi, text[ti]) {
                if ok {
                    pi = next;
                    ti += 1;
                    continue;
                }
            }
            if let Some(sp) = star_p {
                pi = sp + 1;
                star_t += 1;
                ti = star_t;
                continue;
            }
            return false;
        }
        if pi < pat.len() && (pat[pi] == '?' || eq_ci(pat[pi], text[ti])) {
            pi += 1;
            ti += 1;
            continue;
        }
        if pi < pat.len() && pat[pi] == '*' {
            star_p = Some(pi);
            star_t = ti;
            pi += 1;
            continue;
        }
        if let Some(sp) = star_p {
            pi = sp + 1;
            star_t += 1;
            ti = star_t;
            continue;
        }
        return false;
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
}

fn match_class(pat: &[char], mut pi: usize, ch: char) -> Option<(bool, usize)> {
    if pi >= pat.len() || pat[pi] != '[' {
        return None;
    }
    pi += 1;
    let neg = matches!(pat.get(pi), Some('!' | '^'));
    if neg {
        pi += 1;
    }
    let mut matched = false;
    let mut saw = false;
    while pi < pat.len() && pat[pi] != ']' {
        saw = true;
        if pi + 2 < pat.len() && pat[pi + 1] == '-' && pat[pi + 2] != ']' {
            if in_range_ci(ch, pat[pi], pat[pi + 2]) {
                matched = true;
            }
            pi += 3;
        } else {
            if eq_ci(pat[pi], ch) {
                matched = true;
            }
            pi += 1;
        }
    }
    if pi < pat.len() && pat[pi] == ']' {
        pi += 1;
    }
    if !saw {
        return Some((false, pi));
    }
    Some((if neg { !matched } else { matched }, pi))
}

fn eq_ci(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

fn in_range_ci(ch: char, start: char, end: char) -> bool {
    let c = ch.to_ascii_lowercase();
    let a = start.to_ascii_lowercase();
    let b = end.to_ascii_lowercase();
    (a <= c && c <= b) || (b <= c && c <= a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ignore_drops_node_modules() {
        let i = IgnoreList::defaults();
        assert!(i.drops("C:/src/foo/node_modules/x"));
        assert!(i.drops(r"C:\src\foo\node_modules\x"));
        assert!(i.drops("C:/src/foo/target/debug"));
        assert!(!i.drops("C:/src/foo/src/main.rs"));
    }

    #[test]
    fn user_globs_and_path_patterns() {
        let i = IgnoreList::with_user(&["*.tmp".into(), "**/[Cc]ache/**".into()]);
        assert!(i.drops("C:/src/foo/scratch.tmp"));
        assert!(i.drops("C:/src/foo/Cache/x"));
        assert!(!i.drops("C:/src/foo/notes.md"));
    }
}
