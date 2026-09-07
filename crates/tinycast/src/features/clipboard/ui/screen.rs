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
    filter_title_lang(filter, tinycast_pure::i18n::UiLang::En)
}

pub fn filter_title_lang(
    filter: ClipboardFilter,
    lang: tinycast_pure::i18n::UiLang,
) -> &'static str {
    let key = match filter {
        ClipboardFilter::All => "all",
        ClipboardFilter::Text => "text",
        ClipboardFilter::Images => "images",
        ClipboardFilter::Links => "links",
        ClipboardFilter::Emails => "emails",
    };
    tinycast_pure::i18n::clipboard_filter_title(key, lang)
}

pub fn empty_message(filter: ClipboardFilter) -> &'static str {
    empty_message_lang(filter, tinycast_pure::i18n::UiLang::En)
}

pub fn empty_message_lang(
    filter: ClipboardFilter,
    lang: tinycast_pure::i18n::UiLang,
) -> &'static str {
    let key = match filter {
        ClipboardFilter::All => "all",
        ClipboardFilter::Text => "text",
        ClipboardFilter::Images => "images",
        ClipboardFilter::Links => "links",
        ClipboardFilter::Emails => "emails",
    };
    tinycast_pure::i18n::clipboard_empty(key, lang)
}

pub fn paint_items(
    rows: &[ClipboardItem],
    selection: usize,
    lang: tinycast_pure::i18n::UiLang,
) -> Vec<PaintItem> {
    rows.iter()
        .enumerate()
        .map(|(i, item)| {
            let title = match item.kind {
                ClipKind::Image => format!("[{}]", tinycast_pure::i18n::clipboard_image(lang)),
                ClipKind::Text => item
                    .text
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(80)
                    .collect(),
            };
            let trailing = if item.is_pinned() {
                tinycast_pure::i18n::clipboard_pinned(lang)
            } else {
                ""
            };
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
    preview_text_lang(item, tinycast_pure::i18n::UiLang::En)
}

pub fn preview_text_lang(
    item: Option<&ClipboardItem>,
    lang: tinycast_pure::i18n::UiLang,
) -> String {
    match item {
        Some(item) if item.kind == ClipKind::Image => item
            .image_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| tinycast_pure::i18n::clipboard_image(lang).into()),
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
