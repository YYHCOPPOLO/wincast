use crate::palette_placement::DipRect;
use crate::settings_tab::{SettingsSection, SettingsTab};
use crate::theme;

pub const SIDEBAR_SECTION_HEADER_HEIGHT: f32 = 22.0;
pub const SIDEBAR_TAB_ROW_HEIGHT: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarRowKind {
    Header(SettingsSection),
    Tab(SettingsTab),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarRow {
    pub y: f32,
    pub height: f32,
    pub kind: SidebarRowKind,
}

pub fn sidebar_rows() -> Vec<SidebarRow> {
    let mut y = theme::spacing::XL;
    let mut rows = Vec::new();
    for (i, section) in SettingsSection::all().into_iter().enumerate() {
        if i > 0 {
            y += theme::spacing::SECTION_SPACING;
        }
        rows.push(SidebarRow {
            y,
            height: SIDEBAR_SECTION_HEADER_HEIGHT,
            kind: SidebarRowKind::Header(section),
        });
        y += SIDEBAR_SECTION_HEADER_HEIGHT;
        for &tab in section.tabs() {
            rows.push(SidebarRow {
                y,
                height: SIDEBAR_TAB_ROW_HEIGHT,
                kind: SidebarRowKind::Tab(tab),
            });
            y += SIDEBAR_TAB_ROW_HEIGHT;
        }
    }
    rows
}

pub fn sidebar_content_height() -> f32 {
    sidebar_rows()
        .last()
        .map(|row| row.y + row.height + theme::spacing::XL)
        .unwrap_or(0.0)
}

pub fn clamp_sidebar_scroll(scroll: f32, viewport_h: f32) -> f32 {
    let max = (sidebar_content_height() - viewport_h).max(0.0);
    scroll.clamp(0.0, max)
}

pub fn wheel_targets_sidebar(x: f32) -> bool {
    x >= 0.0 && x < theme::size::SETTINGS_SIDEBAR
}

pub fn tab_at(x: f32, y: f32, scroll: f32) -> Option<SettingsTab> {
    if !wheel_targets_sidebar(x) {
        return None;
    }
    let y = y + scroll;
    for row in sidebar_rows() {
        if y >= row.y && y < row.y + row.height {
            return match row.kind {
                SidebarRowKind::Tab(tab) => Some(tab),
                SidebarRowKind::Header(_) => None,
            };
        }
    }
    None
}

pub fn sidebar_rect(window_h: f32) -> DipRect {
    DipRect {
        x: 0.0,
        y: 0.0,
        w: theme::size::SETTINGS_SIDEBAR,
        h: window_h,
    }
}

pub fn detail_rect(window_w: f32, window_h: f32) -> DipRect {
    let x = theme::size::SETTINGS_SIDEBAR;
    DipRect {
        x,
        y: 0.0,
        w: (window_w - x).max(0.0),
        h: window_h,
    }
}

pub fn columns_overlap(a: DipRect, b: DipRect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w
}

pub fn x_to_detail_local(x: f32) -> Option<f32> {
    let side = crate::theme::size::SETTINGS_SIDEBAR;
    if x < side {
        None
    } else {
        Some(x - side)
    }
}

pub fn recorder_well() -> DipRect {
    DipRect {
        x: 0.0,
        y: 0.0,
        w: theme::size::SHORTCUT_RECORDER,
        h: 24.0,
    }
}

pub fn recorder_callout_above(well: DipRect) -> DipRect {
    let (w, h) = theme::size::SHORTCUT_POPOVER;
    DipRect {
        x: well.x + (well.w - w) / 2.0,
        y: well.y - theme::spacing::SM - h,
        w,
        h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_detail_starts_after_sidebar() {
        let side = sidebar_rect(700.0);
        let detail = detail_rect(860.0, 700.0);
        assert_eq!(side.w, 215.0);
        assert_eq!(detail.x, 215.0);
        assert!(!columns_overlap(side, detail));
        assert_eq!(side.x + side.w, detail.x);
    }

    #[test]
    fn shortcut_popover_is_132_wide() {
        assert_eq!(crate::theme::size::SHORTCUT_POPOVER.0, 132.0);
        assert_eq!(crate::theme::size::SHORTCUT_RECORDER, 120.0);
    }

    #[test]
    fn recorder_well_is_120() {
        let r = recorder_well();
        assert_eq!(r.w, 120.0);
    }

    #[test]
    fn detail_hit_is_local_to_sidebar() {
        let detail = crate::layout::settings::detail_rect(860.0, 700.0);
        assert_eq!(detail.x, 215.0);
        let local = x_to_detail_local(100.0);
        assert!(local.is_none());
        assert_eq!(x_to_detail_local(215.0), Some(0.0));
        assert_eq!(x_to_detail_local(315.0), Some(100.0));
    }

    #[test]
    fn sidebar_content_is_taller_than_the_settings_window() {
        assert!(sidebar_content_height() > theme::size::SETTINGS_WINDOW.1);
        assert_eq!(
            sidebar_rows().last().map(|r| r.kind),
            Some(SidebarRowKind::Tab(SettingsTab::About))
        );
    }

    #[test]
    fn sidebar_scroll_reveals_about_tab() {
        let about = sidebar_rows()
            .into_iter()
            .find(|r| r.kind == SidebarRowKind::Tab(SettingsTab::About))
            .unwrap();
        assert!(about.y + about.height > theme::size::SETTINGS_WINDOW.1);
        assert_eq!(
            tab_at(theme::spacing::MD, about.y + 1.0, 0.0),
            Some(SettingsTab::About)
        );
        let on_screen = 40.0;
        let scroll = about.y - on_screen;
        assert_eq!(
            tab_at(theme::spacing::MD, on_screen + 1.0, scroll),
            Some(SettingsTab::About)
        );
        assert_ne!(
            tab_at(theme::spacing::MD, on_screen + 1.0, 0.0),
            Some(SettingsTab::About)
        );
        assert!(wheel_targets_sidebar(0.0));
        assert!(wheel_targets_sidebar(theme::size::SETTINGS_SIDEBAR - 1.0));
        assert!(!wheel_targets_sidebar(theme::size::SETTINGS_SIDEBAR));
        assert_eq!(
            clamp_sidebar_scroll(10_000.0, theme::size::SETTINGS_WINDOW.1),
            (sidebar_content_height() - theme::size::SETTINGS_WINDOW.1).max(0.0)
        );
        assert_eq!(clamp_sidebar_scroll(-20.0, 700.0), 0.0);
    }
}
