//! The quicklink editor's shared surface: the draft struct. The sheet itself
//! is the Slint `EditorWindow` hosted by `ui_host::editor`; the width and the
//! labels come from the custom-command editor's constants.

use crate::features::custom_commands::ui::editor;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuicklinkDraft {
    pub name: String,
    pub destination: String,
}

pub const EDITOR_WIDTH_DIP: f32 = editor::EDITOR_WIDTH_DIP;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_sheet_is_480_dip() {
        assert_eq!(EDITOR_WIDTH_DIP, 480.0);
    }
}
