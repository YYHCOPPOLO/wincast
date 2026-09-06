pub mod spacing {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 10.0;
    pub const XL: f32 = 12.0;
    pub const XXL: f32 = 20.0;
    pub const XXXL: f32 = 28.0;
    pub const SECTION_HEADER_BOTTOM: f32 = 4.0;
    pub const SECTION_SPACING: f32 = 12.0;
}

pub mod radius {
    pub const PANEL: f32 = 26.0;
    pub const ROW: f32 = 10.0;
    pub const MENU: f32 = 6.0;
    pub const MENU_ROW: f32 = 10.0;
    pub const MENU_PANEL: f32 = 16.0;
    pub const DIALOG: f32 = 20.0;
    pub const THUMBNAIL: f32 = 6.0;
    pub const CARD: f32 = 10.0;
    pub const KEY_CAP: f32 = 6.0;
    pub const RECORDER_KEY_CAP: f32 = 4.0;
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
    pub const PALETTE_SNAP_DISTANCE: f32 = 24.0;
    pub const PALETTE_MINIMUM_VISIBLE: f32 = 44.0;
    pub const DROP_GUIDE_DASH: f32 = 4.0;
    pub const DROP_GUIDE_WIDTH: f32 = 2.0;
    pub const BOTTOM_BAR_HEIGHT: f32 = 52.0;
    pub const BAR_BUTTON_HEIGHT: f32 = 28.0;
    pub const ROW_ICON: f32 = 24.0;
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
    pub const MARKDOWN_LIST_MARKER: f32 = 20.0;
    pub const MARKDOWN_QUOTE_BAR: f32 = 2.0;
    pub const CHECKBOX: f32 = 16.0;
    pub const CLIPBOARD_LIST_WIDTH: f32 = 290.0;
    pub const EMOJI_CELL: f32 = 56.0;
    pub const MENU_WIDTH: f32 = 276.0;
    pub const CLIPBOARD_FILTER_MENU_WIDTH: f32 = 200.0;
    pub const MENU_ICON: f32 = 20.0;
    pub const MENU_BRAND_ICON: f32 = 14.0;
    pub const BAR_BRAND_ICON: f32 = 12.0;
    pub const CALC_CARD_HEIGHT: f32 = 96.0;
    pub const CHAT_IMAGE_THUMB: f32 = 96.0;
    pub const CHAT_ATTACHMENT_GLYPH: f32 = 16.0;
    pub const SETTINGS_WINDOW: (f32, f32) = (860.0, 700.0);
    pub const SETTINGS_SIDEBAR: f32 = 215.0;
    pub const SETTINGS_DETAIL_MINIMUM: f32 = 420.0;
    pub const SETTINGS_ROW_ICON: f32 = 20.0;
    pub const EDITOR_SHEET_WIDTH: f32 = 480.0;
    pub const FORM_LABEL_WIDTH: f32 = 110.0;
    pub const EDITOR_TEXT_HEIGHT: f32 = 120.0;
    pub const ARGUMENT_PROMPT_WIDTH: f32 = 220.0;
    pub const HUD_MAX_WIDTH: f32 = 420.0;
    pub const HUD_EDGE_OFFSET: f32 = 48.0;
    pub const DIALOG_WIDTH: f32 = 420.0;
    pub const DIALOG_ICON: f32 = 32.0;
    pub const CAMERA_PREVIEW: (f32, f32) = (420.0, 236.0);
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
}

pub mod duration {
    pub const ENTER_SECS: f32 = 0.18;
    pub const EXIT_SECS: f32 = 0.12;
    pub const MESSAGE_HUD_SECS: f32 = 2.4;
    pub const VOLUME_HUD_SECS: f32 = 1.6;
}

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
    fn typography_matches_v0102_dip() {
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
    fn ramp_inverts_ink() {
        assert_eq!(colors::ramp_rgba(0, 0.60, 0.60), (1.0, 1.0, 1.0, 0.60));
        assert_eq!(colors::ramp_rgba(1, 0.60, 0.60), (0.0, 0.0, 0.0, 0.60));
    }

    #[test]
    fn scrim_is_inverse_of_ink() {
        assert_eq!(colors::scrim_rgba(0), (0.0, 0.0, 0.0, 0.40));
        assert_eq!(colors::scrim_rgba(1), (1.0, 1.0, 1.0, 0.55));
    }
}
