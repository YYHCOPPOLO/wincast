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

pub fn compact_favorite_slot(index: usize, search_right: f32) -> DipRect {
    let size = theme::size::ROW_ICON;
    DipRect {
        x: search_right + theme::spacing::MD + index as f32 * (size + theme::spacing::SM),
        y: theme::size::HEADER_PADDING + (theme::size::HEADER_HEIGHT - size) / 2.0,
        w: size,
        h: size,
    }
}

pub fn compact_favorites_trailing(count: usize) -> f32 {
    let n = count.min(5) as f32;
    if n <= 0.0 {
        0.0
    } else {
        theme::spacing::MD + n * (theme::size::ROW_ICON + theme::spacing::SM)
    }
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
    fn compact_favorite_slot_is_row_icon() {
        let r = compact_favorite_slot(0, 400.0);
        assert_eq!(r.w, 24.0);
        assert_eq!(r.h, 24.0);
        let r1 = compact_favorite_slot(1, 400.0);
        assert!((r1.x - r.x - 24.0 - theme::spacing::SM).abs() < 0.01);
    }

    #[test]
    fn tab_hint_hidden_when_compact() {
        assert!(!tab_hint_visible(false, true));
        assert!(tab_hint_visible(true, true));
        assert!(!tab_hint_visible(true, false));
    }
}
