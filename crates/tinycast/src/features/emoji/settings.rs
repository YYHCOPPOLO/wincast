//! Settings → Emoji & Symbols: skin tone.

use tinycast_pure::emoji::EmojiSkinTone;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

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

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    tone_raw: &str,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let lang = formats.lang;
    let (section, enable, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Emoji,
            lang,
        ),
        false,
    );
    ds::paint_grouped_section(
        target,
        formats.header,
        formats.caption,
        &section,
        appearance,
    )?;
    let tone = EmojiSkinTone::from_raw(tone_raw);
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::emoji_skin_tone_title(lang),
        tinycast_pure::i18n::emoji_skin_tone_subtitle(lang),
        enable.y,
        width,
        true,
        appearance,
        ds::RowTrailing::Label(tinycast_pure::i18n::emoji_skin_tone_label(
            tone.as_raw(),
            lang,
        )),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
