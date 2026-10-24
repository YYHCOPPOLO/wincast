//! The command/quicklink editor's shared surface: the draft struct, the labels
//! and the focus order. The sheet itself is the Slint `EditorWindow` hosted by
//! `ui_host::editor`; the old nested-`GetMessageW` native window is gone.

pub const EDITOR_WIDTH_DIP: f32 = 480.0;

/// Field order for the Slint sheet's Tab cycle: name, value, confirm, save,
/// cancel. The prompt variant hides the name and confirm rows.
pub const ID_NAME: usize = 101;
pub const ID_COMMAND: usize = 102;
pub const ID_CONFIRM: usize = 103;
pub const ID_SAVE: usize = 104;
pub const ID_CANCEL: usize = 105;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDraft {
    pub name: String,
    pub command: String,
    pub confirm: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorKey {
    Save,
    Cancel,
    None,
}

pub fn focus_order(show_confirm: bool) -> &'static [usize] {
    if show_confirm {
        &[ID_NAME, ID_COMMAND, ID_CONFIRM, ID_SAVE, ID_CANCEL]
    } else {
        &[ID_NAME, ID_COMMAND, ID_SAVE, ID_CANCEL]
    }
}

pub fn map_key(vk: u16) -> EditorKey {
    match vk {
        0x1B => EditorKey::Cancel,
        0x0D => EditorKey::Save,
        _ => EditorKey::None,
    }
}

pub fn editor_colors(
    appearance: u8,
) -> (
    windows::Win32::Foundation::COLORREF,
    windows::Win32::Foundation::COLORREF,
) {
    let (r, g, b) = crate::design_system::settings::detail_rgb(appearance);
    let bg = windows::Win32::Foundation::COLORREF(
        ((b * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (r * 255.0) as u32,
    );
    let fg = if appearance == 0 {
        windows::Win32::Foundation::COLORREF(0x00FFFFFF)
    } else {
        windows::Win32::Foundation::COLORREF(0x00000000)
    };
    (bg, fg)
}

#[derive(Clone, Copy)]
pub struct EditorLabels {
    pub title: &'static str,
    pub value_label: &'static str,
    pub show_confirm: bool,
}

pub const COMMAND_LABELS: EditorLabels = EditorLabels {
    title: "Custom Command",
    value_label: "Command",
    show_confirm: true,
};

pub const QUICKLINK_LABELS: EditorLabels = EditorLabels {
    title: "Quicklink",
    value_label: "Destination",
    show_confirm: false,
};

pub fn command_labels(lang: tinycast_pure::i18n::UiLang) -> EditorLabels {
    EditorLabels {
        title: tinycast_pure::i18n::editor_command_title(lang),
        value_label: tinycast_pure::i18n::editor_command_label(lang),
        show_confirm: true,
    }
}

pub fn quicklink_labels(lang: tinycast_pure::i18n::UiLang) -> EditorLabels {
    EditorLabels {
        title: tinycast_pure::i18n::editor_quicklink_title(lang),
        value_label: tinycast_pure::i18n::editor_destination_label(lang),
        show_confirm: false,
    }
}

pub fn escape_belongs_to_sheet(sheet: isize, msg_hwnd: isize, parent: Option<isize>) -> bool {
    msg_hwnd != 0 && sheet != 0 && (msg_hwnd == sheet || parent == Some(sheet))
}

pub fn field_acc_names(
    value_label: &'static str,
    lang: tinycast_pure::i18n::UiLang,
) -> (&'static str, &'static str) {
    (tinycast_pure::i18n::editor_name_label(lang), value_label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_sheet_is_480_dip() {
        assert_eq!(EDITOR_WIDTH_DIP, 480.0);
    }

    #[test]
    fn tab_order_includes_confirm_only_when_present() {
        assert_eq!(
            focus_order(true),
            &[ID_NAME, ID_COMMAND, ID_CONFIRM, ID_SAVE, ID_CANCEL]
        );
        assert_eq!(
            focus_order(false),
            &[ID_NAME, ID_COMMAND, ID_SAVE, ID_CANCEL]
        );
        assert!(!focus_order(false).contains(&ID_CONFIRM));
    }

    #[test]
    fn enter_saves_and_escape_cancels() {
        assert_eq!(map_key(0x0D), EditorKey::Save);
        assert_eq!(map_key(0x1B), EditorKey::Cancel);
        assert_eq!(map_key(0x09), EditorKey::None);
    }

    #[test]
    fn dark_and_light_editor_tokens_invert() {
        let (dark_bg, dark_fg) = editor_colors(0);
        let (light_bg, light_fg) = editor_colors(1);
        assert_ne!(dark_bg, light_bg);
        assert_ne!(dark_fg, light_fg);
        assert_eq!(dark_fg.0, 0x00FFFFFF);
        assert_eq!(light_fg.0, 0x00000000);
    }

    #[test]
    fn labels_follow_language() {
        let zh = command_labels(tinycast_pure::i18n::UiLang::ZhHans);
        assert_eq!(zh.title, "自定义命令");
        assert_eq!(zh.value_label, "命令");
        let en = command_labels(tinycast_pure::i18n::UiLang::En);
        assert_eq!(en.title, COMMAND_LABELS.title);
        let q = quicklink_labels(tinycast_pure::i18n::UiLang::En);
        assert_eq!(q.value_label, QUICKLINK_LABELS.value_label);
        assert!(!q.show_confirm);
        let zh = field_acc_names(
            quicklink_labels(tinycast_pure::i18n::UiLang::ZhHans).value_label,
            tinycast_pure::i18n::UiLang::ZhHans,
        );
        assert_eq!(zh.0, "名称");
        assert_eq!(zh.1, "目标");
    }

    #[test]
    fn escape_only_cancels_sheet_or_child() {
        assert!(escape_belongs_to_sheet(10, 10, None));
        assert!(escape_belongs_to_sheet(10, 11, Some(10)));
        assert!(!escape_belongs_to_sheet(10, 11, Some(99)));
        assert!(!escape_belongs_to_sheet(10, 0, None));
        assert!(!escape_belongs_to_sheet(0, 10, None));
    }
}
