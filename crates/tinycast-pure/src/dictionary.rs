//! ECDICT data model and the plain-text formatter for the lookup panel.
//!
//! `SCHEMA` is the one schema definition, used by the build example and by the
//! read-only store. `format_entry` is the one place that decides what the panel
//! shows, so every stored field has exactly one rendering and nothing is
//! summarised or dropped.

use crate::i18n::{
    dictionary_bnc_label, dictionary_closest, dictionary_coca_label, dictionary_collins_label,
    dictionary_exchange_label, dictionary_no_entry, dictionary_no_entry_detail,
    dictionary_oxford_label, dictionary_section_chinese, dictionary_section_english,
    dictionary_section_forms, dictionary_section_frequency, dictionary_section_more,
    dictionary_section_pos, dictionary_tag_label, UiLang,
};

/// The dictionary database the app opens read-only:
/// `%APPDATA%\com.tinycast.win\Dictionary\ecdict.sqlite`.
///
/// `sw` is the folded search key (`search_key`), so a prefix scan is a range
/// scan over the index instead of a `LIKE` over 770k rows.
pub const SCHEMA: &str = "\
CREATE TABLE words (
  word TEXT PRIMARY KEY COLLATE NOCASE,
  sw TEXT NOT NULL,
  phonetic TEXT NOT NULL,
  definition TEXT NOT NULL,
  translation TEXT NOT NULL,
  pos TEXT NOT NULL,
  collins INTEGER NOT NULL,
  oxford INTEGER NOT NULL,
  tag TEXT NOT NULL,
  bnc INTEGER NOT NULL,
  frq INTEGER NOT NULL,
  exchange TEXT NOT NULL,
  detail TEXT NOT NULL,
  audio TEXT NOT NULL
) WITHOUT ROWID;
CREATE INDEX words_sw ON words(sw);
PRAGMA user_version = 1;
";

/// Prefix suggestions shown under a miss; the panel stays read-only.
pub const MAX_SUGGESTIONS: usize = 8;

/// One row of `words`, exactly as ECDICT ships it. Empty text fields are empty
/// strings, numeric fields are 0, so the formatter can skip them without a
/// second "is it set" flag.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DictionaryEntry {
    pub word: String,
    pub phonetic: String,
    pub definition: String,
    pub translation: String,
    pub pos: String,
    pub collins: i64,
    pub oxford: i64,
    pub tag: String,
    pub bnc: i64,
    pub frq: i64,
    pub exchange: String,
    pub detail: String,
    pub audio: String,
}

/// ECDICT's `exchange` codes: one letter per inflected form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExchangeKind {
    /// `0` — the lemma this word is an inflection of.
    Lemma,
    /// `p` — past tense.
    Past,
    /// `d` — past participle.
    PastParticiple,
    /// `i` — present participle.
    PresentParticiple,
    /// `3` — third person singular.
    ThirdPerson,
    /// `r` — comparative.
    Comparative,
    /// `t` — superlative.
    Superlative,
    /// `s` — plural.
    Plural,
    /// `1` — the inflections of this lemma.
    DerivedForms,
}

impl ExchangeKind {
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "0" => Some(Self::Lemma),
            "p" => Some(Self::Past),
            "d" => Some(Self::PastParticiple),
            "i" => Some(Self::PresentParticiple),
            "3" => Some(Self::ThirdPerson),
            "r" => Some(Self::Comparative),
            "t" => Some(Self::Superlative),
            "s" => Some(Self::Plural),
            "1" => Some(Self::DerivedForms),
            _ => None,
        }
    }
}

/// The `sw` column and the prefix range: lowercase, ASCII alphanumeric only.
pub fn search_key(word: &str) -> String {
    word.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// `d:given/p:gave/0:give` → the known pairs, in ECDICT's own order. Unknown
/// codes and empty values are dropped rather than printed raw.
pub fn parse_exchange(raw: &str) -> Vec<(ExchangeKind, String)> {
    raw.split('/')
        .filter_map(|item| {
            let (code, word) = item.split_once(':')?;
            let word = word.trim();
            if word.is_empty() {
                return None;
            }
            let kind = ExchangeKind::from_code(code.trim())?;
            Some((kind, word.to_string()))
        })
        .collect()
}

/// Every non-empty field of `entry` as the panel's text: no truncation, no
/// summarising, one section per kind of content.
pub fn format_entry(entry: &DictionaryEntry, lang: UiLang) -> String {
    let mut out = String::new();
    out.push_str(entry.word.trim());
    let phonetic = entry.phonetic.trim();
    if !phonetic.is_empty() {
        out.push_str("  ");
        out.push_str(phonetic);
    }
    let badges = badge_line(entry, lang);
    if !badges.is_empty() {
        out.push('\n');
        out.push_str(&badges);
    }
    push_section(&mut out, dictionary_section_chinese(lang), &entry.translation);
    push_section(&mut out, dictionary_section_english(lang), &entry.definition);
    push_forms(&mut out, entry, lang);
    push_section(&mut out, dictionary_section_pos(lang), &pos_line(&entry.pos));
    let ranks = rank_line(entry, lang);
    push_section(&mut out, dictionary_section_frequency(lang), &ranks);
    push_section(&mut out, dictionary_section_more(lang), &raw_extras(entry));
    out
}

/// The miss panel: the query, then the closest headwords the store found.
pub fn format_no_match(query: &str, suggestions: &[String], lang: UiLang) -> String {
    let query = query.trim();
    let mut out = String::from(dictionary_no_entry(lang));
    if !query.is_empty() {
        out.push('\n');
        out.push_str(&dictionary_no_entry_detail(query, lang));
    }
    let mut shown = suggestions.iter().filter(|s| !s.is_empty());
    if shown.clone().next().is_some() {
        out.push_str("\n\n");
        out.push_str(dictionary_closest(lang));
        for suggestion in shown.by_ref().take(MAX_SUGGESTIONS) {
            out.push('\n');
            out.push_str(suggestion);
        }
    }
    out
}

fn badge_line(entry: &DictionaryEntry, lang: UiLang) -> String {
    let mut badges: Vec<String> = Vec::new();
    if entry.collins > 0 {
        badges.push(format!(
            "{} {}",
            dictionary_collins_label(lang),
            "★".repeat(entry.collins.clamp(1, 5) as usize)
        ));
    }
    if entry.oxford > 0 {
        badges.push(dictionary_oxford_label(lang).to_string());
    }
    for tag in entry.tag.split_whitespace() {
        badges.push(match dictionary_tag_label(tag, lang) {
            Some(label) => label.to_string(),
            None => tag.to_string(),
        });
    }
    badges.join(" · ")
}

fn rank_line(entry: &DictionaryEntry, lang: UiLang) -> String {
    let mut ranks: Vec<String> = Vec::new();
    if entry.bnc > 0 {
        ranks.push(format!(
            "{} {}",
            dictionary_bnc_label(lang),
            thousands(entry.bnc)
        ));
    }
    if entry.frq > 0 {
        ranks.push(format!(
            "{} {}",
            dictionary_coca_label(lang),
            thousands(entry.frq)
        ));
    }
    ranks.join(" · ")
}

/// `v:72/n:28` → `v 72% · n 28%`. Tokens without a share stay as they are.
fn pos_line(raw: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for token in raw.replace('\\', " ").split('/') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        parts.push(match token.split_once(':') {
            Some((kind, share)) if !share.trim().is_empty() => {
                format!("{} {}%", kind.trim(), share.trim())
            }
            _ => token.to_string(),
        });
    }
    parts.join(" · ")
}

fn raw_extras(entry: &DictionaryEntry) -> String {
    let mut extras: Vec<&str> = Vec::new();
    let detail = entry.detail.trim();
    if !detail.is_empty() {
        extras.push(detail);
    }
    let audio = entry.audio.trim();
    if !audio.is_empty() {
        extras.push(audio);
    }
    extras.join("\n")
}

fn push_forms(out: &mut String, entry: &DictionaryEntry, lang: UiLang) {
    let forms = parse_exchange(&entry.exchange);
    if forms.is_empty() {
        return;
    }
    let mut body = String::new();
    for (index, (kind, word)) in forms.iter().enumerate() {
        if index > 0 {
            body.push('\n');
        }
        body.push_str(dictionary_exchange_label(*kind, lang));
        body.push(' ');
        body.push_str(word);
    }
    push_section(out, dictionary_section_forms(lang), &body);
}

/// Appends `\n\n<header>\n<body>` when the body has any line left.
fn push_section(out: &mut String, header: &str, body: &str) {
    let lines = body_lines(body);
    if lines.is_empty() {
        return;
    }
    out.push_str("\n\n");
    out.push_str(header);
    out.push('\n');
    out.push_str(&lines.join("\n"));
}

/// ECDICT packs multi-line fields with a literal `\n`; both that and a real
/// newline break lines, and blank lines are dropped so sections stay tight.
fn body_lines(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    let expanded = raw.replace("\\r", "\n").replace("\\n", "\n");
    expanded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn thousands(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    if value < 0 {
        out.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}
