use std::collections::HashMap;

use crate::app_entry::{AppEntry, AppKind};
use crate::search_relevance::SearchFields;
use crate::template::{expand_destination_template, ExpandContext, ExpandOutput};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Quicklink {
    pub id: String,
    pub name: String,
    pub destination: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "default_true")]
    pub show_in_root: bool,
    #[serde(default)]
    pub pin_order: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestinationKind {
    Path,
    Web,
    Network,
    Deeplink,
}

impl Quicklink {
    pub fn as_entry(&self) -> AppEntry {
        AppEntry {
            id: format!("quicklink:{}", self.id),
            kind: AppKind::Quicklink,
            name: self.name.clone(),
            fields: SearchFields {
                display_name: self.name.clone(),
                ..Default::default()
            },
            hotkey: None,
        }
    }

    pub fn precedes(&self, other: &Quicklink) -> std::cmp::Ordering {
        match (self.pinned, other.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => self
                .pin_order
                .cmp(&other.pin_order)
                .then_with(|| self.name.to_lowercase().cmp(&other.name.to_lowercase())),
            (false, false) => self
                .name
                .to_lowercase()
                .cmp(&other.name.to_lowercase())
                .then_with(|| self.id.cmp(&other.id)),
        }
    }
}

pub fn sort_quicklinks(links: &mut [Quicklink]) {
    links.sort_by(|a, b| a.precedes(b));
}

pub fn expand_destination(
    destination: &str,
    ctx: &ExpandContext,
    args: &HashMap<String, String>,
) -> ExpandOutput {
    let auto_percent = uses_url_encoding(destination);
    expand_destination_template(destination, ctx, args, auto_percent)
}

pub fn uses_url_encoding(destination: &str) -> bool {
    !matches!(detect_kind(destination), Some(DestinationKind::Path))
}

pub fn detect_kind(raw: &str) -> Option<DestinationKind> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with("~/") || t.starts_with('/') || t.starts_with("file:") {
        return Some(DestinationKind::Path);
    }
    if looks_like_windows_path(t) {
        return Some(DestinationKind::Path);
    }
    if let Some((scheme, rest)) = t.split_once(':') {
        let scheme = scheme.to_ascii_lowercase();
        if scheme.len() == 1 && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return Some(DestinationKind::Path);
        }
        if rest.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        return Some(match scheme.as_str() {
            "http" | "https" => DestinationKind::Web,
            "smb" | "afp" | "nfs" | "ftp" | "sftp" | "ftps" => DestinationKind::Network,
            "file" => DestinationKind::Path,
            _ => DestinationKind::Deeplink,
        });
    }
    if looks_like_host(t) {
        return Some(DestinationKind::Web);
    }
    None
}

fn looks_like_windows_path(t: &str) -> bool {
    let bytes = t.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/'))
        || t.starts_with(r"\\")
}

fn looks_like_host(t: &str) -> bool {
    let host = t.split(['/', '?', '#']).next().unwrap_or(t);
    let mut parts = host.rsplitn(2, '.');
    let tld = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    !rest.is_empty() && tld.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && tld.len() >= 2
}

pub fn id_from_entry(entry_id: &str) -> Option<&str> {
    entry_id.strip_prefix("quicklink:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quicklink_url_encodes_arguments_unless_raw() {
        let ctx = ExpandContext {
            clipboard: vec![],
            selection: None,
            now: 0,
            locale: "en".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        };
        let args: HashMap<String, String> = [("q".into(), "a b".into())].into();
        let o = expand_destination("https://ex.com/q={argument name=\"q\"}", &ctx, &args);
        assert!(o.text.contains("a%20b"));
        let raw = expand_destination(
            "https://ex.com/q={argument name=\"q\" | raw}",
            &ctx,
            &args,
        );
        assert!(raw.text.contains("a b"));
        assert!(!raw.text.contains("a%20b"));
    }

    #[test]
    fn detect_path_and_web() {
        assert_eq!(detect_kind("~/Notes/x.md"), Some(DestinationKind::Path));
        assert_eq!(
            detect_kind("https://github.com"),
            Some(DestinationKind::Web)
        );
        assert_eq!(
            detect_kind("spotify://track/1"),
            Some(DestinationKind::Deeplink)
        );
        assert_eq!(detect_kind(r"C:\Windows"), Some(DestinationKind::Path));
        assert_eq!(detect_kind("github.com/a"), Some(DestinationKind::Web));
        assert_eq!(detect_kind("not a link"), None);
    }

    #[test]
    fn destination_keeps_cursor_and_snippet_refs_literal() {
        let ctx = ExpandContext {
            clipboard: vec![],
            selection: None,
            now: 0,
            locale: "en".into(),
            tz: "UTC".into(),
            uuid: || "u".into(),
        };
        let o = expand_destination(
            "https://ex.com/{cursor}/{snippet:Name}/{snippet name=\"X\"}",
            &ctx,
            &Default::default(),
        );
        assert!(
            o.text.contains("{cursor}"),
            "cursor must stay literal, got {}",
            o.text
        );
        assert!(o.text.contains("{snippet:Name}"));
        assert!(o.text.contains("{snippet name=\"X\"}"));
        assert!(o.cursor.is_none());
    }
}
