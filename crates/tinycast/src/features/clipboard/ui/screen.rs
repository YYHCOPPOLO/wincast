//! Clipboard palette list + preview pane (list 290 DIP).

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;

use crate::features::clipboard::service::store::{ClipKind, ClipboardFilter, ClipboardItem};
use crate::features::launcher::ui::list::PaintItem;

pub const FILTER_BUTTON_WIDTH: f32 = 128.0;

pub fn filter_trailing_width() -> f32 {
    theme::spacing::MD + FILTER_BUTTON_WIDTH
}

pub fn filter_button_rect(panel_w: f32) -> DipRect {
    let h = theme::size::BAR_BUTTON_HEIGHT;
    DipRect {
        x: panel_w - theme::spacing::XXL - FILTER_BUTTON_WIDTH,
        y: theme::size::HEADER_PADDING + (theme::size::HEADER_HEIGHT - h) / 2.0,
        w: FILTER_BUTTON_WIDTH,
        h,
    }
}

pub fn filter_title(filter: ClipboardFilter) -> &'static str {
    match filter {
        ClipboardFilter::All => "All Types",
        ClipboardFilter::Text => "Text Only",
        ClipboardFilter::Images => "Images Only",
        ClipboardFilter::Links => "Links Only",
        ClipboardFilter::Emails => "Emails Only",
    }
}

pub fn empty_message(filter: ClipboardFilter) -> &'static str {
    match filter {
        ClipboardFilter::All => "Clipboard history is empty",
        ClipboardFilter::Text => "No text in clipboard history",
        ClipboardFilter::Images => "No images in clipboard history",
        ClipboardFilter::Links => "No links in clipboard history",
        ClipboardFilter::Emails => "No email addresses in clipboard history",
    }
}

pub fn paint_items(rows: &[ClipboardItem], selection: usize) -> Vec<PaintItem> {
    rows.iter()
        .enumerate()
        .map(|(i, item)| {
            let title = match item.kind {
                ClipKind::Image => "[Image]".to_string(),
                ClipKind::Text => item
                    .text
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect(),
            };
            let trailing = if item.is_pinned() { "Pinned" } else { "" };
            PaintItem::Row {
                title,
                alias: None,
                trailing: trailing.to_string(),
                keycap: None,
                icon_source: None,
                selected: i == selection,
            }
        })
        .collect()
}

pub fn preview_text(item: Option<&ClipboardItem>) -> String {
    match item {
        Some(item) if item.kind == ClipKind::Image => item
            .image_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Image".into()),
        Some(item) => item.text.clone().unwrap_or_default(),
        None => String::new(),
    }
}

pub fn filter_from_id(id: &str) -> Option<ClipboardFilter> {
    Some(match id {
        "clip-filter-all" => ClipboardFilter::All,
        "clip-filter-text" => ClipboardFilter::Text,
        "clip-filter-images" => ClipboardFilter::Images,
        "clip-filter-links" => ClipboardFilter::Links,
        "clip-filter-emails" => ClipboardFilter::Emails,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::palette_menu::point_in;

    #[test]
    fn filter_button_sits_in_header_trailing_slot() {
        let panel_w = theme::size::PANEL_WIDTH;
        let rect = filter_button_rect(panel_w);
        assert_eq!(rect.w, FILTER_BUTTON_WIDTH);
        assert!(rect.x > panel_w / 2.0);
        assert!(rect.y >= theme::size::HEADER_PADDING);
        assert!(rect.y + rect.h <= theme::size::COMPACT_HEIGHT);
        assert!(point_in(rect, rect.x + 1.0, rect.y + 1.0));
        assert_eq!(
            filter_trailing_width(),
            theme::spacing::MD + FILTER_BUTTON_WIDTH
        );
    }

    #[test]
    fn empty_message_names_the_filter() {
        assert_eq!(
            empty_message(ClipboardFilter::All),
            "Clipboard history is empty"
        );
        assert_eq!(
            empty_message(ClipboardFilter::Images),
            "No images in clipboard history"
        );
    }
}
