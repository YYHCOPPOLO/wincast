#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MeetingLink {
    pub provider: Provider,
    pub url: String,
}

impl MeetingLink {
    pub fn app_url(&self) -> Option<String> {
        self.provider.app_url(&self.url)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Provider {
    Zoom,
    GoogleMeet,
    Teams,
    Webex,
    Jitsi,
    Whereby,
    Chime,
    GotoMeeting,
    BlueJeans,
    Skype,
    Generic,
}

impl Provider {
    pub fn title(self) -> &'static str {
        match self {
            Provider::Zoom => "Zoom",
            Provider::GoogleMeet => "Google Meet",
            Provider::Teams => "Microsoft Teams",
            Provider::Webex => "Webex",
            Provider::Jitsi => "Jitsi",
            Provider::Whereby => "Whereby",
            Provider::Chime => "Amazon Chime",
            Provider::GotoMeeting => "GoTo Meeting",
            Provider::BlueJeans => "BlueJeans",
            Provider::Skype => "Skype",
            Provider::Generic => "Meeting Link",
        }
    }

    fn from_host(host: &str) -> Option<Self> {
        let host = host.to_ascii_lowercase();
        const TABLE: &[(Provider, &[&str])] = &[
            (Provider::Zoom, &["zoom.us", "zoom.com", "zoomgov.com"]),
            (Provider::GoogleMeet, &["meet.google.com"]),
            (
                Provider::Teams,
                &["teams.microsoft.com", "teams.microsoft.us", "teams.live.com"],
            ),
            (Provider::Webex, &["webex.com", "webex.com.cn"]),
            (Provider::Jitsi, &["meet.jit.si", "8x8.vc"]),
            (Provider::Whereby, &["whereby.com"]),
            (Provider::Chime, &["chime.aws"]),
            (
                Provider::GotoMeeting,
                &["gotomeeting.com", "gotomeet.me", "app.goto.com"],
            ),
            (Provider::BlueJeans, &["bluejeans.com"]),
            (Provider::Skype, &["join.skype.com"]),
        ];
        for (provider, suffixes) in TABLE {
            if suffixes
                .iter()
                .any(|s| host == *s || host.ends_with(&format!(".{s}")))
            {
                return Some(*provider);
            }
        }
        None
    }

    fn admits(self, path: &str) -> bool {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect();
        match self {
            Provider::Zoom => segments.iter().any(|s| matches!(s.as_str(), "j" | "w" | "s" | "my")),
            Provider::GoogleMeet => segments.len() == 1 && segments[0] != "tel",
            Provider::Teams => {
                segments.iter().any(|s| s == "meetup-join" || s == "meet")
            }
            Provider::Webex
            | Provider::Jitsi
            | Provider::Whereby
            | Provider::Chime
            | Provider::GotoMeeting
            | Provider::BlueJeans
            | Provider::Skype
            | Provider::Generic => !segments.is_empty(),
        }
    }

    pub fn app_url(self, url: &str) -> Option<String> {
        match self {
            Provider::Zoom => zoom_app_url(url),
            Provider::Teams => teams_app_url(url),
            _ => None,
        }
    }
}

/// Fields in precedence order; a named provider anywhere beats a bare link found earlier.
pub fn detect_link(fields: &[&str]) -> Option<MeetingLink> {
    let mut fallback = None;
    for field in fields {
        for url in web_urls(field) {
            if let Some(link) = classify(&url) {
                if link.provider != Provider::Generic {
                    return Some(link);
                }
                if fallback.is_none() {
                    fallback = Some(link);
                }
            }
        }
    }
    fallback
}

fn classify(url: &str) -> Option<MeetingLink> {
    let parsed = UrlParts::parse(url)?;
    if parsed.scheme != "http" && parsed.scheme != "https" {
        return None;
    }
    match Provider::from_host(&parsed.host) {
        Some(provider) => {
            if provider.admits(&parsed.path) {
                Some(MeetingLink {
                    provider,
                    url: url.to_string(),
                })
            } else {
                None
            }
        }
        None => Some(MeetingLink {
            provider: Provider::Generic,
            url: url.to_string(),
        }),
    }
}

const TERMINATORS: &[char] = &[' ', '\t', '\n', '\r', '"', '\'', '<', '>', '«', '»', '\u{00A0}'];
const TRAILING: &[char] = &['.', ',', ';', ':', ')', ']', '}', '!', '?'];

fn web_urls(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("http") {
        let start = cursor + rel;
        let rest = &text[start..];
        let end_rel = rest
            .char_indices()
            .find(|(_, c)| TERMINATORS.contains(c))
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let mut candidate = &rest[..end_rel];
        while let Some(last) = candidate.chars().last() {
            if TRAILING.contains(&last) {
                candidate = &candidate[..candidate.len() - last.len_utf8()];
            } else {
                break;
            }
        }
        let lowered = candidate.to_ascii_lowercase();
        if lowered.starts_with("http://") || lowered.starts_with("https://") {
            found.push(candidate.to_string());
        }
        cursor = start + end_rel.max(1);
        if cursor <= start {
            cursor = start + 1;
        }
        if cursor >= text.len() {
            break;
        }
    }
    found
}

struct UrlParts {
    scheme: String,
    host: String,
    path: String,
    query: Option<String>,
}

impl UrlParts {
    fn parse(url: &str) -> Option<Self> {
        let scheme_end = url.find("://")?;
        let scheme = url[..scheme_end].to_ascii_lowercase();
        let rest = &url[scheme_end + 3..];
        let (hostport, pathquery) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        let host = hostport.split(':').next().unwrap_or(hostport).to_string();
        if host.is_empty() {
            return None;
        }
        let (path, query) = match pathquery.find('?') {
            Some(i) => (&pathquery[..i], Some(pathquery[i + 1..].to_string())),
            None => (pathquery, None),
        };
        Some(Self {
            scheme,
            host,
            path: path.to_string(),
            query,
        })
    }
}

fn zoom_app_url(url: &str) -> Option<String> {
    let parts = UrlParts::parse(url)?;
    let segments: Vec<&str> = parts.path.split('/').filter(|s| !s.is_empty()).collect();
    let marker = segments
        .iter()
        .position(|s| matches!(s.to_ascii_lowercase().as_str(), "j" | "w" | "s"))?;
    let conference = segments.get(marker + 1)?;
    let mut out = format!("zoommtg://zoom.us/join?confno={conference}");
    if let Some(query) = &parts.query {
        for pair in query.split('&') {
            if let Some(pwd) = pair.strip_prefix("pwd=") {
                out.push_str("&pwd=");
                out.push_str(pwd);
                break;
            }
        }
    }
    Some(out)
}

fn teams_app_url(url: &str) -> Option<String> {
    let parts = UrlParts::parse(url)?;
    if !parts.host.to_ascii_lowercase().ends_with("teams.microsoft.com") {
        return None;
    }
    let mut out = format!("msteams:{}", parts.path);
    if let Some(query) = parts.query {
        out.push('?');
        out.push_str(&query);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_provider_beats_earlier_generic() {
        let l = detect_link(&[
            "https://example.com/reset",
            "https://meet.google.com/abc-defg-hij",
        ])
        .unwrap();
        assert_eq!(l.provider, Provider::GoogleMeet);
    }

    #[test]
    fn zoom_download_is_rejected_not_generic() {
        assert!(detect_link(&["https://zoom.us/download"]).is_none());
        let l = detect_link(&["https://zoom.us/j/123456789?pwd=secret"]).unwrap();
        assert_eq!(l.provider, Provider::Zoom);
        assert!(l.app_url().unwrap().starts_with("zoommtg://zoom.us/join?confno=123456789"));
    }
}
