//! GitHub Releases feed. Empty `RELEASE_REPO` is valid and is not an error.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tinycast_pure::update::{
    channel_for_bundle, parse_version, Channel, Version, EMPTY_FEED_HUD, RELEASE_REPO,
};

use crate::platform::paths;
use crate::platform::winhttp;

const CACHE_FRESH_SECS: i64 = 24 * 3600;

#[derive(Clone, Debug)]
pub enum FeedOutcome {
    Empty,
    Current,
    Available { version: Version, notes: String, zip_url: String },
    Failed(String),
}

pub fn bundle_id() -> &'static str {
    "com.tinycast.win"
}

pub fn running_channel() -> Channel {
    channel_for_bundle(bundle_id())
}

pub fn command_visible() -> bool {
    running_channel().updates_itself()
}

pub fn check_now() -> FeedOutcome {
    if RELEASE_REPO.is_empty() {
        return FeedOutcome::Empty;
    }
    if !running_channel().updates_itself() {
        return FeedOutcome::Empty;
    }
    match fetch_latest() {
        Ok(Some(release)) => FeedOutcome::Available {
            version: release.version,
            notes: release.notes,
            zip_url: release.zip_url,
        },
        Ok(None) => FeedOutcome::Current,
        Err(err) => FeedOutcome::Failed(err),
    }
}

struct Release {
    version: Version,
    notes: String,
    zip_url: String,
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{RELEASE_REPO}/releases?per_page=20");
    let body = winhttp::get_text(&url)?;
    parse_releases(&body, running_channel())
}

fn parse_releases(body: &str, channel: Channel) -> Result<Option<Release>, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "The update feed was malformed.".to_string())?;
    let Some(items) = value.as_array() else {
        return Err("The update feed was malformed.".into());
    };
    let mut best: Option<Release> = None;
    for item in items {
        let prerelease = item.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false);
        if !channel.accepts_prerelease(prerelease) {
            continue;
        }
        let tag = item
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let Some(version) = parse_version(tag) else {
            continue;
        };
        if version.is_prerelease() != prerelease {
            continue;
        }
        let zip = item
            .get("assets")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .find_map(|asset| {
                let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.ends_with(".zip") {
                    asset
                        .get("browser_download_url")
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            });
        let Some(zip_url) = zip else {
            continue;
        };
        let notes = cut_notes(item.get("body").and_then(|b| b.as_str()).unwrap_or(""));
        if best.as_ref().map(|b| version > b.version).unwrap_or(true) {
            best = Some(Release {
                version,
                notes,
                zip_url,
            });
        }
    }
    Ok(best)
}

fn cut_notes(body: &str) -> String {
    match body.split_once("<!-- tinycast:install -->") {
        Some((keep, _)) => keep.trim().to_string(),
        None => body.trim().to_string(),
    }
}

pub fn cache_path() -> PathBuf {
    paths::local_dir().join("update-check.json")
}

pub fn cache_is_fresh() -> bool {
    let Ok(meta) = std::fs::metadata(cache_path()) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    age.as_secs() as i64 <= CACHE_FRESH_SECS
}

pub fn touch_cache() {
    let _ = std::fs::create_dir_all(paths::local_dir());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(
        cache_path(),
        format!("{{\"lastCheckedAt\":{now}}}"),
    );
}

pub fn empty_hud() -> &'static str {
    EMPTY_FEED_HUD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_repo_is_empty_feed() {
        assert!(RELEASE_REPO.is_empty());
        assert!(matches!(check_now(), FeedOutcome::Empty));
        assert_eq!(empty_hud(), "No updates configured.");
    }

    #[test]
    fn zip_only_assets_are_installable() {
        let json = r#"[{"tag_name":"v0.10.2","prerelease":false,"body":"notes","assets":[{"name":"Tinycast-0.10.2.dmg"},{"name":"Tinycast-0.10.2.zip","browser_download_url":"https://example.com/a.zip"}]}]"#;
        let release = parse_releases(json, Channel::Stable).unwrap().unwrap();
        assert_eq!(release.version.to_string(), "0.10.2");
        assert!(release.zip_url.ends_with(".zip"));
    }
}
