//! Settings → Notes: feature switch.

use crate::design_system::settings as ds;

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
