//! Settings → Quick Actions. Off by default; excluded from backups.

use tinycast_pure::theme;

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickActionsHit {
    Enable,
    Language,
}

pub fn section_header() -> &'static str {
    "Quick Actions"
}

pub fn content_height() -> f32 {
    ds::form_origin() + ROW_H * 2.0 + ds::CARD_PAD + theme::spacing::SECTION_SPACING
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<QuickActionsHit> {
    let y = y + scroll;
    let origin = ds::form_origin();
    if y >= origin && y < origin + ROW_H {
        Some(QuickActionsHit::Enable)
    } else if y >= origin + ROW_H && y < origin + ROW_H * 2.0 {
        Some(QuickActionsHit::Language)
    } else {
        None
    }
}

pub fn cycle_language(current: &str) -> &'static str {
    match current {
        "Spanish" => "French",
        "French" => "German",
        "German" => "English",
        _ => "Spanish",
    }
}
