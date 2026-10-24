//! Models and activation dispatch for the 15 settings tabs phase 8 fills.
//!
//! The split mirrors the old Direct2D window's pane modules: this file builds
//! the Slint models from AppCore and routes row activations back into AppCore,
//! while `settings.rs` keeps the window shell, the sidebar and the shortcut
//! recorder's HWND subclass. The launcher pages (Applications, System
//! Settings, System Actions, Commands) share one sliced table; the feature
//! pages (Quicklinks here, the rest in the later tasks) share one rows page.

use slint::SharedString;
use tinycast_pure::i18n::{empty_list_copy, UiLang};
use tinycast_pure::settings_tab::SettingsTab;
use tinycast_pure::theme::size;

use crate::features::launcher::settings::items::{
    filter_entries, hotkey_action_key, LauncherItemsSection,
};

use super::settings::{Inner, Settings};

pub(super) type ItemRow = tinycast_ui::LauncherItemRow;

/// How far past the viewport the visible slice reaches, in list rows.
const OVERSCAN_ROWS: usize = 4;

/// The launcher list viewport is the window's detail column below the heading,
/// the intro cards, the enable row and the filter; 620 DIP bounds it with room
/// to spare at the 700 DIP minimum.
const LIST_VIEWPORT: f32 = 620.0;

/// The launcher page: intro cards (Commands only), the kind enable switch, and
/// the visible slice of the filtered items.
pub(super) fn sync_launcher(ui: &Settings, inner: &mut Inner, core: &crate::app_core::AppCore) {
    let lang = core.ui_lang();
    let tab = core.settings_tab;
    let Some(section) = LauncherItemsSection::for_lang(tab, lang) else {
        return;
    };
    let kind_on = core.visibility.is_kind_enabled(section.kind);

    // Intro cards: Commands carries its own switches and the command list.
    let intro = if tab == SettingsTab::Commands {
        commands_intro_model(core, lang)
    } else {
        Vec::new()
    };
    inner.launcher_intro.sync(intro);

    ui.set_launcher_heading(super::settings::tab_title(tab, lang));
    ui.set_launcher_enable_title(SharedString::from(section.enable_title_lang(lang)));
    ui.set_launcher_enable_subtitle(SharedString::new());
    ui.set_launcher_kind_on(kind_on);
    ui.set_launcher_filter_hint(SharedString::from(section.search_prompt));
    ui.set_launcher_empty(SharedString::from(empty_list_copy(
        &inner.filter_query,
        lang,
    )));

    let entries = core.settings_entries(section.kind);
    let filtered = filter_entries(&entries, section.kind, &inner.filter_query);
    inner.launcher_ids = filtered.iter().map(|e| e.id.clone()).collect();
    inner.launcher_actions = filtered.iter().map(|e| hotkey_action_key(e)).collect();

    let row_h = size::SETTINGS_ROW_HEIGHT;
    let count = filtered.len();
    ui.set_launcher_items_height((count as f32 * row_h).max(0.0));
    slice_launcher(ui, inner, core);
}

/// Rebuilds only the visible slice: the scroll path calls this so a wheel
/// tick never rebuilds the intro cards or the filter.
pub(super) fn slice_launcher(ui: &Settings, inner: &mut Inner, core: &crate::app_core::AppCore) {
    let lang = core.ui_lang();
    let tab = core.settings_tab;
    let Some(section) = LauncherItemsSection::for_lang(tab, lang) else {
        return;
    };
    let entries = core.settings_entries(section.kind);
    let filtered = filter_entries(&entries, section.kind, &inner.filter_query);
    inner.launcher_ids = filtered.iter().map(|e| e.id.clone()).collect();
    inner.launcher_actions = filtered.iter().map(|e| hotkey_action_key(e)).collect();

    let row_h = size::SETTINGS_ROW_HEIGHT;
    let count = filtered.len();
    let scroll = ui.get_launcher_list_scroll();
    let first = (scroll / row_h).floor().max(0.0) as usize;
    let first = first.saturating_sub(OVERSCAN_ROWS);
    let last = (((scroll + LIST_VIEWPORT) / row_h).ceil() as usize + OVERSCAN_ROWS).min(count);
    let recording_action = inner.recorder.action.clone();
    let mut rows = Vec::with_capacity(last.saturating_sub(first));
    for index in first..last {
        let Some(entry) = filtered.get(index) else {
            break;
        };
        let id = entry.id.clone();
        let action = inner
            .launcher_actions
            .get(index)
            .cloned()
            .flatten()
            .unwrap_or_default();
        let binding = core.hotkeys.get(&action).map(|b| b.label());
        let recording = recording_action.as_deref() == Some(action.as_str()) && !action.is_empty();
        let alias = if inner.alias_target.as_deref() == Some(id.as_str()) {
            inner.alias_draft.clone()
        } else {
            core.aliases.get(&id).unwrap_or("").to_string()
        };
        rows.push(ItemRow {
            index: index as i32,
            top: index as f32 * row_h,
            height: row_h,
            name: entry.name.clone().into(),
            alias: alias.into(),
            hotkey: if recording {
                crate::features::hotkeys::ui::recorder::well_caption_lang(None, true, lang).into()
            } else {
                binding.unwrap_or_default().into()
            },
            visible: core.visibility.is_item_visible(&id),
            recording,
            has_hotkey: !action.is_empty(),
        });
    }
    super::settings::sync_rows(&inner.launcher, rows);
}

/// Commands' own intro: the feature switches, then New and the command list.
fn commands_intro_model(
    core: &crate::app_core::AppCore,
    lang: UiLang,
) -> Vec<(String, String, Vec<super::settings::PageRow>)> {
    use tinycast_pure::i18n::{
        custom_commands_enable_subtitle, custom_commands_enable_title, custom_commands_new,
        custom_commands_show_subtitle, show_in_launcher,
    };

    let enabled = core.settings.custom_commands_enabled;
    let mut switches = vec![
        super::settings::action_row(
            0,
            custom_commands_enable_title(lang),
            custom_commands_enable_subtitle(lang),
            "",
            1,
        ),
        super::settings::action_row(
            1,
            show_in_launcher(lang),
            custom_commands_show_subtitle(lang),
            "",
            1,
        ),
    ];
    // The switch rows dim with the feature, like the old pane.
    switches[1].enabled = enabled;

    let mut commands = vec![super::settings::action_row(
        2,
        custom_commands_new(lang),
        "",
        "",
        0,
    )];
    for (index, command) in core.custom_command_records().iter().enumerate() {
        let mut row = super::settings::action_row(
            1000 + index as i32,
            &command.name,
            &command.command,
            "",
            0,
        );
        row.enabled = enabled;
        commands.push(row);
    }
    vec![
        (String::new(), String::new(), switches),
        (String::new(), String::new(), commands),
    ]
}

/// The feature rows page. Quicklinks in this task; the other feature tabs
/// arrive with their own tasks and fall back to the neutral placeholder.
pub(super) fn sync_feature(ui: &Settings, inner: &mut Inner, core: &crate::app_core::AppCore) {
    let lang = core.ui_lang();
    let tab = core.settings_tab;
    ui.set_feature_heading(super::settings::tab_title(tab, lang));
    let groups = match tab {
        SettingsTab::Quicklinks => quicklinks_model(core, lang),
        _ => Vec::new(),
    };
    if groups.is_empty() {
        return;
    }
    inner.feature.sync(groups);
}

fn quicklinks_model(
    core: &crate::app_core::AppCore,
    lang: UiLang,
) -> Vec<(String, String, Vec<super::settings::PageRow>)> {
    use tinycast_pure::i18n::{
        quicklinks_create, quicklinks_enable_subtitle, quicklinks_enable_title, quicklinks_export,
        quicklinks_import, quicklinks_show_subtitle, show_in_launcher,
    };

    let enabled = core.settings.quicklinks_enabled;
    let mut switches = vec![
        super::settings::action_row(
            10,
            quicklinks_enable_title(lang),
            quicklinks_enable_subtitle(lang),
            "",
            1,
        ),
        super::settings::action_row(
            11,
            show_in_launcher(lang),
            quicklinks_show_subtitle(lang),
            "",
            1,
        ),
    ];
    switches[1].enabled = enabled;

    let mut actions = vec![
        super::settings::action_row(0, quicklinks_create(lang), "", "", 0),
        super::settings::action_row(1, quicklinks_import(lang), "", "", 0),
        super::settings::action_row(2, quicklinks_export(lang), "", "", 0),
    ];
    for (index, link) in core.quicklink_records().iter().enumerate() {
        let mut row =
            super::settings::action_row(100 + index as i32, &link.name, &link.destination, "", 0);
        row.enabled = enabled;
        actions.push(row);
    }
    vec![
        (String::new(), String::new(), switches),
        (String::new(), String::new(), actions),
    ]
}

/// Intro rows (Commands): the switches and New/the command list.
pub(super) fn intro_activated(ptr: *mut Inner, id: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let host = unsafe { (*ptr).host };
    unsafe {
        match id {
            0 => {
                (*core).set_custom_commands_enabled(!(*core).settings.custom_commands_enabled);
            }
            1 => {
                (*core).set_custom_commands_show_in_launcher(
                    !(*core).settings.custom_commands_show_in_launcher,
                );
            }
            2 => {
                commit_alias(ptr);
                (*core).new_custom_command(host);
            }
            index if index >= 1000 => {
                commit_alias(ptr);
                (*core).edit_custom_command_at(host, (index - 1000) as usize);
            }
            _ => {}
        }
    }
}

/// Quicklinks rows: ids 10/11 toggles, 0/1/2 create/import/export, 100+k links.
pub(super) fn feature_row_activated(ptr: *mut Inner, id: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let host = unsafe { (*ptr).host };
    unsafe {
        match id {
            10 => {
                (*core).set_quicklinks_enabled(!(*core).settings.quicklinks_enabled);
            }
            11 => {
                (*core)
                    .set_quicklinks_show_in_launcher(!(*core).settings.quicklinks_show_in_launcher);
            }
            0 => (*core).edit_quicklink(host, None),
            1 => (*core).import_quicklinks(host),
            2 => (*core).export_quicklinks(host),
            index if index >= 100 => (*core).edit_quicklink(host, Some((index - 100) as usize)),
            _ => {}
        }
    }
}

pub(super) fn enable_toggled(ptr: *mut Inner, on: bool) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let lang = unsafe { (*core).ui_lang() };
    let tab = unsafe { (*core).settings_tab };
    if let Some(section) = LauncherItemsSection::for_lang(tab, lang) {
        stop_recording(ptr);
        unsafe {
            (*core).set_kind_enabled(section.kind, on);
        }
    }
}

pub(super) fn item_toggled(ptr: *mut Inner, index: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let inner = unsafe { &mut *ptr };
    let Some(id) = inner.launcher_ids.get(index as usize).cloned() else {
        return;
    };
    unsafe {
        let visible = (*core).visibility.is_item_visible(&id);
        (*core).set_item_visible(&id, !visible);
    }
}

pub(super) fn alias_activated(ptr: *mut Inner, index: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let Some(ui) = ui_of(ptr) else {
        return;
    };
    let inner = unsafe { &mut *ptr };
    let Some(id) = inner.launcher_ids.get(index as usize).cloned() else {
        return;
    };
    // One draft at a time: commit whatever is open first.
    commit_alias(ptr);
    unsafe {
        let draft: String = (*core).aliases.get(&id).unwrap_or("").to_string();
        inner.alias_target = Some(id);
        inner.alias_draft = draft.clone();
        ui.set_launcher_alias_text(draft.into());
        ui.set_launcher_alias_y(index as f32 * size::SETTINGS_ROW_HEIGHT);
        ui.set_launcher_alias_active(index);
        // The editor is (re)created by the property change and takes the
        // keyboard in its own `init`.
    }
}

pub(super) fn alias_edited(ptr: *mut Inner, _index: i32, text: String) {
    let inner = unsafe { &mut *ptr };
    inner.alias_draft = text;
}

/// Commits the open draft (index -1 means "the active one") into AppCore.
pub(super) fn alias_committed(ptr: *mut Inner, index: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let ui = ui_of(ptr);
    let inner = unsafe { &mut *ptr };
    let Some(id) = inner.alias_target.clone() else {
        return;
    };
    // A click on another row's alias moves the draft instead of closing it;
    // only a commit for the active row (or the focus-lost -1) writes it.
    if index >= 0 {
        let next = inner.launcher_ids.get(index as usize).cloned();
        if next.as_deref() != Some(id.as_str()) {
            return;
        }
    }
    unsafe {
        let draft = inner.alias_draft.clone();
        (*core).set_alias_draft(&id, &draft);
        inner.alias_target = None;
        inner.alias_draft = String::new();
        if let Some(ui) = ui.as_ref() {
            ui.set_launcher_alias_active(-1);
        }
    }
}

/// Gives up the draft without writing: a filter change or a tab switch.
pub(super) fn alias_cancelled(ptr: *mut Inner) {
    let ui = ui_of(ptr);
    let inner = unsafe { &mut *ptr };
    inner.alias_target = None;
    inner.alias_draft = String::new();
    if let Some(ui) = ui.as_ref() {
        ui.set_launcher_alias_active(-1);
    }
}

pub(super) fn commit_alias(ptr: *mut Inner) {
    let open = unsafe { (*ptr).alias_target.is_some() };
    if open {
        alias_committed(ptr, -1);
    }
}

pub(super) fn hotkey_clicked(ptr: *mut Inner, index: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let inner = unsafe { &mut *ptr };
    commit_alias(ptr);
    let Some(action) = inner
        .launcher_actions
        .get(index as usize)
        .cloned()
        .flatten()
    else {
        return;
    };
    unsafe {
        if inner.recorder.action.as_deref() == Some(action.as_str()) {
            inner.recorder.cancel();
            (*core).resume_global_hotkeys();
        } else {
            if inner.recorder.is_recording() {
                inner.recorder.cancel();
                (*core).resume_global_hotkeys();
            }
            inner.recorder.begin(action);
            (*core).pause_global_hotkeys();
        }
        let lang = (*core).ui_lang();
        let tab = (*core).settings_tab;
        if LauncherItemsSection::for_lang(tab, lang).is_some() {
            if let Some(ui) = ui_of(ptr) {
                slice_launcher(&ui, inner, &*core);
            }
        }
    }
}

pub(super) fn filter_changed(ptr: *mut Inner, text: String) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    alias_cancelled(ptr);
    stop_recording(ptr);
    let inner = unsafe { &mut *ptr };
    inner.filter_query = text;
    unsafe {
        let lang = (*core).ui_lang();
        let tab = (*core).settings_tab;
        if LauncherItemsSection::for_lang(tab, lang).is_some() {
            if let Some(ui) = ui_of(ptr) {
                ui.set_launcher_empty(SharedString::from(empty_list_copy(
                    &inner.filter_query,
                    lang,
                )));
                slice_launcher(&ui, inner, &*core);
            }
        }
    }
}

pub(super) fn list_scrolled(ptr: *mut Inner) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let Some(ui) = ui_of(ptr) else {
        return;
    };
    let lang = unsafe { (*core).ui_lang() };
    let tab = unsafe { (*core).settings_tab };
    if LauncherItemsSection::for_lang(tab, lang).is_some() {
        slice_launcher(&ui, unsafe { &mut *ptr }, unsafe { &*core });
    }
}

/// The tab moved: a draft never survives it, like the old window.
pub(super) fn tab_changed(inner: *mut Inner) {
    alias_cancelled(inner);
}

/// Cancels an in-flight hotkey capture, if any.
pub(super) fn stop_recording(ptr: *mut Inner) {
    if unsafe { (*ptr).recorder.is_recording() } {
        let inner = unsafe { &mut *ptr };
        inner.recorder.cancel();
        if let Some(core) = core_of(ptr) {
            unsafe {
                (*core).resume_global_hotkeys();
            }
        }
    }
}

fn core_of(inner: *mut Inner) -> Option<*mut crate::app_core::AppCore> {
    unsafe { crate::ui_host::app_core((*inner).host) }
}

fn ui_of(inner: *mut Inner) -> Option<Settings> {
    unsafe { (*inner).weak.upgrade() }
}
