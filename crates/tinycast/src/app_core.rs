use tinycast_pure::app_entry::AppEntry;
use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_placement::{default_anchor, frame_for};
use tinycast_pure::palette_state::{escape_outcome, EscapeOutcome, PaletteState};
use tinycast_pure::settings_tab::SettingsTab;

use crate::app_settings::AppSettings;
use crate::features::launcher::service::app_index::AppIndex;
use crate::palette::physical;
use crate::palette::PaletteWindow;
use crate::platform::screens::{cursor_target_screen, dip_to_px, dip_to_px_with_dpi};
use crate::surfaces::SettingsWindow;

pub struct AppCore {
    pub palette: PaletteState,
    pub palette_visible: bool,
    pub palette_window: Option<PaletteWindow>,
    pub settings_window: Option<SettingsWindow>,
    pub settings_tab: SettingsTab,
    pub entries: Vec<AppEntry>,
    pub app_index: AppIndex,
    expanded: bool,
    anchor: Option<tinycast_pure::palette_placement::PaletteAnchor>,
    anchor_px: Option<physical::Point>,
}

impl AppCore {
    pub fn new() -> Self {
        Self {
            palette: PaletteState::new(),
            palette_visible: false,
            palette_window: None,
            settings_window: None,
            settings_tab: SettingsTab::General,
            entries: Vec::new(),
            app_index: AppIndex::new(),
            expanded: false,
            anchor: None,
            anchor_px: None,
        }
    }

    pub fn start(&mut self) {
        self.palette = PaletteState::new();
        self.palette_visible = false;
        self.expanded = false;
        self.anchor = None;
        self.anchor_px = None;
        if let Some(window) = &self.palette_window {
            window.hide();
        }
        self.app_index.start();
    }

    pub fn install_app_index(&mut self) {
        if let Some(entries) = self.app_index.take_latest() {
            self.entries = entries;
            self.invalidate_palette();
        }
    }

    pub fn open_settings(&mut self) {
        if let Some(window) = &self.settings_window {
            window.show();
        }
    }

    pub fn select_settings_tab(&mut self, tab: SettingsTab) {
        if self.settings_tab == tab {
            return;
        }
        self.settings_tab = tab;
        if let Some(window) = &self.settings_window {
            window.invalidate();
        }
    }

    pub fn toggle_palette(&mut self) {
        if self.palette_visible {
            self.hide_palette();
        } else {
            self.palette.prepare(PaletteMode::Launcher);
            self.palette_visible = true;
            self.expanded = false;
            self.show_palette_window();
        }
    }

    pub fn hide_palette(&mut self) {
        self.palette_visible = false;
        self.expanded = false;
        self.palette.is_composing = false;
        self.anchor = None;
        self.anchor_px = None;
        if let Some(window) = &self.palette_window {
            window.reset_search();
            window.hide();
        }
    }

    pub fn handle_escape(&mut self) {
        let launcher = self.palette.mode == PaletteMode::Launcher;
        match escape_outcome(&self.palette.query, launcher) {
            EscapeOutcome::ClearQuery => {
                self.palette.query.clear();
                self.palette.is_composing = false;
                if let Some(window) = &self.palette_window {
                    window.reset_search();
                }
                self.invalidate_palette();
            }
            EscapeOutcome::Hide => self.hide_palette(),
        }
    }

    pub fn expand_palette(&mut self) {
        if !self.palette_visible {
            return;
        }
        self.expanded = true;
        let Some(anchor_px) = self.anchor_px else {
            return;
        };
        if let Some(window) = &self.palette_window {
            window.set_expanded(true, anchor_px);
        }
    }

    pub fn expand_select_first(&mut self) {
        self.palette.selection = 0;
        self.expand_palette();
    }

    pub fn set_query(&mut self, text: String) {
        self.palette.query = text;
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
        self.invalidate_palette();
    }

    pub fn set_composing(&mut self, composing: bool) {
        if self.palette.is_composing == composing {
            return;
        }
        self.palette.is_composing = composing;
        self.invalidate_palette();
    }

    pub fn append_query_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.palette.query.push(c);
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
        self.invalidate_palette();
    }

    fn invalidate_palette(&self) {
        if let Some(window) = &self.palette_window {
            window.invalidate();
        }
    }

    pub fn relayout_palette(&mut self) {
        if !self.palette_visible {
            return;
        }
        let Some(anchor) = self.anchor else {
            return;
        };
        let Some(hwnd) = self.palette_window.as_ref().map(|w| w.hwnd) else {
            return;
        };
        let frame = frame_for(anchor, self.expanded);
        let rect = physical::Rect::from_win32(dip_to_px(hwnd, frame));
        self.anchor_px = Some(physical::Point {
            x: rect.x,
            y: rect.y,
        });
        if let Some(window) = &self.palette_window {
            window.set_expanded(self.expanded, self.anchor_px.unwrap());
        }
    }

    fn show_palette_window(&mut self) {
        if self.palette_window.is_none() {
            return;
        }
        let open_on_cursor = AppSettings::load().open_on_cursor_screen;
        let Some((screen, dpi)) = cursor_target_screen(open_on_cursor) else {
            return;
        };
        let anchor = default_anchor(screen);
        self.anchor = Some(anchor);
        let frame = frame_for(anchor, false);
        let rect = physical::Rect::from_win32(dip_to_px_with_dpi(frame, dpi));
        self.anchor_px = Some(physical::Point {
            x: rect.x,
            y: rect.y,
        });
        if let Some(window) = &self.palette_window {
            window.reset_search();
            window.show_at(rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_palette_flips_visible_and_prepares_launcher() {
        let mut c = AppCore::new();
        assert!(!c.palette_visible);
        c.toggle_palette();
        assert!(c.palette_visible);
        assert_eq!(
            c.palette.mode,
            tinycast_pure::palette_mode::PaletteMode::Launcher
        );
        c.toggle_palette();
        assert!(!c.palette_visible);
    }

    #[test]
    fn empty_query_stays_compact_until_query_or_down() {
        let mut c = AppCore::new();
        c.toggle_palette();
        assert!(c.palette.query.is_empty());
        assert!(!c.expanded);
        c.expand_palette();
        assert!(c.expanded);
        c.hide_palette();
        c.toggle_palette();
        assert!(!c.expanded);
        c.append_query_char('a');
        assert_eq!(c.palette.query, "a");
        assert!(c.expanded);
    }

    #[test]
    fn set_query_expands_when_non_empty() {
        let mut c = AppCore::new();
        c.toggle_palette();
        c.set_query("abc".into());
        assert_eq!(c.palette.query, "abc");
        assert!(c.expanded);
    }

    #[test]
    fn down_arrow_selects_row_zero_and_expands() {
        let mut c = AppCore::new();
        c.toggle_palette();
        c.palette.selection = 3;
        c.expand_select_first();
        assert!(c.expanded);
        assert_eq!(c.palette.selection, 0);
    }

    #[test]
    fn escape_clears_query_then_hides() {
        let mut c = AppCore::new();
        c.toggle_palette();
        c.set_query("abc".into());
        c.handle_escape();
        assert_eq!(c.palette.query, "");
        assert!(c.palette_visible);
        c.handle_escape();
        assert!(!c.palette_visible);
    }

    #[test]
    fn default_selected_settings_tab_is_general() {
        let c = AppCore::new();
        assert_eq!(c.settings_tab, SettingsTab::General);
    }

    #[test]
    fn open_settings_does_not_change_palette_visibility() {
        let mut c = AppCore::new();
        c.open_settings();
        assert!(!c.palette_visible);
        c.toggle_palette();
        assert!(c.palette_visible);
        c.open_settings();
        assert!(c.palette_visible);
        assert_eq!(c.settings_tab, SettingsTab::General);
    }

    #[test]
    fn select_settings_tab_stores_enum() {
        let mut c = AppCore::new();
        c.select_settings_tab(SettingsTab::Ai);
        assert_eq!(c.settings_tab, SettingsTab::Ai);
        c.select_settings_tab(SettingsTab::About);
        assert_eq!(c.settings_tab, SettingsTab::About);
    }

    #[test]
    fn start_begins_scan_and_install_stores_entries() {
        use crate::features::launcher::service::app_index::fold_apps;
        use crate::features::launcher::service::app_index::ResolvedApp;

        let mut c = AppCore::new();
        c.start();
        assert!(c.entries.is_empty());
        let gen = c.app_index.generation();
        c.app_index.publish(
            gen,
            fold_apps(vec![ResolvedApp {
                name: "Notepad".into(),
                aumid: None,
                target: Some(r"C:\Windows\System32\notepad.exe".into()),
                executable_name: Some("notepad.exe".into()),
                alternate_names: Vec::new(),
            }]),
        );
        c.install_app_index();
        assert!(c.entries.iter().any(|e| e.name == "Notepad"));
        assert!(c
            .entries
            .iter()
            .any(|e| e.kind == tinycast_pure::app_entry::AppKind::SystemSettings));
    }
}
