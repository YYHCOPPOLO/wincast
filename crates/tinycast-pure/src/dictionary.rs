//! ECDICT data model, the structured panel view and the plain-text formatter.
//!
//! `SCHEMA` is the one schema definition, used by the build example and by the
//! read-only store. `view_entry` is the one place that decides what the panel
//! shows: the header line and one ordered list of rows. `format_entry` renders
//! that same view as plain text for the clipboard, so every stored field has
//! exactly one rendering and nothing is summarised or dropped.

use crate::i18n::{
    dictionary_bnc_label, dictionary_coca_label, dictionary_collins_label,
    dictionary_exchange_label, dictionary_oxford_label, dictionary_section_chinese,
    dictionary_section_english, dictionary_section_forms, dictionary_section_frequency,
    dictionary_section_more, dictionary_section_pos, dictionary_tag_label, UiLang,
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

/// Extra space above a section title, in DIP. The first section starts at the
/// body's own top inset, so it carries no gap.
pub const SECTION_GAP: f32 = 14.0;

/// One row of `words`, exactly as ECDICT ships it. Empty text fields are empty
/// strings, numeric fields are 0, so the view can skip them without a second
/// "is it set" flag.
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
    /// `f` — the plural variant ECDICT stores next to `s` (`frescos` for `fresco`).
    AlternatePlural,
    /// `1` — the inflections of this lemma.
    DerivedForms,
}

impl ExchangeKind {
    fn from_char(code: char) -> Option<Self> {
        match code {
            '0' => Some(Self::Lemma),
            'p' => Some(Self::Past),
            'd' => Some(Self::PastParticiple),
            'i' => Some(Self::PresentParticiple),
            '3' => Some(Self::ThirdPerson),
            'r' => Some(Self::Comparative),
            't' => Some(Self::Superlative),
            's' => Some(Self::Plural),
            'f' => Some(Self::AlternatePlural),
            _ => None,
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        match (chars.next(), chars.next()) {
            (Some(code), None) if code == '1' => Some(Self::DerivedForms),
            (Some(code), None) => Self::from_char(code),
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

/// The tag tone a badge renders with: Collins amber, Oxford accent, exam tags
/// neutral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeTone {
    Neutral,
    Accent,
    Amber,
}

/// One chip under the headword.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DictionaryBadge {
    pub text: String,
    pub tone: BadgeTone,
}

/// How one row of the panel body renders. Section rows carry the title in
/// `text`; form rows carry the exchange label in `label`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictionaryRowKind {
    Section,
    Chinese,
    English,
    Form,
    Pos,
    Stat,
    Note,
}

/// One row of the panel body. `gap` is the extra space above it, in DIP.
#[derive(Clone, Debug, PartialEq)]
pub struct DictionaryRow {
    pub kind: DictionaryRowKind,
    pub label: String,
    pub text: String,
    pub gap: f32,
}

/// The whole panel: the header card (word, phonetic, badges) plus the rows the
/// body renders top to bottom.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DictionaryView {
    pub word: String,
    pub phonetic: String,
    pub badges: Vec<DictionaryBadge>,
    pub rows: Vec<DictionaryRow>,
}

/// Every non-empty field of `entry` as the panel's rows: one section per kind
/// of content, every line kept, nothing summarised. A section with no body
/// line is not a section: ECDICT leaves `definition` and `translation` empty
/// for plenty of headwords, and a bare title reads as a bug.
pub fn view_entry(entry: &DictionaryEntry, lang: UiLang) -> DictionaryView {
    let mut view = DictionaryView {
        word: entry.word.trim().to_string(),
        phonetic: entry.phonetic.trim().to_string(),
        badges: view_badges(entry, lang),
        rows: Vec::new(),
    };

    let chinese = body_lines(&entry.translation);
    if !chinese.is_empty() {
        push_section(&mut view.rows, dictionary_section_chinese(lang));
        push_lines(&mut view.rows, DictionaryRowKind::Chinese, chinese);
    }

    let english = body_lines(&entry.definition);
    if !english.is_empty() {
        push_section(&mut view.rows, dictionary_section_english(lang));
        push_lines(&mut view.rows, DictionaryRowKind::English, english);
    }

    let forms = form_rows(entry, lang);
    if !forms.is_empty() {
        push_section(&mut view.rows, dictionary_section_forms(lang));
        for (label, text) in forms {
            view.rows.push(DictionaryRow {
                kind: DictionaryRowKind::Form,
                label,
                text,
                gap: 0.0,
            });
        }
    }

    let pos = pos_line(&entry.pos);
    if !pos.is_empty() {
        push_section(&mut view.rows, dictionary_section_pos(lang));
        view.rows.push(DictionaryRow {
            kind: DictionaryRowKind::Pos,
            label: String::new(),
            text: pos,
            gap: 0.0,
        });
    }

    let ranks = rank_line(entry, lang);
    if !ranks.is_empty() {
        push_section(&mut view.rows, dictionary_section_frequency(lang));
        view.rows.push(DictionaryRow {
            kind: DictionaryRowKind::Stat,
            label: String::new(),
            text: ranks,
            gap: 0.0,
        });
    }

    let extras = extra_lines(entry);
    if !extras.is_empty() {
        push_section(&mut view.rows, dictionary_section_more(lang));
        for line in extras {
            view.rows.push(DictionaryRow {
                kind: DictionaryRowKind::Note,
                label: String::new(),
                text: line,
                gap: 0.0,
            });
        }
    }

    view
}

/// The whole entry as plain text for the clipboard: the header line, then the
/// same rows the panel draws, so a copy never says less than the panel.
pub fn format_entry(entry: &DictionaryEntry, lang: UiLang) -> String {
    let view = view_entry(entry, lang);
    let mut out = String::new();
    out.push_str(&view.word);
    if !view.phonetic.is_empty() {
        out.push_str("  ");
        out.push_str(&view.phonetic);
    }
    if !view.badges.is_empty() {
        out.push('\n');
        let texts: Vec<&str> = view.badges.iter().map(|badge| badge.text.as_str()).collect();
        out.push_str(&texts.join(" · "));
    }
    for row in &view.rows {
        match row.kind {
            DictionaryRowKind::Section => {
                out.push_str("\n\n");
                out.push_str(&row.text);
            }
            _ => {
                out.push('\n');
                if row.kind == DictionaryRowKind::Form && !row.label.is_empty() {
                    out.push_str(&row.label);
                    out.push(' ');
                }
                out.push_str(&row.text);
            }
        }
    }
    out
}

fn view_badges(entry: &DictionaryEntry, lang: UiLang) -> Vec<DictionaryBadge> {
    let mut badges: Vec<DictionaryBadge> = Vec::new();
    if entry.collins > 0 {
        badges.push(DictionaryBadge {
            text: format!(
                "{} {}",
                dictionary_collins_label(lang),
                "★".repeat(entry.collins.clamp(1, 5) as usize)
            ),
            tone: BadgeTone::Amber,
        });
    }
    if entry.oxford > 0 {
        badges.push(DictionaryBadge {
            text: dictionary_oxford_label(lang).to_string(),
            tone: BadgeTone::Accent,
        });
    }
    for tag in entry.tag.split_whitespace() {
        badges.push(DictionaryBadge {
            text: match dictionary_tag_label(tag, lang) {
                Some(label) => label.to_string(),
                None => tag.to_string(),
            },
            tone: BadgeTone::Neutral,
        });
    }
    badges
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

/// `detail` then `audio`, one line each: ECDICT's leftover columns. `""` is a
/// literal ECDICT writes for `a` and for a third of the corpus, not content.
fn extra_lines(entry: &DictionaryEntry) -> Vec<String> {
    let mut extras: Vec<String> = Vec::new();
    for line in body_lines(&entry.detail) {
        if line != "\"\"" {
            extras.push(line);
        }
    }
    extras.extend(body_lines(&entry.audio));
    extras
}

/// The lemma first, then ECDICT's own order. `1:` holds codes rather than
/// words, so it renders as the names of the forms the lemma has.
fn form_rows(entry: &DictionaryEntry, lang: UiLang) -> Vec<(String, String)> {
    let forms = parse_exchange(&entry.exchange);
    let mut ordered: Vec<(ExchangeKind, String)> = forms
        .iter()
        .filter(|(kind, _)| *kind == ExchangeKind::Lemma)
        .cloned()
        .collect();
    ordered.extend(
        forms
            .into_iter()
            .filter(|(kind, _)| *kind != ExchangeKind::Lemma),
    );
    let mut rows = Vec::new();
    for (kind, value) in ordered {
        let rendered = if kind == ExchangeKind::DerivedForms {
            derived_form_labels(&value, lang)
        } else {
            value
        };
        if rendered.is_empty() {
            continue;
        }
        rows.push((dictionary_exchange_label(kind, lang).to_string(), rendered));
    }
    rows
}

/// `pd` → `过去式 / 过去分词` so the `1:` column reads as the forms it names.
fn derived_form_labels(codes: &str, lang: UiLang) -> String {
    let mut labels: Vec<&'static str> = Vec::new();
    for code in codes.chars() {
        if let Some(kind) = ExchangeKind::from_char(code) {
            let label = dictionary_exchange_label(kind, lang);
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    labels.join(" / ")
}

/// Appends a section title row; every section after the first keeps the gap.
fn push_section(rows: &mut Vec<DictionaryRow>, header: &str) {
    let gap = if rows.is_empty() { 0.0 } else { SECTION_GAP };
    rows.push(DictionaryRow {
        kind: DictionaryRowKind::Section,
        label: String::new(),
        text: header.to_string(),
        gap,
    });
}

/// One row per non-empty line of a multi-line ECDICT field.
fn push_lines(rows: &mut Vec<DictionaryRow>, kind: DictionaryRowKind, lines: Vec<String>) {
    for line in lines {
        rows.push(DictionaryRow {
            kind,
            label: String::new(),
            text: line,
            gap: 0.0,
        });
    }
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
