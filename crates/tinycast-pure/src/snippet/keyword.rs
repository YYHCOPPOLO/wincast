//! Keyword matching for snippet expansion. Case-insensitive longest suffix.

pub const TIMEOUT_MS: u64 = 15_000;
pub const MAXIMUM_BUFFER_LENGTH: usize = 256;

pub fn match_suffix(buffer: &str, keywords: &[&str]) -> Option<String> {
    let normalized = buffer.to_lowercase();
    let mut best: Option<&str> = None;
    for keyword in keywords {
        let trimmed = keyword.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAXIMUM_BUFFER_LENGTH {
            continue;
        }
        let needle = trimmed.to_lowercase();
        if normalized.ends_with(&needle)
            && best.is_none_or(|prev| needle.len() > prev.to_lowercase().len())
        {
            best = Some(trimmed);
        }
    }
    best.map(str::to_string)
}

#[derive(Clone, Debug, Default)]
pub struct KeywordBuffer {
    buf: String,
    last_ms: Option<u64>,
    keywords: Vec<String>,
}

impl KeywordBuffer {
    pub fn with_keywords<I, S>(keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut buffer = Self::default();
        buffer.set_keywords(keywords);
        buffer
    }

    pub fn set_keywords<I, S>(&mut self, keywords: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut keywords: Vec<String> = keywords
            .into_iter()
            .map(|s| s.as_ref().trim().to_string())
            .filter(|s| !s.is_empty() && s.chars().count() <= MAXIMUM_BUFFER_LENGTH)
            .collect();
        keywords.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
        self.keywords = keywords;
        self.reset();
    }

    pub fn push(&mut self, ch: char, now_ms: u64) -> Option<String> {
        if self
            .last_ms
            .is_some_and(|last| now_ms.saturating_sub(last) > TIMEOUT_MS)
        {
            self.reset();
        }
        self.last_ms = Some(now_ms);
        self.buf.push(ch);
        if self.buf.chars().count() > MAXIMUM_BUFFER_LENGTH {
            let extra = self.buf.chars().count() - MAXIMUM_BUFFER_LENGTH;
            self.buf = self.buf.chars().skip(extra).collect();
        }
        let keywords: Vec<&str> = self.keywords.iter().map(String::as_str).collect();
        let hit = match_suffix(&self.buf, &keywords);
        if hit.is_some() {
            self.reset();
        }
        hit
    }

    pub fn delete_backward(&mut self, now_ms: u64) {
        self.last_ms = Some(now_ms);
        self.buf.pop();
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.last_ms = None;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeywordInput {
    Text,
    DeleteBackward,
    Reset,
    Ignored,
}

pub fn classify_input(
    is_synthetic: bool,
    is_key_down: bool,
    has_commanding_modifier: bool,
    is_reset_key: bool,
    is_delete_backward: bool,
    produces_text: bool,
) -> KeywordInput {
    if is_synthetic {
        return KeywordInput::Ignored;
    }
    if has_commanding_modifier || is_reset_key {
        return KeywordInput::Reset;
    }
    if !is_key_down {
        return KeywordInput::Ignored;
    }
    if is_delete_backward {
        return KeywordInput::DeleteBackward;
    }
    if produces_text {
        KeywordInput::Text
    } else {
        KeywordInput::Reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_suffix_wins() {
        let kw = ["!n", "!notes"];
        assert_eq!(match_suffix("xx!notes", &kw).as_deref(), Some("!notes"));
        assert_eq!(match_suffix("xx!n", &kw).as_deref(), Some("!n"));
    }

    #[test]
    fn buffer_emits_on_completed_suffix() {
        let mut b = KeywordBuffer::with_keywords(["!notes"]);
        for c in "!note".chars() {
            assert_eq!(b.push(c, 0), None);
        }
        assert_eq!(b.push('s', 0), Some("!notes".into()));
    }

    #[test]
    fn idle_timeout_resets() {
        let mut b = KeywordBuffer::with_keywords(["ab"]);
        assert_eq!(b.push('a', 0), None);
        assert_eq!(b.push('b', TIMEOUT_MS + 1), None);
    }

    #[test]
    fn synthetic_events_are_ignored() {
        assert_eq!(
            classify_input(true, true, false, false, false, true),
            KeywordInput::Ignored
        );
        assert_eq!(
            classify_input(false, true, true, false, false, true),
            KeywordInput::Reset
        );
    }
}
