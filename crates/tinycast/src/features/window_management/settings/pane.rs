//! Settings → Window Management: enable, show in launcher, gap, cycle, per-command recorders.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use tinycast_pure::window_command::{WindowCommandId, WindowGroup};

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;
const CMD_H: f32 = 40.0;
const HEADER_H: f32 = 22.0;
const RECORDER_W: f32 = theme::size::SHORTCUT_RECORDER;
const CLEAR_W: f32 = 22.0;

pub fn section_header() -> &'static str {
    "Window Management"
}

pub const ENABLE_TITLE: &str = "Window Management";
pub const ENABLE_SUBTITLE: &str = "Move and resize the frontmost window. Off by default.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const CYCLE_TITLE: &str = "Cycle halves on repeat";
pub const GAP_TITLE: &str = "Gap";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowHit {
    Enable,
    ShowInLauncher,
    Cycle,
    Gap,
    Visible(usize),
    Recorder(usize),
    RecorderClear(usize),
}

pub fn catalog_commands() -> Vec<WindowCommandId> {
    WindowCommandId::all().to_vec()
}

fn switch_card() -> ds::GroupedSection {
    let mut section = ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), true).0;
    section.body_h = ds::CARD_PAD * 2.0 + ROW_H * 4.0;
    section
}

pub fn toggles_bottom() -> f32 {
    switch_card().next_y()
}

fn toggle_row_y(index: usize) -> f32 {
    ds::form_origin() + index as f32 * ROW_H
}

pub fn recorder_well_for_action(action: &str, width: f32, scroll: f32) -> Option<DipRect> {
    let rest = action.strip_prefix("hotkey.windowCommand.")?;
    let index = WindowCommandId::all()
        .iter()
        .position(|id| id.raw() == rest)?;
    let (_, top) = catalog_layout()
        .commands
        .into_iter()
        .find(|(i, _)| *i == index)?;
    let (rec_left, _) = command_wells(width);
    Some(DipRect {
        x: rec_left,
        y: top - scroll,
        w: RECORDER_W,
        h: 24.0,
    })
}

fn group_blocks() -> Vec<(WindowGroup, Vec<WindowCommandId>)> {
    WindowGroup::all()
        .iter()
        .copied()
        .map(|group| {
            let cmds: Vec<WindowCommandId> = WindowCommandId::all()
                .iter()
                .copied()
                .filter(|id| id.group() == group)
                .collect();
            (group, cmds)
        })
        .filter(|(_, cmds)| !cmds.is_empty())
        .collect()
}

struct CatalogLayout {
    commands: Vec<(usize, f32)>,
    bottom: f32,
}

fn catalog_layout() -> CatalogLayout {
    catalog_layout_lang(tinycast_pure::i18n::UiLang::En)
}

fn catalog_layout_lang(lang: tinycast_pure::i18n::UiLang) -> CatalogLayout {
    let mut y = toggles_bottom() + theme::spacing::XL;
    let mut commands = Vec::new();
    let all = WindowCommandId::all();
    for (group, cmds) in group_blocks() {
        debug_assert!(!tinycast_pure::i18n::window_group_title(group, lang).is_empty());
        y += HEADER_H;
        for id in cmds {
            let index = all.iter().position(|c| *c == id).unwrap_or(0);
            commands.push((index, y));
            y += CMD_H;
        }
        y += theme::spacing::SM;
    }
    CatalogLayout {
        commands,
        bottom: y + 24.0,
    }
}

pub fn content_height() -> f32 {
    catalog_layout().bottom
}

pub fn hit(_x: f32, y: f32, scroll: f32, width: f32) -> Option<WindowHit> {
    let y = y + scroll;
    let toggles = [
        WindowHit::Enable,
        WindowHit::ShowInLauncher,
        WindowHit::Cycle,
        WindowHit::Gap,
    ];
    for (i, hit) in toggles.into_iter().enumerate() {
        let top = toggle_row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    let layout = catalog_layout();
    for (index, top) in layout.commands {
        if y < top || y >= top + CMD_H {
            continue;
        }
        let (rec_left, clear_left) = command_wells(width);
        if _x >= clear_left {
            return Some(WindowHit::RecorderClear(index));
        }
        if _x >= rec_left {
            return Some(WindowHit::Recorder(index));
        }
        return Some(WindowHit::Visible(index));
    }
    None
}

/// Detail-space x of the Record well and clear control. Paint adds `origin_x`.
fn command_wells(width: f32) -> (f32, f32) {
    let pad = theme::spacing::XL;
    let clear_left = width - pad - CLEAR_W;
    let rec_left = clear_left - 8.0 - RECORDER_W;
    (rec_left, clear_left)
}

pub fn cycle_gap(current: i32) -> i32 {
    match current {
        0 => 8,
        8 => 16,
        16 => 24,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_management_hits_four_rows() {
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0, 400.0),
            Some(WindowHit::Enable)
        );
        let (_, _, show) =
            ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), true);
        assert_eq!(
            hit(20.0, show.unwrap().y + 4.0, 0.0, 400.0),
            Some(WindowHit::ShowInLauncher)
        );
        assert_eq!(cycle_gap(0), 8);
        assert_eq!(cycle_gap(24), 0);
    }

    #[test]
    fn window_command_recorder_well_is_120() {
        let key = format!("hotkey.windowCommand.{}", WindowCommandId::all()[0].raw());
        let well = recorder_well_for_action(&key, 400.0, 0.0).unwrap();
        assert_eq!(well.w, theme::size::SHORTCUT_RECORDER);
        assert_eq!(RECORDER_W, 120.0);
    }

    #[test]
    fn window_management_lists_all_commands_with_recorders() {
        assert_eq!(catalog_commands().len(), 34);
        assert!(content_height() > toggles_bottom() + 34.0 * 20.0);
        let layout = catalog_layout();
        assert_eq!(layout.commands.len(), 34);
        let (_, first_y) = layout.commands[0];
        let rec = hit(350.0, first_y + 8.0, 0.0, 400.0);
        assert_eq!(rec, Some(WindowHit::Recorder(0)));
        let vis = hit(20.0, first_y + 8.0, 0.0, 400.0);
        assert_eq!(vis, Some(WindowHit::Visible(0)));
        assert_eq!(
            WindowCommandId::all()[0].entry_id(),
            "window-command:left-half"
        );
    }

    #[test]
    fn painted_record_well_hits_recorder_after_sidebar_translate() {
        let origin_x = theme::size::SETTINGS_SIDEBAR;
        let detail_w = theme::size::SETTINGS_WINDOW.0 - origin_x;
        let (rec_left, clear_left) = command_wells(detail_w);
        let layout = catalog_layout();
        let (_, first_y) = layout.commands[0];
        // paint_command draws Record at origin_x + rec_left (HWND space).
        // settings hit() gets detail_x = window_x - sidebar.
        let painted_record_window_x = origin_x + rec_left + 1.0;
        let detail_x = painted_record_window_x - origin_x;
        assert_eq!(
            hit(detail_x, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::Recorder(0))
        );
        assert_eq!(
            hit(rec_left - 1.0, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::Visible(0))
        );
        assert_eq!(
            hit(clear_left + 1.0, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::RecorderClear(0))
        );
        // Pre-fix paint used HWND x=rec_left without origin; that click is Visible.
        let old_paint_as_detail = rec_left - origin_x;
        assert!(old_paint_as_detail > 0.0);
        assert_eq!(
            hit(old_paint_as_detail + 1.0, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::Visible(0))
        );
    }
}
