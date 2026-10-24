//! The Slint settings window: creation, the Win32 bits winit cannot express,
//! and the bridge that pushes AppCore's settings state into the surface.
//!
//! winit only creates its HWND once the event loop runs, so `create` is called
//! early from `platform::run` and finishes the setup on a short timer, exactly
//! like the palette host. Until then `show` and `invalidate` are remembered and
//! applied once the window is ready.
//!
//! Closing hides the window (Slint's default close response); the component is
//! kept alive by `SettingsWindow`, so opening Settings again shows the same
//! window with its current tab and scroll positions.
//!
//! Phase 7 fills General, Permissions, About and Backup. The other 15 tabs show
//! a neutral placeholder page; phase 8 replaces those placeholders and brings
//! the command/quicklink/directory editors back.

use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use tinycast_pure::hotkey::{CaptureOutcome, Modifiers};
use tinycast_pure::i18n::{
    about_made_with_slint, about_product, about_slint_license, about_slint_link, about_support,
    about_window_title, backup_export, backup_import, backup_raycast, backup_storage_note, chrome,
    general_appearance_trailing, general_language_trailing, general_pop_to_root_subtitle,
    general_pop_to_root_trailing, general_ranking_subtitle, general_reset_label,
    general_row_subtitle, general_row_title, general_section_footer, general_section_title,
    hyper_key_title, launcher_reset_confirm_message, launcher_reset_confirm_title,
    open_settings_action, permission_subtitle, permission_title, reset_ranking_action,
    settings_placeholder_note, settings_section_title, settings_tab_title, support_built_with_love,
    support_keeps_independent, Chrome, GeneralSection, UiLang,
};
use tinycast_pure::layout::settings::{sidebar_content_height, sidebar_rows, SidebarRowKind};
use tinycast_pure::settings_tab::{SettingsSection, SettingsTab};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsWindow, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOP, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, WM_KEYDOWN, WM_KEYUP,
    WM_NCDESTROY, WM_SYSKEYDOWN, WM_SYSKEYUP, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use crate::ui_host::app_core as core_from_host;

pub(super) type Settings = tinycast_ui::SettingsWindow;
pub(super) type PageRow = tinycast_ui::SettingsPageRow;
pub(super) type PageSection = tinycast_ui::SettingsPageSection;
type SidebarRow = tinycast_ui::SettingsSidebarRow;
type ItemRow = tinycast_ui::LauncherItemRow;

const SUBCLASS_ID: usize = 0x53;
/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's `skip_taskbar` leaves this bit set, which would hide the settings
/// window from the taskbar; the styles are corrected once the HWND exists.
const WS_EX_APPWINDOW_BIT: isize = WS_EX_APPWINDOW.0 as isize;
/// The one action the General page records in phase 7 (stage 8 adds the rest).
const RECORDER_ACTION: &str = "hotkey.togglePalette";

pub struct SettingsWindow {
    ui: Settings,
    inner: *mut Inner,
}

pub(super) struct Inner {
    pub(super) host: HWND,
    /// Upgradeable handle for the init timer; does not keep the window alive.
    pub(super) weak: slint::Weak<Settings>,
    /// The 23 sidebar rows (4 section headers and 19 tabs); the model is
    /// created once and updated in place so focus survives a refresh.
    sidebar: Rc<VecModel<SidebarRow>>,
    /// The titled row groups of the section pages. Each group's row models
    /// keep their identity across refreshes, so the delegates (and keyboard
    /// focus) are never recreated.
    general: SectionGroup,
    permissions: SectionGroup,
    backup: SectionGroup,
    about: Rc<VecModel<PageRow>>,
    /// The launcher pages (Applications, System Settings, System Actions,
    /// Commands): the sliced item table, the intro cards and the filter.
    pub(super) launcher: Rc<VecModel<ItemRow>>,
    pub(super) launcher_intro: SectionGroup,
    pub(super) filter_query: String,
    /// The filtered item ids and their hotkey actions for the current tab;
    /// the dispatch maps a slice row's index through these.
    pub(super) launcher_ids: Vec<String>,
    pub(super) launcher_actions: Vec<Option<String>>,
    /// The alias draft: which entry id it belongs to and its current text.
    pub(super) alias_target: Option<String>,
    pub(super) alias_draft: String,
    /// The feature rows page (Quicklinks, the ten feature tabs and AI).
    pub(super) feature: SectionGroup,
    /// Where the listening well sits in window coordinates; the recorder
    /// callout floats there until the capture ends.
    pub(super) recorder_at: Option<(f32, f32)>,
    pub(super) recorder_keys: Rc<VecModel<slint::SharedString>>,
    /// The AI connection editor: which connection is open and its field
    /// drafts. The drafts live here so a refresh never loses them.
    pub(super) ai_editing: Option<usize>,
    pub(super) ai_url: String,
    pub(super) ai_model: String,
    pub(super) ai_key: String,
    pub(super) ai_error: Option<String>,
    /// The local shortcut recorder. It owns only the action name while a well
    /// is listening; the capture itself runs from the HWND subclass.
    pub(super) recorder: crate::features::hotkeys::ui::recorder::Recorder,
    ready: bool,
    /// A show request that arrived before the HWND existed.
    pending_show: bool,
    /// The first show centres the window; later shows keep where the user left it.
    placed: bool,
    /// Focus lands on the selected tab only when the window becomes visible.
    visible: bool,
    /// The real Slint HWND once `ready`; the file dialogs use it as owner.
    hwnd: HWND,
    appearance: u8,
    /// The locale the sidebar labels were built for.
    lang: Option<UiLang>,
}

impl SettingsWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        let ui = Settings::new().map_err(crate::ui_host::platform_error)?;
        let appearance = core_from_host(host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or(1);
        tinycast_ui::apply_appearance(&ui, appearance);
        let inner = Box::into_raw(Box::new(Inner {
            host,
            weak: ui.as_weak(),
            sidebar: Rc::new(VecModel::default()),
            general: SectionGroup::new(5),
            permissions: SectionGroup::new(1),
            backup: SectionGroup::new(1),
            about: Rc::new(VecModel::default()),
            launcher: Rc::new(VecModel::default()),
            launcher_intro: SectionGroup::new(0),
            filter_query: String::new(),
            launcher_ids: Vec::new(),
            launcher_actions: Vec::new(),
            alias_target: None,
            alias_draft: String::new(),
            recorder_at: None,
            recorder_keys: Rc::new(VecModel::default()),
            feature: SectionGroup::new(0),
            ai_editing: None,
            ai_url: String::new(),
            ai_model: String::new(),
            ai_key: String::new(),
            ai_error: None,
            recorder: crate::features::hotkeys::ui::recorder::Recorder::new(),
            ready: false,
            pending_show: false,
            placed: false,
            visible: false,
            hwnd: HWND::default(),
            appearance,
            lang: None,
        }));
        let window = Self { ui, inner };
        window
            .ui
            .set_sidebar_rows(ModelRc::from(unsafe { (*inner).sidebar.clone() }));
        window
            .ui
            .set_general_sections(ModelRc::from(unsafe { (*inner).general.sections.clone() }));
        window.ui.set_permissions_sections(ModelRc::from(unsafe {
            (*inner).permissions.sections.clone()
        }));
        window
            .ui
            .set_backup_sections(ModelRc::from(unsafe { (*inner).backup.sections.clone() }));
        window
            .ui
            .set_about_rows(ModelRc::from(unsafe { (*inner).about.clone() }));
        window
            .ui
            .set_launcher_items(ModelRc::from(unsafe { (*inner).launcher.clone() }));
        window.ui.set_launcher_intro(ModelRc::from(unsafe {
            (*inner).launcher_intro.sections.clone()
        }));
        window
            .ui
            .set_feature_sections(ModelRc::from(unsafe { (*inner).feature.sections.clone() }));
        window
            .ui
            .set_recorder_keys(ModelRc::from(unsafe { (*inner).recorder_keys.clone() }));
        window
            .ui
            .set_sidebar_content_height(sidebar_content_height());
        window.register_callbacks(inner);
        // Closing hides the surface; the component and its state stay alive.
        let ptr = inner as usize;
        window.ui.window().on_close_requested(move || {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                close_requested(inner);
            }
            slint::CloseRequestResponse::HideWindow
        });
        // No HWND exists until the event loop runs; park the window off-screen.
        window.ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        window.ui.show().map_err(crate::ui_host::platform_error)?;
        arm_init(inner);
        Ok(window)
    }

    fn register_callbacks(&self, inner: *mut Inner) {
        let ptr = inner as usize;
        self.ui.on_tab_selected(move |index| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                tab_selected(inner, index);
            }
        });
        let ptr = inner as usize;
        self.ui.on_general_row_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                general_row_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_permissions_row_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                permissions_row_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_backup_row_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                backup_row_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_about_row_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                about_row_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_slint_link_clicked(move || {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                open_slint_link(inner);
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_intro_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::intro_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_enable_toggled(move |on| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::enable_toggled(inner, on);
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_item_toggled(move |index| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::item_toggled(inner, index);
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_alias_activated(move |index| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::alias_activated(inner, index);
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_alias_edited(move |index, text| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::alias_edited(inner, index, text.to_string());
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_alias_committed(move |index| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::alias_committed(inner, index);
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_hotkey_clicked(move |index, x, y| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::hotkey_clicked(inner, index, x, y);
            }
        });
        let ptr = inner as usize;
        self.ui.on_general_well_revealed(move |x, y| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                unsafe {
                    (*inner).recorder_at = Some((x, y));
                }
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_filter_changed(move |text| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::filter_changed(inner, text.to_string());
            }
        });
        let ptr = inner as usize;
        self.ui.on_launcher_list_scrolled(move || {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::list_scrolled(inner);
            }
        });
        let ptr = inner as usize;
        self.ui.on_feature_row_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::feature_row_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_feature_trailing_activated(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::feature_trailing_activated(inner, id);
            }
        });
        let ptr = inner as usize;
        self.ui.on_feature_text_edited(move |id, text| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::feature_text_edited(inner, id, text.to_string());
            }
        });
        let ptr = inner as usize;
        self.ui.on_feature_text_accepted(move |id| {
            let inner = ptr as *mut Inner;
            if !inner.is_null() {
                super::settings_pages::feature_text_accepted(inner, id);
            }
        });
    }

    pub fn show(&self) {
        if !unsafe { (*self.inner).ready } {
            unsafe {
                (*self.inner).pending_show = true;
            }
            return;
        }
        show_impl(&self.ui, self.inner);
    }

    pub fn invalidate(&self) {
        refresh(&self.ui, self.inner);
    }

    pub fn set_locale(&self, locale: &str) {
        // Slint shapes with the system locale and every string is supplied by
        // the host, so there is no per-window text format to rebuild here.
        let _ = locale;
        self.invalidate();
    }

    pub fn on_tab_changed(&self) {
        // A tab change ends any shortcut capture and drops the alias draft,
        // like the Direct2D window's reset_pane_state did.
        stop_recording(self.inner, true);
        super::settings_pages::tab_changed(self.inner);
        // The AI editor never survives a tab change; commit_pending already
        // wrote the draft before AppCore moved the tab.
        refresh(&self.ui, self.inner);
        if unsafe { (*self.inner).ready } {
            reveal_selected_tab(&self.ui, self.inner);
        }
    }

    /// A tab change commits the open AI connection draft first; a failed
    /// commit keeps the tab (the error row stays visible on the page).
    pub fn commit_pending(&self) -> bool {
        super::settings_pages::commit_pending(self.inner)
    }

    /// The real HWND once the event loop has created the Slint window.
    pub fn hwnd(&self) -> HWND {
        window_hwnd(&self.ui).unwrap_or_default()
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        let hwnd = self.hwnd();
        unsafe {
            // The event loop may have destroyed the window first; only unhook a
            // live HWND.
            if !hwnd.is_invalid() && IsWindow(hwnd).as_bool() {
                let _ =
                    RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(settings_subclass), SUBCLASS_ID);
            }
            if !self.inner.is_null() {
                // Quitting: drop the capture without touching AppCore, which is
                // already being torn down by the tray's WM_DESTROY.
                if (*self.inner).recorder.is_recording() {
                    (*self.inner).recorder.cancel();
                }
                drop(Box::from_raw(self.inner));
                self.inner = std::ptr::null_mut();
            }
        }
    }
}

/// Polls for the HWND winit creates on the first event-loop turn. Re-arms
/// itself until the window is ready, then stops for good.
fn arm_init(inner: *mut Inner) {
    let ptr = inner as usize;
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let inner = ptr as *mut Inner;
        if inner.is_null() {
            return;
        }
        let Some(ui) = ui_for(inner) else {
            return;
        };
        if unsafe { (*inner).ready } {
            return;
        }
        if !init_window(&ui, inner) {
            arm_init(inner);
            return;
        }
        window_ready(&ui, inner);
    });
}

/// The live handle for this window, if the component still exists.
fn ui_for(inner: *mut Inner) -> Option<Settings> {
    unsafe { (*inner).weak.upgrade() }
}

/// One-time setup, run as soon as winit has an HWND. Returns `false` while the
/// window is still pending.
fn init_window(ui: &Settings, inner: *mut Inner) -> bool {
    let Some(hwnd) = window_hwnd(ui) else {
        return false;
    };
    apply_taskbar_window(hwnd);
    if !unsafe {
        SetWindowSubclass(
            hwnd,
            SUBCLASSPROC::Some(settings_subclass),
            SUBCLASS_ID,
            inner as usize,
        )
        .as_bool()
    } {
        return false;
    }
    unsafe {
        (*inner).hwnd = hwnd;
    }
    true
}

fn window_ready(ui: &Settings, inner: *mut Inner) {
    unsafe {
        (*inner).ready = true;
    }
    // The window was shown off-screen only so winit would create its HWND.
    let pending = unsafe { (*inner).pending_show };
    if pending {
        unsafe {
            (*inner).pending_show = false;
        }
        show_impl(ui, inner);
    } else {
        let _ = ui.hide();
    }
}

fn show_impl(ui: &Settings, inner: *mut Inner) {
    refresh(ui, inner);
    if ui.show().is_err() {
        return;
    }
    let hwnd = window_hwnd(ui).unwrap_or_default();
    if hwnd.is_invalid() {
        return;
    }
    // A minimized window comes back restored, like the Direct2D window's
    // ShowWindow(SW_RESTORE).
    let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    // Showing re-applies winit's own extended style, so the taskbar bit has to
    // be set again afterwards.
    apply_taskbar_window(hwnd);
    if !unsafe { (*inner).placed } {
        unsafe {
            (*inner).placed = true;
        }
        center(hwnd);
    }
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    // Slint's focus reveal depends on the first layout, which a window shown
    // after a hide has not necessarily done; scroll the selected tab in
    // explicitly so it is always on screen when Settings opens.
    reveal_selected_tab(ui, inner);
    let first_show = !unsafe { (*inner).visible };
    unsafe {
        (*inner).visible = true;
    }
    if first_show {
        ui.invoke_focus_selected_tab();
    }
    // The first layout after a hide/show can still clamp a scroll set in the
    // same turn, so repeat it once the window has painted.
    let ptr = inner as usize;
    slint::Timer::single_shot(Duration::from_millis(40), move || {
        let inner = ptr as *mut Inner;
        if inner.is_null() || !unsafe { (*inner).ready } {
            return;
        }
        if let Some(ui) = ui_for(inner) {
            reveal_selected_tab(&ui, inner);
        }
    });
}

/// Puts the selected sidebar tab on screen: the sidebar is 56 DIP taller than
/// the 700 DIP window, so Backup and About would otherwise open off-screen.
fn reveal_selected_tab(ui: &Settings, inner: *mut Inner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let tab = unsafe { (*core).settings_tab };
    let Some(row) = sidebar_rows()
        .into_iter()
        .find(|row| matches!(row.kind, SidebarRowKind::Tab(candidate) if candidate == tab))
    else {
        return;
    };
    // The window opens at its 700 DIP minimum, and a larger window shows even
    // more of the sidebar; using the token avoids a stale size right after a
    // show on another monitor.
    let viewport = tinycast_pure::theme::size::SETTINGS_WINDOW.1;
    let max = (sidebar_content_height() - viewport).max(0.0);
    let target =
        (row.y + row.height + tinycast_pure::theme::spacing::XL - viewport).clamp(0.0, max);
    ui.set_sidebar_scroll(target);
}

/// Pulls AppCore's settings state into the surface. The models are updated in
/// place: replacing them would recreate every row and drop keyboard focus.
fn refresh(ui: &Settings, inner: *mut Inner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let core = unsafe { &*core };
    let appearance = core.appearance_key();
    if unsafe { (*inner).appearance } != appearance {
        unsafe {
            (*inner).appearance = appearance;
        }
        tinycast_ui::apply_appearance(ui, appearance);
    }
    let lang = core.ui_lang();
    // Slint's font fallback is independent of the old DirectWrite chain.
    // Request the actual CJK UI face for Chinese instead of relying on the
    // generic fallback of Segoe UI (which has no Chinese glyphs).
    let family = match lang {
        UiLang::ZhHans => tinycast_pure::typeface::CJK[0],
        UiLang::En => tinycast_pure::typeface::LATIN_FALLBACK,
    };
    ui.global::<tinycast_ui::Theme>()
        .set_font_family(family.into());
    let tab = core.settings_tab;
    let title = settings_tab_title(tab, lang);
    ui.set_window_title(title.into());
    ui.set_selected_tab(tab as i32);
    ui.set_page_kind(page_kind(tab));
    ui.set_general_heading(settings_tab_title(SettingsTab::General, lang).into());
    ui.set_permissions_heading(settings_tab_title(SettingsTab::Permissions, lang).into());
    ui.set_backup_heading(settings_tab_title(SettingsTab::Backup, lang).into());
    ui.set_about_heading(settings_tab_title(SettingsTab::About, lang).into());
    ui.set_placeholder_heading(settings_tab_title(tab, lang).into());
    ui.set_placeholder_note(settings_placeholder_note(lang).into());
    ui.set_backup_footnote(
        backup_storage_note(
            &crate::app_settings::settings_path().display().to_string(),
            lang,
        )
        .into(),
    );

    ui.set_about_product(about_product(lang).into());
    ui.set_about_version(crate::features::settings::panes::about::version_line().into());
    ui.set_about_tagline(support_keeps_independent(lang).into());
    ui.set_about_heart("heart".into());
    ui.set_about_disclosure_title(about_made_with_slint(lang).into());
    ui.set_about_disclosure_note(about_slint_license(lang).into());
    ui.set_about_disclosure_link(about_slint_link(lang).into());
    ui.set_about_copyright(crate::features::settings::panes::about::copyright_line().into());

    if unsafe { (*inner).lang } != Some(lang) {
        unsafe {
            (*inner).lang = Some(lang);
        }
        sync_rows(unsafe { &(*inner).sidebar }, sidebar_model(lang));
    }
    // The section pages: titles, notes and rows all refresh in place.
    unsafe {
        (*inner).general.sync(general_model(&*inner, core));
        (*inner).permissions.sync(permissions_model(lang));
        (*inner).backup.sync(backup_model(lang));
    }
    sync_rows(unsafe { &(*inner).about }, about_model(lang));
    // The phase-8 pages build from AppCore per tab; the placeholder tabs only
    // carry the heading and note set above.
    let ui_tab = core.settings_tab;
    if page_kind(ui_tab) == 4 {
        super::settings_pages::sync_launcher(ui, unsafe { &mut *inner }, core);
    } else if page_kind(ui_tab) == 5 {
        super::settings_pages::sync_feature(ui, unsafe { &mut *inner }, core);
    }
    push_recorder_bubble(ui, unsafe { &*inner }, core, lang);
}

/// The callout over a listening well: the listening caption, the stored
/// binding's caps and the position the row reported on click.
fn push_recorder_bubble(
    ui: &Settings,
    inner: &Inner,
    core: &crate::app_core::AppCore,
    lang: UiLang,
) {
    let recording = inner.recorder.is_recording();
    ui.set_recorder_visible(recording && inner.recorder_at.is_some());
    if !recording {
        return;
    }
    let caption = crate::features::hotkeys::ui::recorder::well_caption_lang(None, true, lang);
    ui.set_recorder_caption(caption.into());
    let keys: Vec<slint::SharedString> = inner
        .recorder
        .action
        .as_deref()
        .and_then(|action| core.hotkeys.get(action))
        .map(|binding| {
            binding
                .label()
                .split('+')
                .map(slint::SharedString::from)
                .collect()
        })
        .unwrap_or_default();
    sync_rows(&inner.recorder_keys, keys);
    if let Some((x, y)) = inner.recorder_at {
        ui.set_recorder_x(x);
        ui.set_recorder_y(y);
    }
}

fn refresh_inner(inner: *mut Inner) {
    if let Some(ui) = ui_for(inner) {
        refresh(&ui, inner);
    }
}

/// Re-pushes the models after a dispatch changed only part of the state.
pub(super) fn invalidate_inner(inner: *mut Inner) {
    refresh_inner(inner);
}

/// The real Slint HWND once ready; the tray host is the fallback owner.
pub(super) fn settings_owner(inner: *mut Inner) -> HWND {
    let hwnd = unsafe { (*inner).hwnd };
    if hwnd.is_invalid() {
        unsafe { (*inner).host }
    } else {
        hwnd
    }
}

/// One sidebar model row: a section header or a SettingsTab. Headers take the
/// magpie label dress: Latin copy is uppercased (a no-op for Chinese).
fn sidebar_model(lang: UiLang) -> Vec<SidebarRow> {
    sidebar_rows()
        .into_iter()
        .filter_map(|row| match row.kind {
            SidebarRowKind::Header(section) => Some(SidebarRow {
                kind: 0,
                top: row.y,
                height: row.height,
                label: settings_section_title(section, lang).to_uppercase().into(),
                symbol: Default::default(),
                tab: -1,
            }),
            SidebarRowKind::Tab(tab) => Some(SidebarRow {
                kind: 1,
                top: row.y,
                height: row.height,
                label: settings_tab_title(tab, lang).into(),
                symbol: tab.system_image().into(),
                tab: tab as i32,
            }),
        })
        .collect()
}

pub(super) fn action_row(
    id: i32,
    title: &str,
    subtitle: &str,
    trailing: &str,
    control: i32,
) -> PageRow {
    PageRow {
        id,
        title: title.into(),
        subtitle: subtitle.into(),
        trailing: trailing.into(),
        control,
        checked: false,
        active: false,
        enabled: true,
        tone: 0,
        placeholder: Default::default(),
    }
}

/// A fixed set of titled groups on one page. The section structs are
/// refreshed in place while each group's row model keeps its identity, so the
/// row delegates — and their keyboard focus — survive every refresh.
pub(super) struct SectionGroup {
    pub(super) sections: Rc<VecModel<PageSection>>,
    /// One row model per group, grown on demand: a tab whose page has more
    /// groups than the last one simply takes the next models from the pool.
    rows: std::cell::RefCell<Vec<Rc<VecModel<PageRow>>>>,
}

impl SectionGroup {
    fn new(count: usize) -> Self {
        Self {
            sections: Rc::new(VecModel::default()),
            rows: std::cell::RefCell::new(
                (0..count).map(|_| Rc::new(VecModel::default())).collect(),
            ),
        }
    }

    /// One part per group: title, footer note, then the rows. A group-count
    /// change rebuilds the section delegates once; the row models keep their
    /// identity either way.
    pub(super) fn sync(&self, parts: Vec<(String, String, Vec<PageRow>)>) {
        let mut rows = self.rows.borrow_mut();
        while rows.len() < parts.len() {
            rows.push(Rc::new(VecModel::default()));
        }
        if self.sections.row_count() != parts.len() {
            let sections: Vec<PageSection> = rows
                .iter()
                .take(parts.len())
                .map(|model| PageSection {
                    title: Default::default(),
                    note: Default::default(),
                    rows: ModelRc::from(model.clone()),
                })
                .collect();
            self.sections.set_vec(sections);
        }
        for (index, model) in rows.iter().enumerate() {
            let Some((title, note, page_rows)) = parts.get(index) else {
                break;
            };
            sync_rows(model, page_rows.clone());
            self.sections.set_row_data(
                index,
                PageSection {
                    title: title.into(),
                    note: note.into(),
                    rows: ModelRc::from(model.clone()),
                },
            );
        }
    }
}

/// General: the same five groups, row order and trailing values the Direct2D
/// pane drew, plus the magpie section dress (title above the card, the two
/// footer notes the old window drew under Global Shortcuts and Search). Only
/// the widgets are Slint now.
fn general_model(
    inner: &Inner,
    core: &crate::app_core::AppCore,
) -> Vec<(String, String, Vec<PageRow>)> {
    use crate::features::settings::panes::general::{self, GeneralHit, GeneralState};

    let lang = core.ui_lang();
    let palette_binding = core
        .hotkeys
        .get(RECORDER_ACTION)
        .map(tinycast_pure::hotkey::HotKeyBinding::label);
    let state = GeneralState {
        ranking_empty: core.ranking_is_empty(),
        hyper: &core.settings.hyper_key,
        hyper_shift: core.settings.hyper_includes_shift,
        appearance: core.appearance_label(),
        compact: core.settings.compact_mode,
        favorites_in_compact: core.settings.show_favorites_in_compact,
        follow_cursor: core.settings.open_on_cursor_screen,
        draggable: core.settings.palette_draggable,
        launch_at_login: core.settings.launch_at_login,
        show_in_menu_bar: core.settings.show_in_menu_bar,
        pop_to_root: core.settings.pop_to_root_timeout,
        auto_switch: core.settings.auto_switch_input_source,
        chrome: 0,
        palette_binding: palette_binding.as_deref(),
        recording_palette: inner.recorder.action.as_deref() == Some(RECORDER_ACTION),
        lang,
    };
    let hyper_on = general::hyper_includes_shift_enabled(state.hyper);
    let ranking_sub = general_ranking_subtitle(state.ranking_empty, lang);
    let pop_sub = general_pop_to_root_subtitle(state.pop_to_root, lang);
    let pop_trail = general_pop_to_root_trailing(state.pop_to_root, lang);
    let appearance_trail = general_appearance_trailing(state.appearance, lang);
    let language_trail = general_language_trailing(lang);
    let reset_label = general_reset_label(lang);
    let hyper_title = hyper_key_title(state.hyper, lang);
    let hyper_sub = general::hyper_subtitle_lang(state.hyper, lang);
    let well = crate::features::hotkeys::ui::recorder::well_caption_lang(
        state.palette_binding,
        state.recording_palette,
        lang,
    );

    let sections = [
        GeneralSection::GlobalShortcuts,
        GeneralSection::Search,
        GeneralSection::HyperKey,
        GeneralSection::Appearance,
        GeneralSection::General,
    ];
    let mut groups = Vec::new();
    for (section, hits) in sections.into_iter().zip(general::section_rows()) {
        let title = general_section_title(section, lang).to_uppercase();
        let note = general_section_footer(section, lang)
            .unwrap_or("")
            .to_string();
        let mut rows = Vec::new();
        for &hit in hits {
            let row = general::row_for(hit);
            let title = general_row_title(row, lang);
            let static_sub = general_row_subtitle(row, lang).unwrap_or("");
            let (subtitle, control, checked, trailing, enabled) = match hit {
                GeneralHit::PaletteRecorder => {
                    (static_sub.to_string(), 3, false, well.clone(), true)
                }
                GeneralHit::ResetRanking => (
                    ranking_sub.to_string(),
                    2,
                    false,
                    reset_label.to_string(),
                    !state.ranking_empty,
                ),
                GeneralHit::HyperKey => (
                    hyper_sub.to_string(),
                    2,
                    false,
                    hyper_title.to_string(),
                    true,
                ),
                GeneralHit::HyperShift => (
                    static_sub.to_string(),
                    1,
                    state.hyper_shift && hyper_on,
                    String::new(),
                    hyper_on,
                ),
                GeneralHit::Appearance => (
                    static_sub.to_string(),
                    2,
                    false,
                    appearance_trail.to_string(),
                    true,
                ),
                GeneralHit::Language => (
                    static_sub.to_string(),
                    2,
                    false,
                    language_trail.to_string(),
                    true,
                ),
                GeneralHit::Compact => (
                    static_sub.to_string(),
                    1,
                    state.compact,
                    String::new(),
                    true,
                ),
                GeneralHit::FavoritesInCompact => (
                    static_sub.to_string(),
                    1,
                    state.favorites_in_compact,
                    String::new(),
                    state.compact,
                ),
                GeneralHit::FollowCursor => (
                    static_sub.to_string(),
                    1,
                    state.follow_cursor,
                    String::new(),
                    true,
                ),
                GeneralHit::Draggable => (
                    static_sub.to_string(),
                    1,
                    state.draggable,
                    String::new(),
                    true,
                ),
                GeneralHit::LaunchAtLogin => (
                    static_sub.to_string(),
                    1,
                    state.launch_at_login,
                    String::new(),
                    true,
                ),
                GeneralHit::ShowInMenuBar => (
                    static_sub.to_string(),
                    1,
                    state.show_in_menu_bar,
                    String::new(),
                    true,
                ),
                GeneralHit::PopToRoot => (pop_sub.clone(), 2, false, pop_trail.clone(), true),
                GeneralHit::AutoSwitchInput => (
                    static_sub.to_string(),
                    1,
                    state.auto_switch,
                    String::new(),
                    true,
                ),
            };
            let mut page_row = action_row(hit as i32, &title, &subtitle, &trailing, control);
            page_row.checked = checked;
            page_row.enabled = enabled;
            page_row.active = hit == GeneralHit::PaletteRecorder && state.recording_palette;
            rows.push(page_row);
        }
        groups.push((title, note, rows));
    }
    groups
}

fn permissions_model(lang: UiLang) -> Vec<(String, String, Vec<PageRow>)> {
    let open = open_settings_action(lang);
    let rows = (0..crate::features::settings::panes::permissions::ROWS.len())
        .map(|index| {
            action_row(
                index as i32,
                permission_title(index, lang),
                permission_subtitle(index, lang),
                open,
                2,
            )
        })
        .collect();
    vec![(String::new(), String::new(), rows)]
}

fn backup_model(lang: UiLang) -> Vec<(String, String, Vec<PageRow>)> {
    let rows = [
        backup_export(lang),
        backup_import(lang),
        backup_raycast(lang),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, title)| action_row(index as i32, title, "", "", 2))
    .collect();
    vec![(String::new(), String::new(), rows)]
}

fn about_model(lang: UiLang) -> Vec<PageRow> {
    vec![
        action_row(0, about_support(lang), support_built_with_love(lang), "", 2),
        action_row(1, about_window_title(lang), "", "", 2),
    ]
}

/// Updates a fixed-length model row by row: replacing the model would recreate
/// every delegate and throw the keyboard focus away.
pub(super) fn sync_rows<T: Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (index, row) in rows.into_iter().enumerate() {
        model.set_row_data(index, row);
    }
}

/// The localized tab title, shared with the page models.
pub(super) fn tab_title(tab: SettingsTab, lang: UiLang) -> slint::SharedString {
    settings_tab_title(tab, lang).into()
}

/// SettingsTab has no explicit index API; the discriminant is what the Slint
/// sidebar carries, and the enum is fieldless.
fn tab_from_index(index: i32) -> Option<SettingsTab> {
    SettingsSection::all()
        .iter()
        .flat_map(|section| section.tabs())
        .copied()
        .find(|tab| *tab as i32 == index)
}

fn page_kind(tab: SettingsTab) -> i32 {
    match tab {
        SettingsTab::General => 0,
        SettingsTab::Permissions => 1,
        SettingsTab::About => 2,
        SettingsTab::Backup => 3,
        SettingsTab::Applications
        | SettingsTab::SystemSettings
        | SettingsTab::SystemActions
        | SettingsTab::Commands => 4,
        SettingsTab::Quicklinks | SettingsTab::Ai => 5,
        _ => 6,
    }
}

fn tab_selected(inner: *mut Inner, index: i32) {
    let Some(tab) = tab_from_index(index) else {
        return;
    };
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    unsafe {
        (*core).select_settings_tab(tab);
    }
}

fn general_hit_from_id(id: i32) -> Option<crate::features::settings::panes::general::GeneralHit> {
    crate::features::settings::panes::general::section_rows()
        .iter()
        .flat_map(|rows| rows.iter())
        .copied()
        .find(|hit| *hit as i32 == id)
}

fn general_row_activated(inner: *mut Inner, id: i32) {
    use crate::features::settings::panes::general::{self, GeneralHit, GeneralToggle};
    let Some(hit) = general_hit_from_id(id) else {
        return;
    };
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    match hit {
        GeneralHit::PaletteRecorder => start_recording(inner),
        GeneralHit::ResetRanking => confirm_reset_ranking(inner),
        GeneralHit::HyperKey => unsafe { (*core).cycle_hyper_key() },
        GeneralHit::HyperShift => unsafe {
            if general::hyper_includes_shift_enabled(&(*core).settings.hyper_key) {
                (*core).set_hyper_includes_shift(!(*core).settings.hyper_includes_shift);
            }
        },
        GeneralHit::Appearance => unsafe { (*core).cycle_appearance() },
        GeneralHit::Language => unsafe { (*core).cycle_ui_language() },
        GeneralHit::Compact => unsafe { (*core).toggle_setting_bool(GeneralToggle::Compact) },
        GeneralHit::FavoritesInCompact => unsafe {
            (*core).toggle_setting_bool(GeneralToggle::FavoritesInCompact)
        },
        GeneralHit::FollowCursor => unsafe {
            (*core).toggle_setting_bool(GeneralToggle::FollowCursor)
        },
        GeneralHit::Draggable => unsafe { (*core).toggle_setting_bool(GeneralToggle::Draggable) },
        GeneralHit::LaunchAtLogin => unsafe {
            (*core).toggle_setting_bool(GeneralToggle::LaunchAtLogin)
        },
        GeneralHit::ShowInMenuBar => unsafe {
            (*core).toggle_setting_bool(GeneralToggle::ShowInMenuBar)
        },
        GeneralHit::PopToRoot => unsafe { (*core).cycle_pop_to_root() },
        GeneralHit::AutoSwitchInput => unsafe {
            (*core).toggle_setting_bool(GeneralToggle::AutoSwitch)
        },
    }
}

/// Reset ranking asks first, through the shared modal gate (no nested loop).
fn confirm_reset_ranking(inner: *mut Inner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    if unsafe { (*core).ranking_is_empty() } {
        return;
    }
    let lang = unsafe { (*core).ui_lang() };
    let prompt = crate::surfaces::dialog::ConfirmPrompt {
        title: launcher_reset_confirm_title(lang).into(),
        message: launcher_reset_confirm_message(lang).into(),
        accept: reset_ranking_action(lang).into(),
        cancel: chrome(Chrome::Cancel, lang).into(),
        destructive: false,
    };
    let host = unsafe { (*inner).host };
    crate::surfaces::dialog::confirm(&prompt, move || {
        if let Some(core) = core_from_host(host) {
            unsafe {
                (*core).reset_learned_ranking();
            }
        }
    });
}

fn permissions_row_activated(_inner: *mut Inner, id: i32) {
    let Some(row) = crate::features::settings::panes::permissions::ROWS.get(id as usize) else {
        return;
    };
    let _ = crate::features::launcher::ui::coordinator::execute(
        &crate::features::launcher::ui::coordinator::LaunchSpec::Uri(row.uri.to_string()),
    );
}

fn backup_row_activated(inner: *mut Inner, id: i32) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    let owner = {
        let hwnd = unsafe { (*inner).hwnd };
        if hwnd.is_invalid() {
            unsafe { (*inner).host }
        } else {
            hwnd
        }
    };
    match id {
        0 => unsafe { (*core).export_settings(owner) },
        1 => unsafe { (*core).import_settings(owner) },
        2 => unsafe { (*core).import_raycast(owner) },
        _ => {}
    }
}

fn about_row_activated(inner: *mut Inner, id: i32) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    match id {
        0 => unsafe { (*core).show_support() },
        1 => unsafe { (*core).show_about() },
        _ => {}
    }
}

fn open_slint_link(inner: *mut Inner) {
    let lang = core_from_host(unsafe { (*inner).host })
        .map(|core| unsafe { (*core).ui_lang() })
        .unwrap_or_default();
    let _ = crate::features::launcher::ui::coordinator::execute(
        &crate::features::launcher::ui::coordinator::LaunchSpec::Uri(
            about_slint_link(lang).to_string(),
        ),
    );
}

fn start_recording(inner: *mut Inner) {
    let Some(core) = core_from_host(unsafe { (*inner).host }) else {
        return;
    };
    unsafe {
        (*core).pause_global_hotkeys();
        (*inner).recorder.begin(RECORDER_ACTION.into());
    }
    refresh_inner(inner);
}

/// Ends a capture. The stored binding is written by the caller; this only
/// restores global hotkeys and repaints the well.
fn stop_recording(inner: *mut Inner, resume_hotkeys: bool) {
    let recording = unsafe { (*inner).recorder.is_recording() };
    if !recording {
        return;
    }
    unsafe {
        (*inner).recorder.cancel();
    }
    if resume_hotkeys {
        if let Some(core) = core_from_host(unsafe { (*inner).host }) {
            unsafe {
                (*core).resume_global_hotkeys();
            }
        }
    }
    refresh_inner(inner);
}

/// Applies one capture outcome. Always consumes the key while recording.
fn apply_capture(inner: *mut Inner, action: &str, outcome: CaptureOutcome) -> bool {
    match outcome {
        CaptureOutcome::Ignore => return true,
        CaptureOutcome::Cancel => unsafe {
            (*inner).recorder.cancel();
        },
        CaptureOutcome::Clear => {
            unsafe {
                (*inner).recorder.cancel();
            }
            if let Some(core) = core_from_host(unsafe { (*inner).host }) {
                unsafe {
                    (*core).set_hotkey(action, None);
                }
            }
        }
        CaptureOutcome::Commit(binding) => {
            unsafe {
                (*inner).recorder.cancel();
            }
            if let Some(core) = core_from_host(unsafe { (*inner).host }) {
                unsafe {
                    (*core).set_hotkey(action, Some(binding));
                }
            }
        }
    }
    if let Some(core) = core_from_host(unsafe { (*inner).host }) {
        unsafe {
            (*core).resume_global_hotkeys();
        }
    }
    refresh_inner(inner);
    true
}

fn current_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            win: GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0,
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The close button only hides: any draft stays, and the window keeps its HWND.
fn close_requested(inner: *mut Inner) {
    stop_recording(inner, true);
    unsafe {
        (*inner).visible = false;
    }
}

/// WS_EX_APPWINDOW and the taskbar: winit sets skip_taskbar, so the settings
/// window gets its own taskbar button only after clearing TOOLWINDOW.
fn apply_taskbar_window(hwnd: HWND) {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (before | WS_EX_APPWINDOW_BIT) & !(WS_EX_TOOLWINDOW.0 as isize);
        if next != before {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Centres the window on the monitor under the pointer, like the old window.
fn center(hwnd: HWND) {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    let screens = crate::platform::screens::screens_px();
    let screen = crate::platform::screens::target_screen_from_cursor_px((pt.x, pt.y), &screens);
    let (x, y) = match screen {
        Some(s) => {
            let sw = s.work.right - s.work.left;
            let sh = s.work.bottom - s.work.top;
            (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 2)
        }
        None => (100, 100),
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

fn window_hwnd(ui: &Settings) -> Option<HWND> {
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::Win32(win32) = handle.as_raw() else {
                return None;
            };
            Some(HWND(win32.hwnd.get() as *mut core::ffi::c_void))
        })
        .flatten()
}

unsafe extern "system" fn settings_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    data: usize,
) -> LRESULT {
    let inner = data as *mut Inner;
    match msg {
        // A shortcut capture runs here, on the native VK codes, so the old
        // Recorder (chords and double-taps) still decides what a key means.
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if inner.is_null() || !(*inner).recorder.is_recording() {
                return DefSubclassProc(hwnd, msg, wparam, lparam);
            }
            let Some(action) = (*inner).recorder.action.clone() else {
                return DefSubclassProc(hwnd, msg, wparam, lparam);
            };
            let outcome =
                (*inner)
                    .recorder
                    .on_keydown(wparam.0 as u16, current_modifiers(), now_ms());
            if apply_capture(inner, &action, outcome) {
                LRESULT(0)
            } else {
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        WM_KEYUP | WM_SYSKEYUP => {
            if inner.is_null() || !(*inner).recorder.is_recording() {
                return DefSubclassProc(hwnd, msg, wparam, lparam);
            }
            let Some(action) = (*inner).recorder.action.clone() else {
                return DefSubclassProc(hwnd, msg, wparam, lparam);
            };
            let outcome = (*inner).recorder.on_keyup(wparam.0 as u16, now_ms());
            if apply_capture(inner, &action, outcome) {
                LRESULT(0)
            } else {
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(settings_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}
