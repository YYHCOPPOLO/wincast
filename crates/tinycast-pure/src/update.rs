//! GitHub Releases updater policy. Dev channel never updates.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Channel {
    Stable,
    Development,
}

impl Channel {
    pub fn updates_itself(self) -> bool {
        self == Channel::Stable
    }

    pub fn accepts_prerelease(self, prerelease: bool) -> bool {
        match self {
            Channel::Stable => !prerelease,
            Channel::Development => false,
        }
    }
}

pub fn channel_for_bundle(id: &str) -> Channel {
    if id == "com.tinycast.win" {
        Channel::Stable
    } else {
        Channel::Development
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub beta: Option<u32>,
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then(match (self.beta, other.beta) {
                (None, None) => std::cmp::Ordering::Equal,
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(a), Some(b)) => a.cmp(&b),
            })
    }
}

pub fn parse_version(s: &str) -> Option<Version> {
    let mut body = s.trim();
    if let Some(rest) = body.strip_prefix('v') {
        body = rest;
    }
    let (triple, suffix) = match body.split_once('-') {
        Some((left, right)) => (left, Some(right)),
        None => (body, None),
    };
    let mut parts = triple.split('.');
    let major = parse_number(parts.next()?)?;
    let minor = parse_number(parts.next()?)?;
    let patch = parse_number(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    let beta = if let Some(suffix) = suffix {
        let mut bits = suffix.split('.');
        if bits.next()? != "beta" {
            return None;
        }
        let n = parse_number(bits.next()?)?;
        if bits.next().is_some() {
            return None;
        }
        Some(n)
    } else {
        None
    };
    Some(Version {
        major,
        minor,
        patch,
        beta,
    })
}

fn parse_number(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

impl Version {
    pub fn is_prerelease(self) -> bool {
        self.beta.is_some()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.beta {
            Some(n) => write!(f, "{}.{}.{}-beta.{}", self.major, self.minor, self.patch, n),
            None => write!(f, "{}.{}.{}", self.major, self.minor, self.patch),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateActivity {
    pub expanding_snippet: bool,
    pub uninstalling: bool,
    pub recording_hotkey: bool,
    pub dialog_open: bool,
    pub palette_visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateBlocker {
    ExpandingSnippet,
    Uninstalling,
    RecordingHotKey,
    DialogOpen,
    PaletteOpen,
}

pub fn readiness(activity: UpdateActivity) -> Option<UpdateBlocker> {
    if activity.expanding_snippet {
        Some(UpdateBlocker::ExpandingSnippet)
    } else if activity.uninstalling {
        Some(UpdateBlocker::Uninstalling)
    } else if activity.recording_hotkey {
        Some(UpdateBlocker::RecordingHotKey)
    } else if activity.dialog_open {
        Some(UpdateBlocker::DialogOpen)
    } else if activity.palette_visible {
        Some(UpdateBlocker::PaletteOpen)
    } else {
        None
    }
}

pub const EMPTY_FEED_HUD: &str = "No updates configured.";
pub const RELEASE_REPO: &str = "";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_bundle_is_the_only_updater() {
        assert_eq!(channel_for_bundle("com.tinycast.win"), Channel::Stable);
        assert_eq!(
            channel_for_bundle("com.tinycast.win.dev"),
            Channel::Development
        );
    }

    #[test]
    fn semver_beta_below_release() {
        assert!(parse_version("0.10.2").unwrap() > parse_version("0.10.2-beta.1").unwrap());
    }

    #[test]
    fn empty_release_repo_is_valid() {
        assert!(RELEASE_REPO.is_empty());
        assert_eq!(EMPTY_FEED_HUD, "No updates configured.");
    }
}
