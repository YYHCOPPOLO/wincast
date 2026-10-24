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

use super::settings::{Inner, PageRow, Settings};

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

/// The feature rows page: every feature tab's rows build from AppCore here.
pub(super) fn sync_feature(ui: &Settings, inner: &mut Inner, core: &crate::app_core::AppCore) {
    let lang = core.ui_lang();
    let tab = core.settings_tab;
    ui.set_feature_heading(super::settings::tab_title(tab, lang));
    let groups = feature_model(inner, core, lang, tab);
    inner.feature.sync(groups);
}

fn toggle_row(id: i32, title: &str, sub: &str, checked: bool, enabled: bool) -> PageRow {
    let mut row = super::settings::action_row(id, title, sub, "", 1);
    row.checked = checked;
    row.enabled = enabled;
    row
}

/// A cycling value row: the trailing button shows the current value.
fn value_row(id: i32, title: &str, sub: &str, value: &str, enabled: bool) -> PageRow {
    let mut row = super::settings::action_row(id, title, sub, value, 4);
    row.enabled = enabled;
    row
}

fn input_row(id: i32, title: &str, value: &str, placeholder: &str, enabled: bool) -> PageRow {
    let mut row = super::settings::action_row(id, title, "", value, 5);
    row.placeholder = placeholder.into();
    row.enabled = enabled;
    row
}

fn badge_row(title: &str, text: &str, tone: i32) -> PageRow {
    let mut row = super::settings::action_row(0, title, "", text, 6);
    row.tone = tone;
    row
}

/// A row whose trailing is a destructive text button; the row's own click
/// stays `activated`, the button fires `trailing-activated`.
fn danger_row(id: i32, title: &str, sub: &str, label: &str, enabled: bool) -> PageRow {
    let mut row = super::settings::action_row(id, title, sub, label, 7);
    row.enabled = enabled;
    row
}

fn feature_model(
    inner: &Inner,
    core: &crate::app_core::AppCore,
    lang: UiLang,
    tab: SettingsTab,
) -> Vec<(String, String, Vec<PageRow>)> {
    use tinycast_pure::i18n as say;

    match tab {
        SettingsTab::Quicklinks => quicklinks_model(core, lang),

        SettingsTab::QuickActions => {
            vec![(
                String::new(),
                String::new(),
                vec![
                    toggle_row(
                        10,
                        say::qa_enable_title(lang),
                        say::qa_enable_subtitle(lang),
                        core.settings.quick_actions_enabled,
                        true,
                    ),
                    value_row(
                        11,
                        say::qa_translate_into(lang),
                        "",
                        say::qa_language_name(&core.settings.quick_action_language, lang),
                        core.settings.quick_actions_enabled,
                    ),
                ],
            )]
        }

        SettingsTab::Notes => {
            vec![(
                String::new(),
                String::new(),
                vec![toggle_row(
                    10,
                    say::notes_enable_title(lang),
                    say::notes_enable_subtitle(lang),
                    core.settings.notes_enabled,
                    true,
                )],
            )]
        }

        SettingsTab::Emoji => {
            vec![(
                String::new(),
                String::new(),
                vec![value_row(
                    10,
                    say::emoji_skin_tone_title(lang),
                    say::emoji_skin_tone_subtitle(lang),
                    say::emoji_skin_tone_label(&core.settings.emoji_skin_tone, lang),
                    true,
                )],
            )]
        }

        SettingsTab::Snippets => {
            let enabled = core.settings.snippets_enabled;
            vec![(
                String::new(),
                String::new(),
                vec![
                    toggle_row(
                        10,
                        say::snippets_enable_title(lang),
                        say::snippets_enable_subtitle(lang),
                        enabled,
                        true,
                    ),
                    toggle_row(
                        11,
                        say::show_in_launcher(lang),
                        say::snippets_show_subtitle(lang),
                        core.settings.snippets_show_in_launcher,
                        enabled,
                    ),
                ],
            )]
        }

        SettingsTab::Extensions => {
            let enabled = core.settings.extensions_enabled;
            vec![(
                String::new(),
                say::extensions_runtime_notice(lang).to_string(),
                vec![
                    toggle_row(
                        10,
                        say::extensions_enable_title(lang),
                        say::extensions_enable_subtitle(lang),
                        enabled,
                        true,
                    ),
                    toggle_row(
                        11,
                        say::show_in_launcher(lang),
                        say::extensions_show_subtitle(lang),
                        core.settings.extensions_show_in_launcher,
                        enabled,
                    ),
                ],
            )]
        }

        SettingsTab::Calendar => {
            let enabled = core.settings.calendar_enabled;
            vec![(
                String::new(),
                String::new(),
                vec![
                    toggle_row(
                        10,
                        say::calendar_enable_title(lang),
                        say::calendar_enable_subtitle(lang),
                        enabled,
                        true,
                    ),
                    toggle_row(
                        11,
                        say::calendar_auto_join(lang),
                        say::calendar_auto_join_sub(lang),
                        core.settings.auto_join_meetings,
                        enabled,
                    ),
                    toggle_row(
                        12,
                        say::calendar_camera(lang),
                        say::calendar_camera_sub(lang),
                        core.settings.camera_preview,
                        enabled,
                    ),
                    value_row(
                        13,
                        say::calendar_join_window(lang),
                        "",
                        say::calendar_join_minutes(core.settings.join_window_minutes, lang),
                        enabled && core.settings.auto_join_meetings,
                    ),
                ],
            )]
        }

        SettingsTab::WindowManagement => {
            let enabled = core.settings.window_management_enabled;
            vec![(
                String::new(),
                String::new(),
                vec![
                    toggle_row(
                        10,
                        say::window_enable_title(lang),
                        say::window_enable_subtitle(lang),
                        enabled,
                        true,
                    ),
                    toggle_row(
                        11,
                        say::show_in_launcher(lang),
                        say::window_show_subtitle(lang),
                        core.settings.window_management_show_in_launcher,
                        enabled,
                    ),
                    toggle_row(
                        12,
                        say::window_cycle_title(lang),
                        say::window_cycle_subtitle(lang),
                        core.settings.window_cycle_on_repeat,
                        enabled,
                    ),
                    value_row(
                        13,
                        say::window_gap_title(lang),
                        &say::window_gap_subtitle(core.settings.window_gap, lang),
                        &core.settings.window_gap.to_string(),
                        enabled,
                    ),
                ],
            )]
        }

        SettingsTab::FileSearch => {
            let enabled = core.settings.file_search_enabled;
            let switches = vec![toggle_row(
                10,
                say::file_search_enable_title(lang),
                say::file_search_enable_subtitle(lang),
                enabled,
                true,
            )];
            let mut folders = vec![super::settings::action_row(
                30,
                say::file_search_add_folder(lang),
                "",
                "",
                0,
            )];
            // The scopes sit above Add; the row click removes, like the old pane.
            let mut scope_rows: Vec<PageRow> = core
                .settings
                .file_search_scopes
                .iter()
                .enumerate()
                .map(|(index, path)| {
                    super::settings::action_row(20 + index as i32, path, "", "", 0)
                })
                .collect();
            scope_rows.extend(folders.drain(..));
            let mut ignores = vec![super::settings::action_row(
                50,
                say::file_search_add_ignore(lang),
                "",
                "",
                0,
            )];
            let mut ignore_rows: Vec<PageRow> = core
                .settings
                .file_search_ignore_patterns
                .iter()
                .enumerate()
                .map(|(index, pattern)| {
                    super::settings::action_row(40 + index as i32, pattern, "", "", 0)
                })
                .collect();
            ignore_rows.extend(ignores.drain(..));
            vec![
                (String::new(), String::new(), switches),
                (
                    say::file_search_folders(lang).to_string(),
                    String::new(),
                    scope_rows,
                ),
                (
                    say::file_search_ignore(lang).to_string(),
                    String::new(),
                    ignore_rows,
                ),
            ]
        }

        SettingsTab::Clipboard => {
            let disabled = &core.settings.clipboard_disabled_apps;
            let mut apps: Vec<PageRow> = disabled
                .iter()
                .enumerate()
                .map(|(index, app)| super::settings::action_row(20 + index as i32, app, "", "", 0))
                .collect();
            apps.push(super::settings::action_row(
                30,
                say::clipboard_add_application(lang),
                "",
                "",
                0,
            ));
            vec![
                (
                    String::new(),
                    say::clipboard_disabled_caption(lang).to_string(),
                    vec![value_row(
                        10,
                        say::clipboard_keep_for(lang),
                        "",
                        say::clipboard_retention(core.settings.clipboard_retention_days, lang),
                        true,
                    )],
                ),
                (
                    say::clipboard_disabled_apps(lang).to_string(),
                    String::new(),
                    apps,
                ),
                (
                    say::clipboard_clear_section(lang).to_string(),
                    String::new(),
                    vec![super::settings::action_row(
                        40,
                        say::clipboard_clear_history(lang),
                        "",
                        "",
                        0,
                    )],
                ),
            ]
        }

        SettingsTab::Ai => ai_model(inner, core, lang),

        _ => Vec::new(),
    }
}

/// The AI page: the fixed switches and cycling values, then the connection
/// list with its inline editor rows.
fn ai_model(
    inner: &Inner,
    core: &crate::app_core::AppCore,
    lang: UiLang,
) -> Vec<(String, String, Vec<PageRow>)> {
    use crate::features::ai::service::chatgpt::CodexPhase;
    use crate::features::ai::settings::pane as ai;
    use tinycast_pure::ai::ModelSelection;
    use tinycast_pure::i18n as say;

    let enabled = core.settings.ai_enabled;
    let codex = core.chatgpt_phase();
    let codex_label = match codex {
        CodexPhase::Unavailable => say::ai_install_codex(lang).to_string(),
        _ if !ai::can_offer_chatgpt() => say::ai_not_available(lang).to_string(),
        CodexPhase::Connected => say::ai_disconnect_chatgpt(lang).to_string(),
        CodexPhase::Failed => say::ai_try_again(lang).to_string(),
        CodexPhase::Idle => say::ai_connect(lang).to_string(),
    };
    let model_label = match core.settings.ai_default_model.as_ref() {
        Some(ModelSelection::Api { connection, model }) => {
            let name = core
                .settings
                .ai_connections
                .iter()
                .find(|c| c.id.as_str() == connection.as_str())
                .map(|c| c.title())
                .unwrap_or_else(|| "API".into());
            format!("{name} · {model}")
        }
        Some(ModelSelection::ChatGpt { model, .. }) => format!("ChatGPT · {model}"),
        None => say::ai_none(lang).to_string(),
    };

    let top = vec![
        toggle_row(
            10,
            say::ai_enable_title(lang),
            say::ai_enable_subtitle(lang),
            enabled,
            true,
        ),
        toggle_row(
            11,
            say::ai_web_search_title(lang),
            say::ai_web_search_subtitle(lang),
            core.settings.ai_web_search,
            enabled,
        ),
        toggle_row(
            12,
            say::ai_system_prompt_title(lang),
            say::ai_system_prompt_subtitle(lang),
            core.settings.ai_system_prompt_enabled,
            enabled,
        ),
        value_row(
            13,
            say::ai_opens_to_title(lang),
            "",
            say::ai_opens_to_value(core.settings.ai_opens_to != 0, lang),
            enabled,
        ),
        value_row(
            14,
            say::ai_keep_conversations(lang),
            "",
            say::ai_retention_value(core.settings.ai_retention_days, lang),
            enabled,
        ),
        value_row(15, say::ai_default_model(lang), "", &model_label, enabled),
        value_row(
            16,
            say::ai_chatgpt_subscription(lang),
            "",
            &codex_label,
            true,
        ),
        super::settings::action_row(
            17,
            say::ai_add_connection(lang),
            say::ai_add_connection_sub(lang),
            "",
            0,
        ),
    ];

    // The connection list: one row per connection, the edited one expanded
    // into the provider row, the three fields and Discard.
    let mut rows = top;
    let mut editor: Vec<PageRow> = Vec::new();
    let editing = inner.ai_editing;
    for (index, conn) in core.settings.ai_connections.iter().enumerate() {
        let key_saved = core.ai_connection_key_saved(index);
        let sub = if key_saved {
            say::ai_saved_on_pc(lang).to_string()
        } else {
            say::ai_not_set(lang).to_string()
        };
        rows.push(danger_row(
            100 + index as i32,
            &conn.title(),
            &sub,
            say::ai_remove_action(lang),
            true,
        ));
        if editing == Some(index) {
            editor.push(value_row(
                200,
                say::ai_provider(lang),
                "",
                conn.provider.title(),
                true,
            ));
            editor.push(input_row(
                201,
                say::ai_base_url(lang),
                &inner.ai_url,
                "",
                true,
            ));
            editor.push(input_row(
                202,
                say::ai_model_id(lang),
                &inner.ai_model,
                "",
                true,
            ));
            editor.push(input_row(
                203,
                say::ai_api_key(lang),
                &inner.ai_key,
                "",
                true,
            ));
            editor.push(super::settings::action_row(
                205,
                tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang),
                "",
                "",
                0,
            ));
            if let Some(err) = &inner.ai_error {
                editor.push(badge_row("", err, 1));
            }
        }
    }
    let mut groups = vec![(String::new(), String::new(), rows)];
    if !editor.is_empty() {
        groups.push((String::new(), String::new(), editor));
    }
    groups
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
                (*core).begin_new_custom_command();
            }
            index if index >= 1000 => {
                commit_alias(ptr);
                (*core).begin_edit_custom_command_at((index - 1000) as usize);
            }
            _ => {}
        }
    }
}

/// The feature rows page dispatch. Ids: 10+ are the tab's own switches and
/// cycling values; 20+i/40+i remove a file-search scope/ignore pattern;
/// 100+i are AI connections; 200+ are the AI connection editor rows.
pub(super) fn feature_row_activated(ptr: *mut Inner, id: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let lang = unsafe { (*core).ui_lang() };
    let tab = unsafe { (*core).settings_tab };
    let host = unsafe { (*ptr).host };
    unsafe {
        match tab {
            SettingsTab::Quicklinks => match id {
                10 => (*core).set_quicklinks_enabled(!(*core).settings.quicklinks_enabled),
                11 => {
                    (*core).set_quicklinks_show_in_launcher(
                        !(*core).settings.quicklinks_show_in_launcher,
                    );
                }
                0 => (*core).begin_edit_quicklink(None),
                1 => (*core).import_quicklinks(host),
                2 => (*core).export_quicklinks(host),
                index if index >= 100 => (*core).begin_edit_quicklink(Some((index - 100) as usize)),
                _ => {}
            },

            SettingsTab::QuickActions => match id {
                10 => (*core).set_quick_actions_enabled(!(*core).settings.quick_actions_enabled),
                11 => (*core).cycle_quick_action_language(),
                _ => {}
            },

            SettingsTab::Notes => {
                if id == 10 {
                    (*core).set_notes_enabled(!(*core).settings.notes_enabled);
                }
            }

            SettingsTab::Emoji => {
                if id == 10 {
                    (*core).cycle_emoji_skin_tone();
                }
            }

            SettingsTab::Snippets => match id {
                10 => {
                    if (*core).settings.snippets_enabled {
                        confirm_disable_snippets(ptr);
                    } else {
                        (*core).set_snippets_enabled(true);
                    }
                }
                11 => {
                    (*core)
                        .set_snippets_show_in_launcher(!(*core).settings.snippets_show_in_launcher);
                }
                _ => {}
            },

            SettingsTab::Extensions => match id {
                10 => {
                    if let Err(err) =
                        (*core).try_set_extensions_enabled(!(*core).settings.extensions_enabled)
                    {
                        (*core).show_alert("Extensions", err);
                    }
                }
                11 => {
                    (*core).set_extensions_show_in_launcher(
                        !(*core).settings.extensions_show_in_launcher,
                    );
                }
                _ => {}
            },

            SettingsTab::Calendar => match id {
                10 => {
                    let enabled = (*core).settings.calendar_enabled;
                    (*core).set_calendar_enabled(!enabled);
                }
                11 => {
                    (*core).set_auto_join_meetings(!(*core).settings.auto_join_meetings);
                }
                12 => {
                    (*core).set_camera_preview(!(*core).settings.camera_preview);
                }
                13 => (*core).cycle_join_window(),
                _ => {}
            },

            SettingsTab::WindowManagement => match id {
                10 => {
                    (*core)
                        .set_window_management_enabled(!(*core).settings.window_management_enabled);
                }
                11 => {
                    (*core).set_window_management_show_in_launcher(
                        !(*core).settings.window_management_show_in_launcher,
                    );
                }
                12 => {
                    (*core).set_window_cycle_on_repeat(!(*core).settings.window_cycle_on_repeat);
                }
                13 => (*core).cycle_window_gap(),
                _ => {}
            },

            SettingsTab::FileSearch => match id {
                10 => {
                    (*core).set_file_search_enabled(!(*core).settings.file_search_enabled);
                }
                index if (20..40).contains(&index) => {
                    (*core).remove_file_search_scope((index - 20) as usize);
                }
                30 => {
                    if let Some(path) =
                        crate::features::file_search::settings::pane::pick_folder(host)
                    {
                        (*core).add_file_search_scope(path);
                    }
                }
                index if (40..50).contains(&index) => {
                    (*core).remove_file_search_ignore((index - 40) as usize);
                }
                50 => {
                    // The ignore-pattern prompt is the Slint sheet now; the
                    // pattern lands through its accept continuation.
                    let request = crate::ui_host::editor::EditorRequest {
                        title: tinycast_pure::i18n::ignore_pattern_title(lang).to_string(),
                        name_label: String::new(),
                        value_label: tinycast_pure::i18n::ignore_pattern_title(lang).to_string(),
                        confirm_label: String::new(),
                        accept_label: tinycast_pure::i18n::clipboard_add(lang).to_string(),
                        cancel_label: tinycast_pure::i18n::chrome(
                            tinycast_pure::i18n::Chrome::Cancel,
                            lang,
                        )
                        .to_string(),
                        show_confirm: false,
                        name_hidden: true,
                        initial_name: String::new(),
                        initial_value: String::new(),
                        initial_confirm: false,
                    };
                    let _ = crate::ui_host::editor::open(
                        super::settings::settings_owner(ptr),
                        request,
                        Box::new(move |draft| {
                            if let Some(core) = crate::ui_host::app_core(host) {
                                (*core).add_file_search_ignore(draft.command);
                            }
                        }),
                        Box::new(|| {}),
                    );
                }
                _ => {}
            },

            SettingsTab::Clipboard => match id {
                10 => (*core).cycle_clipboard_retention(),
                index if (20..30).contains(&index) => {
                    (*core).remove_clipboard_disabled_app((index - 20) as usize);
                }
                30 => {
                    if let Some(stem) =
                        crate::features::clipboard::settings::pane::pick_application_stem(host)
                    {
                        (*core).add_clipboard_disabled_app(stem);
                    }
                }
                40 => confirm_clear_clipboard(ptr),
                _ => {}
            },

            SettingsTab::Ai => match id {
                10 => {
                    let enabled = (*core).settings.ai_enabled;
                    (*core).set_ai_enabled(!enabled);
                }
                11 => (*core).toggle_ai_web_search(),
                12 => (*core).toggle_ai_system_prompt(),
                13 => (*core).cycle_ai_opens_to(),
                14 => (*core).cycle_ai_retention(),
                15 => (*core).cycle_ai_default_model(),
                16 => (*core).chatgpt_row_action(),
                17 => {
                    if commit_ai_connection(ptr) {
                        (*core).add_ai_connection();
                        let last = (*core).settings.ai_connections.len().saturating_sub(1);
                        open_ai_editor(ptr, last);
                    }
                }
                index if (100..200).contains(&index) => {
                    if commit_ai_connection(ptr) {
                        open_ai_editor(ptr, (index - 100) as usize);
                    }
                }
                200 => {
                    if let Some(index) = (*ptr).ai_editing {
                        (*core).cycle_ai_connection(index);
                        open_ai_editor(ptr, index);
                    }
                }
                205 => discard_ai_editor(ptr),
                _ => {}
            },

            _ => {}
        }
    }
}

/// The destructive trailing button: only the AI connections use it.
pub(super) fn feature_trailing_activated(ptr: *mut Inner, id: i32) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    if unsafe { (*core).settings_tab } != SettingsTab::Ai {
        return;
    }
    if !(100..200).contains(&id) {
        return;
    }
    let index = (id - 100) as usize;
    let _ = commit_ai_connection(ptr);
    let conn_id = unsafe {
        let core = &*core;
        core.settings
            .ai_connections
            .get(index)
            .map(|c| c.id.as_str().to_string())
    };
    if let Some(conn_id) = conn_id {
        confirm_remove_ai_connection(ptr, conn_id);
    }
}

/// The AI editor's field rows write their drafts straight into `Inner`.
pub(super) fn feature_text_edited(ptr: *mut Inner, id: i32, text: String) {
    let inner = unsafe { &mut *ptr };
    match id {
        201 => inner.ai_url = text,
        202 => inner.ai_model = text,
        203 => inner.ai_key = text,
        _ => {}
    }
}

pub(super) fn feature_text_accepted(_ptr: *mut Inner, _id: i32) {
    // Enter in a field keeps the editor open; the commit rides the next
    // navigation, exactly like the Direct2D editor did.
}

unsafe fn open_ai_editor(ptr: *mut Inner, index: usize) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let inner = unsafe { &mut *ptr };
    let conn = unsafe { &*core }
        .settings
        .ai_connections
        .get(index)
        .cloned();
    let Some(conn) = conn else {
        discard_ai_editor(ptr);
        return;
    };
    inner.ai_editing = Some(index);
    inner.ai_error = None;
    inner.ai_url = conn.base_url;
    inner.ai_model = conn.models.first().cloned().unwrap_or_default();
    inner.ai_key = String::new();
    super::settings::invalidate_inner(ptr);
}

unsafe fn discard_ai_editor(ptr: *mut Inner) {
    let inner = unsafe { &mut *ptr };
    inner.ai_editing = None;
    inner.ai_error = None;
    super::settings::invalidate_inner(ptr);
}

/// Writes the field drafts through AppCore; `false` keeps the editor open and
/// shows the redacted error.
pub(super) fn commit_ai_connection(ptr: *mut Inner) -> bool {
    let Some(core) = core_of(ptr) else {
        return true;
    };
    let inner = unsafe { &mut *ptr };
    let Some(index) = inner.ai_editing else {
        inner.ai_error = None;
        return true;
    };
    let url = inner.ai_url.clone();
    let model = inner.ai_model.clone();
    let key = inner.ai_key.clone();
    unsafe {
        match (*core).apply_ai_connection_fields(index, url, model, key) {
            Ok(()) => {
                inner.ai_error = None;
                inner.ai_key = String::new();
                true
            }
            Err(err) => {
                let lang = (*core).ui_lang();
                inner.ai_error = Some(crate::features::ai::settings::pane::redact_ai_error(
                    &err, lang,
                ));
                false
            }
        }
    }
}

/// The tab moved: drop the alias draft and the AI editor. `commit_pending`
/// already wrote the field drafts before AppCore moved the tab.
pub(super) fn tab_changed(ptr: *mut Inner) {
    alias_cancelled(ptr);
    let inner = unsafe { &mut *ptr };
    inner.ai_editing = None;
    inner.ai_error = None;
}

/// The window is closing with an AI editor open: commit, never discard.
pub(super) fn commit_pending(ptr: *mut Inner) -> bool {
    commit_ai_connection(ptr)
}

fn confirm_disable_snippets(ptr: *mut Inner) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let lang = unsafe { (*core).ui_lang() };
    let prompt = crate::surfaces::dialog::ConfirmPrompt {
        title: tinycast_pure::i18n::snippets_confirm_title(lang).into(),
        message: tinycast_pure::i18n::snippets_confirm_message(lang).into(),
        accept: tinycast_pure::i18n::snippets_confirm_action(lang).into(),
        cancel: tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang).into(),
        destructive: false,
    };
    let host = unsafe { (*ptr).host };
    crate::surfaces::dialog::confirm(&prompt, move || {
        if let Some(core) = crate::ui_host::app_core(host) {
            unsafe {
                (*core).set_snippets_enabled(false);
            }
        }
    });
}

fn confirm_clear_clipboard(ptr: *mut Inner) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let lang = unsafe { (*core).ui_lang() };
    let prompt = crate::surfaces::dialog::ConfirmPrompt {
        title: tinycast_pure::i18n::clipboard_confirm_title(lang).into(),
        message: tinycast_pure::i18n::clipboard_confirm_message(lang).into(),
        accept: tinycast_pure::i18n::clipboard_confirm_action(lang).into(),
        cancel: tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang).into(),
        destructive: true,
    };
    let host = unsafe { (*ptr).host };
    crate::surfaces::dialog::confirm(&prompt, move || {
        if let Some(core) = crate::ui_host::app_core(host) {
            unsafe {
                (*core).clear_clipboard_history();
            }
        }
    });
}

fn confirm_remove_ai_connection(ptr: *mut Inner, conn_id: String) {
    let Some(core) = core_of(ptr) else {
        return;
    };
    let lang = unsafe { (*core).ui_lang() };
    let name = unsafe {
        (*core)
            .settings
            .ai_connections
            .iter()
            .find(|c| c.id.as_str() == conn_id)
            .map(|c| c.title())
            .unwrap_or_default()
    };
    let prompt = crate::surfaces::dialog::ConfirmPrompt {
        title: tinycast_pure::i18n::ai_remove_title(lang).into(),
        message: tinycast_pure::i18n::ai_remove_message(&name, lang).into(),
        accept: tinycast_pure::i18n::ai_remove_action(lang).into(),
        cancel: tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang).into(),
        destructive: true,
    };
    let host = unsafe { (*ptr).host };
    crate::surfaces::dialog::confirm(&prompt, move || {
        if let Some(core) = crate::ui_host::app_core(host) {
            unsafe {
                (*core).remove_ai_connection_id(&conn_id);
            }
        }
    });
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
