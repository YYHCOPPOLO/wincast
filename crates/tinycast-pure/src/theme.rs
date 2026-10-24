//! Design tokens, mirrored from upstream Tinycast `Tinycast/DesignSystem/Theme.swift`
//! at `abue-ammar/tinycast@da2c4e44` (the UI oracle since P0). Line numbers in the
//! comments below cite that file unless another path is named. The previous oracle
//! was tag v0.10.2; a value that moved since then says so in its comment.

pub mod spacing {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 10.0;
    pub const XL: f32 = 12.0;
    /// `Spacing.dialogInset` (Theme.swift:15; `Windows/Dialog/DialogView.swift:60`).
    pub const DIALOG_INSET: f32 = 18.0;
    /// Dialog content sits a further `xs` in (`DialogView.swift:53-54`), so 22 in all.
    pub const DIALOG_CONTENT_INSET: f32 = DIALOG_INSET + XS;
    pub const XXL: f32 = 20.0;
    pub const XXXL: f32 = 28.0;
    pub const SECTION_HEADER_BOTTOM: f32 = 4.0;
    pub const SECTION_SPACING: f32 = 12.0;
    /// `Spacing.emojiSectionSpacing` (Theme.swift:30).
    pub const EMOJI_SECTION_SPACING: f32 = 14.0;
    /// `Spacing.chatLine`: transcript line spacing.
    pub const CHAT_LINE: f32 = 4.0;
}

pub mod radius {
    pub const PANEL: f32 = 26.0;
    pub const ROW: f32 = 10.0;
    pub const MENU: f32 = 6.0;
    pub const MENU_ROW: f32 = 10.0;
    pub const MENU_PANEL: f32 = 16.0;
    /// `Radius.dialog` (Theme.swift:47). Still 20 upstream, but since the Liquid Glass
    /// dialogs (#1046) it only dresses the HUD box, Quick Action panel and AI composer.
    pub const DIALOG: f32 = 20.0;
    /// A dialog's own surface clips to `Radius.panel`
    /// (`Windows/Dialog/DialogView.swift:29`, `DialogController.swift:128`). v0.10.2: 20.
    pub const DIALOG_SURFACE: f32 = PANEL;
    /// `Radius.dialogSymbol` (Theme.swift:45): the subject tile inside a dialog.
    pub const DIALOG_SYMBOL: f32 = 16.0;
    /// `Radius.emojiCell` (Theme.swift:37).
    pub const EMOJI_CELL: f32 = 12.0;
    /// `Radius.barControl` (Theme.swift:42): header pop-ups (type filter, AI model).
    pub const BAR_CONTROL: f32 = 8.0;
    pub const THUMBNAIL: f32 = 6.0;
    /// `Radius.glyph` (Theme.swift:50).
    pub const GLYPH: f32 = 2.0;
    /// `Radius.attachmentChip` (Theme.swift:52).
    pub const ATTACHMENT_CHIP: f32 = 8.0;
    pub const CARD: f32 = 10.0;
    pub const KEY_CAP: f32 = 6.0;
    pub const RECORDER_KEY_CAP: f32 = 4.0;
    /// `Radius.tooltip` (Theme.swift:59).
    pub const TOOLTIP: f32 = 8.0;
    /// Slint-side Magpie shape: a flat 16 DIP card for panels, menus and
    /// dialogs. The Direct2D constants above keep their upstream values until
    /// phase 9 deletes that path.
    pub const MAGPIE_PANEL: f32 = 16.0;
    pub const MAGPIE_DIALOG: f32 = 16.0;
    /// Magpie settings list card (`app.css` `.list`: 12).
    pub const MAGPIE_CARD: f32 = 12.0;
    /// Magpie controls (`app.css` `.field` 6-7, inputs 7, wells 8): 8.
    pub const MAGPIE_CONTROL: f32 = 8.0;
}

pub mod size {
    pub const PANEL_WIDTH: f32 = 750.0;
    pub const PANEL_HEIGHT: f32 = 475.0;
    pub const NOTE_WINDOW: (f32, f32) = (440.0, 180.0);
    pub const NOTE_EDITOR_INSET: f32 = 16.0;
    pub const NOTE_EDITOR_TOP_INSET: f32 = 6.0;
    pub const NOTE_SEARCH_HEIGHT: f32 = 34.0;
    pub const NOTE_SWITCHER: (f32, f32) = (300.0, 240.0);
    pub const NOTE_SWITCHER_EMPTY_HEIGHT: f32 = 96.0;
    pub const NOTE_SWITCHER_DROP: f32 = 56.0;
    pub const NOTE_FOOTER_HEIGHT: f32 = 28.0;
    pub const NOTE_TITLEBAR: f32 = 52.0;
    pub const NOTE_TITLE_INSET: f32 = 120.0;
    pub const NOTE_TRAFFIC_LIGHT_INSET: f32 = 20.0;
    pub const PALETTE_TOP_MARGIN_FRACTION: f32 = 0.18;
    pub const HEADER_HEIGHT: f32 = 44.0;
    pub const HEADER_ICON_SLOT: f32 = 22.0;
    pub const HEADER_PADDING: f32 = 10.0;
    pub const COMPACT_HEIGHT: f32 = 64.0; // HEADER_HEIGHT + HEADER_PADDING * 2
    /// `Size.paletteSnapDistance` (Theme.swift:104). v0.10.2: 24.
    pub const PALETTE_SNAP_DISTANCE: f32 = 8.0;
    pub const PALETTE_MINIMUM_VISIBLE: f32 = 44.0;
    /// `Size.dropGuideDash` (Theme.swift:107). v0.10.2: 4.
    pub const DROP_GUIDE_DASH: f32 = 8.0;
    /// `Size.dropGuideGap` (Theme.swift:108).
    pub const DROP_GUIDE_GAP: f32 = 12.0;
    pub const DROP_GUIDE_WIDTH: f32 = 2.0;
    pub const BOTTOM_BAR_HEIGHT: f32 = 52.0;
    pub const BAR_BUTTON_HEIGHT: f32 = 28.0;
    /// The launcher glyph inside the footer's 36 DIP circle: two pills.
    pub const BAR_MENU_LINE_LONG: f32 = 14.0;
    pub const BAR_MENU_LINE_SHORT: f32 = 8.0;
    pub const BAR_MENU_LINE: f32 = 1.5;
    /// `Size.rowIcon` (Theme.swift:116): Settings rows and compact favorites.
    pub const ROW_ICON: f32 = 24.0;
    /// `Size.resultRowIcon` (Theme.swift:117): every palette result slot, whatever fills it.
    /// v0.10.2 had no separate token and used 24.
    pub const RESULT_ROW_ICON: f32 = 26.0;
    /// Palette result row: `resultRowIcon` plus the row's `.vertical sm` padding.
    /// `layout::list::ROW_HEIGHT` reuses this token, so both renderers agree on 38.
    pub const ROW_HEIGHT: f32 = RESULT_ROW_ICON + super::spacing::SM * 2.0;
    /// Every palette section header; `features/launcher/ui/list.rs` uses the same 22.
    pub const SECTION_HEADER_HEIGHT: f32 = 22.0;
    pub const KEY_CAP: f32 = 18.0;
    pub const RECORDER_KEY_CAP: f32 = 16.0;
    pub const SHORTCUT_RECORDER: f32 = 120.0;
    pub const SHORTCUT_POPOVER_LINE: f32 = 14.0;
    pub const CALLOUT_CARET_WIDTH: f32 = 15.0;
    pub const CALLOUT_CARET_HEIGHT: f32 = 7.0;
    pub const CALLOUT_CARET_TIP: f32 = 2.5;
    pub const COMPACT_KEY_CAP: f32 = 15.0;
    pub const HERO_KEY_CAP: f32 = 22.0;
    // sm*2 + hero + sm + line + sm + compact + caret
    pub const SHORTCUT_POPOVER: (f32, f32) = (
        132.0,
        super::spacing::SM * 2.0
            + HERO_KEY_CAP
            + super::spacing::SM
            + SHORTCUT_POPOVER_LINE
            + super::spacing::SM
            + COMPACT_KEY_CAP
            + CALLOUT_CARET_HEIGHT,
    );
    pub const MENU_BUTTON: f32 = 36.0;
    pub const NOTE_GLYPH: f32 = 16.0;
    pub const NOTE_EMPTY_GLYPH: f32 = 28.0;
    pub const CHAT_MESSAGE_ACTION: f32 = 16.0;
    pub const HAIRLINE: f32 = 1.0;
    /// `design_system/scrollbar.rs` `THIN` / `EXPANDED` / `MIN_THUMB`.
    pub const SCROLLBAR_WIDTH: f32 = 6.0;
    pub const SCROLLBAR_HOVER_WIDTH: f32 = 10.0;
    pub const SCROLLBAR_MIN_THUMB: f32 = 28.0;
    /// `design_system/scrollbar.rs` `INSET`: gap between the rail and the panel edge.
    pub const SCROLLBAR_INSET: f32 = 3.0;
    /// `design_system/scrollbar.rs` `HOVER_ZONE`: the rail is 6 DIP, its target is 16.
    pub const SCROLLBAR_HOVER_ZONE: f32 = 16.0;
    /// `features/launcher/ui/list.rs::paint_empty_results`: glyph above the empty text.
    pub const EMPTY_RESULTS_GLYPH: f32 = 32.0;
    pub const MARKDOWN_LIST_MARKER: f32 = 20.0;
    pub const MARKDOWN_QUOTE_BAR: f32 = 2.0;
    /// Magpie checkbox (`app.css` `input[type="checkbox"]`): 14.
    pub const CHECKBOX: f32 = 14.0;
    /// Settings switch (magpie `library.css` `.lib-switch`): 32 x 19.
    pub const TOGGLE_WIDTH: f32 = 32.0;
    pub const TOGGLE_HEIGHT: f32 = 19.0;
    /// Magpie value select button (`app.css` `.field`): 25.
    pub const MAGPIE_FIELD_HEIGHT: f32 = 25.0;
    /// Magpie single-line form input (`app.css` `.editor input`): 27.
    pub const MAGPIE_INPUT_HEIGHT: f32 = 27.0;
    /// Dense settings list row (the old panes' 36 DIP item rows).
    pub const MAGPIE_LIST_ITEM: f32 = 36.0;
    /// The palette's search field box (magpie picker input): 40 in the 44
    /// DIP header band.
    pub const MAGPIE_SEARCH_HEIGHT: f32 = 40.0;
    /// Settings sidebar tab row; `layout::settings` reuses this token.
    pub const SIDEBAR_ROW_HEIGHT: f32 = 32.0;
    pub const CLIPBOARD_LIST_WIDTH: f32 = 290.0;
    /// A clipboard image row: tall enough for the 52 DIP thumbnail plus the
    /// row's 6 DIP vertical padding.
    pub const CLIPBOARD_IMAGE_ROW_HEIGHT: f32 = 64.0;
    /// `Size.emojiGridInset` (Theme.swift:160).
    pub const EMOJI_GRID_INSET: f32 = 16.0;
    pub const EMOJI_CELL: f32 = 56.0;
    pub const MENU_WIDTH: f32 = 276.0;
    /// `Size.actionMenuWidth` (Theme.swift:163): the ⌘K actions menu.
    pub const ACTION_MENU_WIDTH: f32 = 320.0;
    pub const CLIPBOARD_FILTER_MENU_WIDTH: f32 = 200.0;
    /// `Size.fileSearchFilterMenuWidth` (Theme.swift:166).
    pub const FILE_SEARCH_FILTER_MENU_WIDTH: f32 = 200.0;
    /// `Size.emojiCategoryMenuWidth` (Theme.swift:168).
    pub const EMOJI_CATEGORY_MENU_WIDTH: f32 = 220.0;
    /// `Size.menuRowHeight = menuIcon + Spacing.md * 2` (Theme.swift:170).
    pub const MENU_ROW_HEIGHT: f32 = MENU_ICON + super::spacing::MD * 2.0;
    /// `Size.menuRowSpacing` (Theme.swift:171).
    pub const MENU_ROW_SPACING: f32 = 1.0;
    /// `Size.menuSectionHeader` (Theme.swift:173).
    pub const MENU_SECTION_HEADER: f32 = 16.0;
    /// `Size.menuVisibleRows` (Theme.swift:175): a menu scrolls past five and a half rows.
    pub const MENU_VISIBLE_ROWS: f32 = 5.5;
    pub const MENU_ICON: f32 = 20.0;
    pub const MENU_BRAND_ICON: f32 = 14.0;
    pub const BAR_BRAND_ICON: f32 = 12.0;
    pub const CALC_CARD_HEIGHT: f32 = 96.0;
    pub const CHAT_IMAGE_THUMB: f32 = 96.0;
    pub const CHAT_ATTACHMENT_GLYPH: f32 = 16.0;
    /// `Size.settingsWindow` (Theme.swift:203). v0.10.2: 860 × 700.
    pub const SETTINGS_WINDOW: (f32, f32) = (900.0, 700.0);
    pub const SETTINGS_SIDEBAR: f32 = 215.0;
    pub const SETTINGS_DETAIL_MINIMUM: f32 = 420.0;
    pub const SETTINGS_ROW_ICON: f32 = 20.0;
    // One settings page row (design_system/settings.rs ROW_H).
    /// Settings page row (magpie `app.css` `.row` window min-height): 48.
    pub const SETTINGS_ROW_HEIGHT: f32 = 48.0;
    pub const EDITOR_SHEET_WIDTH: f32 = 480.0;
    pub const FORM_LABEL_WIDTH: f32 = 110.0;
    pub const EDITOR_TEXT_HEIGHT: f32 = 120.0;
    pub const ARGUMENT_PROMPT_WIDTH: f32 = 220.0;
    pub const HUD_MAX_WIDTH: f32 = 420.0;
    pub const HUD_EDGE_OFFSET: f32 = 48.0;
    /// `Size.dialogCompactWidth` (Theme.swift:271): confirmations, notices, the volume prompt.
    pub const DIALOG_COMPACT_WIDTH: f32 = 290.0;
    /// `Size.dialogWidth` (Theme.swift:273): form dialogs (event draft, snippet arguments).
    pub const DIALOG_WIDTH: f32 = 420.0;
    /// `Size.dialogButtonHeight = menuButton - 2` (Theme.swift:274). Buttons, fields, segments.
    pub const DIALOG_BUTTON_HEIGHT: f32 = MENU_BUTTON - 2.0;
    /// `Size.dialogSymbol` (Theme.swift:276).
    pub const DIALOG_SYMBOL: f32 = 28.0;
    /// `Size.dialogSymbolContainer` (Theme.swift:277).
    pub const DIALOG_SYMBOL_CONTAINER: f32 = 52.0;
    pub const DIALOG_ICON: f32 = 32.0;
    pub const CAMERA_PREVIEW: (f32, f32) = (420.0, 236.0);
    /// The Support/About window: client size and the heart tile.
    pub const SUPPORT_WINDOW: (f32, f32) = (460.0, 360.0);
    pub const SUPPORT_ICON: f32 = 76.0;
    pub const QUICK_ACTION_PANEL: f32 = 520.0;
    pub const QUICK_ACTION_HEADER_ICON: f32 = 14.0;
    pub const QUICK_ACTION_SCROLL_FADE: f32 = 40.0;
    pub const QUICK_ACTION_PANEL_BODY: f32 = 320.0;
    pub const QUICK_ACTION_PANEL_MIN_BODY: f32 = 44.0;
    pub const HUD_WIDTH: f32 = 200.0;
    pub const HUD_HEIGHT: f32 = 100.0;
    pub const VOLUME_TRACK_HEIGHT: f32 = 6.0;
    pub const VOLUME_KNOB: f32 = 16.0;
    pub const VOLUME_READOUT: f32 = 38.0;
}

/// Scroll-edge treatments (`DesignSystem/Scrolling/`).
pub mod fade {
    /// `EdgeDissolve.swift:5-9`: the top band overshoots the header by 32,
    /// the bottom band overshoots the bottom bar by 28.
    pub const EDGE_TOP_OVERSHOOT: f32 = 32.0;
    pub const EDGE_BOTTOM_OVERSHOOT: f32 = 28.0;
    /// `EdgeDissolve.swift:10-11`: mid-scroll alpha floors.
    pub const EDGE_TOP_FLOOR: f32 = 0.15;
    pub const EDGE_BOTTOM_FLOOR: f32 = 0.25;
    /// `OverflowFade.swift:6`: Settings lists and the Notes switcher.
    pub const OVERFLOW_BAND: f32 = 24.0;
    /// Popups opt into a progressive 30pt band (`docs/ui.md` "The overflow fade").
    pub const POPUP_BAND: f32 = 30.0;

    /// Top band length: `headerHeight + headerPadding + 32`.
    pub fn edge_top_band() -> f32 {
        super::size::HEADER_HEIGHT + super::size::HEADER_PADDING + EDGE_TOP_OVERSHOOT
    }

    /// Bottom band length: `bottomBarHeight + 28`.
    pub fn edge_bottom_band() -> f32 {
        super::size::BOTTOM_BAR_HEIGHT + EDGE_BOTTOM_OVERSHOOT
    }

    /// Mid-band alpha: `1 − (1 − floor)·clamp(hidden/band, 0, 1)` (`docs/ui.md` "The edge dissolve").
    pub fn edge_alpha(floor: f32, hidden: f32, band: f32) -> f32 {
        if band <= 0.0 {
            return 1.0;
        }
        1.0 - (1.0 - floor) * (hidden / band).clamp(0.0, 1.0)
    }
}

/// `ThinScrollbar.swift:88`: thumb alpha at rest, under the pointer, and while dragging.
pub mod scrollbar {
    pub const THUMB_REST_ALPHA: f32 = 0.30;
    pub const THUMB_HOVER_ALPHA: f32 = 0.42;
    pub const THUMB_DRAG_ALPHA: f32 = 0.5;
}

pub mod typography {
    pub const SEARCH_FIELD: f32 = 20.0;
    pub const HEADER_ICON: f32 = 18.0;
    pub const ROW_TITLE: f32 = 13.0;
    pub const ROW_TRAILING: f32 = 12.0;
    pub const BAR: f32 = 12.0;
    pub const SECTION_HEADER: f32 = 11.0;
    pub const KEY_CAP: f32 = 10.0;
    pub const PANEL_TITLE: f32 = 13.0;
    pub const CALC_RESULT: f32 = 22.0;
    pub const CALC_BADGE: f32 = 11.0;
    /// Alias pill ink in a result row; the old renderer reused `calcBadge`.
    pub const CHIP: f32 = CALC_BADGE;
    /// Emoji grid glyph; `features/launcher/ui/list.rs` drew Segoe UI Emoji at 32.
    pub const EMOJI: f32 = 32.0;
}

pub mod duration {
    /// `Theme.Duration.enter`. Window fade-in and `PanelEntrance` scale.
    pub const ENTER_SECS: f32 = 0.18;
    /// `Theme.Duration.exit`. Window fade-out, shorter than the entrance.
    pub const EXIT_SECS: f32 = 0.12;
    /// `Duration.dialogEnter` (Theme.swift:323): 3pt rise and alpha from 8%.
    pub const DIALOG_ENTER_SECS: f32 = 0.12;
    /// `Duration.dialogExit` (Theme.swift:324).
    pub const DIALOG_EXIT_SECS: f32 = 0.10;
    /// `Theme.Duration.tooltip`.
    pub const TOOLTIP_SECS: f32 = 0.15;
    /// `Duration.tooltipDelay` (Theme.swift:328): deliberate hover before a tooltip shows.
    pub const TOOLTIP_DELAY_SECS: f32 = 0.4;
    /// `Duration.hover` (Theme.swift:330).
    pub const HOVER_SECS: f32 = 0.12;
    /// `Duration.menuChevron` (Theme.swift:332).
    pub const MENU_CHEVRON_SECS: f32 = 0.34;
    /// `Duration.dropGuide` (Theme.swift:327).
    pub const DROP_GUIDE_SECS: f32 = 0.24;
    /// `Theme.Duration.copyFeedback`. How long a copied check stays up.
    pub const COPY_FEEDBACK_SECS: f32 = 1.2;
    /// `Theme.Duration.chatFooter`.
    pub const CHAT_FOOTER_SECS: f32 = 0.12;
    /// Thin scrollbar fade, `easeOut(duration: 0.18)`.
    pub const SCROLLBAR_FADE_SECS: f32 = 0.18;
    /// Scroll thumb stays up this long after the wheel stops.
    pub const SCROLLBAR_IDLE_SECS: f32 = 0.8;
    /// Hover has to leave the track this long before the rail collapses.
    pub const SCROLLBAR_HOVER_EXIT_SECS: f32 = 0.2;
    /// `ThinScrollbar.morphCurve`: `spring(response: 0.28, dampingFraction: 0.85)`.
    pub const SCROLLBAR_SPRING_RESPONSE: f32 = 0.28;
    pub const SCROLLBAR_SPRING_DAMPING: f32 = 0.85;
    /// Onboarding step change, `easeInOut(duration: 0.2)`.
    pub const ONBOARDING_SECS: f32 = 0.2;
    /// Shortcut recorder hover fill, `easeOut(duration: 0.12)`.
    pub const SHORTCUT_HOVER_SECS: f32 = 0.12;
    /// Shortcut recorder callout, `easeOut(duration: 0.14)`.
    pub const SHORTCUT_POPOVER_SECS: f32 = 0.14;
    /// Extension toast, `easeOut(duration: 0.16)`.
    pub const TOAST_SECS: f32 = 0.16;
    /// System `Toggle` thumb. The original does not name a duration; this is the
    /// control spring the switch actually plays (`response` 0.32, damping 0.86).
    pub const TOGGLE_SPRING_RESPONSE: f32 = 0.32;
    pub const TOGGLE_SPRING_DAMPING: f32 = 0.86;
    pub const MESSAGE_HUD_SECS: f32 = 2.4;
    pub const VOLUME_HUD_SECS: f32 = 1.6;
}

/// `Theme.DialogMotion` (Theme.swift:352-353).
pub mod dialog_motion {
    /// A dialog starts this many points below its rest position.
    pub const OFFSET: f32 = 3.0;
    /// ... at this opacity.
    pub const INITIAL_OPACITY: f32 = 0.08;
}

/// `Theme.MenuMotion` (Theme.swift:358-371), played by `Palette/MenuPanel.swift`:
/// alpha 0→1 over expansion+settle on the expansion curve; scale 0.94→1.003 on the
/// expansion curve, then →1 on the settle curve. Exit drops scale by 0.04 (floored at
/// 0.90) and alpha to 0 on the exit curve. v0.10.2 had no menu motion; wincast used
/// 0.96 + `easeOut(0.14)`.
pub mod menu_motion {
    pub const ENTRY_SCALE: f32 = 0.94;
    pub const MAXIMUM_SCALE: f32 = 1.003;
    pub const EXIT_SCALE_DELTA: f32 = 0.04;
    pub const EXPANSION_SECS: f32 = 0.10;
    pub const SETTLE_SECS: f32 = 0.05;
    pub const EXIT_SECS: f32 = 0.18;
    /// `CAMediaTimingFunction(controlPoints:)` x1, y1, x2, y2.
    pub const EXPANSION_CURVE: (f32, f32, f32, f32) = (0.2, 0.7, 0.2, 1.0);
    pub const SETTLE_CURVE: (f32, f32, f32, f32) = (0.42, 0.0, 0.58, 1.0);
    pub const EXIT_CURVE: (f32, f32, f32, f32) = (0.4, 0.0, 1.0, 1.0);
    /// `chevronAnimation`: `timingCurve(0.16, 1, 0.3, 1, duration: menuChevron)`.
    pub const CHEVRON_CURVE: (f32, f32, f32, f32) = (0.16, 1.0, 0.3, 1.0);
}

/// Alpha ramp (`Theme.Colors`, Theme.swift:416-527). Dark is frozen; Light is the
/// ink inverted. `glassFrost` (v0.10.2) no longer exists upstream and has no token here.
pub mod colors {
    pub const PANEL_SCRIM_DARK_ALPHA: f32 = 0.40;
    pub const PANEL_SCRIM_LIGHT_ALPHA: f32 = 0.55;
    pub const SELECTION_DARK_ALPHA: f32 = 0.10;
    pub const SELECTION_LIGHT_ALPHA: f32 = 0.09;
    pub const ROW_HOVER_DARK_ALPHA: f32 = 0.05;
    pub const ROW_HOVER_LIGHT_ALPHA: f32 = 0.045;
    pub const TEXT_PRIMARY_ALPHA: f32 = 1.0;
    pub const TEXT_SECONDARY_ALPHA: f32 = 0.60;
    pub const TEXT_TERTIARY_DARK_ALPHA: f32 = 0.40;
    pub const TEXT_TERTIARY_LIGHT_ALPHA: f32 = 0.42;
    pub const CONTROL_SURFACE_DARK_ALPHA: f32 = 0.10;
    pub const CONTROL_SURFACE_LIGHT_ALPHA: f32 = 0.08;
    pub const BORDER_DARK_ALPHA: f32 = 0.20;
    pub const BORDER_LIGHT_ALPHA: f32 = 0.18;
    pub const CARD_FILL_DARK_ALPHA: f32 = 0.05;
    pub const CARD_FILL_LIGHT_ALPHA: f32 = 0.04;
    pub const CARD_STROKE_ALPHA: f32 = 0.10;
    pub const SEPARATOR_DARK_ALPHA: f32 = 0.10;
    pub const SEPARATOR_LIGHT_ALPHA: f32 = 0.12;
    /// `menuHover` (Theme.swift:455).
    pub const MENU_HOVER_DARK_ALPHA: f32 = 0.10;
    pub const MENU_HOVER_LIGHT_ALPHA: f32 = 0.09;
    /// `controlHover` / `controlPressed` (Theme.swift:460-462).
    pub const CONTROL_HOVER_DARK_ALPHA: f32 = 0.16;
    pub const CONTROL_HOVER_LIGHT_ALPHA: f32 = 0.14;
    pub const CONTROL_PRESSED_DARK_ALPHA: f32 = 0.24;
    pub const CONTROL_PRESSED_LIGHT_ALPHA: f32 = 0.20;
    /// `menuSymbol` (Theme.swift:469).
    pub const MENU_SYMBOL_ALPHA: f32 = 0.70;
    /// `noteText` (Theme.swift:470).
    pub const NOTE_TEXT_DARK_ALPHA: f32 = 0.90;
    pub const NOTE_TEXT_LIGHT_ALPHA: f32 = 0.85;
    /// `iconPlaceholder` (Theme.swift:471).
    pub const ICON_PLACEHOLDER_ALPHA: f32 = 0.06;
    /// `sheen` (Theme.swift:473).
    pub const SHEEN_ALPHA: f32 = 0.04;
    /// `dropGuide` (Theme.swift:505).
    pub const DROP_GUIDE_ALPHA: f32 = 0.35;
    /// `emojiCell` / `emojiHoverBorder` / `emojiSelectionBorder` (Theme.swift:450-452).
    pub const EMOJI_CELL_DARK_ALPHA: f32 = 0.045;
    pub const EMOJI_CELL_LIGHT_ALPHA: f32 = 0.04;
    pub const EMOJI_HOVER_BORDER_DARK_ALPHA: f32 = 0.42;
    pub const EMOJI_HOVER_BORDER_LIGHT_ALPHA: f32 = 0.34;
    pub const EMOJI_SELECTION_BORDER_DARK_ALPHA: f32 = 0.92;
    pub const EMOJI_SELECTION_BORDER_LIGHT_ALPHA: f32 = 0.72;
    /// `dialogDimming` (Theme.swift:430): black 0.34 in both appearances.
    pub const DIALOG_DIMMING_ALPHA: f32 = 0.34;
    /// `tooltipShadow` (Theme.swift:442): black 0.18 in both appearances.
    pub const TOOLTIP_SHADOW_ALPHA: f32 = 0.18;
    /// `composerSend` (Theme.swift:516).
    pub const COMPOSER_SEND_ALPHA: f32 = 0.92;
    /// Destructive action ink (the dialog's red label and its tone). Straight
    /// alpha; the same in both appearances.
    pub const DESTRUCTIVE: (f32, f32, f32, f32) = (0.86, 0.22, 0.22, 1.0);

    /// 0 = Dark, 1 = Light.
    pub fn ramp_rgba(appearance: u8, dark_alpha: f32, light_alpha: f32) -> (f32, f32, f32, f32) {
        if appearance == 0 {
            (1.0, 1.0, 1.0, dark_alpha)
        } else {
            (0.0, 0.0, 0.0, light_alpha)
        }
    }

    pub fn scrim_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        if appearance == 0 {
            (0.0, 0.0, 0.0, PANEL_SCRIM_DARK_ALPHA)
        } else {
            (1.0, 1.0, 1.0, PANEL_SCRIM_LIGHT_ALPHA)
        }
    }

    /// Opaque page/surface/muted ramp sampled from yetone/magpie. Every Slint
    /// surface draws from these; the alpha ramp above stays for the Direct2D
    /// surfaces that phase 9 replaces.
    pub const SURFACE_DARK: (f32, f32, f32) = (35.0 / 255.0, 35.0 / 255.0, 39.0 / 255.0);
    pub const SURFACE_LIGHT: (f32, f32, f32) = (1.0, 1.0, 1.0);
    pub const SURFACE_BORDER_DARK: (f32, f32, f32) = (51.0 / 255.0, 51.0 / 255.0, 57.0 / 255.0);
    pub const SURFACE_BORDER_LIGHT: (f32, f32, f32) = (227.0 / 255.0, 227.0 / 255.0, 232.0 / 255.0);
    /// The page behind cards (settings, gallery).
    pub const PAGE_DARK: (f32, f32, f32) = (26.0 / 255.0, 26.0 / 255.0, 30.0 / 255.0);
    pub const PAGE_LIGHT: (f32, f32, f32) = (244.0 / 255.0, 244.0 / 255.0, 246.0 / 255.0);
    /// Secondary surface for control bars, caps and pills.
    pub const MUTED_DARK: (f32, f32, f32) = (45.0 / 255.0, 45.0 / 255.0, 51.0 / 255.0);
    pub const MUTED_LIGHT: (f32, f32, f32) = (241.0 / 255.0, 241.0 / 255.0, 244.0 / 255.0);
    /// Opaque ink ramp. Magpie's text is a solid grey, not an alpha over the fill.
    pub const TEXT_DARK: (f32, f32, f32) = (237.0 / 255.0, 237.0 / 255.0, 241.0 / 255.0);
    pub const TEXT_LIGHT: (f32, f32, f32) = (28.0 / 255.0, 28.0 / 255.0, 33.0 / 255.0);
    pub const TEXT_SECONDARY_DARK: (f32, f32, f32) = (161.0 / 255.0, 161.0 / 255.0, 170.0 / 255.0);
    pub const TEXT_SECONDARY_LIGHT: (f32, f32, f32) = (99.0 / 255.0, 99.0 / 255.0, 107.0 / 255.0);
    pub const TEXT_TERTIARY_DARK: (f32, f32, f32) = (139.0 / 255.0, 139.0 / 255.0, 148.0 / 255.0);
    pub const TEXT_TERTIARY_LIGHT: (f32, f32, f32) = (133.0 / 255.0, 133.0 / 255.0, 141.0 / 255.0);
    /// Magpie's red and green (`app.css` `--red` / `--green`): a calmer 600-ish
    /// step on light, a pastel 300-ish step on dark.
    pub const DESTRUCTIVE_DARK: (f32, f32, f32) = (242.0 / 255.0, 139.0 / 255.0, 130.0 / 255.0);
    pub const DESTRUCTIVE_LIGHT: (f32, f32, f32) = (214.0 / 255.0, 69.0 / 255.0, 69.0 / 255.0);
    pub const SUCCESS_DARK: (f32, f32, f32) = (111.0 / 255.0, 217.0 / 255.0, 155.0 / 255.0);
    pub const SUCCESS_LIGHT: (f32, f32, f32) = (24.0 / 255.0, 154.0 / 255.0, 88.0 / 255.0);
    /// The switch thumb while off (`library.css` `.lib-switch i`): literal white
    /// in both appearances, unlike the on-thumb which flips to the page fill.
    pub const TOGGLE_THUMB: (f32, f32, f32) = (1.0, 1.0, 1.0);
    /// Magpie's accent trio: the indigo ink, its soft tile fill, and the ink
    /// that sits on an accent-filled control (`app.css` `--accent*`).
    pub const ACCENT_DARK: (f32, f32, f32) = (165.0 / 255.0, 180.0 / 255.0, 252.0 / 255.0);
    pub const ACCENT_LIGHT: (f32, f32, f32) = (79.0 / 255.0, 70.0 / 255.0, 229.0 / 255.0);
    pub const ACCENT_SOFT_DARK: (f32, f32, f32) = (43.0 / 255.0, 44.0 / 255.0, 71.0 / 255.0);
    pub const ACCENT_SOFT_LIGHT: (f32, f32, f32) = (238.0 / 255.0, 237.0 / 255.0, 253.0 / 255.0);
    pub const ACCENT_FG_DARK: (f32, f32, f32) = (20.0 / 255.0, 20.0 / 255.0, 42.0 / 255.0);
    pub const ACCENT_FG_LIGHT: (f32, f32, f32) = (1.0, 1.0, 1.0);
    /// The picker's solid selected-row fill, distinct from the translucent
    /// list selection above (`app.css` `--sel`).
    pub const SEL_DARK: (f32, f32, f32) = (51.0 / 255.0, 52.0 / 255.0, 90.0 / 255.0);
    pub const SEL_LIGHT: (f32, f32, f32) = (236.0 / 255.0, 235.0 / 255.0, 251.0 / 255.0);
    /// The weaker rule (`app.css` `--line-2`): an input's hairline, versus the
    /// main `--line` a card or panel uses.
    pub const LINE2_DARK: (f32, f32, f32) = (44.0 / 255.0, 44.0 / 255.0, 50.0 / 255.0);
    pub const LINE2_LIGHT: (f32, f32, f32) = (236.0 / 255.0, 236.0 / 255.0, 240.0 / 255.0);
    /// Second card step behind an expanded editor (`app.css` `--card-2`).
    pub const CARD2_DARK: (f32, f32, f32) = (31.0 / 255.0, 31.0 / 255.0, 35.0 / 255.0);
    pub const CARD2_LIGHT: (f32, f32, f32) = (249.0 / 255.0, 249.0 / 255.0, 251.0 / 255.0);
    /// The remaining ink steps (`app.css` `--fg-2`, `--faint`, `--ctl-fg`).
    /// `--fg`/`--muted`/`--ctl-fg` map to text/destructive names above;
    /// fg-2 and faint complete the ramp.
    pub const FG2_DARK: (f32, f32, f32) = (185.0 / 255.0, 185.0 / 255.0, 193.0 / 255.0);
    pub const FG2_LIGHT: (f32, f32, f32) = (75.0 / 255.0, 75.0 / 255.0, 83.0 / 255.0);
    pub const FAINT_DARK: (f32, f32, f32) = (86.0 / 255.0, 86.0 / 255.0, 95.0 / 255.0);
    pub const FAINT_LIGHT: (f32, f32, f32) = (185.0 / 255.0, 185.0 / 255.0, 194.0 / 255.0);
    pub const CTL_FG_DARK: (f32, f32, f32) = (161.0 / 255.0, 161.0 / 255.0, 170.0 / 255.0);
    pub const CTL_FG_LIGHT: (f32, f32, f32) = (99.0 / 255.0, 99.0 / 255.0, 107.0 / 255.0);
    /// Hovered pill/chip fill; the rest state is `MUTED_*` (`--pill-hover`).
    pub const PILL_HOVER_DARK: (f32, f32, f32) = (54.0 / 255.0, 54.0 / 255.0, 61.0 / 255.0);
    pub const PILL_HOVER_LIGHT: (f32, f32, f32) = (233.0 / 255.0, 233.0 / 255.0, 238.0 / 255.0);
    /// Status colours: magpie's amber plus the soft tile behind each badge
    /// (`--amber`, `--green-soft`, `--red-soft`, `--amber-soft`).
    pub const AMBER_DARK: (f32, f32, f32) = (242.0 / 255.0, 181.0 / 255.0, 68.0 / 255.0);
    pub const AMBER_LIGHT: (f32, f32, f32) = (180.0 / 255.0, 83.0 / 255.0, 9.0 / 255.0);
    pub const GREEN_SOFT_DARK: (f32, f32, f32) = (30.0 / 255.0, 58.0 / 255.0, 43.0 / 255.0);
    pub const GREEN_SOFT_LIGHT: (f32, f32, f32) = (228.0 / 255.0, 246.0 / 255.0, 236.0 / 255.0);
    pub const RED_SOFT_DARK: (f32, f32, f32) = (69.0 / 255.0, 39.0 / 255.0, 42.0 / 255.0);
    pub const RED_SOFT_LIGHT: (f32, f32, f32) = (251.0 / 255.0, 233.0 / 255.0, 233.0 / 255.0);
    pub const AMBER_SOFT_DARK: (f32, f32, f32) = (63.0 / 255.0, 50.0 / 255.0, 32.0 / 255.0);
    pub const AMBER_SOFT_LIGHT: (f32, f32, f32) = (251.0 / 255.0, 240.0 / 255.0, 223.0 / 255.0);
    /// The header's segmented view switch (`--seg-track` / `--seg-thumb`).
    /// Dark is translucent white ink over the header fill, exactly as authored.
    pub const SEG_TRACK_DARK: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 0.035);
    pub const SEG_TRACK_LIGHT: (f32, f32, f32, f32) =
        (230.0 / 255.0, 230.0 / 255.0, 235.0 / 255.0, 1.0);
    pub const SEG_THUMB_DARK: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 0.15);
    pub const SEG_THUMB_LIGHT: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 1.0);
    /// Selection and hover are still translucent ink, just lighter than the
    /// alpha ramp above: Light 5% / 3%, Dark 8% / 4%.
    pub const MAGPIE_SELECTION_DARK_ALPHA: f32 = 0.08;
    pub const MAGPIE_SELECTION_LIGHT_ALPHA: f32 = 0.05;
    pub const MAGPIE_ROW_HOVER_DARK_ALPHA: f32 = 0.04;
    pub const MAGPIE_ROW_HOVER_LIGHT_ALPHA: f32 = 0.03;
    pub const MAGPIE_MENU_HOVER_DARK_ALPHA: f32 = 0.04;
    pub const MAGPIE_MENU_HOVER_LIGHT_ALPHA: f32 = 0.03;

    fn solid(
        dark: (f32, f32, f32),
        light: (f32, f32, f32),
        appearance: u8,
    ) -> (f32, f32, f32, f32) {
        let (red, green, blue) = if appearance == 0 { dark } else { light };
        (red, green, blue, 1.0)
    }

    pub fn surface_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(SURFACE_DARK, SURFACE_LIGHT, appearance)
    }

    pub fn surface_border_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(SURFACE_BORDER_DARK, SURFACE_BORDER_LIGHT, appearance)
    }

    pub fn page_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(PAGE_DARK, PAGE_LIGHT, appearance)
    }

    pub fn muted_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(MUTED_DARK, MUTED_LIGHT, appearance)
    }

    pub fn text_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(TEXT_DARK, TEXT_LIGHT, appearance)
    }

    pub fn text_secondary_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(TEXT_SECONDARY_DARK, TEXT_SECONDARY_LIGHT, appearance)
    }

    pub fn text_tertiary_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(TEXT_TERTIARY_DARK, TEXT_TERTIARY_LIGHT, appearance)
    }

    pub fn destructive_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(DESTRUCTIVE_DARK, DESTRUCTIVE_LIGHT, appearance)
    }

    pub fn success_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(SUCCESS_DARK, SUCCESS_LIGHT, appearance)
    }

    pub fn toggle_thumb_rgba(_appearance: u8) -> (f32, f32, f32, f32) {
        let (r, g, b) = TOGGLE_THUMB;
        (r, g, b, 1.0)
    }

    pub fn accent_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(ACCENT_DARK, ACCENT_LIGHT, appearance)
    }

    pub fn accent_soft_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(ACCENT_SOFT_DARK, ACCENT_SOFT_LIGHT, appearance)
    }

    pub fn accent_fg_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(ACCENT_FG_DARK, ACCENT_FG_LIGHT, appearance)
    }

    pub fn sel_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(SEL_DARK, SEL_LIGHT, appearance)
    }

    pub fn line2_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(LINE2_DARK, LINE2_LIGHT, appearance)
    }

    pub fn card2_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(CARD2_DARK, CARD2_LIGHT, appearance)
    }

    pub fn fg2_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(FG2_DARK, FG2_LIGHT, appearance)
    }

    pub fn faint_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(FAINT_DARK, FAINT_LIGHT, appearance)
    }

    pub fn ctl_fg_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(CTL_FG_DARK, CTL_FG_LIGHT, appearance)
    }

    pub fn pill_hover_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(PILL_HOVER_DARK, PILL_HOVER_LIGHT, appearance)
    }

    pub fn amber_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(AMBER_DARK, AMBER_LIGHT, appearance)
    }

    pub fn green_soft_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(GREEN_SOFT_DARK, GREEN_SOFT_LIGHT, appearance)
    }

    pub fn red_soft_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(RED_SOFT_DARK, RED_SOFT_LIGHT, appearance)
    }

    pub fn amber_soft_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        solid(AMBER_SOFT_DARK, AMBER_SOFT_LIGHT, appearance)
    }

    /// The segmented-control pair keeps its authored alpha, so this returns the
    /// stored 4-tuple instead of forcing 1.0 like `solid`.
    fn layered(
        dark: (f32, f32, f32, f32),
        light: (f32, f32, f32, f32),
        appearance: u8,
    ) -> (f32, f32, f32, f32) {
        if appearance == 0 {
            dark
        } else {
            light
        }
    }

    pub fn seg_track_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        layered(SEG_TRACK_DARK, SEG_TRACK_LIGHT, appearance)
    }

    pub fn seg_thumb_rgba(appearance: u8) -> (f32, f32, f32, f32) {
        layered(SEG_THUMB_DARK, SEG_THUMB_LIGHT, appearance)
    }
}

pub mod settings_chrome {
    pub const SIDEBAR_DARK: (f32, f32, f32) = (28.0 / 255.0, 28.0 / 255.0, 28.0 / 255.0);
    pub const DETAIL_DARK: (f32, f32, f32) = (41.0 / 255.0, 41.0 / 255.0, 41.0 / 255.0);
    pub const SIDEBAR_LIGHT: (f32, f32, f32) = (230.0 / 255.0, 230.0 / 255.0, 230.0 / 255.0);
    pub const DETAIL_LIGHT: (f32, f32, f32) = (242.0 / 255.0, 242.0 / 255.0, 242.0 / 255.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_height_is_header_plus_symmetric_padding() {
        assert_eq!(
            size::COMPACT_HEIGHT,
            size::HEADER_HEIGHT + size::HEADER_PADDING * 2.0
        );
    }
    #[test]
    fn dark_scrim_is_frozen() {
        assert_eq!(colors::PANEL_SCRIM_DARK_ALPHA, 0.40);
    }

    #[test]
    fn typography_matches_upstream_dip() {
        assert_eq!(typography::SEARCH_FIELD, 20.0);
        assert_eq!(typography::ROW_TITLE, 13.0);
        assert_eq!(typography::SECTION_HEADER, 11.0);
        assert_eq!(typography::KEY_CAP, 10.0);
    }

    #[test]
    fn hud_durations_are_split() {
        assert_eq!(duration::MESSAGE_HUD_SECS, 2.4);
        assert_eq!(duration::VOLUME_HUD_SECS, 1.6);
    }

    #[test]
    fn durations_match_upstream() {
        assert!((duration::ENTER_SECS - 0.18).abs() < 0.0001);
        assert!((duration::EXIT_SECS - 0.12).abs() < 0.0001);
        assert!(duration::ENTER_SECS > duration::EXIT_SECS);
        assert!((duration::TOOLTIP_SECS - 0.15).abs() < 0.0001);
        assert!((duration::COPY_FEEDBACK_SECS - 1.2).abs() < 0.0001);
        assert!((duration::CHAT_FOOTER_SECS - 0.12).abs() < 0.0001);
        assert!((duration::ONBOARDING_SECS - 0.2).abs() < 0.0001);
        assert!((duration::SHORTCUT_HOVER_SECS - 0.12).abs() < 0.0001);
        assert!((duration::SHORTCUT_POPOVER_SECS - 0.14).abs() < 0.0001);
        assert!((duration::TOAST_SECS - 0.16).abs() < 0.0001);
        assert!((duration::SCROLLBAR_FADE_SECS - 0.18).abs() < 0.0001);
        assert!((duration::SCROLLBAR_IDLE_SECS - 0.8).abs() < 0.0001);
    }

    #[test]
    fn ramp_inverts_ink() {
        assert_eq!(colors::ramp_rgba(0, 0.60, 0.60), (1.0, 1.0, 1.0, 0.60));
        assert_eq!(colors::ramp_rgba(1, 0.60, 0.60), (0.0, 0.0, 0.0, 0.60));
    }

    #[test]
    fn scrim_is_inverse_of_ink() {
        assert_eq!(colors::scrim_rgba(0), (0.0, 0.0, 0.0, 0.40));
        assert_eq!(colors::scrim_rgba(1), (1.0, 1.0, 1.0, 0.55));
    }

    #[test]
    fn settings_chrome_colors_are_pinned() {
        use crate::theme::settings_chrome::*;
        assert!((SIDEBAR_DARK.0 - 28.0 / 255.0).abs() < 0.001);
        assert!((DETAIL_LIGHT.0 - 242.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn dialog_tokens_match_upstream_main() {
        assert_eq!(radius::DIALOG_SURFACE, 26.0);
        assert_eq!(radius::DIALOG, 20.0);
        assert_eq!(size::DIALOG_COMPACT_WIDTH, 290.0);
        assert_eq!(size::DIALOG_WIDTH, 420.0);
        assert_eq!(size::DIALOG_BUTTON_HEIGHT, 34.0);
        assert_eq!(spacing::DIALOG_INSET, 18.0);
        assert_eq!(spacing::DIALOG_CONTENT_INSET, 22.0);
        assert!((duration::DIALOG_ENTER_SECS - 0.12).abs() < 1e-6);
        assert!((duration::DIALOG_EXIT_SECS - 0.10).abs() < 1e-6);
        assert_eq!(dialog_motion::OFFSET, 3.0);
        assert_eq!(dialog_motion::INITIAL_OPACITY, 0.08);
    }

    #[test]
    fn new_radius_and_size_tokens_match_upstream_main() {
        assert_eq!(radius::BAR_CONTROL, 8.0);
        assert_eq!(radius::TOOLTIP, 8.0);
        assert_eq!(radius::EMOJI_CELL, 12.0);
        assert_eq!(radius::DIALOG_SYMBOL, 16.0);
        assert_eq!(size::RESULT_ROW_ICON, 26.0);
        assert_eq!(size::ROW_ICON, 24.0);
        assert_eq!(size::ACTION_MENU_WIDTH, 320.0);
        assert_eq!(size::MENU_ROW_HEIGHT, 36.0);
        assert_eq!(size::SETTINGS_WINDOW, (900.0, 700.0));
        assert_eq!(size::PALETTE_SNAP_DISTANCE, 8.0);
        assert_eq!(size::DROP_GUIDE_DASH, 8.0);
        assert_eq!(size::HAIRLINE, 1.0);
    }

    #[test]
    fn menu_motion_matches_upstream_main() {
        assert_eq!(menu_motion::ENTRY_SCALE, 0.94);
        assert_eq!(menu_motion::MAXIMUM_SCALE, 1.003);
        assert_eq!(menu_motion::EXIT_SCALE_DELTA, 0.04);
        assert!((menu_motion::EXPANSION_SECS - 0.10).abs() < 1e-6);
        assert!((menu_motion::SETTLE_SECS - 0.05).abs() < 1e-6);
        assert!((menu_motion::EXIT_SECS - 0.18).abs() < 1e-6);
        assert_eq!(menu_motion::EXPANSION_CURVE, (0.2, 0.7, 0.2, 1.0));
        assert_eq!(menu_motion::EXIT_CURVE, (0.4, 0.0, 1.0, 1.0));
    }

    #[test]
    fn edge_fade_bands_and_floors() {
        assert_eq!(fade::edge_top_band(), 44.0 + 10.0 + 32.0);
        assert_eq!(fade::edge_bottom_band(), 52.0 + 28.0);
        assert_eq!(fade::edge_alpha(0.15, 0.0, 86.0), 1.0);
        assert!((fade::edge_alpha(0.15, 1000.0, 86.0) - 0.15).abs() < 1e-6);
        assert!((fade::edge_alpha(0.25, 40.0, 80.0) - 0.625).abs() < 1e-6);
        assert_eq!(fade::OVERFLOW_BAND, 24.0);
    }

    #[test]
    fn new_color_stops_match_upstream_main() {
        assert_eq!(colors::CONTROL_HOVER_DARK_ALPHA, 0.16);
        assert_eq!(colors::CONTROL_PRESSED_DARK_ALPHA, 0.24);
        assert_eq!(colors::MENU_HOVER_DARK_ALPHA, 0.10);
        assert_eq!(colors::DIALOG_DIMMING_ALPHA, 0.34);
        assert_eq!(colors::EMOJI_SELECTION_BORDER_LIGHT_ALPHA, 0.72);
        assert_eq!(scrollbar::THUMB_HOVER_ALPHA, 0.42);
    }
}
