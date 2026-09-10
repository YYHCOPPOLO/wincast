use std::collections::HashSet;

use super::rules::{folded, matchable_forms, owns};

pub const RUNNING_BUNDLE_ID: &str = "com.tinycast.win";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallIdentity {
    pub bundle_id: String,
    pub allows_prefix: bool,
    pub other_bundle_ids: HashSet<String>,
}

impl UninstallIdentity {
    pub fn make(bundle_id: &str, running_id: &str) -> Option<Self> {
        Self::make_with_others(bundle_id, running_id, &[])
    }

    pub fn make_with_others(
        bundle_id: &str,
        running_id: &str,
        other_bundle_ids: &[&str],
    ) -> Option<Self> {
        let id = folded(bundle_id);
        if id.is_empty() {
            return None;
        }
        if id == folded(running_id) {
            return None;
        }
        Some(Self {
            allows_prefix: id.split('.').count() >= 3,
            other_bundle_ids: other_bundle_ids
                .iter()
                .map(|s| folded(s))
                .filter(|s| s != &id)
                .collect(),
            bundle_id: id,
        })
    }

    pub fn matches_bundle(&self, component: &str) -> bool {
        matchable_forms(component).iter().any(|form| {
            let folded_form = folded(form);
            if !owns(&folded_form, &self.bundle_id, self.allows_prefix) {
                return false;
            }
            !self
                .other_bundle_ids
                .iter()
                .any(|other| other.len() > self.bundle_id.len() && owns(&folded_form, other, true))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub locked: bool,
    pub is_bundle: bool,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UninstallSelection {
    checked: HashSet<String>,
}

impl UninstallSelection {
    pub fn new() -> Self {
        Self {
            checked: HashSet::new(),
        }
    }

    pub fn toggle(&mut self, id: &str) {
        if !self.checked.insert(id.to_string()) {
            self.checked.remove(id);
        }
    }

    pub fn intersect_removable(&mut self, removable: &[&str]) {
        let allowed: HashSet<String> = removable.iter().map(|s| (*s).to_string()).collect();
        self.checked.retain(|id| allowed.contains(id));
    }

    pub fn contains(&self, id: &str) -> bool {
        self.checked.contains(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.checked.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.checked.is_empty()
    }
}

pub fn removable_ids(candidates: &[UninstallCandidate]) -> Vec<String> {
    candidates
        .iter()
        .filter(|c| !c.locked)
        .map(|c| c.id.clone())
        .collect()
}

/// Bundle last so a failed leftover pass can be retried while the app still exists.
pub fn recycle_order(
    candidates: &[UninstallCandidate],
    selected: &UninstallSelection,
) -> Vec<String> {
    let mut leftovers = Vec::new();
    let mut bundles = Vec::new();
    for c in candidates {
        if !selected.contains(&c.id) {
            continue;
        }
        if c.is_bundle {
            bundles.push(c.path.clone());
        } else {
            leftovers.push(c.path.clone());
        }
    }
    leftovers.extend(bundles);
    leftovers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_self() {
        assert!(UninstallIdentity::make("com.tinycast.win", "com.tinycast.win").is_none());
        assert!(UninstallIdentity::make("com.foo.bar", "com.tinycast.win").is_some());
    }

    #[test]
    fn locked_cannot_enter_selection() {
        let mut sel = UninstallSelection::new();
        sel.toggle("locked");
        sel.intersect_removable(&["a"]);
        assert!(!sel.contains("locked"));
    }

    #[test]
    fn bundle_recycles_last() {
        let candidates = vec![
            UninstallCandidate {
                id: "bundle".into(),
                name: "Foo".into(),
                path: "C:/Program Files/Foo".into(),
                locked: false,
                is_bundle: true,
                size: None,
            },
            UninstallCandidate {
                id: "cache".into(),
                name: "cache".into(),
                path: "C:/Users/me/AppData/Local/Foo".into(),
                locked: false,
                is_bundle: false,
                size: None,
            },
        ];
        let mut sel = UninstallSelection::new();
        sel.toggle("bundle");
        sel.toggle("cache");
        sel.intersect_removable(&["bundle", "cache"]);
        let order = recycle_order(&candidates, &sel);
        assert_eq!(order.last().unwrap(), "C:/Program Files/Foo");
    }
}
