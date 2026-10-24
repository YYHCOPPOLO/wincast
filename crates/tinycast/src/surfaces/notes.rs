//! The Notes surface is a Slint window pair; its host lives in `ui_host`.
//!
//! `NotesWindow` keeps the API the Direct2D window had (`create`, `show`,
//! `hide`, `set_title`, `set_body`, the switcher calls, `take_pending_body`)
//! so `AppCore` does not change. The body keeps the native RichEdit child and
//! the switcher search a native EDIT; the chrome, the list and the empty
//! state are Slint.

pub use crate::ui_host::notes::NotesWindow;

#[cfg(test)]
mod tests {
    use crate::ui_host::notes::{
        note_fg_colorref, CHARFORMAT_WPARAMS, SCF_ALL, SCF_DEFAULT, SWITCHER_HEIGHT, SWITCHER_WIDTH,
    };
    use windows::Win32::Foundation::COLORREF;

    #[test]
    fn notes_titlebar_is_52() {
        assert_eq!(tinycast_pure::layout::notes::titlebar_height(), 52.0);
    }

    #[test]
    fn charformat_applies_default_and_all() {
        assert!(CHARFORMAT_WPARAMS.contains(&SCF_DEFAULT));
        assert!(CHARFORMAT_WPARAMS.contains(&SCF_ALL));
    }

    #[test]
    fn note_fg_derives_from_note_text() {
        let (r, g, b, a) = crate::design_system::text::note_text(0);
        let bg = 0x1A as f32 / 255.0;
        let comp = |c: f32| ((c * a + bg * (1.0 - a)) * 255.0).round() as u32;
        let expected = COLORREF(comp(b) << 16 | comp(g) << 8 | comp(r));
        assert_eq!(note_fg_colorref(0), expected);
    }

    #[test]
    fn notes_file_has_no_textout() {
        let src = include_str!("notes.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }

    #[test]
    fn switcher_is_300_by_240() {
        assert_eq!(SWITCHER_WIDTH, 300);
        assert_eq!(SWITCHER_HEIGHT, 240);
        assert_eq!(tinycast_pure::theme::size::NOTE_SWITCHER, (300.0, 240.0));
    }
}
