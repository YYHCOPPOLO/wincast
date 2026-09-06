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

pub fn calc_card_rect(panel_w: f32, y: f32) -> DipRect {
    DipRect {
        x: theme::spacing::XL,
        y,
        w: (panel_w - theme::spacing::XL * 2.0).max(0.0),
        h: theme::size::CALC_CARD_HEIGHT,
    }
}

pub fn calc_card_inner_rect(panel_w: f32, y: f32) -> DipRect {
    let card = calc_card_rect(panel_w, y);
    DipRect {
        x: card.x + theme::spacing::XL,
        y: card.y + theme::spacing::XXXL,
        w: (card.w - theme::spacing::XL * 2.0).max(0.0),
        h: (card.h - theme::spacing::XXXL * 2.0).max(0.0),
    }
}

pub fn calc_error_icon_rect(inner: DipRect) -> DipRect {
    let size = theme::typography::HEADER_ICON;
    DipRect {
        x: inner.x,
        y: inner.y + (inner.h - size).max(0.0) / 2.0,
        w: size,
        h: size,
    }
}

pub fn empty_results_center(panel_w: f32, panel_h: f32) -> DipRect {
    let top = content_top();
    let bottom = (panel_h - theme::size::BOTTOM_BAR_HEIGHT).max(top);
    let h = theme::typography::HEADER_ICON * 2.0 + theme::spacing::MD + theme::typography::ROW_TITLE;
    DipRect {
        x: 0.0,
        y: top + ((bottom - top - h) / 2.0).max(0.0),
        w: panel_w,
        h,
    }
}

pub fn chat_pad_x() -> f32 {
    theme::spacing::XXL
}

pub fn chat_pad_top() -> f32 {
    theme::spacing::XL
}

pub fn chat_pad_bottom() -> f32 {
    theme::spacing::XXXL
}

pub fn chat_message_gap() -> f32 {
    theme::spacing::XL
}

pub const CHAT_NOTICE_HEIGHT: f32 = 48.0;

fn chat_wrap_inner(is_user: bool, panel_w: f32) -> f32 {
    let pad = chat_pad_x();
    if is_user {
        (panel_w - pad * 2.0 - theme::spacing::XL * 2.0).max(40.0)
    } else {
        (panel_w - pad * 2.0 - theme::spacing::SM * 2.0).max(40.0)
    }
}

pub fn chat_text_height(text: &str, inner_w: f32) -> f32 {
    let line = theme::typography::ROW_TITLE;
    if text.is_empty() {
        return line;
    }
    let cols = ((inner_w / (line * 0.5)).floor() as usize).max(1);
    let mut lines = 0usize;
    for para in text.split('\n') {
        let n = para.chars().count().max(1);
        lines += n.div_ceil(cols);
    }
    (lines as f32 * line).max(line)
}

pub fn chat_message_height(is_user: bool, text: &str, panel_w: f32) -> f32 {
    let display = if text.is_empty() { "Thinking" } else { text };
    let th = chat_text_height(display, chat_wrap_inner(is_user, panel_w));
    let pad_y = theme::spacing::MD * 2.0;
    if is_user {
        (th + pad_y).max(theme::size::BAR_BUTTON_HEIGHT)
    } else {
        (th + pad_y).max(theme::typography::ROW_TITLE + pad_y)
    }
}

/// Scrollable transcript height: top pad + bubbles + gaps + optional notice + bottom pad (28).
pub fn chat_transcript_height<'a, I>(blocks: I, notice: Option<&str>, panel_w: f32) -> f32
where
    I: IntoIterator<Item = (bool, &'a str)>,
{
    let mut h = chat_pad_top();
    let mut n = 0usize;
    for (is_user, text) in blocks {
        if n > 0 {
            h += chat_message_gap();
        }
        h += chat_message_height(is_user, text, panel_w);
        n += 1;
    }
    if notice.filter(|s| !s.is_empty()).is_some() {
        if n > 0 {
            h += chat_message_gap();
        }
        h += CHAT_NOTICE_HEIGHT;
    }
    h + chat_pad_bottom()
}

pub fn chat_user_bubble(panel_w: f32, y: f32, bubble_w: f32) -> DipRect {
    let pad = chat_pad_x();
    let max_w = (panel_w - pad - pad).max(0.0);
    let w = bubble_w.min(max_w);
    DipRect {
        x: (panel_w - pad - w).max(pad),
        y,
        w,
        h: 0.0,
    }
}

pub fn chat_assistant_rect(panel_w: f32, y: f32) -> DipRect {
    let pad = chat_pad_x();
    DipRect {
        x: pad,
        y,
        w: (panel_w - pad - pad).max(0.0),
        h: 0.0,
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

    #[test]
    fn empty_results_is_centered() {
        let r = empty_results_center(750.0, 475.0);
        assert!((r.x + r.w / 2.0 - 375.0).abs() < 1.0);
        assert!(r.y > 64.0 && r.y < 400.0);
    }

    #[test]
    fn user_bubble_is_trailing_fill() {
        let r = chat_user_bubble(750.0, 80.0, 200.0);
        assert!(r.x > 20.0);
        assert!((r.x + r.w - (750.0 - 20.0)).abs() < 0.01);
    }

    #[test]
    fn chat_transcript_includes_bottom_pad() {
        let empty = chat_transcript_height(std::iter::empty(), None, 750.0);
        assert_eq!(chat_pad_bottom(), theme::spacing::XXXL);
        assert_eq!(chat_pad_bottom(), 28.0);
        assert!((empty - (chat_pad_top() + chat_pad_bottom())).abs() < 0.01);
        let one = chat_transcript_height(std::iter::once((true, "hi")), None, 750.0);
        assert!(one > empty);
        assert!(one + 0.01 >= empty + theme::size::BAR_BUTTON_HEIGHT);
        let notice = chat_transcript_height(std::iter::empty(), Some("err"), 750.0);
        assert!((notice - (empty + CHAT_NOTICE_HEIGHT)).abs() < 0.01);
    }

    #[test]
    fn chat_transcript_can_exceed_view() {
        let texts: Vec<String> = (0..24).map(|i| format!("message {i} with extra words")).collect();
        let h = chat_transcript_height(texts.iter().map(|t| (true, t.as_str())), None, 750.0);
        assert!(h > view_height(theme::size::PANEL_HEIGHT));
    }

    #[test]
    fn calc_card_inner_uses_xl_xxxl() {
        let inner = calc_card_inner_rect(750.0, 64.0);
        assert_eq!(inner.x, theme::spacing::XL * 2.0);
        assert_eq!(inner.y, 64.0 + theme::spacing::XXXL);
        assert!((inner.h - (96.0 - theme::spacing::XXXL * 2.0)).abs() < 0.01);
        assert_eq!(theme::radius::CARD, 10.0);
        let icon = calc_error_icon_rect(inner);
        assert!(icon.x >= inner.x);
        assert!(icon.w >= 16.0);
    }
}
