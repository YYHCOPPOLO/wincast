//! Settings → Commands: Custom Commands switches, list, and New.

use tinycast_pure::theme;

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;
const ITEM_H: f32 = 36.0;

pub fn section_header() -> &'static str {
    "Commands"
}

pub const ENABLE_TITLE: &str = "Custom Commands";
pub const ENABLE_SUBTITLE: &str = "Run your own commands from the launcher. Off by default.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const NEW_LABEL: &str = "New command";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomCommandsHit {
    Enable,
    ShowInLauncher,
    New,
    Item(usize),
}

pub fn content_height(count: usize) -> f32 {
    ds::switch_section_next_y(true)
        + ITEM_H
        + theme::spacing::SM
        + ITEM_H * count as f32
        + theme::spacing::SECTION_SPACING
}

pub fn hit(x: f32, y: f32, scroll: f32, count: usize) -> Option<CustomCommandsHit> {
    let y = y + scroll;
    let mut row = ds::form_origin();
    if y >= row && y < row + ROW_H {
        return Some(CustomCommandsHit::Enable);
    }
    row += ROW_H;
    if y >= row && y < row + ROW_H {
        return Some(CustomCommandsHit::ShowInLauncher);
    }
    row = ds::switch_section_next_y(true);
    if y >= row && y < row + ITEM_H {
        return Some(CustomCommandsHit::New);
    }
    row += ITEM_H + theme::spacing::SM;
    for i in 0..count {
        if y >= row && y < row + ITEM_H {
            let _ = x;
            return Some(CustomCommandsHit::Item(i));
        }
        row += ITEM_H;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_commands_settings_exposes_feature_switch() {
        assert_eq!(ENABLE_TITLE, "Custom Commands");
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0, 0),
            Some(CustomCommandsHit::Enable)
        );
        assert_eq!(
            hit(20.0, ds::form_origin() + ROW_H + 4.0, 0.0, 0),
            Some(CustomCommandsHit::ShowInLauncher)
        );
        assert_eq!(
            hit(20.0, ds::switch_section_next_y(true) + 4.0, 0.0, 0),
            Some(CustomCommandsHit::New)
        );
    }
}
