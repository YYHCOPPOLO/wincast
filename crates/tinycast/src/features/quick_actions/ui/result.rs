//! Quick Action result panel: the surface is Slint (`ui_host::result`); the
//! low-level hook and the buttons post the same `WM_QA_*` messages as before,
//! so the coordinator is unchanged. 520 DIP wide, Replace / Copy / Esc.

pub const PANEL_WIDTH_DIP: f32 = tinycast_pure::theme::size::QUICK_ACTION_PANEL;

pub use crate::ui_host::result::ResultPanel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_is_520_dip() {
        assert_eq!(PANEL_WIDTH_DIP, 520.0);
        let _ = tinycast_pure::theme::spacing::XL;
    }

    #[test]
    fn qa_result_has_no_textout() {
        let src = include_str!("result.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
