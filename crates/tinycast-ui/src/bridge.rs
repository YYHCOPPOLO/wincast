//! Fills the Slint `Theme` and `Motion` globals from the Rust design tokens.
//!
//! Every colour and metric lives in `tinycast_pure::theme` / `motion`; this
//! module is the only place that copies them into Slint.

use tinycast_pure::{motion, theme};

use crate::{Motion, Theme};

/// `0` is Dark and `1` is Light, matching `tinycast_pure::theme` and `AppCore`.
pub fn apply_appearance<C>(ui: &C, appearance: u8)
where
    C: slint::ComponentHandle,
    for<'a> Theme<'a>: slint::Global<'a, C>,
    for<'a> Motion<'a>: slint::Global<'a, C>,
{
    let theme_global: Theme<'_> = ui.global::<Theme>();
    let motion_global: Motion<'_> = ui.global::<Motion>();
    let t = &theme_global;
    let m = &motion_global;

    t.set_dark(appearance == 0);

    // Colors (`theme::colors`, `theme::settings_chrome`).
    t.set_scrim(rgba(theme::colors::scrim_rgba(appearance)));
    t.set_selection(ink(
        appearance,
        theme::colors::SELECTION_DARK_ALPHA,
        theme::colors::SELECTION_LIGHT_ALPHA,
    ));
    t.set_row_hover(ink(
        appearance,
        theme::colors::ROW_HOVER_DARK_ALPHA,
        theme::colors::ROW_HOVER_LIGHT_ALPHA,
    ));
    t.set_text_primary(ink(
        appearance,
        theme::colors::TEXT_PRIMARY_ALPHA,
        theme::colors::TEXT_PRIMARY_ALPHA,
    ));
    t.set_destructive(rgba(theme::colors::DESTRUCTIVE));
    t.set_text_secondary(ink(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    ));
    t.set_text_tertiary(ink(
        appearance,
        theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    ));
    t.set_control_surface(ink(
        appearance,
        theme::colors::CONTROL_SURFACE_DARK_ALPHA,
        theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
    ));
    t.set_border(ink(
        appearance,
        theme::colors::BORDER_DARK_ALPHA,
        theme::colors::BORDER_LIGHT_ALPHA,
    ));
    t.set_card_fill(ink(
        appearance,
        theme::colors::CARD_FILL_DARK_ALPHA,
        theme::colors::CARD_FILL_LIGHT_ALPHA,
    ));
    t.set_card_stroke(ink(
        appearance,
        theme::colors::CARD_STROKE_ALPHA,
        theme::colors::CARD_STROKE_ALPHA,
    ));
    t.set_separator(ink(
        appearance,
        theme::colors::SEPARATOR_DARK_ALPHA,
        theme::colors::SEPARATOR_LIGHT_ALPHA,
    ));
    t.set_menu_hover(ink(
        appearance,
        theme::colors::MENU_HOVER_DARK_ALPHA,
        theme::colors::MENU_HOVER_LIGHT_ALPHA,
    ));
    t.set_menu_fill(rgba(theme::colors::glass_tint_rgba(appearance)));
    t.set_control_hover(ink(
        appearance,
        theme::colors::CONTROL_HOVER_DARK_ALPHA,
        theme::colors::CONTROL_HOVER_LIGHT_ALPHA,
    ));
    t.set_control_pressed(ink(
        appearance,
        theme::colors::CONTROL_PRESSED_DARK_ALPHA,
        theme::colors::CONTROL_PRESSED_LIGHT_ALPHA,
    ));
    t.set_menu_symbol(ink(
        appearance,
        theme::colors::MENU_SYMBOL_ALPHA,
        theme::colors::MENU_SYMBOL_ALPHA,
    ));
    t.set_note_text(ink(
        appearance,
        theme::colors::NOTE_TEXT_DARK_ALPHA,
        theme::colors::NOTE_TEXT_LIGHT_ALPHA,
    ));
    t.set_icon_placeholder(ink(
        appearance,
        theme::colors::ICON_PLACEHOLDER_ALPHA,
        theme::colors::ICON_PLACEHOLDER_ALPHA,
    ));
    t.set_sheen(ink(
        appearance,
        theme::colors::SHEEN_ALPHA,
        theme::colors::SHEEN_ALPHA,
    ));
    t.set_emoji_cell(ink(
        appearance,
        theme::colors::EMOJI_CELL_DARK_ALPHA,
        theme::colors::EMOJI_CELL_LIGHT_ALPHA,
    ));
    t.set_emoji_hover_border(ink(
        appearance,
        theme::colors::EMOJI_HOVER_BORDER_DARK_ALPHA,
        theme::colors::EMOJI_HOVER_BORDER_LIGHT_ALPHA,
    ));
    t.set_emoji_selection_border(ink(
        appearance,
        theme::colors::EMOJI_SELECTION_BORDER_DARK_ALPHA,
        theme::colors::EMOJI_SELECTION_BORDER_LIGHT_ALPHA,
    ));
    t.set_dialog_dimming(black(theme::colors::DIALOG_DIMMING_ALPHA));
    t.set_tooltip_shadow(black(theme::colors::TOOLTIP_SHADOW_ALPHA));
    t.set_composer_send(ink(
        appearance,
        theme::colors::COMPOSER_SEND_ALPHA,
        theme::colors::COMPOSER_SEND_ALPHA,
    ));
    t.set_drop_guide(ink(
        appearance,
        theme::colors::DROP_GUIDE_ALPHA,
        theme::colors::DROP_GUIDE_ALPHA,
    ));
    t.set_settings_sidebar(solid(settings_sidebar(appearance)));
    t.set_settings_detail(solid(settings_detail(appearance)));

    // Typography (`theme::typography`), in DIP.
    t.set_font_search(theme::typography::SEARCH_FIELD);
    t.set_font_header_icon(theme::typography::HEADER_ICON);
    t.set_font_row(theme::typography::ROW_TITLE);
    t.set_font_row_trailing(theme::typography::ROW_TRAILING);
    t.set_font_bar(theme::typography::BAR);
    t.set_font_section(theme::typography::SECTION_HEADER);
    t.set_font_keycap(theme::typography::KEY_CAP);
    t.set_font_panel_title(theme::typography::PANEL_TITLE);
    t.set_font_calc_result(theme::typography::CALC_RESULT);
    t.set_font_calc_badge(theme::typography::CALC_BADGE);
    t.set_font_chip(theme::typography::CHIP);
    t.set_font_emoji(theme::typography::EMOJI);

    // Spacing (`theme::spacing`).
    t.set_spacing_xxs(theme::spacing::XXS);
    t.set_spacing_xs(theme::spacing::XS);
    t.set_spacing_sm(theme::spacing::SM);
    t.set_spacing_md(theme::spacing::MD);
    t.set_spacing_lg(theme::spacing::LG);
    t.set_spacing_xl(theme::spacing::XL);
    t.set_spacing_xxl(theme::spacing::XXL);
    t.set_spacing_xxxl(theme::spacing::XXXL);
    t.set_spacing_dialog_inset(theme::spacing::DIALOG_INSET);
    t.set_spacing_dialog_content_inset(theme::spacing::DIALOG_CONTENT_INSET);
    t.set_spacing_section_header_bottom(theme::spacing::SECTION_HEADER_BOTTOM);
    t.set_spacing_section(theme::spacing::SECTION_SPACING);
    t.set_spacing_emoji_section(theme::spacing::EMOJI_SECTION_SPACING);
    t.set_spacing_chat_line(theme::spacing::CHAT_LINE);

    // Corner radii (`theme::radius`).
    t.set_radius_panel(theme::radius::PANEL);
    t.set_radius_row(theme::radius::ROW);
    t.set_radius_menu(theme::radius::MENU);
    t.set_radius_menu_row(theme::radius::MENU_ROW);
    t.set_radius_menu_panel(theme::radius::MENU_PANEL);
    t.set_radius_dialog(theme::radius::DIALOG);
    t.set_radius_dialog_surface(theme::radius::DIALOG_SURFACE);
    t.set_radius_dialog_symbol(theme::radius::DIALOG_SYMBOL);
    t.set_radius_emoji_cell(theme::radius::EMOJI_CELL);
    t.set_radius_bar_control(theme::radius::BAR_CONTROL);
    t.set_radius_thumbnail(theme::radius::THUMBNAIL);
    t.set_radius_glyph(theme::radius::GLYPH);
    t.set_radius_attachment_chip(theme::radius::ATTACHMENT_CHIP);
    t.set_radius_card(theme::radius::CARD);
    t.set_radius_key_cap(theme::radius::KEY_CAP);
    t.set_radius_recorder_key_cap(theme::radius::RECORDER_KEY_CAP);
    t.set_radius_tooltip(theme::radius::TOOLTIP);

    // Sizes (`theme::size`).
    t.set_size_panel_width(theme::size::PANEL_WIDTH);
    t.set_size_panel_height(theme::size::PANEL_HEIGHT);
    t.set_size_header_height(theme::size::HEADER_HEIGHT);
    t.set_size_header_icon_slot(theme::size::HEADER_ICON_SLOT);
    t.set_size_header_padding(theme::size::HEADER_PADDING);
    t.set_size_compact_height(theme::size::COMPACT_HEIGHT);
    t.set_size_palette_snap_distance(theme::size::PALETTE_SNAP_DISTANCE);
    t.set_size_palette_minimum_visible(theme::size::PALETTE_MINIMUM_VISIBLE);
    t.set_size_palette_top_margin_fraction(theme::size::PALETTE_TOP_MARGIN_FRACTION);
    t.set_size_drop_guide_dash(theme::size::DROP_GUIDE_DASH);
    t.set_size_drop_guide_gap(theme::size::DROP_GUIDE_GAP);
    t.set_size_drop_guide_width(theme::size::DROP_GUIDE_WIDTH);
    t.set_size_bottom_bar_height(theme::size::BOTTOM_BAR_HEIGHT);
    t.set_size_bar_button_height(theme::size::BAR_BUTTON_HEIGHT);
    t.set_size_bar_menu_line_long(theme::size::BAR_MENU_LINE_LONG);
    t.set_size_bar_menu_line_short(theme::size::BAR_MENU_LINE_SHORT);
    t.set_size_bar_menu_line(theme::size::BAR_MENU_LINE);
    t.set_size_bar_circle(theme::size::MENU_BUTTON);
    t.set_size_row_height(theme::size::ROW_HEIGHT);
    t.set_size_section_header_height(theme::size::SECTION_HEADER_HEIGHT);
    t.set_size_row_icon(theme::size::ROW_ICON);
    t.set_size_result_row_icon(theme::size::RESULT_ROW_ICON);
    t.set_size_row_padding_x(theme::spacing::MD);
    t.set_size_row_padding_y(theme::spacing::SM);
    t.set_size_row_content_left(tinycast_pure::layout::list::row_content_left());
    t.set_size_row_icon_gap(theme::spacing::LG);
    t.set_size_scrollbar_width(theme::size::SCROLLBAR_WIDTH);
    t.set_size_scrollbar_hover_width(theme::size::SCROLLBAR_HOVER_WIDTH);
    t.set_size_scrollbar_min_thumb(theme::size::SCROLLBAR_MIN_THUMB);
    t.set_size_scrollbar_inset(theme::size::SCROLLBAR_INSET);
    t.set_size_scrollbar_hover_zone(theme::size::SCROLLBAR_HOVER_ZONE);
    t.set_size_empty_results_glyph(theme::size::EMPTY_RESULTS_GLYPH);
    t.set_size_key_cap(theme::size::KEY_CAP);
    t.set_size_compact_key_cap(theme::size::COMPACT_KEY_CAP);
    t.set_size_hero_key_cap(theme::size::HERO_KEY_CAP);
    t.set_size_shortcut_recorder(theme::size::SHORTCUT_RECORDER);
    t.set_size_shortcut_popover_line(theme::size::SHORTCUT_POPOVER_LINE);
    t.set_size_callout_caret_width(theme::size::CALLOUT_CARET_WIDTH);
    t.set_size_callout_caret_height(theme::size::CALLOUT_CARET_HEIGHT);
    t.set_size_callout_caret_tip(theme::size::CALLOUT_CARET_TIP);
    t.set_size_shortcut_popover_width(theme::size::SHORTCUT_POPOVER.0);
    t.set_size_shortcut_popover_height(theme::size::SHORTCUT_POPOVER.1);
    t.set_size_menu_button(theme::size::MENU_BUTTON);
    t.set_size_menu_width(theme::size::MENU_WIDTH);
    t.set_size_action_menu_width(theme::size::ACTION_MENU_WIDTH);
    t.set_size_clipboard_filter_menu_width(theme::size::CLIPBOARD_FILTER_MENU_WIDTH);
    t.set_size_file_search_filter_menu_width(theme::size::FILE_SEARCH_FILTER_MENU_WIDTH);
    t.set_size_emoji_category_menu_width(theme::size::EMOJI_CATEGORY_MENU_WIDTH);
    t.set_size_menu_row_height(theme::size::MENU_ROW_HEIGHT);
    t.set_size_menu_row_spacing(theme::size::MENU_ROW_SPACING);
    t.set_size_menu_section_header(theme::size::MENU_SECTION_HEADER);
    t.set_size_menu_visible_rows(theme::size::MENU_VISIBLE_ROWS);
    t.set_size_menu_icon(theme::size::MENU_ICON);
    t.set_size_menu_brand_icon(theme::size::MENU_BRAND_ICON);
    t.set_size_bar_brand_icon(theme::size::BAR_BRAND_ICON);
    t.set_size_calc_card_height(theme::size::CALC_CARD_HEIGHT);
    t.set_size_chat_image_thumb(theme::size::CHAT_IMAGE_THUMB);
    t.set_size_chat_attachment_glyph(theme::size::CHAT_ATTACHMENT_GLYPH);
    t.set_size_settings_window_width(theme::size::SETTINGS_WINDOW.0);
    t.set_size_settings_window_height(theme::size::SETTINGS_WINDOW.1);
    t.set_size_settings_sidebar(theme::size::SETTINGS_SIDEBAR);
    t.set_size_settings_detail_minimum(theme::size::SETTINGS_DETAIL_MINIMUM);
    t.set_size_settings_row_icon(theme::size::SETTINGS_ROW_ICON);
    t.set_size_editor_sheet_width(theme::size::EDITOR_SHEET_WIDTH);
    t.set_size_form_label_width(theme::size::FORM_LABEL_WIDTH);
    t.set_size_editor_text_height(theme::size::EDITOR_TEXT_HEIGHT);
    t.set_size_argument_prompt_width(theme::size::ARGUMENT_PROMPT_WIDTH);
    t.set_size_hud_max_width(theme::size::HUD_MAX_WIDTH);
    t.set_size_hud_edge_offset(theme::size::HUD_EDGE_OFFSET);
    t.set_size_hud_width(theme::size::HUD_WIDTH);
    t.set_size_hud_height(theme::size::HUD_HEIGHT);
    t.set_size_dialog_compact_width(theme::size::DIALOG_COMPACT_WIDTH);
    t.set_size_dialog_width(theme::size::DIALOG_WIDTH);
    t.set_size_dialog_button_height(theme::size::DIALOG_BUTTON_HEIGHT);
    t.set_size_dialog_symbol(theme::size::DIALOG_SYMBOL);
    t.set_size_dialog_symbol_container(theme::size::DIALOG_SYMBOL_CONTAINER);
    t.set_size_dialog_icon(theme::size::DIALOG_ICON);
    t.set_size_camera_preview_width(theme::size::CAMERA_PREVIEW.0);
    t.set_size_camera_preview_height(theme::size::CAMERA_PREVIEW.1);
    t.set_size_quick_action_panel(theme::size::QUICK_ACTION_PANEL);
    t.set_size_quick_action_header_icon(theme::size::QUICK_ACTION_HEADER_ICON);
    t.set_size_quick_action_scroll_fade(theme::size::QUICK_ACTION_SCROLL_FADE);
    t.set_size_quick_action_panel_body(theme::size::QUICK_ACTION_PANEL_BODY);
    t.set_size_quick_action_panel_min_body(theme::size::QUICK_ACTION_PANEL_MIN_BODY);
    t.set_size_volume_track(theme::size::VOLUME_TRACK_HEIGHT);
    t.set_size_volume_knob(theme::size::VOLUME_KNOB);
    t.set_size_volume_readout(theme::size::VOLUME_READOUT);
    t.set_size_note_window_width(theme::size::NOTE_WINDOW.0);
    t.set_size_note_window_height(theme::size::NOTE_WINDOW.1);
    t.set_size_note_editor_inset(theme::size::NOTE_EDITOR_INSET);
    t.set_size_note_editor_top_inset(theme::size::NOTE_EDITOR_TOP_INSET);
    t.set_size_note_search_height(theme::size::NOTE_SEARCH_HEIGHT);
    t.set_size_note_switcher_width(theme::size::NOTE_SWITCHER.0);
    t.set_size_note_switcher_height(theme::size::NOTE_SWITCHER.1);
    t.set_size_note_switcher_empty_height(theme::size::NOTE_SWITCHER_EMPTY_HEIGHT);
    t.set_size_note_switcher_drop(theme::size::NOTE_SWITCHER_DROP);
    t.set_size_note_footer(theme::size::NOTE_FOOTER_HEIGHT);
    t.set_size_note_titlebar(theme::size::NOTE_TITLEBAR);
    t.set_size_note_title_inset(theme::size::NOTE_TITLE_INSET);
    t.set_size_note_traffic_light_inset(theme::size::NOTE_TRAFFIC_LIGHT_INSET);
    t.set_size_note_glyph(theme::size::NOTE_GLYPH);
    t.set_size_note_empty_glyph(theme::size::NOTE_EMPTY_GLYPH);
    t.set_size_emoji_cell(theme::size::EMOJI_CELL);
    t.set_size_emoji_grid_inset(theme::size::EMOJI_GRID_INSET);
    t.set_size_clipboard_list_width(theme::size::CLIPBOARD_LIST_WIDTH);
    t.set_size_checkbox(theme::size::CHECKBOX);
    t.set_size_toggle_width(theme::size::TOGGLE_WIDTH);
    t.set_size_toggle_height(theme::size::TOGGLE_HEIGHT);
    t.set_size_sidebar_row(theme::size::SIDEBAR_ROW_HEIGHT);
    t.set_size_hairline(theme::size::HAIRLINE);
    t.set_size_chat_message_action(theme::size::CHAT_MESSAGE_ACTION);
    t.set_size_markdown_list_marker(theme::size::MARKDOWN_LIST_MARKER);
    t.set_size_markdown_quote_bar(theme::size::MARKDOWN_QUOTE_BAR);

    // Edge dissolve bands (`theme::fade`).
    t.set_fade_edge_top_overshoot(theme::fade::EDGE_TOP_OVERSHOOT);
    t.set_fade_edge_bottom_overshoot(theme::fade::EDGE_BOTTOM_OVERSHOOT);
    t.set_fade_edge_top_floor(theme::fade::EDGE_TOP_FLOOR);
    t.set_fade_edge_bottom_floor(theme::fade::EDGE_BOTTOM_FLOOR);
    t.set_fade_edge_top_band(theme::fade::edge_top_band());
    t.set_fade_edge_bottom_band(theme::fade::edge_bottom_band());
    t.set_fade_overflow_band(theme::fade::OVERFLOW_BAND);
    t.set_fade_popup_band(theme::fade::POPUP_BAND);

    // Thin scrollbar alphas (`theme::scrollbar`).
    t.set_scrollbar_rest_alpha(theme::scrollbar::THUMB_REST_ALPHA);
    t.set_scrollbar_hover_alpha(theme::scrollbar::THUMB_HOVER_ALPHA);
    t.set_scrollbar_drag_alpha(theme::scrollbar::THUMB_DRAG_ALPHA);

    // Durations (`theme::duration`, `theme::menu_motion`).
    t.set_duration_enter(secs(theme::duration::ENTER_SECS));
    t.set_duration_exit(secs(theme::duration::EXIT_SECS));
    t.set_duration_dialog_enter(secs(theme::duration::DIALOG_ENTER_SECS));
    t.set_duration_dialog_exit(secs(theme::duration::DIALOG_EXIT_SECS));
    t.set_duration_tooltip(secs(theme::duration::TOOLTIP_SECS));
    t.set_duration_tooltip_delay(secs(theme::duration::TOOLTIP_DELAY_SECS));
    t.set_duration_hover(secs(theme::duration::HOVER_SECS));
    t.set_duration_menu_chevron(secs(theme::duration::MENU_CHEVRON_SECS));
    t.set_duration_drop_guide(secs(theme::duration::DROP_GUIDE_SECS));
    t.set_duration_copy_feedback(secs(theme::duration::COPY_FEEDBACK_SECS));
    t.set_duration_chat_footer(secs(theme::duration::CHAT_FOOTER_SECS));
    t.set_duration_scrollbar_fade(secs(theme::duration::SCROLLBAR_FADE_SECS));
    t.set_duration_scrollbar_idle(secs(theme::duration::SCROLLBAR_IDLE_SECS));
    t.set_duration_scrollbar_hover_exit(secs(theme::duration::SCROLLBAR_HOVER_EXIT_SECS));
    t.set_duration_scrollbar_spring(secs(theme::duration::SCROLLBAR_SPRING_RESPONSE));
    t.set_duration_toggle_spring(secs(theme::duration::TOGGLE_SPRING_RESPONSE));
    t.set_duration_onboarding(secs(theme::duration::ONBOARDING_SECS));
    t.set_duration_shortcut_hover(secs(theme::duration::SHORTCUT_HOVER_SECS));
    t.set_duration_shortcut_popover(secs(theme::duration::SHORTCUT_POPOVER_SECS));
    t.set_duration_toast(secs(theme::duration::TOAST_SECS));
    t.set_duration_message_hud(secs(theme::duration::MESSAGE_HUD_SECS));
    t.set_duration_volume_hud(secs(theme::duration::VOLUME_HUD_SECS));
    t.set_duration_menu_open(secs(
        theme::menu_motion::EXPANSION_SECS + theme::menu_motion::SETTLE_SECS,
    ));
    t.set_duration_menu_exit(secs(theme::menu_motion::EXIT_SECS));

    // Motion (`tinycast_pure::motion`).
    m.set_enter_scale(motion::ENTER_SCALE);
    m.set_callout_scale(motion::CALLOUT_SCALE);
    m.set_enter(secs(theme::duration::ENTER_SECS));
    m.set_exit(secs(theme::duration::EXIT_SECS));
    m.set_dialog_enter(secs(theme::duration::DIALOG_ENTER_SECS));
    m.set_dialog_exit(secs(theme::duration::DIALOG_EXIT_SECS));
    m.set_dialog_offset(theme::dialog_motion::OFFSET);
    m.set_dialog_initial_opacity(theme::dialog_motion::INITIAL_OPACITY);
    m.set_menu_entry_scale(theme::menu_motion::ENTRY_SCALE);
    m.set_menu_max_scale(theme::menu_motion::MAXIMUM_SCALE);
    m.set_menu_exit_scale_delta(theme::menu_motion::EXIT_SCALE_DELTA);
    m.set_menu_expansion(secs(theme::menu_motion::EXPANSION_SECS));
    m.set_menu_settle(secs(theme::menu_motion::SETTLE_SECS));
    m.set_menu_exit(secs(theme::menu_motion::EXIT_SECS));
    m.set_menu_expansion_x1(theme::menu_motion::EXPANSION_CURVE.0);
    m.set_menu_expansion_y1(theme::menu_motion::EXPANSION_CURVE.1);
    m.set_menu_expansion_x2(theme::menu_motion::EXPANSION_CURVE.2);
    m.set_menu_expansion_y2(theme::menu_motion::EXPANSION_CURVE.3);
    m.set_menu_settle_x1(theme::menu_motion::SETTLE_CURVE.0);
    m.set_menu_settle_y1(theme::menu_motion::SETTLE_CURVE.1);
    m.set_menu_settle_x2(theme::menu_motion::SETTLE_CURVE.2);
    m.set_menu_settle_y2(theme::menu_motion::SETTLE_CURVE.3);
    m.set_menu_exit_x1(theme::menu_motion::EXIT_CURVE.0);
    m.set_menu_exit_y1(theme::menu_motion::EXIT_CURVE.1);
    m.set_menu_exit_x2(theme::menu_motion::EXIT_CURVE.2);
    m.set_menu_exit_y2(theme::menu_motion::EXIT_CURVE.3);
    m.set_menu_chevron_x1(theme::menu_motion::CHEVRON_CURVE.0);
    m.set_menu_chevron_y1(theme::menu_motion::CHEVRON_CURVE.1);
    m.set_menu_chevron_x2(theme::menu_motion::CHEVRON_CURVE.2);
    m.set_menu_chevron_y2(theme::menu_motion::CHEVRON_CURVE.3);
    m.set_scrollbar_spring_bounce(1.0 - theme::duration::SCROLLBAR_SPRING_DAMPING);
    m.set_scrollbar_spring(secs(theme::duration::SCROLLBAR_SPRING_RESPONSE));
    m.set_toggle_spring_bounce(1.0 - theme::duration::TOGGLE_SPRING_DAMPING);
    m.set_toggle_spring(secs(theme::duration::TOGGLE_SPRING_RESPONSE));
    m.set_hud_message(secs(theme::duration::MESSAGE_HUD_SECS));
    m.set_hud_volume(secs(theme::duration::VOLUME_HUD_SECS));
}

fn ink(appearance: u8, dark_alpha: f32, light_alpha: f32) -> slint::Color {
    rgba(theme::colors::ramp_rgba(
        appearance,
        dark_alpha,
        light_alpha,
    ))
}

fn black(alpha: f32) -> slint::Color {
    slint::Color::from_argb_f32(alpha, 0.0, 0.0, 0.0)
}

fn rgba((red, green, blue, alpha): (f32, f32, f32, f32)) -> slint::Color {
    slint::Color::from_argb_f32(alpha, red, green, blue)
}

fn solid((red, green, blue): (f32, f32, f32)) -> slint::Color {
    slint::Color::from_rgb_f32(red, green, blue)
}

fn settings_sidebar(appearance: u8) -> (f32, f32, f32) {
    if appearance == 0 {
        theme::settings_chrome::SIDEBAR_DARK
    } else {
        theme::settings_chrome::SIDEBAR_LIGHT
    }
}

fn settings_detail(appearance: u8) -> (f32, f32, f32) {
    if appearance == 0 {
        theme::settings_chrome::DETAIL_DARK
    } else {
        theme::settings_chrome::DETAIL_LIGHT
    }
}

/// Slint stores `duration` as whole milliseconds.
fn secs(seconds: f32) -> i64 {
    (seconds * 1000.0).round() as i64
}
