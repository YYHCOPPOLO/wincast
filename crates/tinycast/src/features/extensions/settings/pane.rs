//! Settings → Extensions: original switch, no JS runtime.

use tinycast_pure::extensions::RUNTIME_ABSENT;

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;
const NOTICE_H: f32 = 40.0;

pub const ENABLE_TITLE: &str = "Enable extensions";
pub const ENABLE_SUBTITLE: &str =
    "Run Raycast extensions natively. A running command holds a JavaScript engine in memory until you leave it.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const LAUNCHER_SUBTITLE: &str = "List every extension's commands in launcher search.";
pub const RUNTIME_NOTICE: &str = RUNTIME_ABSENT;

pub fn section_header() -> &'static str {
    "Extensions"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionsHit {
    Enable,
    ShowInLauncher,
}

pub fn content_height() -> f32 {
    ds::switch_section_next_y(true) + NOTICE_H + 24.0
}

fn row_y(i: usize) -> f32 {
    ds::form_origin() + i as f32 * ROW_H
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<ExtensionsHit> {
    let y = y + scroll;
    if y >= row_y(0) && y < row_y(0) + ROW_H {
        return Some(ExtensionsHit::Enable);
    }
    if y >= row_y(1) && y < row_y(1) + ROW_H {
        return Some(ExtensionsHit::ShowInLauncher);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_switch_is_visible_and_runtime_copy_matches_reject() {
        assert_eq!(ENABLE_TITLE, "Enable extensions");
        assert_eq!(SHOW_IN_LAUNCHER, "Show in launcher");
        assert_eq!(RUNTIME_NOTICE, RUNTIME_ABSENT);
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0),
            Some(ExtensionsHit::Enable)
        );
        assert_eq!(
            hit(20.0, row_y(1) + 4.0, 0.0),
            Some(ExtensionsHit::ShowInLauncher)
        );
        assert!(content_height() > row_y(1) + ROW_H);
        let (_, _, show) =
            ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), true);
        assert_eq!(
            hit(20.0, show.unwrap().y + 4.0, 0.0),
            Some(ExtensionsHit::ShowInLauncher)
        );
    }
}
