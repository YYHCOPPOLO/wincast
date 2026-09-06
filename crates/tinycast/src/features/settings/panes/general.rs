//! Settings → General: shortcuts, search, Hyper, appearance, launch.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_ROUNDED_RECT,
};

use crate::design_system::settings::{
    self as ds, GroupedSection, RowTrailing, CARD_PAD, ROW_H,
};
use crate::features::hotkeys::service::hyper::HyperKey;
use crate::features::launcher::settings::items::Formats;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralToggle {
    Compact,
    FavoritesInCompact,
    FollowCursor,
    Draggable,
    LaunchAtLogin,
    ShowInMenuBar,
    AutoSwitch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralHit {
    PaletteRecorder,
    ResetRanking,
    HyperKey,
    HyperShift,
    Appearance,
    Compact,
    FavoritesInCompact,
    FollowCursor,
    Draggable,
    LaunchAtLogin,
    ShowInMenuBar,
    PopToRoot,
    AutoSwitchInput,
}

pub struct GeneralState<'a> {
    pub ranking_empty: bool,
    pub hyper: &'a str,
    pub hyper_shift: bool,
    pub appearance: &'a str,
    pub compact: bool,
    pub favorites_in_compact: bool,
    pub follow_cursor: bool,
    pub draggable: bool,
    pub launch_at_login: bool,
    pub show_in_menu_bar: bool,
    pub pop_to_root: i64,
    pub auto_switch: bool,
    pub chrome: u8,
}

pub fn general_sections() -> [&'static str; 5] {
    [
        "Global Shortcuts",
        "Search",
        "Hyper Key",
        "Appearance",
        "General",
    ]
}

fn section_specs() -> [(&'static str, Option<&'static str>, &'static [GeneralHit]); 5] {
    [
        (
            "Global Shortcuts",
            Some("Summon the fuzzy app launcher."),
            &[GeneralHit::PaletteRecorder],
        ),
        (
            "Search",
            Some("Tinycast privately learns which results you choose for each query. Reset all learned choices to restore the default order."),
            &[GeneralHit::ResetRanking],
        ),
        (
            "Hyper Key",
            None,
            &[GeneralHit::HyperKey, GeneralHit::HyperShift],
        ),
        (
            "Appearance",
            None,
            &[
                GeneralHit::Appearance,
                GeneralHit::Compact,
                GeneralHit::FavoritesInCompact,
                GeneralHit::FollowCursor,
                GeneralHit::Draggable,
            ],
        ),
        (
            "General",
            None,
            &[
                GeneralHit::LaunchAtLogin,
                GeneralHit::ShowInMenuBar,
                GeneralHit::PopToRoot,
                GeneralHit::AutoSwitchInput,
            ],
        ),
    ]
}

fn grouped_at(y: f32, width: f32, header: &'static str, footer: Option<&'static str>, rows: usize) -> GroupedSection {
    GroupedSection {
        header: Some(header),
        footer,
        y,
        width,
        body_h: CARD_PAD * 2.0 + ROW_H * rows as f32,
    }
}

fn row_rect(card: DipRect, index: usize) -> DipRect {
    DipRect {
        x: card.x,
        y: card.y + CARD_PAD + index as f32 * ROW_H,
        w: card.w,
        h: ROW_H,
    }
}

pub fn layout_general(scroll: f32) -> Vec<(GeneralHit, DipRect)> {
    layout_general_sized(
        theme::size::SETTINGS_WINDOW.0 - theme::size::SETTINGS_SIDEBAR,
        scroll,
    )
}

fn layout_general_sized(width: f32, scroll: f32) -> Vec<(GeneralHit, DipRect)> {
    let mut y = ds::CARD_INSET;
    let mut hits = Vec::new();
    for (header, footer, rows) in section_specs() {
        let section = grouped_at(y, width, header, footer, rows.len());
        let card = section.card_rect();
        for (i, hit) in rows.iter().copied().enumerate() {
            let mut rect = row_rect(card, i);
            rect.y -= scroll;
            hits.push((hit, rect));
        }
        y = section.next_y();
    }
    hits
}

pub fn content_height() -> f32 {
    let width = theme::size::SETTINGS_WINDOW.0 - theme::size::SETTINGS_SIDEBAR;
    let mut y = ds::CARD_INSET;
    for (header, footer, rows) in section_specs() {
        y = grouped_at(y, width, header, footer, rows.len()).next_y();
    }
    y + ds::CARD_INSET
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<GeneralHit> {
    for (hit, rect) in layout_general(scroll) {
        if y >= rect.y && y < rect.y + rect.h {
            return Some(hit);
        }
    }
    None
}

pub fn hyper_includes_shift_enabled(hyper: &str) -> bool {
    hyper != "none"
}

pub fn hyper_subtitle(raw: &str) -> &'static str {
    match HyperKey::from_raw(raw) {
        HyperKey::CapsLock => "Caps Lock. Takes effect after logoff; cleared on quit.",
        HyperKey::None => "Off",
        other => other.title(),
    }
}

pub fn cycle_hyper(current: &str) -> &'static str {
    match current {
        "none" => "capsLock",
        "capsLock" => "rightControl",
        "rightControl" => "rightShift",
        "rightShift" => "rightOption",
        "rightOption" => "rightCommand",
        _ => "none",
    }
}

pub fn cycle_pop_to_root(current: i64) -> i64 {
    match current {
        0 => 10,
        10 => 30,
        30 => 60,
        _ => 0,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    state: &GeneralState<'_>,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let ranking_sub = if state.ranking_empty {
        "No learned ranking yet."
    } else {
        "Clear privately learned result order."
    };
    let pop_sub = format!("{} s idle timeout (0 = never).", state.pop_to_root);
    let hyper_on = hyper_includes_shift_enabled(state.hyper);
    let hyper_title = HyperKey::from_raw(state.hyper).title();
    let mut y = ds::CARD_INSET - scroll;
    for (header, footer, rows) in section_specs() {
        let section = grouped_at(y, width, header, footer, rows.len());
        ds::paint_grouped_section(
            target,
            formats.header,
            formats.caption,
            &section,
            state.chrome,
        )?;
        let card = section.card_rect();
        for (i, hit) in rows.iter().copied().enumerate() {
            let rect = row_rect(card, i);
            let pad_x = rect.x + CARD_PAD;
            let (title, sub, enabled, trailing) = match hit {
                GeneralHit::PaletteRecorder => (
                    "App Launcher",
                    "Toggle palette recorder — click to rebind.",
                    true,
                    RowTrailing::None,
                ),
                GeneralHit::ResetRanking => (
                    "Learned ranking",
                    ranking_sub,
                    !state.ranking_empty,
                    RowTrailing::Label("Reset…"),
                ),
                GeneralHit::HyperKey => (
                    "Hyper Key",
                    hyper_subtitle(state.hyper),
                    true,
                    RowTrailing::Label(hyper_title),
                ),
                GeneralHit::HyperShift => (
                    "Include Shift",
                    "Hyper Key will remap with Shift in the chord.",
                    hyper_on,
                    RowTrailing::Toggle(state.hyper_shift && hyper_on),
                ),
                GeneralHit::Appearance => (
                    "Theme",
                    "Match the system, or pin Light or Dark.",
                    true,
                    RowTrailing::Label(state.appearance),
                ),
                GeneralHit::Compact => (
                    "Compact mode",
                    "Open the launcher as a slim search bar.",
                    true,
                    RowTrailing::Toggle(state.compact),
                ),
                GeneralHit::FavoritesInCompact => (
                    "Show favorites in compact mode",
                    "Pin favorite app icons to the compact bar.",
                    state.compact,
                    RowTrailing::Toggle(state.favorites_in_compact),
                ),
                GeneralHit::FollowCursor => (
                    "Follow the cursor",
                    "Open the launcher on the pointer’s display.",
                    true,
                    RowTrailing::Toggle(state.follow_cursor),
                ),
                GeneralHit::Draggable => (
                    "Drag to reposition",
                    "Grab the strip above search to move the launcher.",
                    true,
                    RowTrailing::Toggle(state.draggable),
                ),
                GeneralHit::LaunchAtLogin => (
                    "Launch at login",
                    "Start Tinycast automatically when you log in.",
                    true,
                    RowTrailing::Toggle(state.launch_at_login),
                ),
                GeneralHit::ShowInMenuBar => (
                    "Show in menu bar",
                    "Keep the Tinycast icon in the menu bar. Shortcuts still work when hidden.",
                    true,
                    RowTrailing::Toggle(state.show_in_menu_bar),
                ),
                GeneralHit::PopToRoot => (
                    "Pop to Root",
                    pop_sub.as_str(),
                    true,
                    RowTrailing::Label(""),
                ),
                GeneralHit::AutoSwitchInput => (
                    "Auto-switch input source",
                    "Switch IME when the palette opens.",
                    true,
                    RowTrailing::Toggle(state.auto_switch),
                ),
            };
            let trailing = match hit {
                GeneralHit::PopToRoot => RowTrailing::Label(pop_sub.as_str()),
                _ => trailing,
            };
            ds::paint_settings_row(
                target,
                formats.body,
                formats.caption,
                title,
                sub,
                rect.y,
                width,
                pad_x,
                enabled,
                state.chrome,
                trailing,
            )?;
            if hit == GeneralHit::PaletteRecorder {
                paint_recorder_well(target, rect)?;
            }
        }
        y = section.next_y();
    }
    Ok(())
}

fn paint_recorder_well(target: &ID2D1RenderTarget, row: DipRect) -> windows::core::Result<()> {
    let w = theme::size::SHORTCUT_RECORDER;
    let h = 24.0;
    let rect = D2D_RECT_F {
        left: row.x + row.w - CARD_PAD - w,
        top: row.y + (row.h - h) / 2.0,
        right: row.x + row.w - CARD_PAD,
        bottom: row.y + (row.h - h) / 2.0 + h,
    };
    let fill = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.06,
            },
            None,
        )?
    };
    let stroke = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: theme::colors::CARD_STROKE_ALPHA,
            },
            None,
        )?
    };
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: theme::radius::MENU,
        radiusY: theme::radius::MENU,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &fill);
        target.DrawRoundedRectangle(&rounded, &stroke, theme::size::HAIRLINE, None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_section_order() {
        assert_eq!(
            general_sections(),
            [
                "Global Shortcuts",
                "Search",
                "Hyper Key",
                "Appearance",
                "General"
            ]
        );
    }

    #[test]
    fn general_hits_hyper_and_menu_bar() {
        let hits = layout_general(0.0);
        let hyper = hits
            .iter()
            .find(|(h, _)| *h == GeneralHit::HyperKey)
            .unwrap();
        let menu = hits
            .iter()
            .find(|(h, _)| *h == GeneralHit::ShowInMenuBar)
            .unwrap();
        assert_eq!(hit(10.0, hyper.1.y + 4.0, 0.0), Some(GeneralHit::HyperKey));
        assert_eq!(
            hit(10.0, menu.1.y + 4.0, 0.0),
            Some(GeneralHit::ShowInMenuBar)
        );
        assert_eq!(cycle_hyper("none"), "capsLock");
        assert_eq!(cycle_pop_to_root(0), 10);
        assert!(hyper_subtitle("capsLock").contains("logoff"));
        assert!(!hyper_includes_shift_enabled("none"));
        assert!(hyper_includes_shift_enabled("capsLock"));
    }
}
