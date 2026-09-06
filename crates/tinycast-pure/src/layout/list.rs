use crate::palette_placement::DipRect;
use crate::theme;

pub const ROW_HEIGHT: f32 = 36.0;

pub fn row_icon_rect(row_y: f32) -> DipRect {
    let size = theme::size::ROW_ICON;
    DipRect {
        x: theme::spacing::MD,
        y: row_y + (ROW_HEIGHT - size) / 2.0,
        w: size,
        h: size,
    }
}

pub fn row_fill_rect(panel_w: f32, row_y: f32) -> DipRect {
    DipRect {
        x: theme::spacing::MD,
        y: row_y,
        w: panel_w - theme::spacing::MD * 2.0,
        h: ROW_HEIGHT,
    }
}

pub fn content_top() -> f32 {
    theme::size::COMPACT_HEIGHT
}

pub fn paint_clip_top() -> f32 {
    0.0
}

/// Inset-excluded list viewport (between floating header and footer).
pub fn view_height(panel_h: f32) -> f32 {
    (panel_h - content_top() - theme::size::BOTTOM_BAR_HEIGHT).max(0.0)
}

pub fn edge_dissolve_top_band() -> f32 {
    theme::size::HEADER_HEIGHT + theme::size::HEADER_PADDING + 32.0
}

pub fn edge_dissolve_bottom_band() -> f32 {
    theme::size::BOTTOM_BAR_HEIGHT + 28.0
}

pub fn section_header_height(is_first: bool) -> f32 {
    if is_first {
        22.0
    } else {
        30.0
    }
}

pub fn clipboard_columns(panel_w: f32) -> (DipRect, DipRect) {
    let w = theme::size::CLIPBOARD_LIST_WIDTH.min(panel_w);
    (
        DipRect {
            x: 0.0,
            y: 0.0,
            w,
            h: 0.0,
        },
        DipRect {
            x: w,
            y: 0.0,
            w: (panel_w - w).max(0.0),
            h: 0.0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    #[test]
    fn row_height_is_icon_plus_vertical_padding() {
        assert_eq!(ROW_HEIGHT, theme::size::ROW_ICON + theme::spacing::SM * 2.0);
        assert_eq!(ROW_HEIGHT, 36.0);
    }

    #[test]
    fn row_icon_x_is_md() {
        let r = row_icon_rect(100.0);
        assert_eq!(r.x, theme::spacing::MD);
        assert_eq!(r.w, 24.0);
        assert_eq!(r.h, 24.0);
        assert!((r.y - (100.0 + 6.0)).abs() < 0.01);
    }

    #[test]
    fn dissolve_bands_overshoot_bars() {
        assert_eq!(edge_dissolve_top_band(), 44.0 + 10.0 + 32.0);
        assert_eq!(edge_dissolve_bottom_band(), 52.0 + 28.0);
        assert_eq!(paint_clip_top(), 0.0);
        assert_eq!(content_top(), theme::size::COMPACT_HEIGHT);
    }

    #[test]
    fn view_height_is_between_bars() {
        assert!((view_height(475.0) - 359.0).abs() < 0.01);
        assert_eq!(
            view_height(theme::size::PANEL_HEIGHT),
            theme::size::PANEL_HEIGHT
                - theme::size::COMPACT_HEIGHT
                - theme::size::BOTTOM_BAR_HEIGHT
        );
    }

    #[test]
    fn clipboard_list_column_is_290() {
        let (list, preview) = clipboard_columns(750.0);
        assert_eq!(list.w, 290.0);
        assert_eq!(preview.x, 290.0);
        assert!((preview.w - 460.0).abs() < 0.01);
    }
}
