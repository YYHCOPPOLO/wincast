//! Cheap-first clipboard text classifier. Never persisted; see clipboard.md.

const DETECTION_LIMIT: usize = 2048;

const COMMON_TLDS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "io", "co", "ai", "app", "dev", "me", "info", "biz", "xyz",
    "tv", "ly", "gg", "to", "uk", "us", "eu", "de", "fr", "es", "it", "nl", "se", "no", "fi", "dk",
    "ch", "at", "be", "ie", "cz", "ru", "ua", "tr", "cn", "jp", "kr", "hk", "sg", "au", "nz", "ca",
    "mx", "br", "ar", "za",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextForm {
    Plain,
    Link,
    Email,
}

pub fn text_form(text: &str) -> TextForm {
    if text.len() > DETECTION_LIMIT {
        return TextForm::Plain;
    }
    let token = text.trim();
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return TextForm::Plain;
    }
    if let Some(form) = scheme_form(token) {
        return form;
    }
    if is_address(token) {
        return TextForm::Email;
    }
    if is_bare_domain(token) {
        TextForm::Link
    } else {
        TextForm::Plain
    }
}

fn scheme_form(token: &str) -> Option<TextForm> {
    if token.len() >= 7 && token[..7].eq_ignore_ascii_case("mailto:") {
        return Some(TextForm::Email);
    }
    let sep = token.find("://")?;
    let scheme = &token[..sep];
    if scheme.is_empty() {
        return None;
    }
    if !scheme
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '+' || c == '-' || c == '.')
    {
        return None;
    }
    Some(TextForm::Link)
}

fn is_address(token: &str) -> bool {
    let mut parts = token.split('@');
    let local = parts.next().unwrap_or("");
    let host = parts.next();
    let rest = parts.next();
    if rest.is_some() || local.is_empty() {
        return false;
    }
    host.is_some_and(|h| top_level_label(h).is_some())
}

fn is_bare_domain(token: &str) -> bool {
    let host_end = token.find(|c| "/?#:".contains(c)).unwrap_or(token.len());
    let host = &token[..host_end];
    if host.len() >= 4 && host[..4].eq_ignore_ascii_case("www.") {
        return true;
    }
    if host.chars().any(|c| c.is_uppercase()) {
        return false;
    }
    match top_level_label(host) {
        Some(tld) => COMMON_TLDS.contains(&tld),
        None => false,
    }
}

fn top_level_label(host: &str) -> Option<&str> {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|l| is_host_label(l)) {
        return None;
    }
    let tld = *labels.last()?;
    if tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(tld)
    } else {
        None
    }
}

fn is_host_label(label: &str) -> bool {
    !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier_url_is_link_not_text() {
        assert_eq!(text_form("https://example.com"), TextForm::Link);
        assert_eq!(text_form("not a url"), TextForm::Plain);
    }
}
