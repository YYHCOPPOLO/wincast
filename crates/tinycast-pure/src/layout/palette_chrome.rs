use crate::palette_placement::DipRect;
use crate::theme;

pub fn header_icon_rect() -> DipRect {
    DipRect {
        x: theme::spacing::MD * 2.0,
        y: theme::size::HEADER_PADDING,
        w: theme::size::HEADER_ICON_SLOT,
        h: theme::size::HEADER_HEIGHT,
    }
}

pub fn search_field_rect(panel_w: f32, trailing: f32) -> DipRect {
    let x = theme::spacing::MD * 2.0 + theme::size::HEADER_ICON_SLOT + theme::spacing::MD;
    let w = (panel_w - x - theme::spacing::MD * 2.0 - trailing).max(60.0);
    DipRect {
        x,
        y: theme::size::HEADER_PADDING,
        w,
        h: theme::size::HEADER_HEIGHT,
    }
}

pub fn tab_hint_visible(expanded: bool, tab_opens_ai: bool) -> bool {
    expanded && tab_opens_ai
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    #[test]
    fn header_icon_aligns_with_md_gutter() {
        let r = header_icon_rect();
        assert_eq!(r.x, theme::spacing::MD * 2.0);
        assert_eq!(r.w, theme::size::HEADER_ICON_SLOT);
        assert_eq!(r.h, theme::size::HEADER_HEIGHT);
    }

    #[test]
    fn search_field_starts_after_icon_slot() {
        let r = search_field_rect(750.0, 0.0);
        assert!((r.x - (theme::spacing::MD * 2.0 + 22.0 + theme::spacing::MD)).abs() < 0.01);
        assert_eq!(r.y, theme::size::HEADER_PADDING);
        assert_eq!(r.h, theme::size::HEADER_HEIGHT);
    }

    #[test]
    fn tab_hint_hidden_when_compact() {
        assert!(!tab_hint_visible(false, true));
        assert!(tab_hint_visible(true, true));
        assert!(!tab_hint_visible(true, false));
    }
}
