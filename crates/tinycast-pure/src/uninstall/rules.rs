use super::plan::UninstallIdentity;

pub const STRIPPED_EXTENSIONS: &[&str] = &[
    "plist", "savedstate", "binarycookies", "lockfile", "lock", "sfl", "sfl2", "sfl3",
];

pub fn matchable_forms(name: &str) -> Vec<String> {
    let mut forms = vec![name.to_string()];
    let mut current = name.to_string();
    for _ in 0..3 {
        let ext = current
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        if ext.is_empty() || !STRIPPED_EXTENSIONS.contains(&ext.as_str()) {
            break;
        }
        current = current.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_default();
        if current.is_empty() {
            break;
        }
        forms.push(current.clone());
    }
    forms
}

pub fn owns(folded: &str, id: &str, allowing_prefix: bool) -> bool {
    if folded == id {
        return true;
    }
    if !allowing_prefix || folded.len() <= id.len() || !folded.starts_with(id) {
        return false;
    }
    matches!(folded.as_bytes().get(id.len()), Some(b'.') | Some(b'-'))
}

pub fn owns_bundle(component: &str, identity: &UninstallIdentity) -> bool {
    identity.matches_bundle(component)
}

pub fn is_home_root(path: &str, home: &str) -> bool {
    let p = normalize(path);
    let h = normalize(home);
    p == h
}

pub fn is_acceptable_candidate(path: &str, root: &str, home: &str, bundle: &str) -> bool {
    let path = normalize(path);
    let root = normalize(root);
    let home = normalize(home);
    let bundle = normalize(bundle);
    if path.is_empty() || path == "/" || path == home || path == root {
        return false;
    }
    if is_home_root(&path, &home) {
        return false;
    }
    let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    if parent != root {
        return false;
    }
    if path == bundle || is_descendant(&path, &bundle) || is_descendant(&bundle, &path) {
        return false;
    }
    true
}

pub fn is_descendant(path: &str, ancestor: &str) -> bool {
    let path = normalize(path);
    let ancestor = normalize(ancestor);
    path.starts_with(&(ancestor.clone() + "/"))
}

pub fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

pub fn folded(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uninstall::UninstallIdentity;

    #[test]
    fn prefix_bundle_does_not_eat_sibling() {
        // com.foo.bar must not claim com.foo.bar.beta
        let id = UninstallIdentity::make_with_others(
            "com.foo.bar",
            "com.other.app",
            &["com.foo.bar.beta"],
        )
        .unwrap();
        assert!(!id.matches_bundle("com.foo.bar.beta"));
        assert!(id.matches_bundle("com.foo.bar"));
        assert!(id.matches_bundle("com.foo.bar.CacheDelete"));
        assert!(!id.matches_bundle("com.foo.barx"));
    }

    #[test]
    fn home_is_not_an_acceptable_candidate() {
        assert!(!is_acceptable_candidate("C:/Users/me", "C:/Users/me", "C:/Users/me", "C:/apps/x"));
        assert!(!is_home_root("C:/Users/me/AppData", "C:/Users/me"));
        assert!(is_home_root("C:/Users/me", "C:/Users/me"));
    }
}
