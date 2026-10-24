//! Settings → General: shortcuts, search, Hyper, appearance, launch.

use tinycast_pure::i18n::{
    general_hyper_caps_subtitle, general_hyper_off, general_row_title, general_section_footer,
    general_section_title, GeneralRow, GeneralSection, UiLang,
};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;

use crate::design_system::settings::{self as ds, GroupedSection, CARD_PAD, ROW_H};
use crate::features::hotkeys::service::hyper::HyperKey;

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
    Language,
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
    pub palette_binding: Option<&'a str>,
    pub recording_palette: bool,
    pub lang: UiLang,
}

const PALETTE_ROWS: &[GeneralHit] = &[GeneralHit::PaletteRecorder];
const SEARCH_ROWS: &[GeneralHit] = &[GeneralHit::ResetRanking];
const HYPER_ROWS: &[GeneralHit] = &[GeneralHit::HyperKey, GeneralHit::HyperShift];
const APPEARANCE_ROWS: &[GeneralHit] = &[
    GeneralHit::Appearance,
    GeneralHit::Language,
    GeneralHit::Compact,
    GeneralHit::FavoritesInCompact,
    GeneralHit::FollowCursor,
    GeneralHit::Draggable,
];
const GENERAL_ROWS: &[GeneralHit] = &[
    GeneralHit::LaunchAtLogin,
    GeneralHit::ShowInMenuBar,
    GeneralHit::PopToRoot,
    GeneralHit::AutoSwitchInput,
];

/// The five row groups in page order, shared with the Slint settings host so
/// the section order cannot drift from the Direct2D pane's.
pub fn section_rows() -> [&'static [GeneralHit]; 5] {
    [
        PALETTE_ROWS,
        SEARCH_ROWS,
        HYPER_ROWS,
        APPEARANCE_ROWS,
        GENERAL_ROWS,
    ]
}

/// The i18n row a hit draws.
pub fn row_for(hit: GeneralHit) -> GeneralRow {
    copy_row(hit)
}

pub fn general_sections(lang: UiLang) -> [&'static str; 5] {
    [
        general_section_title(GeneralSection::GlobalShortcuts, lang),
        general_section_title(GeneralSection::Search, lang),
        general_section_title(GeneralSection::HyperKey, lang),
        general_section_title(GeneralSection::Appearance, lang),
        general_section_title(GeneralSection::General, lang),
    ]
}

fn section_specs(
    lang: UiLang,
) -> [(
    GeneralSection,
    &'static str,
    Option<&'static str>,
    &'static [GeneralHit],
); 5] {
    [
        (
            GeneralSection::GlobalShortcuts,
            general_section_title(GeneralSection::GlobalShortcuts, lang),
            general_section_footer(GeneralSection::GlobalShortcuts, lang),
            PALETTE_ROWS,
        ),
        (
            GeneralSection::Search,
            general_section_title(GeneralSection::Search, lang),
            general_section_footer(GeneralSection::Search, lang),
            SEARCH_ROWS,
        ),
        (
            GeneralSection::HyperKey,
            general_section_title(GeneralSection::HyperKey, lang),
            general_section_footer(GeneralSection::HyperKey, lang),
            HYPER_ROWS,
        ),
        (
            GeneralSection::Appearance,
            general_section_title(GeneralSection::Appearance, lang),
            general_section_footer(GeneralSection::Appearance, lang),
            APPEARANCE_ROWS,
        ),
        (
            GeneralSection::General,
            general_section_title(GeneralSection::General, lang),
            general_section_footer(GeneralSection::General, lang),
            GENERAL_ROWS,
        ),
    ]
}

fn grouped_at(
    y: f32,
    width: f32,
    section: GeneralSection,
    header: &'static str,
    footer: Option<&'static str>,
    rows: usize,
) -> GroupedSection {
    GroupedSection {
        header: Some(header),
        footer,
        footer_h: match footer {
            Some(_) if section == GeneralSection::Search => ds::footer_block_h(3),
            Some(_) => ds::footer_block_h(1),
            None => 0.0,
        },
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

pub fn layout_general(width: f32, scroll: f32) -> Vec<(GeneralHit, DipRect)> {
    layout_general_sized(width, scroll)
}

fn layout_general_sized(width: f32, scroll: f32) -> Vec<(GeneralHit, DipRect)> {
    let mut y = ds::CARD_INSET;
    let mut hits = Vec::new();
    for (kind, header, footer, rows) in section_specs(UiLang::En) {
        let section = grouped_at(y, width, kind, header, footer, rows.len());
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
    for (kind, header, footer, rows) in section_specs(UiLang::En) {
        y = grouped_at(y, width, kind, header, footer, rows.len()).next_y();
    }
    y + ds::CARD_INSET
}

pub fn hit(_x: f32, y: f32, scroll: f32, width: f32) -> Option<GeneralHit> {
    for (hit, rect) in layout_general(width, scroll) {
        if y >= rect.y && y < rect.y + rect.h {
            return Some(hit);
        }
    }
    None
}

fn row_semantics(
    hit: GeneralHit,
    state: &GeneralState<'_>,
) -> (
    crate::features::settings::focus::FocusRole,
    bool,
    Option<bool>,
) {
    use crate::features::settings::focus::FocusRole;
    let hyper_on = hyper_includes_shift_enabled(state.hyper);
    match hit {
        GeneralHit::PaletteRecorder => (FocusRole::Button, true, None),
        GeneralHit::ResetRanking => (FocusRole::Button, !state.ranking_empty, None),
        GeneralHit::HyperKey => (FocusRole::Button, true, None),
        GeneralHit::HyperShift => (
            FocusRole::Toggle,
            hyper_on,
            Some(state.hyper_shift && hyper_on),
        ),
        GeneralHit::Appearance => (FocusRole::Button, true, None),
        GeneralHit::Language => (FocusRole::Button, true, None),
        GeneralHit::Compact => (FocusRole::Toggle, true, Some(state.compact)),
        GeneralHit::FavoritesInCompact => (
            FocusRole::Toggle,
            state.compact,
            Some(state.favorites_in_compact),
        ),
        GeneralHit::FollowCursor => (FocusRole::Toggle, true, Some(state.follow_cursor)),
        GeneralHit::Draggable => (FocusRole::Toggle, true, Some(state.draggable)),
        GeneralHit::LaunchAtLogin => (FocusRole::Toggle, true, Some(state.launch_at_login)),
        GeneralHit::ShowInMenuBar => (FocusRole::Toggle, true, Some(state.show_in_menu_bar)),
        GeneralHit::PopToRoot => (FocusRole::Button, true, None),
        GeneralHit::AutoSwitchInput => (FocusRole::Toggle, true, Some(state.auto_switch)),
    }
}

pub fn keyboard_items(
    width: f32,
    scroll: f32,
    state: &GeneralState<'_>,
) -> Vec<crate::features::settings::focus::FocusItem> {
    layout_general(width, scroll)
        .into_iter()
        .map(|(hit, rect)| {
            let (role, enabled, checked) = row_semantics(hit, state);
            crate::features::settings::focus::content_item(
                general_row_title(copy_row(hit), state.lang),
                role,
                enabled,
                checked,
                false,
                rect,
                None,
            )
        })
        .collect()
}

pub fn hyper_includes_shift_enabled(hyper: &str) -> bool {
    hyper != "none"
}

pub fn hyper_subtitle(raw: &str) -> &'static str {
    hyper_subtitle_lang(raw, UiLang::En)
}

pub fn hyper_subtitle_lang(raw: &str, lang: UiLang) -> &'static str {
    match HyperKey::from_raw(raw) {
        HyperKey::CapsLock => general_hyper_caps_subtitle(lang),
        HyperKey::None => general_hyper_off(lang),
        other => other.title(),
    }
}

fn copy_row(hit: GeneralHit) -> GeneralRow {
    match hit {
        GeneralHit::PaletteRecorder => GeneralRow::PaletteRecorder,
        GeneralHit::ResetRanking => GeneralRow::ResetRanking,
        GeneralHit::HyperKey => GeneralRow::HyperKey,
        GeneralHit::HyperShift => GeneralRow::HyperShift,
        GeneralHit::Appearance => GeneralRow::Appearance,
        GeneralHit::Language => GeneralRow::Language,
        GeneralHit::Compact => GeneralRow::Compact,
        GeneralHit::FavoritesInCompact => GeneralRow::FavoritesInCompact,
        GeneralHit::FollowCursor => GeneralRow::FollowCursor,
        GeneralHit::Draggable => GeneralRow::Draggable,
        GeneralHit::LaunchAtLogin => GeneralRow::LaunchAtLogin,
        GeneralHit::ShowInMenuBar => GeneralRow::ShowInMenuBar,
        GeneralHit::PopToRoot => GeneralRow::PopToRoot,
        GeneralHit::AutoSwitchInput => GeneralRow::AutoSwitchInput,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_section_order() {
        assert_eq!(
            general_sections(tinycast_pure::i18n::UiLang::En),
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
    fn general_language_row_exists() {
        let hits: Vec<_> = crate::features::settings::panes::general::layout_general(400.0, 0.0)
            .into_iter()
            .map(|(h, _)| h)
            .collect();
        assert!(hits.contains(&GeneralHit::Language));
        let appearance = hits
            .iter()
            .position(|h| *h == GeneralHit::Appearance)
            .unwrap();
        let language = hits
            .iter()
            .position(|h| *h == GeneralHit::Language)
            .unwrap();
        assert_eq!(language, appearance + 1);
    }

    #[test]
    fn cycle_ui_language_flips_zh_and_en() {
        assert_eq!(
            tinycast_pure::i18n::UiLang::parse("zh-Hans")
                .cycle()
                .as_str(),
            "en"
        );
    }

    #[test]
    fn general_hits_hyper_and_menu_bar() {
        let width = theme::size::SETTINGS_WINDOW.0 - theme::size::SETTINGS_SIDEBAR;
        let hits = layout_general(width, 0.0);
        let hyper = hits
            .iter()
            .find(|(h, _)| *h == GeneralHit::HyperKey)
            .unwrap();
        let menu = hits
            .iter()
            .find(|(h, _)| *h == GeneralHit::ShowInMenuBar)
            .unwrap();
        assert_eq!(
            hit(10.0, hyper.1.y + 4.0, 0.0, width),
            Some(GeneralHit::HyperKey)
        );
        assert_eq!(
            hit(10.0, menu.1.y + 4.0, 0.0, width),
            Some(GeneralHit::ShowInMenuBar)
        );
        assert_eq!(cycle_hyper("none"), "capsLock");
        assert_eq!(cycle_pop_to_root(0), 10);
        assert!(hyper_subtitle("capsLock").contains("logoff"));
        assert!(!hyper_includes_shift_enabled("none"));
        assert!(hyper_includes_shift_enabled("capsLock"));
    }

    #[test]
    fn compact_toggle_is_keyboard_reachable_and_favorites_follow() {
        let state = GeneralState {
            ranking_empty: true,
            hyper: "none",
            hyper_shift: false,
            appearance: "dark",
            compact: true,
            favorites_in_compact: true,
            follow_cursor: false,
            draggable: false,
            launch_at_login: false,
            show_in_menu_bar: true,
            pop_to_root: 0,
            auto_switch: false,
            chrome: 0,
            palette_binding: None,
            recording_palette: false,
            lang: UiLang::En,
        };
        let items = keyboard_items(420.0, 0.0, &state);
        let compact = items
            .iter()
            .find(|i| i.name == "Compact mode")
            .expect("compact");
        assert_eq!(
            compact.role,
            crate::features::settings::focus::FocusRole::Toggle
        );
        assert_eq!(compact.checked, Some(true));
        let fav = items
            .iter()
            .find(|i| i.name == "Show favorites in compact mode")
            .expect("favorites");
        assert!(fav.enabled);
        let hyper_shift = items
            .iter()
            .find(|i| i.name == "Include Shift")
            .expect("shift");
        assert!(!hyper_shift.enabled);
        assert!(items.iter().any(|i| i.name == "Pop to Root"));
    }

    #[test]
    fn palette_well_tracks_live_detail_width() {
        let narrow = layout_general(420.0, 0.0)
            .into_iter()
            .find(|(h, _)| *h == GeneralHit::PaletteRecorder)
            .unwrap()
            .1;
        let wide = layout_general(800.0, 0.0)
            .into_iter()
            .find(|(h, _)| *h == GeneralHit::PaletteRecorder)
            .unwrap()
            .1;
        let n = crate::features::hotkeys::ui::recorder::well_in_row(narrow);
        let w = crate::features::hotkeys::ui::recorder::well_in_row(wide);
        assert_eq!(n.w, theme::size::SHORTCUT_RECORDER);
        assert_eq!(w.w, theme::size::SHORTCUT_RECORDER);
        assert!(w.x > n.x);
    }
}
