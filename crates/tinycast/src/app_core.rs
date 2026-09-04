use tinycast_pure::alias::AliasStore;
use tinycast_pure::app_entry::{AppEntry, AppKind};
use tinycast_pure::calc::{evaluate, CalcResult, CalculatorHistoryStore};
use tinycast_pure::command_id::CommandID;
use tinycast_pure::favorites::FavoritesStore;
use tinycast_pure::feature_flags::FeatureFlags;
use tinycast_pure::hotkey::HotKeyBinding;
use tinycast_pure::hotkey_store::HotKeyStore;
use tinycast_pure::launcher_ranking::LauncherRankingStore;
use tinycast_pure::launcher_results::{
    is_category_listing, ordered_results, selectable_rows, LauncherSection,
};
use tinycast_pure::palette_menu::{
    action_group_rects, actions_for, can_open_actions, clamp_menu_selection, menu_frame,
    menu_row_at, point_in, ActionContext, MenuItem, OpenMenu, ID_COPY_PATH, ID_FAVORITE,
    ID_MOVE_DOWN, ID_MOVE_UP, ID_OPEN, ID_RESET_RANKING, ID_SHOW_IN_FOLDER, ID_UNINSTALL,
};
use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_placement::{default_anchor, frame_for};
use tinycast_pure::palette_row_index::{clamp_selection, selectable_count};
use tinycast_pure::palette_state::{escape_outcome, EscapeOutcome, PaletteState};
use tinycast_pure::palette_tab::{tab_from, TabHop};
use tinycast_pure::settings_tab::SettingsTab;
use tinycast_pure::theme;
use tinycast_pure::visibility::VisibilityStore;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, VK_SHIFT,
    VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, PostMessageW};

use crate::app_settings::AppSettings;
use crate::features::calculator::service::rates::CurrencyRateStore;
use crate::features::calculator::ui::{card, coordinator as calc_coordinator};
use crate::features::clipboard::service::manager as clip_manager;
use crate::features::clipboard::service::store::{ClipboardFilter, ClipboardStore};
use crate::features::clipboard::ui::screen as clip_screen;
use crate::features::launcher::service::app_index::AppIndex;
use crate::features::launcher::settings::items::{
    commands_catalog, commit_alias_text, hotkey_action_key,
};
use crate::features::launcher::ui::coordinator::{
    copy_path_text, copy_text, execute, launch_spec, record_if_needed, reveal_path, show_in_folder,
    LaunchSpec,
};
use crate::features::launcher::ui::list::{
    clamp_scroll, content_height, ensure_visible, list_bottom, list_top, paint_items, row_y,
    selectable_at_y, slots_of, PaintItem, ROW_HEIGHT,
};
use crate::features::custom_commands::service::store::CustomCommandStore;
use crate::features::quicklinks::service::store::QuicklinkStore;
use crate::features::quicklinks::ui::coordinator as quicklink_coordinator;
use crate::features::custom_commands::ui::coordinator as custom_coordinator;
use crate::features::snippets::service::injector;
use crate::features::snippets::service::listener::KeywordListener;
use crate::features::snippets::service::repository::SnippetRepository;
use crate::features::snippets::ui::coordinator as snippet_coordinator;
use crate::features::window_management::service::mover::WindowMover;
use crate::features::window_management::ui::coordinator as window_coordinator;
use crate::palette::menu::{FooterPaint, MenuPaint};
use crate::palette::physical;
use crate::palette::PaletteWindow;
use crate::platform::clock::{local_naive_unix, unix_now};
use crate::platform::messages::WM_QUIT_APP;
use crate::platform::paths;
use crate::platform::screens::{cursor_target_screen, dip_to_px, dip_to_px_with_dpi};
use crate::surfaces::{MessageHud, SettingsWindow, StubWindow};

pub struct AppCore {
    pub palette: PaletteState,
    pub palette_visible: bool,
    pub palette_window: Option<PaletteWindow>,
    pub settings_window: Option<SettingsWindow>,
    pub about_window: Option<StubWindow>,
    pub support_window: Option<StubWindow>,
    pub notes_window: Option<crate::surfaces::NotesWindow>,
    pub settings_tab: SettingsTab,
    pub entries: Vec<AppEntry>,
    pub app_index: AppIndex,
    pub ranking: LauncherRankingStore,
    pub visibility: VisibilityStore,
    pub favorites: FavoritesStore,
    pub aliases: AliasStore,
    pub hotkeys: HotKeyStore,
    pub settings: AppSettings,
    pub currency_rates: CurrencyRateStore,
    pub calc_history: CalculatorHistoryStore,
    pub clipboard: ClipboardStore,
    clipboard_filter: ClipboardFilter,
    snippet_repo: SnippetRepository,
    snippet_records: Vec<tinycast_pure::snippet::StoredSnippet>,
    pub(crate) custom_commands: CustomCommandStore,
    pub(crate) quicklinks: QuicklinkStore,
    snippet_listener: KeywordListener,
    file_search: crate::features::file_search::service::session::FileSearchSession,
    notes: crate::features::notes::service::store::NotesStore,
    notes_switcher: Vec<tinycast_pure::note::NoteSummary>,
    calendar: crate::features::calendar::service::store::CalendarStore,
    window_mover: WindowMover,
    argument_session: Option<snippet_coordinator::ArgumentSession>,
    hud: Option<MessageHud>,
    previous_hwnd: HWND,
    host: HWND,
    list_scroll: f32,
    expanded: bool,
    anchor: Option<tinycast_pure::palette_placement::PaletteAnchor>,
    anchor_px: Option<physical::Point>,
    menu: OpenMenu,
    menu_selection: usize,
    menu_items: Vec<MenuItem>,
    menu_header: String,
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
            notes_window: None,
            settings_tab: SettingsTab::General,
            entries: Vec::new(),
            app_index: AppIndex::new(),
            ranking: LauncherRankingStore::load(store_path("launcher-ranking.json")),
            visibility: VisibilityStore::load(store_path("visibility.json")),
            favorites: FavoritesStore::load(store_path("favorites.json")),
            aliases: AliasStore::load(store_path("aliases.json")),
            hotkeys: {
                let path = store_path("hotkeys.json");
                let mut store = HotKeyStore::load(&path);
                if !path.exists() {
                    store.set(
                        "hotkey.togglePalette".into(),
                        Some(tinycast_pure::hotkey::default_toggle_palette()),
                    );
                }
                store
            },
            settings: AppSettings::load(),
            currency_rates: CurrencyRateStore::new(),
            calc_history: CalculatorHistoryStore::load(store_path("calculator-history.json")),
            clipboard: ClipboardStore::open(crate::platform::paths::local_dir()),
            clipboard_filter: ClipboardFilter::All,
            snippet_repo: SnippetRepository::in_roaming(),
            snippet_records: Vec::new(),
            custom_commands: CustomCommandStore::load(),
            quicklinks: QuicklinkStore::load(),
            snippet_listener: KeywordListener::new(),
            file_search: crate::features::file_search::service::session::FileSearchSession::from_settings(
                &[],
                &[],
                HWND::default(),
            ),
            notes: crate::features::notes::service::store::NotesStore::in_roaming(),
            notes_switcher: Vec::new(),
            calendar: crate::features::calendar::service::store::CalendarStore::new(),
            window_mover: WindowMover::new(),
            argument_session: None,
            hud: None,
            previous_hwnd: HWND::default(),
            host: HWND::default(),
            list_scroll: 0.0,
            expanded: false,
            anchor: None,
            anchor_px: None,
            menu: OpenMenu::None,
            menu_selection: 0,
            menu_items: Vec::new(),
            menu_header: String::new(),
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
        self.close_menu();
        self.app_index.start();
        self.currency_rates.start(self.host);
        clip_manager::listen(self.host);
        self.apply_clipboard_retention();
        self.apply_snippets_enabled();
        self.apply_file_search_policy();
        if self.settings.calendar_enabled {
            self.calendar.refresh();
            self.maybe_auto_join();
            self.apply_calendar_tooltip();
        }
        self.sync_hotkeys();
        crate::platform::tray::set_icon_visible(self.host, self.settings.show_in_menu_bar);
        if self.hud.is_none() && !self.host.is_invalid() {
            self.hud = MessageHud::create(self.host).ok();
        }
    }

    pub fn cycle_clipboard_retention(&mut self) {
        self.settings.clipboard_retention_days =
            crate::features::clipboard::settings::pane::cycle_retention(
                self.settings.clipboard_retention_days,
            );
        let _ = self.settings.save();
        self.apply_clipboard_retention();
        self.invalidate_settings();
    }

    fn apply_clipboard_retention(&mut self) {
        let days = self.settings.clipboard_retention_days;
        if days > 0 {
            let secs = days * 86_400;
            self.clipboard.set_max_age_secs(secs);
            self.clipboard.prune_unpinned_older_than(secs);
        } else {
            self.clipboard.set_max_age_secs(-1);
        }
    }

    pub fn clear_clipboard_history(&mut self) {
        self.clipboard.clear();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn remove_clipboard_disabled_app(&mut self, index: usize) {
        if index < self.settings.clipboard_disabled_apps.len() {
            self.settings.clipboard_disabled_apps.remove(index);
            let _ = self.settings.save();
            self.invalidate_settings();
        }
    }

    pub fn add_clipboard_disabled_app(&mut self, stem: String) {
        if stem.is_empty() {
            return;
        }
        if self
            .settings
            .clipboard_disabled_apps
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&stem))
        {
            return;
        }
        self.settings.clipboard_disabled_apps.push(stem);
        let _ = self.settings.save();
        self.invalidate_settings();
    }

    pub fn capture_clipboard(&mut self) {
        clip_manager::capture(&mut self.clipboard, &self.settings, self.host);
        if self.palette.mode == PaletteMode::Clipboard {
            self.clamp_selection();
            self.invalidate_palette();
        }
    }

    pub fn install_clipboard_images(&mut self) {
        let mut inserted = false;
        for path in clip_manager::take_pending_images() {
            self.clipboard.insert_image(path);
            inserted = true;
        }
        if inserted && self.palette.mode == PaletteMode::Clipboard {
            self.clamp_selection();
            self.invalidate_palette();
        }
    }

    pub fn search_trailing_width(&self) -> f32 {
        if self.palette.mode == PaletteMode::Clipboard {
            clip_screen::filter_trailing_width()
        } else {
            0.0
        }
    }

    pub fn clipboard_filter_paint(&self) -> Option<crate::palette::d2d::FilterButtonPaint<'_>> {
        if self.palette.mode != PaletteMode::Clipboard {
            return None;
        }
        Some(crate::palette::d2d::FilterButtonPaint {
            title: clip_screen::filter_title(self.clipboard_filter),
            open: self.menu == OpenMenu::ClipboardFilter,
            rect: clip_screen::filter_button_rect(theme::size::PANEL_WIDTH),
        })
    }

    pub fn tab_hint(&self) -> Option<&'static str> {
        match tab_from(
            self.palette.mode,
            self.settings.ai_enabled,
            self.palette.mode == PaletteMode::QuicklinkArguments,
        ) {
            TabHop::Clipboard => Some("Clipboard"),
            TabHop::Launcher => Some("Launcher"),
            TabHop::Ai => {
                if self.settings.ai_enabled {
                    Some("AI Chat")
                } else {
                    None
                }
            }
            TabHop::StayForArguments => None,
        }
    }

    pub fn clipboard_preview(&self) -> Option<String> {
        if self.palette.mode != PaletteMode::Clipboard {
            return None;
        }
        let rows = self
            .clipboard
            .search(&self.palette.query, self.clipboard_filter);
        Some(clip_screen::preview_text(rows.get(self.palette.selection)))
    }

    pub fn install_rates(&mut self) {
        self.currency_rates.install();
        self.invalidate_palette();
    }

    pub fn set_host(&mut self, host: HWND) {
        self.host = host;
        self.app_index.set_host(host);
        self.file_search.set_host(host);
    }

    pub fn set_file_search_enabled(&mut self, enabled: bool) {
        if self.settings.file_search_enabled == enabled {
            return;
        }
        self.settings.file_search_enabled = enabled;
        let _ = self.settings.save();
        if crate::features::file_search::ui::coordinator::should_leave_file_search(
            enabled,
            self.palette.mode,
        ) {
            self.file_search.cancel();
            self.palette.prepare(PaletteMode::Launcher);
            self.relayout_palette();
        }
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn add_file_search_scope(&mut self, path: String) {
        if path.is_empty() {
            return;
        }
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let mut scopes = self.settings.file_search_scopes.clone();
        scopes.push(path);
        self.settings.file_search_scopes =
            tinycast_pure::file_search::policy::normalize_scopes(&scopes, &home);
        let _ = self.settings.save();
        self.apply_file_search_policy();
        self.invalidate_settings();
    }

    pub fn remove_file_search_scope(&mut self, index: usize) {
        if index < self.settings.file_search_scopes.len() {
            self.settings.file_search_scopes.remove(index);
            let _ = self.settings.save();
            self.apply_file_search_policy();
            self.invalidate_settings();
        }
    }

    pub fn add_file_search_ignore(&mut self, pattern: String) {
        let pattern = pattern.trim().to_string();
        if pattern.is_empty() {
            return;
        }
        if !self
            .settings
            .file_search_ignore_patterns
            .iter()
            .any(|p| p == &pattern)
        {
            self.settings.file_search_ignore_patterns.push(pattern);
            let _ = self.settings.save();
            self.apply_file_search_policy();
            self.invalidate_settings();
        }
    }

    pub fn remove_file_search_ignore(&mut self, index: usize) {
        if index < self.settings.file_search_ignore_patterns.len() {
            self.settings.file_search_ignore_patterns.remove(index);
            let _ = self.settings.save();
            self.apply_file_search_policy();
            self.invalidate_settings();
        }
    }

    fn apply_file_search_policy(&mut self) {
        self.file_search.apply(
            &self.settings.file_search_scopes,
            &self.settings.file_search_ignore_patterns,
        );
    }

    pub fn install_file_search(&mut self) {
        self.file_search.take_done();
        self.clamp_selection();
        self.invalidate_palette();
    }

    pub fn set_notes_enabled(&mut self, enabled: bool) {
        if self.settings.notes_enabled == enabled {
            return;
        }
        self.settings.notes_enabled = enabled;
        let _ = self.settings.save();
        if !enabled {
            self.notes_hide();
        }
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn notes_show(&mut self) {
        if !crate::features::notes::ui::coordinator::guarded(self.settings.notes_enabled) {
            return;
        }
        self.hide_palette();
        if let Err(err) = self.notes.show_last() {
            crate::surfaces::dialog::alert("Notes", &err.to_string());
            return;
        }
        self.ensure_notes_window();
        self.sync_notes_window();
        if let Some(window) = &self.notes_window {
            window.close_switcher();
            window.show();
            window.focus_editor();
        }
    }

    pub fn notes_create(&mut self) {
        if !crate::features::notes::ui::coordinator::guarded(self.settings.notes_enabled) {
            return;
        }
        self.hide_palette();
        if let Err(err) = self.notes.create() {
            crate::surfaces::dialog::alert("Notes", &err.to_string());
            return;
        }
        self.ensure_notes_window();
        self.sync_notes_window();
        if let Some(window) = &self.notes_window {
            window.close_switcher();
            window.show();
            window.focus_editor();
        }
    }

    pub fn notes_search(&mut self) {
        if !crate::features::notes::ui::coordinator::guarded(self.settings.notes_enabled) {
            return;
        }
        self.notes_show();
        self.notes_toggle_switcher();
    }

    pub fn notes_hide(&mut self) {
        let _ = self.notes.flush();
        if let Some(window) = &self.notes_window {
            window.hide();
        }
    }

    pub fn notes_toggle_switcher(&mut self) {
        if !crate::features::notes::ui::coordinator::guarded(self.settings.notes_enabled) {
            return;
        }
        self.ensure_notes_window();
        let open = self
            .notes_window
            .as_ref()
            .map(|w| w.switcher_is_open())
            .unwrap_or(false);
        if open {
            if let Some(window) = &self.notes_window {
                window.close_switcher();
            }
            return;
        }
        let _ = self.notes.reload();
        self.notes_switcher = self.notes.search("");
        if let Some(window) = &self.notes_window {
            window.fill_switcher(
                &self
                    .notes_switcher
                    .iter()
                    .map(|s| s.title.clone())
                    .collect::<Vec<_>>(),
            );
            window.open_switcher();
            window.focus_switcher();
        }
    }

    pub fn notes_filter_switcher(&mut self, query: &str) {
        self.notes_switcher = self.notes.search(query);
        if let Some(window) = &self.notes_window {
            window.fill_switcher(
                &self
                    .notes_switcher
                    .iter()
                    .map(|s| s.title.clone())
                    .collect::<Vec<_>>(),
            );
        }
    }

    pub fn notes_pick_switcher(&mut self, index: usize) {
        let Some(id) = self.notes_switcher.get(index).map(|s| s.id.clone()) else {
            return;
        };
        if let Err(err) = self.notes.select(&id) {
            crate::surfaces::dialog::alert("Notes", &err.to_string());
            return;
        }
        self.sync_notes_window();
        if let Some(window) = &self.notes_window {
            window.close_switcher();
            window.focus_editor();
        }
    }

    pub fn notes_body_changed(&mut self, body: String) {
        self.notes.set_body(body);
        let _ = self.notes.flush();
    }

    pub fn notes_open_folder(&mut self) {
        let path = self.notes.notes_directory().display().to_string();
        let _ = crate::features::launcher::ui::coordinator::execute(&LaunchSpec::Path(path));
    }

    fn ensure_notes_window(&mut self) {
        if self.notes_window.is_none() && !self.host.is_invalid() {
            self.notes_window = crate::surfaces::NotesWindow::create(self.host).ok();
        }
    }

    pub fn set_calendar_enabled(&mut self, enabled: bool) {
        if self.settings.calendar_enabled == enabled {
            return;
        }
        if enabled {
            if !crate::features::calendar::ui::coordinator::consent() {
                return;
            }
            if self.calendar.request_and_refresh().is_err() {
                crate::surfaces::dialog::alert(
                    "Calendar",
                    "Calendar access was denied. Meetings will stay empty.",
                );
            }
            self.calendar.arm_now();
        } else if self.palette.mode == PaletteMode::Schedule {
            self.palette.prepare(PaletteMode::Launcher);
        }
        self.settings.calendar_enabled = enabled;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
        self.apply_calendar_tooltip();
    }

    pub fn set_auto_join_meetings(&mut self, enabled: bool) {
        self.settings.auto_join_meetings = enabled;
        let _ = self.settings.save();
        if enabled {
            self.calendar.arm_now();
        }
        self.invalidate_settings();
    }

    pub fn set_camera_preview(&mut self, enabled: bool) {
        self.settings.camera_preview = enabled;
        let _ = self.settings.save();
        self.invalidate_settings();
    }

    pub fn cycle_join_window(&mut self) {
        self.settings.join_window_minutes =
            crate::features::calendar::settings::pane::cycle_join_window(
                self.settings.join_window_minutes,
            );
        let _ = self.settings.save();
        self.invalidate_settings();
        self.invalidate_palette();
    }

    fn calendar_window(&self) -> tinycast_pure::meeting::UpcomingWindow {
        crate::features::calendar::ui::coordinator::window(self.settings.join_window_minutes)
    }

    fn meeting_card(&self) -> Option<tinycast_pure::meeting::MeetingEvent> {
        if !crate::features::calendar::ui::coordinator::should_show_card(
            self.palette.mode,
            &self.palette.query,
            self.settings.calendar_enabled,
        ) {
            return None;
        }
        self.calendar_window()
            .carded(self.calendar.events(), unix_now())
    }

    fn calendar_join_next(&mut self) {
        if !self.settings.calendar_enabled {
            return;
        }
        self.calendar.refresh();
        let now = unix_now();
        let Some(event) = self
            .calendar_window()
            .joinable(self.calendar.events(), now)
        else {
            self.hide_palette();
            self.show_message_hud_tone(
                crate::features::calendar::ui::coordinator::NOTHING_TO_JOIN,
                tinycast_pure::dialog::DialogTone::Neutral,
            );
            return;
        };
        self.join_meeting(&event);
    }

    fn calendar_copy_link(&mut self) {
        if !self.settings.calendar_enabled {
            return;
        }
        let now = unix_now();
        let Some(event) = self
            .calendar_window()
            .joinable(self.calendar.events(), now)
        else {
            self.show_message_hud_tone(
                crate::features::calendar::ui::coordinator::NOTHING_TO_JOIN,
                tinycast_pure::dialog::DialogTone::Neutral,
            );
            return;
        };
        if let Some(url) = crate::features::calendar::ui::coordinator::copy_url(&event) {
            let _ = copy_text(&url);
            self.show_message_hud_tone("Copied meeting link", tinycast_pure::dialog::DialogTone::Success);
        }
    }

    fn calendar_open_app(&mut self) {
        self.hide_palette();
        let _ = execute(&LaunchSpec::Uri("outlookcal:".into()));
    }

    fn calendar_create_event(&mut self) {
        if !self.settings.calendar_enabled {
            return;
        }
        self.hide_palette();
        let _ = execute(&LaunchSpec::Uri("outlookcal:".into()));
    }

    fn open_schedule(&mut self) {
        if !self.settings.calendar_enabled {
            return;
        }
        self.calendar.refresh();
        self.close_menu();
        let was_visible = self.palette_visible;
        if !was_visible {
            self.remember_previous_hwnd();
        }
        self.palette.prepare(PaletteMode::Schedule);
        self.palette_visible = true;
        self.expanded = true;
        self.list_scroll = 0.0;
        if was_visible {
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.relayout_palette();
            self.invalidate_palette();
        } else {
            self.show_palette_window();
            self.expand_palette();
        }
    }

    fn join_meeting(&mut self, event: &tinycast_pure::meeting::MeetingEvent) {
        self.calendar.mark_joined(&event.id);
        self.hide_palette();
        let _ = crate::features::calendar::ui::coordinator::camera_preview_optional(
            self.settings.camera_preview,
        );
        if let Some(link) = &event.link {
            if crate::features::calendar::ui::coordinator::open_link(link).is_err() {
                let _ = execute(&LaunchSpec::Uri("outlookcal:".into()));
            }
        } else {
            let _ = execute(&LaunchSpec::Uri("outlookcal:".into()));
        }
    }

    fn maybe_auto_join(&mut self) {
        if !self.settings.calendar_enabled || !self.settings.auto_join_meetings {
            return;
        }
        let now = unix_now();
        let policy = tinycast_pure::meeting::AutoJoinPolicy {
            armed_at: self.calendar.armed_at(),
        };
        let joined = self.calendar.joined().clone();
        if let Some(event) = policy.meeting(
            self.calendar.events(),
            now,
            self.calendar_window(),
            &joined,
        ) {
            if self.settings.auto_join_confirms {
                let ok = crate::surfaces::dialog::confirm(&crate::surfaces::dialog::ConfirmPrompt {
                    title: format!("Join {}?", event.title),
                    message: "This meeting is starting now.".into(),
                    accept: "Join".into(),
                    cancel: "Cancel".into(),
                });
                self.calendar.mark_joined(&event.id);
                if !ok {
                    return;
                }
            }
            self.join_meeting(&event);
        }
    }

    fn apply_calendar_tooltip(&self) {
        if self.host.is_invalid() {
            return;
        }
        let summary = tinycast_pure::meeting::MenuBarSummary {
            lead_minutes: self.settings.join_window_minutes,
            hide_after_minutes: if self.settings.hide_current_event == 0 {
                None
            } else {
                Some(self.settings.hide_current_event)
            },
            linked_only: self.settings.menu_bar_linked_events_only,
        };
        let text = if self.settings.calendar_enabled {
            summary
                .event(self.calendar.events(), unix_now())
                .map(|e| {
                    format!(
                        "{} · {}",
                        tinycast_pure::meeting::MenuBarSummary::title(&e.title),
                        tinycast_pure::meeting::UpcomingWindow::countdown(e.start, unix_now())
                    )
                })
                .unwrap_or_else(|| "Tinycast".into())
        } else {
            "Tinycast".into()
        };
        crate::platform::tray::set_tooltip(self.host, &text);
    }

    fn schedule_rows(&self) -> Vec<tinycast_pure::meeting::MeetingEvent> {
        let q = self.palette.query.to_lowercase();
        tinycast_pure::meeting::UpcomingWindow::agenda(self.calendar.events(), unix_now())
            .into_iter()
            .filter(|e| q.is_empty() || e.title.to_lowercase().contains(&q))
            .collect()
    }

    fn sync_notes_window(&mut self) {
        let title = self
            .notes
            .active()
            .map(|n| n.title())
            .unwrap_or_else(|| "Notes".into());
        let body = self
            .notes
            .active()
            .map(|n| n.body.clone())
            .unwrap_or_default();
        if let Some(window) = &self.notes_window {
            window.set_title(&title);
            window.set_body(&body);
        }
    }

    pub fn set_snippets_enabled(&mut self, enabled: bool) {
        if self.settings.snippets_enabled == enabled {
            return;
        }
        self.settings.snippets_enabled = enabled;
        let _ = self.settings.save();
        if enabled {
            let _ = snippet_coordinator::ensure_ui_automation();
        }
        self.apply_snippets_enabled();
        self.invalidate_settings();
    }

    pub fn set_snippets_show_in_launcher(&mut self, show: bool) {
        self.settings.snippets_show_in_launcher = show;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn custom_command_records(&self) -> &[tinycast_pure::custom_command::CustomCommand] {
        self.custom_commands.commands()
    }

    pub fn quicklink_records(&self) -> &[tinycast_pure::quicklink::Quicklink] {
        self.quicklinks.links()
    }

    pub fn set_custom_commands_enabled(&mut self, enabled: bool) {
        self.settings.custom_commands_enabled = enabled;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_custom_commands_show_in_launcher(&mut self, show: bool) {
        self.settings.custom_commands_show_in_launcher = show;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_quicklinks_enabled(&mut self, enabled: bool) {
        self.settings.quicklinks_enabled = enabled;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_quicklinks_show_in_launcher(&mut self, show: bool) {
        self.settings.quicklinks_show_in_launcher = show;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn cycle_emoji_skin_tone(&mut self) {
        let next =
            tinycast_pure::emoji::EmojiSkinTone::from_raw(&self.settings.emoji_skin_tone).cycle();
        self.settings.emoji_skin_tone = next.as_raw().to_string();
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn new_custom_command(&mut self, owner: HWND) {
        if !self.custom_commands.is_available() {
            crate::surfaces::dialog::alert(
                "Library unavailable",
                crate::features::custom_commands::service::store::STORAGE_UNAVAILABLE,
            );
            return;
        }
        self.pause_global_hotkeys();
        let drafted = crate::features::custom_commands::ui::editor::edit(owner, None);
        self.resume_global_hotkeys();
        let Some(draft) = drafted else {
            return;
        };
        let command = tinycast_pure::custom_command::CustomCommand {
            id: new_item_id(),
            name: draft.name,
            command: draft.command,
            confirm: draft.confirm,
        };
        if let Err(err) = self.custom_commands.upsert(command) {
            crate::surfaces::dialog::alert("Could not save command", &err);
        }
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn edit_custom_command_at(&mut self, owner: HWND, index: usize) {
        if !self.custom_commands.is_available() {
            crate::surfaces::dialog::alert(
                "Library unavailable",
                crate::features::custom_commands::service::store::STORAGE_UNAVAILABLE,
            );
            return;
        }
        let Some(existing) = self.custom_commands.commands().get(index).cloned() else {
            return;
        };
        let initial = crate::features::custom_commands::ui::editor::CommandDraft {
            name: existing.name.clone(),
            command: existing.command.clone(),
            confirm: existing.confirm,
        };
        self.pause_global_hotkeys();
        let drafted =
            crate::features::custom_commands::ui::editor::edit(owner, Some(&initial));
        self.resume_global_hotkeys();
        let Some(draft) = drafted else {
            return;
        };
        let mut updated = existing;
        updated.name = draft.name;
        updated.command = draft.command;
        updated.confirm = draft.confirm;
        if let Err(err) = self.custom_commands.upsert(updated) {
            crate::surfaces::dialog::alert("Could not save command", &err);
        }
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn edit_quicklink(&mut self, owner: HWND, index: Option<usize>) {
        if !self.quicklinks.is_available() {
            crate::surfaces::dialog::alert(
                "Library unavailable",
                crate::features::quicklinks::service::store::STORAGE_UNAVAILABLE,
            );
            return;
        }
        let initial = index.and_then(|i| {
            self.quicklinks.links().get(i).map(|link| {
                crate::features::quicklinks::ui::editor::QuicklinkDraft {
                    name: link.name.clone(),
                    destination: link.destination.clone(),
                }
            })
        });
        let existing = index.and_then(|i| self.quicklinks.links().get(i).cloned());
        self.pause_global_hotkeys();
        let drafted = crate::features::quicklinks::ui::editor::edit(owner, initial.as_ref());
        self.resume_global_hotkeys();
        let Some(draft) = drafted else {
            return;
        };
        let mut link = existing.unwrap_or_else(|| tinycast_pure::quicklink::Quicklink {
            id: new_item_id(),
            name: String::new(),
            destination: String::new(),
            pinned: false,
            show_in_root: true,
            pin_order: 0,
        });
        link.name = draft.name;
        link.destination = draft.destination;
        if let Err(err) = self.quicklinks.upsert(link) {
            crate::surfaces::dialog::alert("Could not save quicklink", &err);
        }
        self.invalidate_palette();
        self.invalidate_settings();
    }

    fn create_quicklink_from_command(&mut self) {
        self.hide_palette();
        self.edit_quicklink(self.host, None);
    }

    pub fn import_quicklinks(&mut self, owner: HWND) {
        if !self.quicklinks.is_available() {
            crate::surfaces::dialog::alert(
                "Library unavailable",
                crate::features::quicklinks::service::store::STORAGE_UNAVAILABLE,
            );
            return;
        }
        let Some(path) = crate::features::quicklinks::settings::pane::pick_open_json(owner) else {
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => match self.quicklinks.import_from_bytes(&bytes) {
                Ok(_) => {
                    self.invalidate_palette();
                    self.invalidate_settings();
                }
                Err(err) => crate::surfaces::dialog::alert("Import failed", &err),
            },
            Err(err) => crate::surfaces::dialog::alert("Import failed", &err.to_string()),
        }
    }

    pub fn export_quicklinks(&mut self, owner: HWND) {
        if !self.quicklinks.is_available() {
            crate::surfaces::dialog::alert(
                "Library unavailable",
                crate::features::quicklinks::service::store::STORAGE_UNAVAILABLE,
            );
            return;
        }
        let Some(path) = crate::features::quicklinks::settings::pane::pick_save_json(owner) else {
            return;
        };
        match self.quicklinks.export_bytes() {
            Ok(bytes) => {
                if let Err(err) = std::fs::write(&path, bytes) {
                    crate::surfaces::dialog::alert("Export failed", &err.to_string());
                }
            }
            Err(err) => crate::surfaces::dialog::alert("Export failed", &err),
        }
    }

    pub fn on_snippet_keyword(&mut self) {
        let Some(pending) =
            crate::features::snippets::service::listener::take_pending()
        else {
            return;
        };
        if !self.settings.snippets_enabled {
            return;
        }
        let fg = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        if fg.0 as isize != pending.target {
            return;
        }
        let needle = pending.keyword.to_lowercase();
        let Some(record) = self
            .snippet_records
            .iter()
            .filter(|r| r.enabled)
            .filter(|r| {
                r.keyword
                    .as_deref()
                    .is_some_and(|k| k.trim().eq_ignore_ascii_case(&needle))
            })
            .min_by(|a, b| a.path.cmp(&b.path))
            .cloned()
        else {
            return;
        };
        if injector::insertion_refused(fg) {
            return;
        }
        let selection = snippet_coordinator::capture_if_uses_selection(&record.body, || {
            injector::capture_selection(fg, true)
        });
        let ctx = snippet_coordinator::expansion_context(
            self.clipboard.recent_text(20),
            selection,
            crate::platform::clock::local_naive_unix(),
            user_locale(),
        );
        let output = snippet_coordinator::expand_record(
            &record,
            &self.snippet_records,
            &ctx,
            &Default::default(),
        );
        if !output.arguments.is_empty() {
            self.previous_hwnd = fg;
            self.argument_session = Some(snippet_coordinator::ArgumentSession {
                kind: snippet_coordinator::ArgumentKind::Snippet,
                snippet_path: record.path.to_string_lossy().into_owned(),
                snippet_name: record.name.clone(),
                show_confirmation: record.show_confirmation,
                specs: output.arguments,
                values: std::collections::HashMap::new(),
                index: 0,
                clipboard: ctx.clipboard,
                selection: ctx.selection,
                now: ctx.now,
                locale: ctx.locale,
                tz: ctx.tz,
                keyword: Some(pending.keyword),
                target: pending.target,
            });
            self.palette.prepare(PaletteMode::QuicklinkArguments);
            self.palette_visible = true;
            self.expanded = true;
            self.show_palette_window();
            self.relayout_palette();
            self.invalidate_palette();
            return;
        }
        if injector::deliver_keyword(fg, &pending.keyword, &output.text, output.cursor) {
            if record.show_confirmation {
                self.show_message_hud(&record.name);
            }
        }
    }

    pub fn on_custom_command_failed(&mut self) {
        if let Some(err) = crate::features::custom_commands::service::runner::take_pending_error() {
            crate::surfaces::dialog::alert("Command failed", &err);
        }
    }

    pub fn install_snippets(&mut self) {
        if !self.settings.snippets_enabled {
            return;
        }
        if let Ok(snap) = self.snippet_repo.load() {
            self.snippet_records = snap.records;
            let keywords: Vec<String> = self
                .snippet_records
                .iter()
                .filter(|r| r.enabled)
                .filter_map(|r| r.keyword.clone())
                .collect();
            self.snippet_listener.update_keywords(keywords);
            self.clamp_selection();
            self.invalidate_palette();
            self.invalidate_settings();
        }
    }

    pub fn search_placeholder(&self) -> String {
        if self.palette.mode == PaletteMode::QuicklinkArguments {
            if let Some(session) = &self.argument_session {
                return session.current_name().to_string();
            }
        }
        crate::palette::edit::PLACEHOLDER_LAUNCHER.to_string()
    }

    pub fn install_app_index(&mut self) {
        if let Some(entries) = self.app_index.take_latest() {
            self.entries = entries;
            self.clamp_selection();
            self.invalidate_palette();
            self.invalidate_settings();
        }
    }

    pub fn launcher_paint_items(&self) -> Vec<PaintItem> {
        if self.palette.mode == PaletteMode::Emoji {
            let tone = tinycast_pure::emoji::EmojiSkinTone::from_raw(&self.settings.emoji_skin_tone);
            return crate::features::emoji::paint_items(
                &self.palette.query,
                tone,
                self.palette.selection,
                theme::size::PANEL_WIDTH,
            );
        }
        if self.palette.mode == PaletteMode::Quicklinks {
            let q = self.palette.query.to_lowercase();
            let mut items = Vec::new();
            for (i, link) in self
                .quicklinks
                .links()
                .iter()
                .filter(|l| q.is_empty() || l.name.to_lowercase().contains(&q))
                .enumerate()
            {
                items.push(PaintItem::Row {
                    title: link.name.clone(),
                    alias: None,
                    trailing: String::new(),
                    keycap: None,
                    icon_source: None,
                    selected: i == self.palette.selection,
                });
            }
            return items;
        }
        if self.palette.mode == PaletteMode::QuicklinkArguments {
            if let Some(session) = &self.argument_session {
                return snippet_coordinator::paint_argument_items(
                    session,
                    &self.palette.query,
                    self.palette.selection,
                );
            }
        }
        if self.palette.mode == PaletteMode::Clipboard {
            let rows = self
                .clipboard
                .search(&self.palette.query, self.clipboard_filter);
            return clip_screen::paint_items(&rows, self.palette.selection);
        }
        if self.palette.mode == PaletteMode::FileSearch {
            return crate::features::file_search::ui::screen::paint_items(
                self.file_search.state(),
                &self.palette.query,
                self.file_search.results(),
                self.palette.selection,
            );
        }
        if self.palette.mode == PaletteMode::Schedule {
            let mut items = Vec::new();
            for (i, event) in self.schedule_rows().into_iter().enumerate() {
                items.push(PaintItem::Row {
                    title: event.title,
                    alias: None,
                    trailing: tinycast_pure::meeting::UpcomingWindow::countdown(
                        event.start,
                        unix_now(),
                    ),
                    keycap: None,
                    icon_source: None,
                    selected: i == self.palette.selection,
                });
            }
            return items;
        }
        if self.palette.mode == PaletteMode::CalculatorHistory {
            return self.calc_history_paint_items();
        }
        let card = self.calc_result();
        let meeting = self.meeting_card();
        let lead = if card.is_some() || meeting.is_some() { 1 } else { 0 };
        let row_sel = self.palette.selection.checked_sub(lead).unwrap_or(usize::MAX);
        let mut items = paint_items(&self.sections(), row_sel, &self.favorites);
        if let Some(result) = card {
            items.insert(0, card::paint_item(&result, self.palette.selection == 0));
        } else if let Some(event) = meeting {
            items.insert(
                0,
                PaintItem::Row {
                    title: event.title,
                    alias: None,
                    trailing: tinycast_pure::meeting::UpcomingWindow::countdown(
                        event.start,
                        unix_now(),
                    ),
                    keycap: None,
                    icon_source: None,
                    selected: self.palette.selection == 0,
                },
            );
        }
        items
    }

    pub fn list_scroll(&self) -> f32 {
        self.list_scroll
    }

    pub fn appearance_key(&self) -> u8 {
        0
    }

    pub fn menu_is_open(&self) -> bool {
        self.menu.is_open()
    }

    pub fn footer_paint(&self) -> FooterPaint<'_> {
        FooterPaint {
            show_action_group: self.footer_action_group_visible(),
            primary_label: self.primary_label(),
        }
    }

    pub fn menu_paint(&self) -> Option<MenuPaint<'_>> {
        if !self.menu.is_open() || self.menu_items.is_empty() {
            return None;
        }
        Some(MenuPaint {
            header: self.menu_header.as_str(),
            items: &self.menu_items,
            selection: self.menu_selection,
            kind: self.menu,
        })
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
            window.on_tab_changed();
        }
    }

    pub fn feature_flags(&self) -> FeatureFlags {
        self.settings.feature_flags()
    }

    pub fn settings_entries(&self, kind: AppKind) -> Vec<AppEntry> {
        let mut entries = match kind {
            AppKind::Command => commands_catalog(),
            AppKind::SystemAction => tinycast_pure::system_action::SystemActionId::all()
                .iter()
                .copied()
                .map(tinycast_pure::system_action::SystemActionId::as_entry)
                .collect(),
            _ => self
                .entries
                .iter()
                .filter(|e| e.kind == kind)
                .cloned()
                .collect(),
        };
        self.apply_prefs(&mut entries);
        entries
    }

    pub fn set_kind_enabled(&mut self, kind: AppKind, on: bool) {
        self.visibility.set_kind_enabled(kind, on);
        self.persist_visibility();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_item_visible(&mut self, id: &str, visible: bool) {
        self.visibility.set_item_visible(id, visible);
        self.persist_visibility();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_alias_draft(&mut self, id: &str, draft: &str) {
        self.aliases.set(id.to_string(), commit_alias_text(draft));
        self.persist_aliases();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_hotkey(&mut self, action: &str, binding: Option<HotKeyBinding>) {
        self.hotkeys.set(action.to_string(), binding);
        self.persist_hotkeys();
        self.sync_hotkeys();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn ranking_is_empty(&self) -> bool {
        self.ranking.is_empty()
    }

    pub fn reset_learned_ranking(&mut self) {
        self.ranking.reset_all();
        let _ = self.ranking.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn pause_global_hotkeys(&self) {
        crate::features::hotkeys::service::center::pause(self.host);
    }

    pub fn resume_global_hotkeys(&self) {
        crate::features::hotkeys::service::center::resume(self.host, &self.hotkeys);
    }

    pub fn sync_hotkeys(&mut self) {
        crate::platform::keyboard_ll::set_host(self.host);
        crate::features::hotkeys::service::center::set_host(self.host);
        crate::features::hotkeys::service::center::sync(self.host, &self.hotkeys);
        crate::features::hotkeys::service::hyper::configure(
            crate::features::hotkeys::service::hyper::HyperKey::from_raw(&self.settings.hyper_key),
            self.settings.hyper_includes_shift,
        );
    }

    pub fn on_hotkey_action(&mut self) {
        let Some(action) = crate::features::hotkeys::service::center::take_pending() else {
            return;
        };
        self.perform_hotkey(&action);
    }

    pub fn perform_hotkey(&mut self, action: &str) {
        if action == "hotkey.togglePalette" {
            self.toggle_palette();
            return;
        }
        if let Some(raw) = action.strip_prefix("hotkey.systemAction.") {
            if let Some(id) = tinycast_pure::system_action::SystemActionId::from_raw(raw) {
                self.run_system_action(id);
            }
            return;
        }
        if let Some(raw) = action.strip_prefix("hotkey.windowCommand.") {
            if let Some(id) = tinycast_pure::window_command::WindowCommandId::from_raw(raw) {
                self.run_window_command(id);
            }
            return;
        }
        if let Some(id) = CommandID::from_hotkey_key(action) {
            if id.shows_in_launcher(self.feature_flags())
                && self.visibility.allows_hotkey(AppKind::Command)
            {
                self.activate_entry(&id.as_entry());
            }
            return;
        }
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| hotkey_action_key(e).as_deref() == Some(action))
        {
            if self.visibility.allows_hotkey(entry.kind) {
                let entry = entry.clone();
                self.activate_entry(&entry);
            }
        }
    }

    pub fn shutdown_hotkeys(&mut self) {
        crate::features::hotkeys::service::hyper::shutdown();
        crate::features::hotkeys::service::center::pause(self.host);
    }

    pub fn toggle_palette(&mut self) {
        if self.palette_visible {
            self.hide_palette();
        } else {
            self.remember_previous_hwnd();
            self.palette.prepare(PaletteMode::Launcher);
            self.palette_visible = true;
            self.expanded = false;
            self.app_index.start();
            self.show_palette_window();
        }
    }

    pub fn hide_palette(&mut self) {
        self.argument_session = None;
        if self.palette.mode == PaletteMode::FileSearch {
            self.file_search.cancel();
        }
        self.close_menu();
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
        if self.menu.is_open() {
            self.close_menu();
            return;
        }
        let launcher = self.palette.mode == PaletteMode::Launcher;
        match escape_outcome(&self.palette.query, launcher) {
            EscapeOutcome::ClearQuery => {
                self.palette.query.clear();
                self.palette.is_composing = false;
                if self.palette.mode == PaletteMode::FileSearch {
                    self.file_search.cancel();
                }
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
        if self.menu.is_open() {
            return;
        }
        if self.palette.query != text {
            self.palette.selection = 0;
            self.list_scroll = 0.0;
        }
        self.palette.query = text;
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
        if self.palette.mode == PaletteMode::FileSearch {
            self.file_search.search(&self.palette.query);
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
        if self.menu.is_open() || c.is_control() {
            return;
        }
        self.palette.query.push(c);
        self.palette.selection = 0;
        self.list_scroll = 0.0;
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
        if self.palette.mode == PaletteMode::FileSearch {
            self.file_search.search(&self.palette.query);
        }
        self.clamp_selection();
        self.invalidate_palette();
    }

    pub fn handle_key(&mut self, vk: u16) -> bool {
        if vk == VK_ESCAPE.0 {
            self.handle_escape();
            return true;
        }
        if vk == 0x08
            && self.palette.mode == PaletteMode::QuicklinkArguments
            && self.palette.query.is_empty()
        {
            self.step_back_argument();
            return true;
        }
        if vk == 0x09 {
            self.hop_tab();
            return true;
        }
        if vk == 0x4B && ctrl_down() && !shift_down() && !alt_down() {
            self.toggle_actions();
            return true;
        }
        if self.palette.mode == PaletteMode::Clipboard {
            if vk == 0x50 && ctrl_down() && !shift_down() && !alt_down() {
                self.toggle_clipboard_filter_menu();
                return true;
            }
            if vk == 0xBE && ctrl_down() && !shift_down() && !alt_down() {
                self.toggle_clipboard_pin();
                return true;
            }
        }
        if vk == 0x46 && ctrl_down() && shift_down() && !alt_down() {
            if self.expanded {
                self.toggle_favorite_selected();
                self.close_menu();
            }
            return true;
        }
        if vk == 0x43 && ctrl_down() && alt_down() && !shift_down() {
            self.copy_path_selected();
            return true;
        }
        if (vk == VK_UP.0 || vk == VK_DOWN.0) && ctrl_down() && alt_down() {
            if self.expanded {
                let delta = if vk == VK_UP.0 { -1 } else { 1 };
                self.move_favorite_selected(delta);
                self.close_menu();
            }
            return self.expanded;
        }
        if vk == VK_RETURN.0 {
            if ctrl_down() && !alt_down() {
                self.reveal_selected();
                return true;
            }
            if self.menu.is_open() {
                self.activate_menu_item();
                return true;
            }
            self.activate_selected();
            return true;
        }
        if self.menu.is_open() {
            if vk == VK_DOWN.0 {
                self.move_menu(1);
                return true;
            }
            if vk == VK_UP.0 {
                self.move_menu(-1);
                return true;
            }
            if ctrl_down() {
                if let Some(n) = FavoritesStore::digit_from_vk(vk) {
                    self.activate_favorite_slot(n);
                    return true;
                }
            }
            return true;
        }
        if self.palette.mode == PaletteMode::Emoji {
            let cols = crate::features::emoji::columns(theme::size::PANEL_WIDTH) as i32;
            if vk == VK_LEFT.0 {
                self.move_selection(-1);
                return true;
            }
            if vk == VK_RIGHT.0 {
                self.move_selection(1);
                return true;
            }
            if vk == VK_DOWN.0 {
                self.move_selection(cols);
                return true;
            }
            if vk == VK_UP.0 {
                self.move_selection(-cols);
                return true;
            }
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
                if self.palette.mode == PaletteMode::Clipboard {
                    self.activate_clipboard_pin_slot(n);
                } else {
                    self.activate_favorite_slot(n);
                }
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
        let count = self.selectable_len();
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
        if !self.expanded || self.menu.is_open() {
            return;
        }
        let notches = wheel_delta as f32 / 120.0;
        self.list_scroll -= notches * ROW_HEIGHT;
        self.clamp_scroll();
        self.invalidate_palette();
    }

    pub fn select_at_y(&mut self, y_dip: f32) -> bool {
        self.select_at(0.0, y_dip)
    }

    pub fn select_at(&mut self, x_dip: f32, y_dip: f32) -> bool {
        if !self.expanded {
            return false;
        }
        if self.palette.mode == PaletteMode::Emoji {
            let items = self.launcher_paint_items();
            if let Some(index) = crate::features::emoji::hit_index(
                &items,
                x_dip,
                y_dip,
                self.list_scroll,
                theme::size::PANEL_WIDTH,
            ) {
                self.palette.selection = index;
                self.invalidate_palette();
                return true;
            }
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
        if self.palette.mode == PaletteMode::Emoji {
            let tone = tinycast_pure::emoji::EmojiSkinTone::from_raw(&self.settings.emoji_skin_tone);
            if let Some(glyph) =
                crate::features::emoji::glyph_at(&self.palette.query, tone, self.palette.selection)
            {
                let previous = self.previous_hwnd;
                let _ = crate::features::launcher::ui::coordinator::copy_text(&glyph);
                self.hide_palette();
                crate::platform::paster::paste_into(previous);
            }
            return;
        }
        if self.palette.mode == PaletteMode::Quicklinks {
            let q = self.palette.query.to_lowercase();
            let links: Vec<_> = self
                .quicklinks
                .links()
                .iter()
                .filter(|l| q.is_empty() || l.name.to_lowercase().contains(&q))
                .cloned()
                .collect();
            if let Some(link) = links.get(self.palette.selection) {
                self.open_quicklink(&link.as_entry().id);
            }
            return;
        }
        if self.palette.mode == PaletteMode::QuicklinkArguments {
            self.commit_argument();
            return;
        }
        if !self.expanded {
            self.expand_select_first();
            return;
        }
        if self.meeting_card().is_some() && self.palette.selection == 0 {
            if let Some(event) = self.meeting_card() {
                self.join_meeting(&event);
            }
            return;
        }
        let card = self.calc_result();
        if card.is_some() && self.palette.selection == 0 {
            if let Some(result) = card {
                if calc_coordinator::copy_calculator_result(&mut self.calc_history, &result) {
                    self.persist_calc_history();
                    self.hide_palette();
                }
            }
            return;
        }
        let row_index = if card.is_some() {
            self.palette.selection - 1
        } else {
            self.palette.selection
        };
        if self.palette.mode == PaletteMode::CalculatorHistory {
            if let Some(entry) = self
                .calc_history
                .search(&self.palette.query)
                .get(row_index)
                .cloned()
            {
                if calc_coordinator::copy_history_result(&entry.result) {
                    self.hide_palette();
                }
            }
            return;
        }
        if self.palette.mode == PaletteMode::Clipboard {
            self.paste_clipboard_at(row_index);
            return;
        }
        if self.palette.mode == PaletteMode::FileSearch {
            self.activate_file_search_at(self.palette.selection);
            return;
        }
        if self.palette.mode == PaletteMode::Schedule {
            if let Some(event) = self.schedule_rows().get(self.palette.selection).cloned() {
                self.join_meeting(&event);
            }
            return;
        }
        let sections = self.sections();
        let Some(entry) = selectable_rows(&sections).get(row_index).cloned() else {
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
            LaunchSpec::OpenCalculatorHistory => self.open_calculator_history(),
            LaunchSpec::OpenClipboardHistory => self.open_clipboard_history(),
            LaunchSpec::ExpandSnippet(id) => self.begin_snippet_expansion(&id),
            LaunchSpec::RunCustomCommand(id) => self.run_custom_command(&id),
            LaunchSpec::OpenQuicklink(id) => self.open_quicklink(&id),
            LaunchSpec::SearchQuicklinks => self.open_quicklinks_search(),
            LaunchSpec::SearchEmoji => self.open_emoji(),
            LaunchSpec::SearchFiles => self.open_file_search(),
            LaunchSpec::ShowNotes => self.notes_show(),
            LaunchSpec::CreateNote => self.notes_create(),
            LaunchSpec::SearchNotes => self.notes_search(),
            LaunchSpec::JoinNextMeeting => self.calendar_join_next(),
            LaunchSpec::CopyMeetingLink => self.calendar_copy_link(),
            LaunchSpec::MySchedule => self.open_schedule(),
            LaunchSpec::OpenInCalendar => self.calendar_open_app(),
            LaunchSpec::CreateEvent => self.calendar_create_event(),
            LaunchSpec::CreateQuicklink => self.create_quicklink_from_command(),
            LaunchSpec::ImportQuicklinks => {
                self.hide_palette();
                self.import_quicklinks(self.host);
            }
            LaunchSpec::ExportQuicklinks => {
                self.hide_palette();
                self.export_quicklinks(self.host);
            }
            LaunchSpec::RunSystemAction(id) => {
                if let Some(action) =
                    tinycast_pure::system_action::SystemActionId::from_entry_id(&id)
                {
                    self.run_system_action(action);
                }
            }
            LaunchSpec::RunWindowCommand(id) => {
                if let Some(command) =
                    tinycast_pure::window_command::WindowCommandId::from_entry_id(&id)
                {
                    self.run_window_command(command);
                }
            }
            other => {
                self.hide_palette();
                let _ = execute(&other);
            }
        }
    }

    pub fn run_system_action(&mut self, id: tinycast_pure::system_action::SystemActionId) {
        use crate::features::system_actions::ui::coordinator as sys;
        let previous = self.action_target();
        if self.palette_visible {
            self.hide_palette();
        }
        if sys::is_stage_manager(id) {
            self.show_message_hud_tone(
                sys::STAGE_MANAGER_UNAVAILABLE,
                tinycast_pure::dialog::DialogTone::Neutral,
            );
            return;
        }
        match sys::plan(id) {
            sys::RunPlan::Confirm {
                title,
                message,
                accept,
            } => {
                if !sys::gated_run(sys::confirm(&title, &message, &accept)) {
                    return;
                }
                self.apply_system_action(id, previous);
            }
            sys::RunPlan::PickVolume => {
                let current = crate::features::system_actions::service::runner::current_volume()
                    .unwrap_or(0.5);
                let Some(level) = crate::surfaces::dialog::pick_volume(current) else {
                    return;
                };
                if let Err(failure) =
                    crate::features::system_actions::service::runner::set_volume(level)
                {
                    self.present_system_action_failure(id, failure);
                    return;
                }
                self.show_volume_hud();
            }
            sys::RunPlan::Execute => self.apply_system_action(id, previous),
        }
    }

    fn apply_system_action(
        &mut self,
        id: tinycast_pure::system_action::SystemActionId,
        previous: HWND,
    ) {
        use crate::features::system_actions::ui::coordinator as sys;
        match sys::execute(id, previous) {
            Ok(feedback) => {
                if sys::shows_volume(id) {
                    self.show_volume_hud();
                } else if let Some(feedback) = feedback {
                    self.show_message_hud_tone(&feedback.message, sys::tone_for(&feedback));
                }
            }
            Err(failure) => self.present_system_action_failure(id, failure),
        }
    }

    fn present_system_action_failure(
        &mut self,
        id: tinycast_pure::system_action::SystemActionId,
        failure: crate::features::system_actions::service::runner::Failure,
    ) {
        crate::surfaces::dialog::alert(
            &format!("“{}” Failed", id.name()),
            &failure.message,
        );
        if let Some(uri) = failure.settings_uri {
            let _ = crate::features::launcher::ui::coordinator::execute(
                &LaunchSpec::Uri(uri.to_string()),
            );
        }
    }

    fn show_message_hud_tone(&mut self, message: &str, tone: tinycast_pure::dialog::DialogTone) {
        if self.hud.is_none() && !self.host.is_invalid() {
            self.hud = MessageHud::create(self.host).ok();
        }
        if let Some(hud) = &self.hud {
            hud.show_message(message, tone);
        }
    }

    pub fn run_window_command(&mut self, id: tinycast_pure::window_command::WindowCommandId) {
        let previous = self.action_target();
        if self.palette_visible {
            self.hide_palette();
        }
        match window_coordinator::run(
            &mut self.window_mover,
            id,
            previous,
            self.settings.window_management_enabled,
            self.settings.window_gap as f32,
            self.settings.window_cycle_on_repeat,
        ) {
            window_coordinator::Outcome::Hud(message) => {
                self.show_message_hud_tone(message, tinycast_pure::dialog::DialogTone::Neutral);
            }
            window_coordinator::Outcome::Disabled
            | window_coordinator::Outcome::Quiet
            | window_coordinator::Outcome::Moved => {}
        }
    }

    pub fn set_window_management_enabled(&mut self, enabled: bool) {
        self.settings.window_management_enabled = enabled;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_window_management_show_in_launcher(&mut self, show: bool) {
        self.settings.window_management_show_in_launcher = show;
        let _ = self.settings.save();
        self.invalidate_palette();
        self.invalidate_settings();
    }

    pub fn set_window_cycle_on_repeat(&mut self, on: bool) {
        self.settings.window_cycle_on_repeat = on;
        let _ = self.settings.save();
        self.invalidate_settings();
    }

    pub fn cycle_window_gap(&mut self) {
        self.settings.window_gap =
            crate::features::window_management::settings::pane::cycle_gap(self.settings.window_gap);
        let _ = self.settings.save();
        self.invalidate_settings();
    }

    pub fn cycle_hyper_key(&mut self) {
        self.settings.hyper_key =
            crate::features::settings::panes::general::cycle_hyper(&self.settings.hyper_key)
                .to_string();
        let _ = self.settings.save();
        self.sync_hotkeys();
        self.invalidate_settings();
    }

    pub fn set_hyper_includes_shift(&mut self, on: bool) {
        if crate::features::hotkeys::service::hyper::HyperKey::from_raw(&self.settings.hyper_key)
            == crate::features::hotkeys::service::hyper::HyperKey::None
        {
            return;
        }
        let snapshot = self.hotkeys.snapshot();
        let retargeted =
            tinycast_pure::hotkey::retarget_hyper_bindings(&snapshot, on);
        for (action, binding) in retargeted {
            self.hotkeys.set(action, Some(binding));
        }
        self.settings.hyper_includes_shift = on;
        let _ = self.settings.save();
        self.persist_hotkeys();
        self.sync_hotkeys();
        self.invalidate_settings();
    }

    pub fn cycle_appearance(&mut self) {
        self.settings.appearance = match self.settings.appearance {
            crate::app_settings::Appearance::System => crate::app_settings::Appearance::Light,
            crate::app_settings::Appearance::Light => crate::app_settings::Appearance::Dark,
            crate::app_settings::Appearance::Dark => crate::app_settings::Appearance::System,
        };
        let _ = self.settings.save();
        self.invalidate_settings();
        self.invalidate_palette();
    }

    pub fn toggle_setting_bool(&mut self, which: crate::features::settings::panes::general::GeneralToggle) {
        use crate::features::settings::panes::general::GeneralToggle;
        match which {
            GeneralToggle::Compact => self.settings.compact_mode = !self.settings.compact_mode,
            GeneralToggle::FavoritesInCompact => {
                self.settings.show_favorites_in_compact = !self.settings.show_favorites_in_compact
            }
            GeneralToggle::FollowCursor => {
                self.settings.open_on_cursor_screen = !self.settings.open_on_cursor_screen
            }
            GeneralToggle::Draggable => {
                self.settings.palette_draggable = !self.settings.palette_draggable
            }
            GeneralToggle::LaunchAtLogin => {
                self.settings.launch_at_login = !self.settings.launch_at_login
            }
            GeneralToggle::ShowInMenuBar => {
                self.settings.show_in_menu_bar = !self.settings.show_in_menu_bar;
                crate::platform::tray::set_icon_visible(self.host, self.settings.show_in_menu_bar);
            }
            GeneralToggle::AutoSwitch => {
                self.settings.auto_switch_input_source = !self.settings.auto_switch_input_source
            }
        }
        let _ = self.settings.save();
        self.invalidate_settings();
        self.invalidate_palette();
    }

    pub fn cycle_pop_to_root(&mut self) {
        self.settings.pop_to_root_timeout =
            crate::features::settings::panes::general::cycle_pop_to_root(
                self.settings.pop_to_root_timeout,
            );
        let _ = self.settings.save();
        self.invalidate_settings();
    }

    pub fn appearance_label(&self) -> &'static str {
        match self.settings.appearance {
            crate::app_settings::Appearance::System => "system",
            crate::app_settings::Appearance::Light => "light",
            crate::app_settings::Appearance::Dark => "dark",
        }
    }

    fn show_volume_hud(&mut self) {
        let (level, muted) =
            crate::features::system_actions::service::runner::output_state().unwrap_or((0.0, true));
        if self.hud.is_none() && !self.host.is_invalid() {
            self.hud = MessageHud::create(self.host).ok();
        }
        if let Some(hud) = &self.hud {
            hud.show_volume(level, muted);
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
        let flags = self.feature_flags();
        let mut entries = self.entries.clone();
        if self.settings.quicklinks_enabled && self.settings.quicklinks_show_in_launcher {
            entries.extend(
                self.quicklinks
                    .links()
                    .iter()
                    .filter(|l| l.show_in_root)
                    .map(|l| l.as_entry()),
            );
        }
        if self.settings.custom_commands_enabled && self.settings.custom_commands_show_in_launcher {
            entries.extend(self.custom_commands.commands().iter().map(|c| c.as_entry()));
        }
        if self.settings.snippets_enabled && self.settings.snippets_show_in_launcher {
            entries.extend(
                self.snippet_records
                    .iter()
                    .filter(|record| record.enabled)
                    .map(snippet_coordinator::snippet_entry),
            );
        }
        entries.extend(
            tinycast_pure::system_action::SystemActionId::all()
                .iter()
                .copied()
                .map(tinycast_pure::system_action::SystemActionId::as_entry),
        );
        if self.settings.window_management_enabled
            && self.settings.window_management_show_in_launcher
        {
            entries.extend(
                tinycast_pure::window_command::WindowCommandId::all()
                    .iter()
                    .copied()
                    .map(tinycast_pure::window_command::WindowCommandId::as_entry),
            );
        }
        entries.extend(
            CommandID::all()
                .iter()
                .copied()
                .filter(|id| id.shows_in_launcher(flags))
                .map(CommandID::as_entry),
        );
        self.apply_prefs(&mut entries);
        entries
    }

    fn apply_prefs(&self, entries: &mut [AppEntry]) {
        for entry in entries {
            entry.fields.user_alias = self.aliases.get(&entry.id).map(str::to_string);
            entry.hotkey = hotkey_action_key(entry).and_then(|key| self.hotkeys.get(&key).cloned());
        }
    }

    pub fn toggle_actions(&mut self) {
        if self.menu == OpenMenu::Actions {
            self.close_menu();
            return;
        }
        if !can_open_actions(self.expanded, self.has_selectable_rows()) {
            return;
        }
        self.open_actions();
    }

    pub fn pointer_down(&mut self, x: f32, y: f32, panel_w: f32, panel_h: f32, double: bool) {
        if self.menu.is_open() {
            let has_header = !self.menu_header.is_empty();
            let frame = menu_frame(
                self.menu,
                panel_w,
                panel_h,
                self.menu_items.len(),
                has_header,
            );
            if point_in(frame, x, y) {
                if let Some(index) = menu_row_at(frame, has_header, self.menu_items.len(), x, y) {
                    self.menu_selection = index;
                    self.activate_menu_item();
                }
                return;
            }
            self.close_menu();
            return;
        }
        if self.palette.mode == PaletteMode::Clipboard {
            let button = clip_screen::filter_button_rect(panel_w);
            if point_in(button, x, y) {
                self.toggle_clipboard_filter_menu();
                return;
            }
        }
        if self.footer_action_group_visible() {
            if let Some(group) = action_group_rects(panel_w, panel_h) {
                if point_in(group.actions, x, y) {
                    self.toggle_actions();
                    return;
                }
                if point_in(group.primary, x, y) {
                    self.activate_selected();
                    return;
                }
            }
        }
        if self.select_at(x, y) && double {
            self.activate_selected();
        }
    }

    pub fn pointer_move(&mut self, x: f32, y: f32, panel_w: f32, panel_h: f32) {
        if !self.menu.is_open() {
            return;
        }
        let has_header = !self.menu_header.is_empty();
        let frame = menu_frame(
            self.menu,
            panel_w,
            panel_h,
            self.menu_items.len(),
            has_header,
        );
        if let Some(index) = menu_row_at(frame, has_header, self.menu_items.len(), x, y) {
            if index != self.menu_selection {
                self.menu_selection = index;
                self.invalidate_palette();
            }
        }
    }

    fn open_actions(&mut self) {
        if self.palette.mode == PaletteMode::FileSearch {
            let Some(hit) = self.file_search.results().get(self.palette.selection) else {
                return;
            };
            self.menu_header = hit.name.clone();
            self.menu_items = vec![
                MenuItem {
                    id: ID_OPEN,
                    label: "Open",
                    shortcut: Some("↵"),
                },
                MenuItem {
                    id: ID_SHOW_IN_FOLDER,
                    label: "Show in Folder",
                    shortcut: Some("Ctrl+↵"),
                },
                MenuItem {
                    id: ID_COPY_PATH,
                    label: "Copy Path",
                    shortcut: Some("Ctrl+Alt+C"),
                },
            ];
            self.menu_selection = 0;
            self.menu = OpenMenu::Actions;
            if let Some(window) = &self.palette_window {
                window.set_search_caret_visible(false);
            }
            self.invalidate_palette();
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let items = actions_for(self.action_context(&entry));
        if items.is_empty() {
            return;
        }
        self.menu_header = entry.name.clone();
        self.menu_items = items;
        self.menu_selection = 0;
        self.menu = OpenMenu::Actions;
        if let Some(window) = &self.palette_window {
            window.set_search_caret_visible(false);
        }
        self.invalidate_palette();
    }

    fn close_menu(&mut self) {
        if self.menu == OpenMenu::None && self.menu_items.is_empty() {
            return;
        }
        self.menu = OpenMenu::None;
        self.menu_items.clear();
        self.menu_header.clear();
        self.menu_selection = 0;
        if let Some(window) = &self.palette_window {
            window.set_search_caret_visible(true);
        }
        self.invalidate_palette();
    }

    fn activate_menu_item(&mut self) {
        let Some(item) = self.menu_items.get(self.menu_selection).copied() else {
            self.close_menu();
            return;
        };
        match item.id {
            ID_OPEN => {
                self.close_menu();
                self.activate_selected();
            }
            ID_FAVORITE => {
                self.toggle_favorite_selected();
                self.close_menu();
            }
            ID_MOVE_UP => {
                self.move_favorite_selected(-1);
                self.close_menu();
            }
            ID_MOVE_DOWN => {
                self.move_favorite_selected(1);
                self.close_menu();
            }
            ID_RESET_RANKING => {
                self.reset_ranking_selected();
                self.close_menu();
            }
            ID_SHOW_IN_FOLDER => {
                self.close_menu();
                self.reveal_selected();
            }
            ID_COPY_PATH => {
                self.copy_path_selected();
                self.close_menu();
            }
            ID_UNINSTALL => {
                self.close_menu();
            }
            other => {
                if let Some(filter) = clip_screen::filter_from_id(other) {
                    self.clipboard_filter = filter;
                    self.palette.selection = 0;
                    self.list_scroll = 0.0;
                }
                self.close_menu();
            }
        }
    }

    fn move_menu(&mut self, delta: i32) {
        let count = self.menu_items.len();
        if count == 0 {
            return;
        }
        let next = if delta < 0 {
            self.menu_selection
                .saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.menu_selection.saturating_add(delta as usize)
        };
        self.menu_selection = clamp_menu_selection(next, count);
        self.invalidate_palette();
    }

    fn footer_action_group_visible(&self) -> bool {
        self.expanded && self.has_selectable_rows()
    }

    fn has_selectable_rows(&self) -> bool {
        self.selectable_len() > 0
    }

    fn selected_entry(&self) -> Option<AppEntry> {
        if self.palette.mode == PaletteMode::CalculatorHistory
            || self.palette.mode == PaletteMode::Clipboard
            || self.palette.mode == PaletteMode::FileSearch
            || self.palette.mode == PaletteMode::Schedule
        {
            return None;
        }
        let lead = if self.calc_result().is_some() || self.meeting_card().is_some() {
            1
        } else {
            0
        };
        let index = self.palette.selection.checked_sub(lead)?;
        selectable_rows(&self.sections())
            .get(index)
            .cloned()
            .cloned()
    }

    fn action_context(&self, entry: &AppEntry) -> ActionContext {
        let pins = self.palette.query.is_empty();
        let pinned = if pins {
            self.pinned_favorite_ids()
        } else {
            Vec::new()
        };
        let fav_i = pinned.iter().position(|id| id == &entry.id);
        ActionContext {
            kind: entry.kind,
            is_favorite: self.favorites.contains(&entry.id),
            can_move_up: fav_i.map(|i| i > 0).unwrap_or(false),
            can_move_down: fav_i.map(|i| i + 1 < pinned.len()).unwrap_or(false),
            has_ranking: self.ranking.has_ranking(&entry.id),
            running: false,
        }
    }

    fn pinned_favorite_ids(&self) -> Vec<String> {
        let catalog = self.catalog();
        let visible: Vec<&str> = catalog
            .iter()
            .filter(|e| {
                self.visibility.is_kind_enabled(e.kind) && self.visibility.is_item_visible(&e.id)
            })
            .map(|e| e.id.as_str())
            .collect();
        self.favorites
            .ids
            .iter()
            .filter(|id| visible.iter().any(|v| v == id))
            .cloned()
            .collect()
    }

    fn toggle_favorite_selected(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let id = entry.id.clone();
        let removing = self.favorites.contains(&id);
        let fav_index = if self.palette.query.is_empty() {
            self.pinned_favorite_ids().iter().position(|k| k == &id)
        } else {
            None
        };
        self.favorites.toggle(id.clone());
        self.persist_favorites();
        if self.palette.query.is_empty() {
            if removing {
                self.palette.selection = fav_index.unwrap_or(0).saturating_sub(1);
            } else if let Some(next) = selectable_rows(&self.sections())
                .iter()
                .position(|row| row.id == id)
            {
                self.palette.selection = next;
            }
        }
        self.clamp_selection();
        self.ensure_selection_visible();
        self.invalidate_palette();
    }

    fn move_favorite_selected(&mut self, delta: i32) {
        if !self.palette.query.is_empty() {
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let pinned = self.pinned_favorite_ids();
        let Some(index) = pinned.iter().position(|id| id == &entry.id) else {
            return;
        };
        let target = index as i32 + delta;
        if target < 0 || target >= pinned.len() as i32 {
            return;
        }
        let other = pinned[target as usize].clone();
        self.favorites.exchange(&entry.id, &other);
        self.persist_favorites();
        if let Some(next) = selectable_rows(&self.sections())
            .iter()
            .position(|row| row.id == entry.id)
        {
            self.palette.selection = next;
        }
        self.ensure_selection_visible();
        self.invalidate_palette();
    }

    fn reset_ranking_selected(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        self.ranking.reset(&entry.id);
        let _ = self.ranking.save();
        self.clamp_selection();
        self.ensure_selection_visible();
        self.invalidate_palette();
    }

    fn copy_path_selected(&mut self) {
        if self.palette.mode == PaletteMode::FileSearch {
            if let Some(hit) = self.file_search.results().get(self.palette.selection) {
                let _ = copy_text(&hit.path.replace('/', "\\"));
            }
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let Some(text) = copy_path_text(&entry) else {
            return;
        };
        let _ = copy_text(&text);
    }

    fn reveal_selected(&mut self) {
        if self.palette.mode == PaletteMode::FileSearch {
            if let Some(hit) = self.file_search.results().get(self.palette.selection).cloned()
            {
                self.hide_palette();
                let _ = crate::features::file_search::ui::coordinator::reveal(&hit);
            }
            return;
        }
        let Some(entry) = self.selected_entry() else {
            return;
        };
        let Some(path) = reveal_path(&entry) else {
            return;
        };
        self.hide_palette();
        let _ = show_in_folder(&path);
    }

    fn open_file_search(&mut self) {
        if !crate::features::file_search::ui::coordinator::guarded_show(
            self.settings.file_search_enabled,
        ) {
            return;
        }
        self.apply_file_search_policy();
        self.close_menu();
        let was_visible = self.palette_visible;
        if !was_visible {
            self.remember_previous_hwnd();
        }
        self.palette.prepare(PaletteMode::FileSearch);
        self.file_search.cancel();
        self.palette_visible = true;
        self.expanded = true;
        self.list_scroll = 0.0;
        if was_visible {
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.relayout_palette();
            self.invalidate_palette();
        } else {
            self.show_palette_window();
            self.expand_palette();
        }
    }

    fn activate_file_search_at(&mut self, index: usize) {
        let Some(hit) = self.file_search.results().get(index).cloned() else {
            return;
        };
        self.hide_palette();
        if crate::features::file_search::ui::coordinator::open(&hit).is_err() {
            crate::surfaces::dialog::alert(
                &format!("Couldn’t Open {}", hit.name),
                "The file could not be opened.",
            );
        }
    }

    fn persist_visibility(&self) {
        let _ = self.visibility.save(store_path("visibility.json"));
    }

    fn persist_aliases(&self) {
        let _ = self.aliases.save(store_path("aliases.json"));
    }

    fn persist_hotkeys(&self) {
        let _ = self.hotkeys.save(store_path("hotkeys.json"));
    }

    fn persist_favorites(&self) {
        let _ = self.favorites.save(store_path("favorites.json"));
    }

    fn persist_calc_history(&self) {
        let _ = self
            .calc_history
            .save(store_path("calculator-history.json"));
    }

    fn calc_result(&self) -> Option<CalcResult> {
        match self.palette.mode {
            PaletteMode::Launcher | PaletteMode::CalculatorHistory => evaluate(
                &self.palette.query,
                local_naive_unix(),
                self.currency_rates.rates(),
                self.currency_rates.region().as_deref(),
            ),
            _ => None,
        }
    }

    fn selectable_len(&self) -> usize {
        match self.palette.mode {
            PaletteMode::CalculatorHistory => selectable_count(
                self.calc_result().is_some(),
                self.calc_history.search(&self.palette.query).len(),
            ),
            PaletteMode::Clipboard => self
                .clipboard
                .search(&self.palette.query, self.clipboard_filter)
                .len(),
            PaletteMode::FileSearch => self.file_search.results().len(),
            PaletteMode::Schedule => self.schedule_rows().len(),
            PaletteMode::Emoji => {
                let tone = tinycast_pure::emoji::EmojiSkinTone::from_raw(&self.settings.emoji_skin_tone);
                tinycast_pure::emoji::search_emoji_with_tone(&self.palette.query, tone).len()
            }
            PaletteMode::Quicklinks => {
                let q = self.palette.query.to_lowercase();
                self.quicklinks
                    .links()
                    .iter()
                    .filter(|l| q.is_empty() || l.name.to_lowercase().contains(&q))
                    .count()
            }
            PaletteMode::QuicklinkArguments => self
                .argument_session
                .as_ref()
                .map(|s| s.filtered_options(&self.palette.query).len())
                .unwrap_or(0),
            PaletteMode::Launcher => selectable_count(
                self.calc_result().is_some() || self.meeting_card().is_some(),
                selectable_rows(&self.sections()).len(),
            ),
            _ => selectable_rows(&self.sections()).len(),
        }
    }

    fn calc_history_paint_items(&self) -> Vec<PaintItem> {
        let card = self.calc_result();
        let mut items = Vec::new();
        let mut index = 0usize;
        if let Some(result) = &card {
            items.push(card::paint_item(result, self.palette.selection == 0));
            index = 1;
        }
        for entry in self.calc_history.search(&self.palette.query) {
            items.push(PaintItem::Row {
                title: entry.expression,
                alias: None,
                trailing: entry.result,
                keycap: None,
                icon_source: None,
                selected: self.palette.selection == index,
            });
            index += 1;
        }
        items
    }

    fn primary_label(&self) -> &'static str {
        if let Some(result) = self.calc_result() {
            if self.palette.selection == 0 {
                return if card::is_actionable(&result) {
                    "Copy Answer"
                } else {
                    "Open"
                };
            }
        }
        if self.palette.mode == PaletteMode::CalculatorHistory {
            return "Copy Answer";
        }
        if self.palette.mode == PaletteMode::Clipboard {
            return "Paste";
        }
        if self.palette.mode == PaletteMode::FileSearch {
            return "Open";
        }
        if self.palette.mode == PaletteMode::QuicklinkArguments {
            return "Continue";
        }
        self.selected_entry()
            .map(|e| e.kind.open_verb())
            .unwrap_or("Open")
    }

    fn hop_tab(&mut self) {
        if !self.palette_visible {
            return;
        }
        let hop = tab_from(
            self.palette.mode,
            self.settings.ai_enabled,
            self.palette.mode == PaletteMode::QuicklinkArguments,
        );
        let query = self.palette.query.clone();
        match hop {
            TabHop::StayForArguments => return,
            TabHop::Clipboard => {
                if !self.previous_hwnd.is_invalid() {
                    // keep existing previous app
                }
                self.palette.mode = PaletteMode::Clipboard;
            }
            TabHop::Launcher => {
                self.palette.mode = PaletteMode::Launcher;
            }
            TabHop::Ai => {
                if !self.settings.ai_enabled {
                    return;
                }
                self.palette.mode = PaletteMode::Ai;
            }
        }
        self.palette.query = query;
        self.palette.selection = 0;
        self.list_scroll = 0.0;
        self.expanded = true;
        self.close_menu();
        self.relayout_palette();
        self.invalidate_palette();
    }

    fn remember_previous_hwnd(&mut self) {
        let fg = unsafe { GetForegroundWindow() };
        if !fg.is_invalid() {
            self.previous_hwnd = fg;
        }
    }

    fn action_target(&self) -> HWND {
        let fg = unsafe { GetForegroundWindow() };
        window_coordinator::resolve_action_target(
            self.palette_visible,
            self.previous_hwnd,
            fg,
            window_coordinator::hwnd_is_ours(fg),
        )
    }

    fn open_clipboard_history(&mut self) {
        self.close_menu();
        let was_visible = self.palette_visible;
        if !was_visible {
            self.remember_previous_hwnd();
        }
        self.palette.prepare(PaletteMode::Clipboard);
        self.palette_visible = true;
        self.expanded = true;
        self.list_scroll = 0.0;
        if was_visible {
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.relayout_palette();
            self.invalidate_palette();
        } else {
            self.show_palette_window();
            self.expand_palette();
        }
    }

    fn clipboard_rows(&self) -> Vec<crate::features::clipboard::service::store::ClipboardItem> {
        self.clipboard
            .search(&self.palette.query, self.clipboard_filter)
    }

    fn paste_clipboard_at(&mut self, index: usize) {
        let Some(item) = self.clipboard_rows().get(index).cloned() else {
            return;
        };
        let previous = self.previous_hwnd;
        let wrote = match item.kind {
            crate::features::clipboard::service::store::ClipKind::Text => item
                .text
                .as_deref()
                .map(|text| crate::platform::clipboard::write_text_marked(text).is_ok())
                .unwrap_or(false),
            crate::features::clipboard::service::store::ClipKind::Image => item
                .image_path
                .as_ref()
                .and_then(|path| std::fs::read(path).ok())
                .map(|png| crate::platform::clipboard::write_png_marked(&png).is_ok())
                .unwrap_or(false),
        };
        if !wrote {
            return;
        }
        if !item.is_pinned() {
            self.clipboard.promote(&item.id);
        }
        self.hide_palette();
        crate::platform::paster::paste_into(previous);
    }

    fn toggle_clipboard_pin(&mut self) {
        let Some(item) = self.clipboard_rows().get(self.palette.selection).cloned() else {
            return;
        };
        self.clipboard.toggle_pin(&item.id);
        if let Some(idx) =
            self.clipboard
                .row_index(&item.id, &self.palette.query, self.clipboard_filter)
        {
            self.palette.selection = idx;
            self.ensure_selection_visible();
        } else {
            self.clamp_selection();
        }
        self.invalidate_palette();
    }

    fn activate_clipboard_pin_slot(&mut self, n: u8) {
        let index = if n == 0 {
            9usize
        } else {
            (n as usize).saturating_sub(1)
        };
        let Some(item) =
            self.clipboard
                .pinned_item(index, &self.palette.query, self.clipboard_filter)
        else {
            return;
        };
        let rows = self.clipboard_rows();
        if let Some(pos) = rows.iter().position(|r| r.id == item.id) {
            self.paste_clipboard_at(pos);
        }
    }

    fn toggle_clipboard_filter_menu(&mut self) {
        if self.menu == OpenMenu::ClipboardFilter {
            self.close_menu();
            return;
        }
        self.menu_header = clip_screen::filter_title(self.clipboard_filter).to_string();
        self.menu_items = vec![
            tinycast_pure::palette_menu::MenuItem {
                id: "clip-filter-all",
                label: "All Types",
                shortcut: None,
            },
            tinycast_pure::palette_menu::MenuItem {
                id: "clip-filter-text",
                label: "Text Only",
                shortcut: None,
            },
            tinycast_pure::palette_menu::MenuItem {
                id: "clip-filter-images",
                label: "Images Only",
                shortcut: None,
            },
            tinycast_pure::palette_menu::MenuItem {
                id: "clip-filter-links",
                label: "Links Only",
                shortcut: None,
            },
            tinycast_pure::palette_menu::MenuItem {
                id: "clip-filter-emails",
                label: "Emails Only",
                shortcut: None,
            },
        ];
        self.menu_selection = match self.clipboard_filter {
            ClipboardFilter::All => 0,
            ClipboardFilter::Text => 1,
            ClipboardFilter::Images => 2,
            ClipboardFilter::Links => 3,
            ClipboardFilter::Emails => 4,
        };
        self.menu = OpenMenu::ClipboardFilter;
        self.invalidate_palette();
    }

    fn open_calculator_history(&mut self) {
        self.close_menu();
        let was_visible = self.palette_visible;
        self.palette.prepare(PaletteMode::CalculatorHistory);
        self.palette_visible = true;
        self.expanded = true;
        self.list_scroll = 0.0;
        if was_visible {
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.relayout_palette();
            self.invalidate_palette();
        } else {
            self.show_palette_window();
            self.expand_palette();
        }
    }

    fn invalidate_settings(&self) {
        if let Some(window) = &self.settings_window {
            window.invalidate();
        }
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
        let n = self.selectable_len();
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

    fn open_emoji(&mut self) {
        self.close_menu();
        let was_visible = self.palette_visible;
        if !was_visible {
            self.remember_previous_hwnd();
        }
        self.palette.prepare(PaletteMode::Emoji);
        self.palette_visible = true;
        self.expanded = true;
        self.list_scroll = 0.0;
        if was_visible {
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.relayout_palette();
            self.invalidate_palette();
        } else {
            self.show_palette_window();
            self.expand_palette();
        }
    }

    fn open_quicklinks_search(&mut self) {
        if !self.settings.quicklinks_enabled {
            return;
        }
        self.close_menu();
        let was_visible = self.palette_visible;
        if !was_visible {
            self.remember_previous_hwnd();
        }
        self.palette.prepare(PaletteMode::Quicklinks);
        self.palette_visible = true;
        self.expanded = true;
        self.list_scroll = 0.0;
        if was_visible {
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.relayout_palette();
            self.invalidate_palette();
        } else {
            self.show_palette_window();
            self.expand_palette();
        }
    }

    fn open_quicklink(&mut self, entry_id: &str) {
        if !self.settings.quicklinks_enabled {
            return;
        }
        let Some(id) = tinycast_pure::quicklink::id_from_entry(entry_id) else {
            return;
        };
        let Some(link) = self.quicklinks.get(id).cloned() else {
            return;
        };
        let ctx = snippet_coordinator::expansion_context(
            self.clipboard.recent_text(20),
            injector::capture_selection(self.previous_hwnd, !self.palette_visible),
            crate::platform::clock::local_naive_unix(),
            user_locale(),
        );
        let output = tinycast_pure::quicklink::expand_destination(
            &link.destination,
            &ctx,
            &Default::default(),
        );
        if output.arguments.is_empty() {
            self.hide_palette();
            if let Err(err) = quicklink_coordinator::open_destination(&output.text) {
                self.show_message_hud(&err);
            }
            return;
        }
        self.argument_session = Some(snippet_coordinator::ArgumentSession {
            kind: snippet_coordinator::ArgumentKind::Quicklink,
            snippet_path: link.id,
            snippet_name: link.name,
            show_confirmation: false,
            specs: output.arguments,
            values: std::collections::HashMap::new(),
            index: 0,
            clipboard: ctx.clipboard,
            selection: ctx.selection,
            now: ctx.now,
            locale: ctx.locale,
            tz: ctx.tz,
            keyword: None,
            target: 0,
        });
        self.palette.prepare(PaletteMode::QuicklinkArguments);
        self.palette_visible = true;
        self.expanded = true;
        if let Some(window) = &self.palette_window {
            window.reset_search();
        }
        self.relayout_palette();
        self.invalidate_palette();
    }

    fn run_custom_command(&mut self, entry_id: &str) {
        if !self.settings.custom_commands_enabled {
            return;
        }
        let Some(id) = tinycast_pure::custom_command::CustomCommand::id_from_entry(entry_id)
        else {
            return;
        };
        let Some(command) = self.custom_commands.get(id).cloned() else {
            return;
        };
        self.hide_palette();
        match custom_coordinator::request_run(&command) {
            custom_coordinator::RunRequest::Confirm => {
                if !custom_coordinator::confirm(&command) {
                    return;
                }
            }
            custom_coordinator::RunRequest::Execute => {}
        }
        custom_coordinator::start(&command, self.host);
    }

    fn apply_snippets_enabled(&mut self) {
        if self.settings.snippets_enabled {
            if let Ok(snap) = self.snippet_repo.load() {
                self.snippet_records = snap.records;
            }
            if !self.host.is_invalid() {
                self.snippet_repo.start_watch(self.host);
            }
            let keywords: Vec<String> = self
                .snippet_records
                .iter()
                .filter(|r| r.enabled)
                .filter_map(|r| r.keyword.clone())
                .collect();
            self.snippet_listener.start(self.host, keywords);
        } else {
            self.snippet_listener.stop();
            self.snippet_repo.stop_watch();
            self.snippet_records.clear();
        }
        self.invalidate_palette();
    }

    fn begin_snippet_expansion(&mut self, entry_id: &str) {
        let Some(path) = snippet_coordinator::path_from_entry_id(entry_id) else {
            return;
        };
        let Some(record) = self
            .snippet_records
            .iter()
            .find(|r| r.path.to_string_lossy() == path)
            .cloned()
        else {
            return;
        };
        let ctx = snippet_coordinator::expansion_context(
            self.clipboard.recent_text(20),
            snippet_coordinator::capture_if_uses_selection(&record.body, || {
                injector::capture_selection(self.previous_hwnd, !self.palette_visible)
            }),
            crate::platform::clock::local_naive_unix(),
            user_locale(),
        );
        let output = snippet_coordinator::expand_record(
            &record,
            &self.snippet_records,
            &ctx,
            &Default::default(),
        );
        if output.arguments.is_empty() {
            self.deliver_snippet(&record, output);
            return;
        }
        self.argument_session = Some(snippet_coordinator::ArgumentSession {
            kind: snippet_coordinator::ArgumentKind::Snippet,
            snippet_path: path.to_string(),
            snippet_name: record.name.clone(),
            show_confirmation: record.show_confirmation,
            specs: output.arguments,
            values: std::collections::HashMap::new(),
            index: 0,
            clipboard: ctx.clipboard,
            selection: ctx.selection,
            now: ctx.now,
            locale: ctx.locale,
            tz: ctx.tz,
            keyword: None,
            target: 0,
        });
        self.palette.prepare(PaletteMode::QuicklinkArguments);
        self.palette_visible = true;
        self.expanded = true;
        if let Some(window) = &self.palette_window {
            window.reset_search();
        }
        self.relayout_palette();
        self.invalidate_palette();
    }

    fn commit_argument(&mut self) {
        let Some(session) = self.argument_session.as_mut() else {
            return;
        };
        let options = session.filtered_options(&self.palette.query);
        let value = if !options.is_empty() {
            options
                .get(self.palette.selection)
                .cloned()
                .unwrap_or_else(|| self.palette.query.clone())
        } else {
            self.palette.query.clone()
        };
        if session.current_options().is_empty() && value.is_empty() {
            return;
        }
        let done = session.commit(value);
        if done {
            self.finish_argument_session();
        } else {
            self.palette.query.clear();
            self.palette.selection = 0;
            if let Some(window) = &self.palette_window {
                window.reset_search();
            }
            self.invalidate_palette();
        }
    }

    fn step_back_argument(&mut self) {
        let Some(session) = self.argument_session.as_mut() else {
            return;
        };
        match session.back() {
            None => self.hide_palette(),
            Some(prev) => {
                self.palette.query = prev.clone();
                self.palette.selection = 0;
                if let Some(window) = &self.palette_window {
                    window.set_search_text(&prev);
                }
                self.invalidate_palette();
            }
        }
    }

    fn finish_argument_session(&mut self) {
        let Some(session) = self.argument_session.take() else {
            return;
        };
        if session.kind == snippet_coordinator::ArgumentKind::Quicklink {
            let Some(link) = self.quicklinks.get(&session.snippet_path).cloned() else {
                self.hide_palette();
                return;
            };
            let ctx = snippet_coordinator::expansion_context(
                session.clipboard,
                session.selection,
                session.now,
                session.locale,
            );
            let output = tinycast_pure::quicklink::expand_destination(
                &link.destination,
                &ctx,
                &session.values,
            );
            self.hide_palette();
            if let Err(err) = quicklink_coordinator::open_destination(&output.text) {
                self.show_message_hud(&err);
            }
            return;
        }
        let Some(record) = self
            .snippet_records
            .iter()
            .find(|r| r.path.to_string_lossy() == session.snippet_path)
            .cloned()
        else {
            self.hide_palette();
            return;
        };
        let ctx = snippet_coordinator::expansion_context(
            session.clipboard,
            session.selection,
            session.now,
            session.locale,
        );
        let output =
            snippet_coordinator::expand_record(&record, &self.snippet_records, &ctx, &session.values);
        let _ = session.tz;
        if let Some(keyword) = session.keyword {
            let target = HWND(session.target as *mut core::ffi::c_void);
            self.hide_palette();
            if injector::deliver_keyword(target, &keyword, &output.text, output.cursor) {
                if session.show_confirmation {
                    self.show_message_hud(&session.snippet_name);
                }
            }
            return;
        }
        self.deliver_snippet_with(
            &session.snippet_name,
            session.show_confirmation,
            output,
        );
    }

    fn deliver_snippet(
        &mut self,
        record: &tinycast_pure::snippet::StoredSnippet,
        output: tinycast_pure::template::ExpandOutput,
    ) {
        self.deliver_snippet_with(&record.name, record.show_confirmation, output);
    }

    fn deliver_snippet_with(
        &mut self,
        name: &str,
        show_hud: bool,
        output: tinycast_pure::template::ExpandOutput,
    ) {
        let previous = self.previous_hwnd;
        if injector::insertion_refused(previous) {
            return;
        }
        self.hide_palette();
        injector::inject_into(previous, &output.text, output.cursor);
        if show_hud {
            self.show_message_hud(name);
        }
    }

    fn show_message_hud(&mut self, message: &str) {
        if self.hud.is_none() && !self.host.is_invalid() {
            self.hud = MessageHud::create(self.host).ok();
        }
        if let Some(hud) = &self.hud {
            hud.show(message);
        }
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

fn ctrl_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 }
}

fn shift_down() -> bool {
    unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 }
}

fn alt_down() -> bool {
    unsafe { GetKeyState(VK_MENU.0 as i32) < 0 }
}

fn user_locale() -> String {
    let mut buf = [0u16; 85];
    let n = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buf) };
    if n > 1 {
        String::from_utf16_lossy(&buf[..n as usize - 1])
    } else {
        "en".into()
    }
}

fn new_item_id() -> String {
    match unsafe { windows::Win32::System::Com::CoCreateGuid() } {
        Ok(g) => format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            g.data1,
            g.data2,
            g.data3,
            g.data4[0],
            g.data4[1],
            g.data4[2],
            g.data4[3],
            g.data4[4],
            g.data4[5],
            g.data4[6],
            g.data4[7]
        ),
        Err(_) => format!("{}", unix_now()),
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
    fn perform_hotkey_dispatches_command_id_actions() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.perform_hotkey("hotkey.searchFiles");
        assert!(!c.palette_visible);
        c.settings.file_search_enabled = true;
        c.perform_hotkey("hotkey.searchFiles");
        assert!(c.palette_visible);
        assert_eq!(c.palette.mode, PaletteMode::FileSearch);
        c.perform_hotkey("hotkey.showNotes");
        assert!(c.notes_window.is_none());
        c.settings.notes_enabled = true;
        c.perform_hotkey("hotkey.toggleClipboard");
        assert!(c.palette_visible);
        assert_eq!(c.palette.mode, PaletteMode::Clipboard);
        c.perform_hotkey("hotkey.toggleEmoji");
        assert_eq!(c.palette.mode, PaletteMode::Emoji);
    }

    #[test]
    fn perform_hotkey_command_id_respects_kind_off() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.visibility.set_kind_enabled(AppKind::Command, false);
        c.perform_hotkey("hotkey.toggleEmoji");
        assert!(!c.palette_visible);
        assert_ne!(c.palette.mode, PaletteMode::Emoji);
        c.visibility.set_kind_enabled(AppKind::Command, true);
        c.perform_hotkey("hotkey.toggleEmoji");
        assert!(c.palette_visible);
        assert_eq!(c.palette.mode, PaletteMode::Emoji);
    }

    #[test]
    fn cleared_toggle_palette_stays_unbound_across_pause_resume() {
        let mut c = AppCore::new();
        c.hotkeys.set("hotkey.togglePalette".into(), None);
        c.pause_global_hotkeys();
        c.resume_global_hotkeys();
        assert!(c.hotkeys.get("hotkey.togglePalette").is_none());
        assert!(tinycast_pure::hotkey::registered_combos(&c.hotkeys.snapshot())
            .iter()
            .all(|(action, _)| action != "hotkey.togglePalette"));
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
    fn tab_ring_keeps_query_between_launcher_and_clipboard() {
        let mut c = AppCore::new();
        c.toggle_palette();
        c.set_query("hello".into());
        c.handle_key(0x09);
        assert_eq!(c.palette.mode, PaletteMode::Clipboard);
        assert_eq!(c.palette.query, "hello");
        c.handle_key(0x09);
        assert_eq!(c.palette.mode, PaletteMode::Launcher);
        assert_eq!(c.palette.query, "hello");
        assert_eq!(c.tab_hint(), Some("Clipboard"));
        assert_ne!(c.tab_hint(), Some("AI Chat"));
    }

    #[test]
    fn calculator_card_occupies_selection_zero() {
        let mut c = AppCore::new();
        c.toggle_palette();
        c.set_query("2+2".into());
        assert_eq!(c.palette.selection, 0);
        let items = c.launcher_paint_items();
        assert!(matches!(
            &items[0],
            PaintItem::Calc {
                selected: true,
                display,
                ..
            } if display == "4"
        ));
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
    fn snippet_rows_appear_when_enabled_and_shown() {
        use tinycast_pure::snippet::{SnippetSourceRevision, StoredSnippet};
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.settings.snippets_enabled = true;
        c.settings.snippets_show_in_launcher = true;
        c.snippet_records = vec![StoredSnippet {
            path: std::path::PathBuf::from("Meeting Notes.md"),
            name: "Meeting Notes".into(),
            keyword: Some("!notes".into()),
            enabled: true,
            show_confirmation: false,
            body: "Hi".into(),
            source_revision: SnippetSourceRevision::new(""),
        }];
        c.toggle_palette();
        c.set_query("!notes".into());
        let items = c.launcher_paint_items();
        assert!(items.iter().any(|item| matches!(
            item,
            crate::features::launcher::ui::list::PaintItem::Row { title, .. } if title == "Meeting Notes"
        )));
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
        assert!(c
            .settings_entries(AppKind::Application)
            .iter()
            .any(|e| e.name == "Notepad"));
    }

    #[test]
    fn showing_palette_starts_a_fresh_index_scan() {
        let mut c = AppCore::new();
        assert_eq!(c.app_index.generation(), 0);
        c.toggle_palette();
        assert!(c.palette_visible);
        assert_eq!(c.app_index.generation(), 1);
        c.toggle_palette();
        assert!(!c.palette_visible);
        assert_eq!(c.app_index.generation(), 1);
        c.toggle_palette();
        assert_eq!(c.app_index.generation(), 2);
    }

    fn application_named(id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            kind: AppKind::Application,
            name: name.into(),
            fields: tinycast_pure::search_relevance::SearchFields {
                display_name: name.into(),
                ..Default::default()
            },
            hotkey: None,
        }
    }

    struct RestoreFile {
        path: std::path::PathBuf,
        previous: Option<Vec<u8>>,
    }

    impl Drop for RestoreFile {
        fn drop(&mut self) {
            match &self.previous {
                Some(bytes) => {
                    let _ = std::fs::write(&self.path, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(&self.path);
                }
            }
        }
    }

    #[test]
    fn add_to_favorites_on_empty_query_keeps_selection_on_toggled_row() {
        let _restore = RestoreFile {
            path: store_path("favorites.json"),
            previous: std::fs::read(store_path("favorites.json")).ok(),
        };
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.entries = vec![
            application_named("app:alpha", "Alpha"),
            application_named("app:beta", "Beta"),
        ];
        c.favorites.toggle("app:alpha".into());
        c.toggle_palette();
        c.expand_select_first();
        let beta = selectable_rows(&c.sections())
            .iter()
            .position(|row| row.id == "app:beta")
            .expect("beta in applications");
        assert!(beta > 0, "alpha is already pinned at slot 1");
        c.palette.selection = beta;
        assert_eq!(c.selected_entry().unwrap().id, "app:beta");
        c.toggle_favorite_selected();
        assert_eq!(c.favorites.ids, ["app:alpha", "app:beta"]);
        assert_eq!(c.selected_entry().unwrap().id, "app:beta");
        assert_eq!(c.palette.selection, 1);
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

    #[test]
    fn escape_closes_actions_menu_before_hiding() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.toggle_palette();
        c.expand_select_first();
        assert!(!c.menu_is_open());
        c.toggle_actions();
        assert!(c.menu_is_open());
        c.handle_escape();
        assert!(!c.menu_is_open());
        assert!(c.palette_visible);
        c.handle_escape();
        assert!(!c.palette_visible);
    }

    #[test]
    fn ctrl_k_swallowed_in_compact_bar() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.toggle_palette();
        assert!(!c.expanded);
        c.toggle_actions();
        assert!(!c.menu_is_open());
    }

    #[test]
    fn toggle_actions_is_one_menu_and_frozen_query() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.toggle_palette();
        c.expand_select_first();
        c.toggle_actions();
        assert!(c.menu_is_open());
        let before = c.palette.query.clone();
        c.set_query("should-not-apply".into());
        c.append_query_char('x');
        assert_eq!(c.palette.query, before);
        c.toggle_actions();
        assert!(!c.menu_is_open());
        let paint = c.menu_paint();
        assert!(paint.is_none());
    }

    #[test]
    fn feature_off_commands_are_absent_from_launcher_and_present_in_settings() {
        use crate::features::launcher::ui::list::PaintItem;
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.settings = AppSettings::default();
        c.toggle_palette();
        let items = c.launcher_paint_items();
        assert!(!items.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "AI Chat"
        )));
        assert!(!items.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Search Files"
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Quit Tinycast"
        )));
        let listed = c.settings_entries(AppKind::Command);
        assert_eq!(listed.len(), CommandID::all().len());
        assert!(listed.iter().any(|e| e.name == "AI Chat"));
        assert!(listed.iter().any(|e| e.name == "Search Files"));
    }

    #[test]
    fn confirm_custom_command_gate_returns_false_when_a_dialog_is_up() {
        use crate::features::custom_commands::ui::coordinator::{
            confirm, confirm_prompt, request_run, RunRequest,
        };
        let command = tinycast_pure::custom_command::CustomCommand {
            id: "1".into(),
            name: "Format Disk".into(),
            command: "format C:".into(),
            confirm: true,
        };
        assert!(matches!(request_run(&command), RunRequest::Confirm));
        let prompt = confirm_prompt(&command);
        assert_eq!(prompt.title, "Format Disk");
        assert_eq!(prompt.message, "format C:");
        assert_eq!(prompt.accept, "Continue");
        assert_eq!(prompt.cancel, "Cancel");
        assert!(crate::surfaces::dialog::begin());
        assert!(
            !confirm(&command),
            "Cancel / stacked dialog must not run the command"
        );
        crate::surfaces::dialog::end();
    }

    #[test]
    fn custom_commands_stay_hidden_until_enabled() {
        let path = std::env::temp_dir().join(format!(
            "tinycast-cc-core-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        let mut store = CustomCommandStore::load_from(path.clone());
        store
            .upsert(tinycast_pure::custom_command::CustomCommand {
                id: "1".into(),
                name: "Format Disk".into(),
                command: "echo".into(),
                confirm: true,
            })
            .unwrap();
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.custom_commands = store;
        c.settings.custom_commands_enabled = false;
        c.toggle_palette();
        let hidden = c.launcher_paint_items();
        assert!(!hidden.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Format Disk"
        )));
        c.settings.custom_commands_enabled = true;
        c.settings.custom_commands_show_in_launcher = true;
        let shown = c.launcher_paint_items();
        assert!(shown.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Format Disk"
        )));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn create_import_export_quicklink_are_not_noop() {
        assert_eq!(
            launch_spec(&CommandID::CreateQuicklink.as_entry()),
            LaunchSpec::CreateQuicklink
        );
        assert_eq!(
            launch_spec(&CommandID::ImportQuicklinks.as_entry()),
            LaunchSpec::ImportQuicklinks
        );
        assert_eq!(
            launch_spec(&CommandID::ExportQuicklinks.as_entry()),
            LaunchSpec::ExportQuicklinks
        );
        assert_ne!(
            launch_spec(&CommandID::CreateQuicklink.as_entry()),
            LaunchSpec::Noop
        );
    }

    #[test]
    fn system_actions_publish_on() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.favorites = tinycast_pure::favorites::FavoritesStore::default();
        c.aliases = tinycast_pure::alias::AliasStore::default();
        c.toggle_palette();
        c.expand_select_first();
        let items = c.launcher_paint_items();
        assert!(items.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Lock Screen"
        )));
        assert!(items.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Toggle Stage Manager"
        )));
        let listed = c.settings_entries(AppKind::SystemAction);
        assert_eq!(
            listed.len(),
            tinycast_pure::system_action::SystemActionId::all().len()
        );
        assert_eq!(
            launch_spec(&tinycast_pure::system_action::SystemActionId::LockScreen.as_entry()),
            LaunchSpec::RunSystemAction("system-action:lock-screen".into())
        );
    }

    #[test]
    fn window_commands_hidden_until_enabled() {
        let mut c = AppCore::new();
        c.visibility = tinycast_pure::visibility::VisibilityStore::default();
        c.toggle_palette();
        c.expand_select_first();
        let hidden = c.launcher_paint_items();
        assert!(!hidden.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Left Half"
        )));
        c.settings.window_management_enabled = true;
        c.settings.window_management_show_in_launcher = true;
        let shown = c.launcher_paint_items();
        assert!(shown.iter().any(|item| matches!(
            item,
            PaintItem::Row { title, .. } if title == "Left Half"
        )));
        assert_eq!(
            launch_spec(
                &tinycast_pure::window_command::WindowCommandId::LeftHalf.as_entry()
            ),
            LaunchSpec::RunWindowCommand("window-command:left-half".into())
        );
    }
}
