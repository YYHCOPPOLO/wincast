//! Settings → Emoji & Symbols: skin tone.

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;

pub fn section_header() -> &'static str {
    "Emoji & Symbols"
}

pub const SKIN_TONE_TITLE: &str = "Skin tone";
pub const SKIN_TONE_SUBTITLE: &str = "Applied to people emoji in Search Emoji.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiHit {
    SkinTone,
}

pub fn content_height() -> f32 {
    ds::form_origin() + ROW_H + 24.0
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<EmojiHit> {
    let y = y + scroll;
    let origin = ds::form_origin();
    if y >= origin && y < origin + ROW_H {
        Some(EmojiHit::SkinTone)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::emoji::EmojiSkinTone;

    #[test]
    fn emoji_settings_exposes_skin_tone() {
        assert_eq!(SKIN_TONE_TITLE, "Skin tone");
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0),
            Some(EmojiHit::SkinTone)
        );
        assert_eq!(EmojiSkinTone::from_raw("light").label(), "Light");
    }
}
