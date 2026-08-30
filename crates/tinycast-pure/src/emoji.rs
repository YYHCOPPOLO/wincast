//! Bundled compact emoji catalog. Search is case-insensitive substring on name/keywords.

pub const CELL_DIP: f32 = 56.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmojiSkinTone {
    None,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl EmojiSkinTone {
    pub fn from_raw(raw: &str) -> Self {
        match raw {
            "light" => Self::Light,
            "medium-light" => Self::MediumLight,
            "medium" => Self::Medium,
            "medium-dark" => Self::MediumDark,
            "dark" => Self::Dark,
            _ => Self::None,
        }
    }

    pub fn modifier(self) -> Option<char> {
        Some(match self {
            Self::None => return None,
            Self::Light => '\u{1F3FB}',
            Self::MediumLight => '\u{1F3FC}',
            Self::Medium => '\u{1F3FD}',
            Self::MediumDark => '\u{1F3FE}',
            Self::Dark => '\u{1F3FF}',
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Emoji {
    pub glyph: String,
    pub name: String,
    pub keywords: &'static [&'static str],
    pub group: &'static str,
    pub skinnable: bool,
}

const CATALOG: &[(&str, &str, &[&str], &str, bool)] = &[
    ("😀", "grinning face", &["smile", "happy"], "Smileys", false),
    ("😃", "grinning face with big eyes", &["smile", "happy"], "Smileys", false),
    ("😄", "grinning face with smiling eyes", &["smile", "happy"], "Smileys", false),
    ("😁", "beaming face with smiling eyes", &["smile"], "Smileys", false),
    ("😆", "grinning squinting face", &["laugh", "smile"], "Smileys", false),
    ("😅", "grinning face with sweat", &["smile"], "Smileys", false),
    ("🤣", "rolling on the floor laughing", &["laugh"], "Smileys", false),
    ("😂", "face with tears of joy", &["laugh", "smile"], "Smileys", false),
    ("🙂", "slightly smiling face", &["smile"], "Smileys", false),
    ("😊", "smiling face with smiling eyes", &["smile", "happy"], "Smileys", false),
    ("😇", "smiling face with halo", &["smile"], "Smileys", false),
    ("☺", "smiling face", &["smile"], "Smileys", false),
    ("😍", "smiling face with heart-eyes", &["love", "smile"], "Smileys", false),
    ("🤩", "star-struck", &["smile"], "Smileys", false),
    ("😘", "face blowing a kiss", &["love"], "Smileys", false),
    ("👍", "thumbs up", &["yes", "ok"], "People", true),
    ("👎", "thumbs down", &["no"], "People", true),
    ("👏", "clapping hands", &["applause"], "People", true),
    ("🙏", "folded hands", &["please", "thanks"], "People", true),
    ("👋", "waving hand", &["hello", "hi"], "People", true),
    ("🔥", "fire", &["hot"], "Symbols", false),
    ("✨", "sparkles", &["star"], "Symbols", false),
    ("⭐", "star", &["star"], "Symbols", false),
    ("❤️", "red heart", &["love", "heart"], "Symbols", false),
    ("💯", "hundred points", &["100"], "Symbols", false),
    ("✅", "check mark button", &["check", "done"], "Symbols", false),
    ("❌", "cross mark", &["x", "no"], "Symbols", false),
    ("🎉", "party popper", &["party", "tada"], "Activities", false),
    ("🚀", "rocket", &["ship"], "Travel", false),
    ("💡", "light bulb", &["idea"], "Objects", false),
    ("📝", "memo", &["note", "write"], "Objects", false),
    ("🔗", "link", &["url"], "Objects", false),
];

pub fn catalog() -> Vec<Emoji> {
    CATALOG
        .iter()
        .map(|(g, n, k, group, skin)| Emoji {
            glyph: (*g).into(),
            name: (*n).into(),
            keywords: k,
            group: group,
            skinnable: *skin,
        })
        .collect()
}

pub fn search_emoji(query: &str) -> Vec<Emoji> {
    search_emoji_with_tone(query, EmojiSkinTone::None)
}

pub fn search_emoji_with_tone(query: &str, tone: EmojiSkinTone) -> Vec<Emoji> {
    let q = query.trim().to_lowercase();
    catalog()
        .into_iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.keywords.iter().any(|k| k.contains(&q))
        })
        .map(|mut e| {
            if e.skinnable {
                if let Some(modch) = tone.modifier() {
                    e.glyph.push(modch);
                }
            }
            e
        })
        .collect()
}

pub fn columns_for_width(width_dip: f32) -> usize {
    ((width_dip / CELL_DIP).floor() as usize).max(1)
}

pub fn cell_index(columns: usize, row: usize, col: usize) -> usize {
    row * columns.max(1) + col
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_search_matches_short_name() {
        let hits = search_emoji("smile");
        assert!(hits
            .iter()
            .any(|e| e.glyph.contains('☺') || e.glyph.contains('😊') || e.name.contains("smile")));
    }

    #[test]
    fn grid_cell_is_56_dip() {
        assert_eq!(CELL_DIP, 56.0);
        assert!(columns_for_width(750.0) >= 10);
    }
}
