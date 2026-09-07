//! Settings → Notes: feature switch.

use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use crate::design_system::settings::{self as ds, RowTrailing};
use crate::features::launcher::settings::items::Formats;

pub const ENABLE_TITLE: &str = "Enable Notes";
pub const ENABLE_SUBTITLE: &str =
    "A local Markdown editor. Off by default; enabling does not create notes until you ask.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotesHit {
    Enable,
}

pub fn section_header() -> &'static str {
    "Notes"
}

pub fn content_height() -> f32 {
    let (section, _, _) =
        ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), false);
    section.next_y() + ds::CARD_INSET
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<NotesHit> {
    let (_, enable, _) =
        ds::feature_switch_section(420.0, ds::CARD_INSET - scroll, section_header(), false);
    if y >= enable.y && y < enable.y + enable.h {
        Some(NotesHit::Enable)
    } else {
        None
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (section, enable, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Notes,
            formats.lang,
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
    ds::paint_settings_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::notes_enable_title(formats.lang),
        tinycast_pure::i18n::notes_enable_subtitle(formats.lang),
        enable.y,
        width,
        enable.x + ds::CARD_PAD,
        true,
        appearance,
        RowTrailing::Toggle(enabled),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_switch_section_titles() {
        assert_eq!(section_header(), "Notes");
    }

    #[test]
    fn enable_is_hittable() {
        let (_, enable, _) =
            ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), false);
        assert_eq!(hit(20.0, enable.y + 4.0, 0.0), Some(NotesHit::Enable));
    }
}
