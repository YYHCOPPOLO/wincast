//! Emoji palette grid. Cell size is `tinycast_pure::emoji::CELL_DIP` (56).
//! Glyphs are drawn with Segoe UI Emoji in the palette list.

pub mod settings;

use tinycast_pure::emoji::{
    search_emoji_with_tone, EmojiSkinTone, CELL_DIP, GRID_COLUMNS,
};
use tinycast_pure::theme;

use crate::features::launcher::ui::list::PaintItem;

pub fn paint_items(
    query: &str,
    tone: EmojiSkinTone,
    selection: usize,
    _width: f32,
) -> Vec<PaintItem> {
    let hits = search_emoji_with_tone(query, tone);
    let columns = GRID_COLUMNS;
    let mut items = Vec::new();
    let mut index = 0usize;
    let mut i = 0usize;
    while i < hits.len() {
        let group = hits[i].group;
        items.push(PaintItem::Header {
            title: group.to_string(),
        });
        while i < hits.len() && hits[i].group == group {
            let mut glyphs = Vec::new();
            while i < hits.len() && hits[i].group == group && glyphs.len() < columns {
                glyphs.push(hits[i].glyph.clone());
                i += 1;
            }
            let cells = glyphs.len();
            items.push(PaintItem::EmojiRow {
                glyphs,
                columns,
                start: index,
                selected: selection,
            });
            index += cells;
        }
    }
    items
}

pub fn glyph_at(query: &str, tone: EmojiSkinTone, index: usize) -> Option<String> {
    search_emoji_with_tone(query, tone)
        .get(index)
        .map(|e| e.glyph.clone())
}

pub fn columns(_width: f32) -> usize {
    GRID_COLUMNS
}

pub fn cell_dip() -> f32 {
    CELL_DIP
}

pub fn hit_index(items: &[PaintItem], x: f32, y: f32, scroll: f32, panel_w: f32) -> Option<usize> {
    use crate::features::launcher::ui::list::{list_bottom, list_top, slot_height, SlotKind};
    let top = list_top();
    let bottom = list_bottom(theme::size::PANEL_HEIGHT);
    if y < top || y >= bottom {
        return None;
    }
    let mut cursor = top - scroll;
    for item in items {
        let slot = match item {
            PaintItem::Header { .. } => SlotKind::Header,
            PaintItem::Row { .. } => SlotKind::Row,
            PaintItem::Calc { .. } => SlotKind::Calc,
            PaintItem::EmojiRow { glyphs, columns, .. } => SlotKind::EmojiRow {
                cells: glyphs.len(),
                columns: *columns,
            },
        };
        let h = slot_height(slot);
        if y >= cursor && y < cursor + h {
            return match item {
                PaintItem::EmojiRow {
                    glyphs,
                    columns,
                    start,
                    ..
                } => {
                    let col = (x / (panel_w / (*columns).max(1) as f32)).floor() as usize;
                    if col < glyphs.len() {
                        Some(*start + col)
                    } else {
                        None
                    }
                }
                _ => None,
            };
        }
        cursor += h;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_paint_items_are_grid_rows_with_group_headers() {
        let items = paint_items("", EmojiSkinTone::None, 0, theme::size::PANEL_WIDTH);
        assert!(items.iter().any(|i| matches!(i, PaintItem::Header { title } if title == "Smileys")));
        assert!(items
            .iter()
            .any(|i| matches!(i, PaintItem::EmojiRow { glyphs, .. } if !glyphs.is_empty())));
        assert_eq!(cell_dip(), 56.0);
        if let Some(PaintItem::EmojiRow { columns, .. }) = items
            .iter()
            .find(|i| matches!(i, PaintItem::EmojiRow { .. }))
        {
            assert_eq!(*columns, GRID_COLUMNS);
        }
    }

    #[test]
    fn emoji_cell_is_56_and_eight_columns() {
        assert_eq!(tinycast_pure::emoji::CELL_DIP, 56.0);
        let items = paint_items("", EmojiSkinTone::None, 0, 750.0);
        let row = items.iter().find_map(|i| match i {
            PaintItem::EmojiRow { columns, .. } => Some(*columns),
            _ => None,
        });
        assert_eq!(row, Some(8));
    }
}
