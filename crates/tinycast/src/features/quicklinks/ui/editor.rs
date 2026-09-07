//! Quicklink editor sheet. Width is 480 DIP.

use crate::features::custom_commands::ui::editor::{self, CommandDraft, EDITOR_WIDTH_DIP};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuicklinkDraft {
    pub name: String,
    pub destination: String,
}

pub fn edit(
    owner: windows::Win32::Foundation::HWND,
    initial: Option<&QuicklinkDraft>,
    lang: tinycast_pure::i18n::UiLang,
) -> Option<QuicklinkDraft> {
    let seed = initial.map(|d| CommandDraft {
        name: d.name.clone(),
        command: d.destination.clone(),
        confirm: false,
    });
    let drafted = editor::edit_with(
        owner,
        seed.as_ref(),
        editor::QUICKLINK_LABELS,
        lang,
    )?;
    Some(QuicklinkDraft {
        name: drafted.name,
        destination: drafted.command,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_sheet_is_480_dip() {
        assert_eq!(EDITOR_WIDTH_DIP, 480.0);
    }
}
