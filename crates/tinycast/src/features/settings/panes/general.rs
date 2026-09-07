//! Settings → General: shortcuts, search, Hyper, appearance, launch.

use tinycast_pure::i18n::{
    general_appearance_trailing, general_hyper_caps_subtitle, general_hyper_off,
    general_language_trailing, general_pop_to_root_subtitle, general_ranking_subtitle,
    general_reset_label, general_row_subtitle, general_row_title, general_section_footer,
    general_section_title, GeneralRow, GeneralSection, UiLang,
};
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
) -> [(GeneralSection, &'static str, Option<&'static str>, &'static [GeneralHit]); 5] {
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

pub fn hyper_includes_shift_enabled(hyper: &str) -> bool {
    hyper != "none"
}

pub fn hyper_subtitle(raw: &str) -> &'static str {
    hyper_subtitle_lang(raw, UiLang::En)
}

fn hyper_subtitle_lang(raw: &str, lang: UiLang) -> &'static str {
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

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    state: &GeneralState<'_>,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let lang = state.lang;
    let ranking_sub = general_ranking_subtitle(state.ranking_empty, lang);
    let pop_sub = general_pop_to_root_subtitle(state.pop_to_root, lang);
    let appearance_trail = general_appearance_trailing(state.appearance, lang);
    let language_trail = general_language_trailing(lang);
    let reset_label = general_reset_label(lang);
    let hyper_on = hyper_includes_shift_enabled(state.hyper);
    let hyper_title = HyperKey::from_raw(state.hyper).title();
    let hyper_sub = hyper_subtitle_lang(state.hyper, lang);
    let mut y = ds::CARD_INSET - scroll;
    for (kind, header, footer, rows) in section_specs(lang) {
        let section = grouped_at(y, width, kind, header, footer, rows.len());
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
            let row = copy_row(hit);
            let title = general_row_title(row, lang);
            let static_sub = general_row_subtitle(row, lang).unwrap_or("");
            let (sub, enabled, trailing) = match hit {
                GeneralHit::PaletteRecorder => (static_sub, true, RowTrailing::None),
                GeneralHit::ResetRanking => (
                    ranking_sub,
                    !state.ranking_empty,
                    RowTrailing::Label(reset_label),
                ),
                GeneralHit::HyperKey => (hyper_sub, true, RowTrailing::Label(hyper_title)),
                GeneralHit::HyperShift => (
                    static_sub,
                    hyper_on,
                    RowTrailing::Toggle(state.hyper_shift && hyper_on),
                ),
                GeneralHit::Appearance => {
                    (static_sub, true, RowTrailing::Label(appearance_trail))
                }
                GeneralHit::Language => (static_sub, true, RowTrailing::Label(language_trail)),
                GeneralHit::Compact => (static_sub, true, RowTrailing::Toggle(state.compact)),
                GeneralHit::FavoritesInCompact => (
                    static_sub,
                    state.compact,
                    RowTrailing::Toggle(state.favorites_in_compact),
                ),
                GeneralHit::FollowCursor => {
                    (static_sub, true, RowTrailing::Toggle(state.follow_cursor))
                }
                GeneralHit::Draggable => {
                    (static_sub, true, RowTrailing::Toggle(state.draggable))
                }
                GeneralHit::LaunchAtLogin => {
                    (static_sub, true, RowTrailing::Toggle(state.launch_at_login))
                }
                GeneralHit::ShowInMenuBar => {
                    (static_sub, true, RowTrailing::Toggle(state.show_in_menu_bar))
                }
                GeneralHit::PopToRoot => (pop_sub.as_str(), true, RowTrailing::Label("")),
                GeneralHit::AutoSwitchInput => {
                    (static_sub, true, RowTrailing::Toggle(state.auto_switch))
                }
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
                let listening = state.recording_palette;
                let caption = crate::features::hotkeys::ui::recorder::well_caption(
                    state.palette_binding,
                    listening,
                );
                paint_recorder_well(
                    target,
                    formats,
                    rect,
                    &caption,
                    listening || state.palette_binding.filter(|s| !s.is_empty()).is_none(),
                    listening,
                    state.chrome,
                )?;
            }
        }
        y = section.next_y();
    }
    Ok(())
}

fn paint_recorder_well(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    row: DipRect,
    label: &str,
    placeholder: bool,
    listening: bool,
    appearance: u8,
) -> windows::core::Result<()> {
    let well = crate::features::hotkeys::ui::recorder::well_in_row(row);
    let rect = D2D_RECT_F {
        left: well.x,
        top: well.y,
        right: well.x + well.w,
        bottom: well.y + well.h,
    };
    let fill = unsafe { target.CreateSolidColorBrush(&ds::ramp_color(appearance, 0.06), None)? };
    let stroke = unsafe {
        target.CreateSolidColorBrush(
            &if listening {
                D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.47,
                    b: 0.83,
                    a: 1.0,
                }
            } else {
                ds::ramp_color(appearance, theme::colors::CARD_STROKE_ALPHA)
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
    let ink = if placeholder {
        ds::tertiary_ink(appearance)
    } else {
        ds::primary_ink(appearance)
    };
    let brush = unsafe { target.CreateSolidColorBrush(&ink, None)? };
    let wide: Vec<u16> = label.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            formats.caption,
            &D2D_RECT_F {
                left: well.x + theme::spacing::SM,
                top: well.y,
                right: well.x + well.w - theme::spacing::SM,
                bottom: well.y + well.h,
            },
            &brush,
            windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
            windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
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
        let appearance = hits.iter().position(|h| *h == GeneralHit::Appearance).unwrap();
        let language = hits.iter().position(|h| *h == GeneralHit::Language).unwrap();
        assert_eq!(language, appearance + 1);
    }

    #[test]
    fn cycle_ui_language_flips_zh_and_en() {
        assert_eq!(
            tinycast_pure::i18n::UiLang::parse("zh-Hans").cycle().as_str(),
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
