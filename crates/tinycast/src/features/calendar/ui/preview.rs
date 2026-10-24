//! Camera preview: the surface is Slint (`ui_host::preview`); Join continues,
//! Cancel drops. Camera deny is not fatal.

use tinycast_pure::i18n::UiLang;

pub fn probe_camera_nonfatal() -> bool {
    crate::ui_host::preview::probe_camera_nonfatal()
}

/// Shows the card and registers the join continuation on the modal slot.
/// `false` means the caller joins directly: the slot was busy (a dialog or a
/// second preview) or the surface could not be created, like the old create
/// failure. No camera frame is rendered — the card states readiness only,
/// exactly like the Direct2D card it replaces.
pub fn present(title: &str, lang: UiLang, on_join: Box<dyn FnOnce()>) -> bool {
    crate::ui_host::preview::present(title, lang, on_join)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_deny_is_not_fatal() {
        assert!(probe_camera_nonfatal());
    }

    #[test]
    fn camera_and_qa_sizes() {
        assert_eq!(tinycast_pure::theme::size::CAMERA_PREVIEW, (420.0, 236.0));
        assert_eq!(tinycast_pure::theme::size::QUICK_ACTION_PANEL, 520.0);
        assert_eq!(tinycast_pure::theme::size::QUICK_ACTION_PANEL_BODY, 320.0);
    }

    #[test]
    fn camera_preview_has_no_textout() {
        let src = include_str!("preview.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
