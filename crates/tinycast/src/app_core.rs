use tinycast_pure::alias::AliasStore;
use tinycast_pure::app_entry::AppEntry;
use tinycast_pure::command_id::CommandID;
use tinycast_pure::favorites::FavoritesStore;
use tinycast_pure::launcher_ranking::LauncherRankingStore;
use tinycast_pure::launcher_results::{
    is_category_listing, ordered_results, selectable_rows, LauncherSection,
};
use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_placement::{default_anchor, frame_for};
use tinycast_pure::palette_row_index::clamp_selection;
use tinycast_pure::palette_state::{escape_outcome, EscapeOutcome, PaletteState};
use tinycast_pure::settings_tab::SettingsTab;
use tinycast_pure::theme;
use tinycast_pure::visibility::VisibilityStore;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::app_settings::AppSettings;
use crate::features::launcher::service::app_index::AppIndex;
use crate::features::launcher::ui::coordinator::{
    execute, launch_spec, record_if_needed, LaunchSpec,
};
use crate::features::launcher::ui::list::{
    clamp_scroll, content_height, ensure_visible, list_bottom, list_top, paint_items, row_y,
    selectable_at_y, slots_of, PaintItem, ROW_HEIGHT,
};
use crate::palette::physical;
use crate::palette::PaletteWindow;
use crate::platform::messages::WM_QUIT_APP;
use crate::platform::paths;
use crate::platform::screens::{cursor_target_screen, dip_to_px, dip_to_px_with_dpi};
use crate::surfaces::{SettingsWindow, StubWindow};

pub struct AppCore {
    pub palette: PaletteState,
    pub palette_visible: bool,
    pub palette_window: Option<PaletteWindow>,
    pub settings_window: Option<SettingsWindow>,
    pub about_window: Option<StubWindow>,
    pub support_window: Option<StubWindow>,
    pub settings_tab: SettingsTab,
    pub entries: Vec<AppEntry>,
    pub app_index: AppIndex,
    pub ranking: LauncherRankingStore,
    pub visibility: VisibilityStore,
    pub favorites: FavoritesStore,
    pub aliases: AliasStore,
    host: HWND,
    list_scroll: f32,
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
            about_window: None,
            support_window: None,
            settings_tab: SettingsTab::General,
            entries: Vec::new(),
            app_index: AppIndex::new(),
            ranking: LauncherRankingStore::load(store_path("launcher-ranking.json")),
            visibility: VisibilityStore::load(store_path("visibility.json")),
            favorites: FavoritesStore::load(store_path("favorites.json")),
            aliases: AliasStore::load(store_path("aliases.json")),
            host: HWND::default(),
            list_scroll: 0.0,
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
        self.list_scroll = 0.0;
        self.app_index.start();
    }

    pub fn set_host(&mut self, host: HWND) {
        self.host = host;
        self.app_index.set_host(host);
    }

    pub fn install_app_index(&mut self) {
        if let Some(entries) = self.app_index.take_latest() {
            self.entries = entries;
            self.clamp_selection();
            self.invalidate_palette();
        }
    }

    pub fn launcher_paint_items(&self) -> Vec<PaintItem> {
        paint_items(&self.sections(), self.palette.selection, &self.favorites)
    }

    pub fn list_scroll(&self) -> f32 {
        self.list_scroll
    }

    pub fn appearance_key(&self) -> u8 {
        0
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
        self.list_scroll = 0.0;
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
        self.list_scroll = 0.0;
        self.expand_palette();
        self.invalidate_palette();
    }

    pub fn set_query(&mut self, text: String) {
        if self.palette.query != text {
            self.palette.selection = 0;
            self.list_scroll = 0.0;
        }
        self.palette.query = text;
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
        self.clamp_selection();
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
        self.palette.selection = 0;
        self.list_scroll = 0.0;
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
        self.clamp_selection();
        self.invalidate_palette();
    }

    pub fn handle_key(&mut self, vk: u16) -> bool {
        if vk == VK_ESCAPE.0 {
            self.handle_escape();
            return true;
        }
        if vk == VK_RETURN.0 {
            self.activate_selected();
            return true;
        }
        if vk == VK_DOWN.0 {
            self.move_selection(1);
            return true;
        }
        if vk == VK_UP.0 {
            self.move_selection(-1);
            return true;
        }
        if ctrl_down() {
            if let Some(n) = FavoritesStore::digit_from_vk(vk) {
                self.activate_favorite_slot(n);
                return true;
            }
        }
        false
    }

    pub fn move_selection(&mut self, delta: i32) {
        if !self.palette_visible {
            return;
        }
        if !self.expanded {
            if delta > 0 {
                self.expand_select_first();
            }
            return;
        }
        let count = selectable_rows(&self.sections()).len();
        if count == 0 {
            return;
        }
        let next = if delta < 0 {
            self.palette
                .selection
                .saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.palette.selection.saturating_add(delta as usize)
        };
        self.palette.selection = clamp_selection(next, count);
        self.ensure_selection_visible();
        self.invalidate_palette();
    }

    pub fn scroll_list(&mut self, wheel_delta: i16) {
        if !self.expanded {
            return;
        }
        let notches = wheel_delta as f32 / 120.0;
        self.list_scroll -= notches * ROW_HEIGHT;
        self.clamp_scroll();
        self.invalidate_palette();
    }

    pub fn select_at_y(&mut self, y_dip: f32) -> bool {
        if !self.expanded {
            return false;
        }
        let items = self.launcher_paint_items();
        let slots = slots_of(&items);
        let top = list_top();
        let bottom = list_bottom(theme::size::PANEL_HEIGHT);
        if let Some(index) = selectable_at_y(&slots, y_dip, self.list_scroll, top, bottom) {
            self.palette.selection = index;
            self.invalidate_palette();
            true
        } else {
            false
        }
    }

    pub fn activate_selected(&mut self) {
        if !self.palette_visible {
            return;
        }
        if !self.expanded {
            self.expand_select_first();
            return;
        }
        let sections = self.sections();
        let Some(entry) = selectable_rows(&sections)
            .get(self.palette.selection)
            .cloned()
        else {
            return;
        };
        self.activate_entry(&entry);
    }

    pub fn activate_favorite_slot(&mut self, n: u8) {
        let Some(id) = self.favorites.slot(n).map(str::to_string) else {
            return;
        };
        let Some(entry) = self.catalog().into_iter().find(|e| e.id == id) else {
            return;
        };
        if !self.visibility.allows_hotkey(entry.kind) {
            return;
        }
        self.activate_entry(&entry);
    }

    fn activate_entry(&mut self, entry: &AppEntry) {
        let query = self.palette.query.clone();
        let category = is_category_listing(&query);
        record_if_needed(&mut self.ranking, &query, &entry.id, unix_now(), category);
        let spec = launch_spec(entry);
        match spec {
            LaunchSpec::Noop => {}
            LaunchSpec::Quit => {
                self.hide_palette();
                self.post_host(WM_QUIT_APP);
            }
            LaunchSpec::OpenSettings => {
                self.hide_palette();
                self.open_settings();
            }
            LaunchSpec::OpenAbout => {
                self.hide_palette();
                if let Some(window) = &self.about_window {
                    window.show();
                }
            }
            LaunchSpec::OpenSupport => {
                self.hide_palette();
                if let Some(window) = &self.support_window {
                    window.show();
                }
            }
            other => {
                self.hide_palette();
                let _ = execute(&other);
            }
        }
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

    fn catalog(&self) -> Vec<AppEntry> {
        let mut entries = self.entries.clone();
        entries.extend(CommandID::all().iter().copied().map(CommandID::as_entry));
        for entry in &mut entries {
            entry.fields.user_alias = self.aliases.get(&entry.id).map(str::to_string);
        }
        entries
    }

    fn sections(&self) -> Vec<LauncherSection> {
        let entries = self.catalog();
        let query = self.palette.query.as_str();
        let show_sections = query.is_empty() || is_category_listing(query);
        ordered_results(
            &entries,
            query,
            unix_now(),
            &self.ranking,
            &self.visibility,
            &self.favorites,
            show_sections,
        )
    }

    fn clamp_selection(&mut self) {
        let n = selectable_rows(&self.sections()).len();
        self.palette.selection = clamp_selection(self.palette.selection, n);
        self.clamp_scroll();
    }

    fn ensure_selection_visible(&mut self) {
        let items = self.launcher_paint_items();
        let slots = slots_of(&items);
        let Some((top, h)) = row_y(&slots, self.palette.selection) else {
            return;
        };
        let view_h = list_bottom(theme::size::PANEL_HEIGHT) - list_top();
        self.list_scroll = clamp_scroll(
            ensure_visible(self.list_scroll, top, h, view_h),
            content_height(&slots),
            view_h,
        );
    }

    fn clamp_scroll(&mut self) {
        let items = self.launcher_paint_items();
        let slots = slots_of(&items);
        let view_h = list_bottom(theme::size::PANEL_HEIGHT) - list_top();
        self.list_scroll = clamp_scroll(self.list_scroll, content_height(&slots), view_h);
    }

    fn post_host(&self, msg: u32) {
        if self.host.is_invalid() {
            return;
        }
        unsafe {
            let _ = PostMessageW(self.host, msg, WPARAM(0), LPARAM(0));
        }
    }
}

fn store_path(name: &str) -> std::path::PathBuf {
    paths::roaming_dir().join(name)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn ctrl_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 }
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

    #[test]
    fn empty_query_lists_commands_and_skips_headers_for_selection() {
        use crate::features::launcher::ui::list::PaintItem;
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.toggle_palette();
        let items = c.launcher_paint_items();
        assert!(items.iter().any(|item| matches!(
            item,
            PaintItem::Header { title } if title == "Commands"
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Quit Tinycast"
        )));
        c.expand_select_first();
        assert_eq!(c.palette.selection, 0);
        c.move_selection(1);
        assert_eq!(c.palette.selection, 1);
    }

    #[test]
    fn enter_on_compact_expands_without_launching() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.toggle_palette();
        assert!(c.handle_key(VK_RETURN.0));
        assert!(c.expanded);
        assert!(c.palette_visible);
    }

    #[test]
    fn favorite_slot_missing_is_noop() {
        let mut c = AppCore::new();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.toggle_palette();
        c.activate_favorite_slot(1);
        assert!(c.palette_visible);
    }
}
