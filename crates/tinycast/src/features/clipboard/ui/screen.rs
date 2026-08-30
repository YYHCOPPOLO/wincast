//! Clipboard palette list + preview pane (list 290 DIP).

use crate::features::clipboard::service::store::{ClipboardFilter, ClipboardItem, ClipKind};
use crate::features::launcher::ui::list::PaintItem;

pub fn filter_title(filter: ClipboardFilter) -> &'static str {
    match filter {
        ClipboardFilter::All => "All Types",
        ClipboardFilter::Text => "Text Only",
        ClipboardFilter::Images => "Images Only",
        ClipboardFilter::Links => "Links Only",
        ClipboardFilter::Emails => "Emails Only",
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
        Some(item) if item.kind == ClipKind::Image => {
            item.image_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Image".into())
        }
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
