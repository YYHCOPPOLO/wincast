//! `PaintItem` → Slint rows for the visible slice of the palette list.
//!
//! Only the rows intersecting the viewport (plus one row of overscan on each
//! side) are pushed into the model, so a two thousand entry launcher never
//! builds two thousand components.

use std::collections::HashMap;

use slint::{ModelRc, SharedString, VecModel};
use tinycast_pure::ai::{ChatMessage, ChatRole, ChatState};
use tinycast_pure::layout::list as chat_layout;
use tinycast_pure::theme;
use tinycast_ui::{ChatRow, PaletteRow};

use crate::features::launcher::ui::list::{
    slot_height, slots_of, IconCache, IconPixels, PaintItem,
};

/// Row kind numbers shared with `ui/surfaces/palette.slint`.
const KIND_ROW: i32 = 0;
const KIND_HEADER: i32 = 1;
const KIND_CALC: i32 = 2;
const KIND_EMOJI: i32 = 3;

/// The uninstall list's locked rows draw a lock instead of a box.
const CHECKBOX_LOCKED: u8 = 3;

/// Transcript roles shared with `ui/surfaces/palette.slint`.
const CHAT_USER: i32 = 0;
const CHAT_ASSISTANT: i32 = 1;
const CHAT_NOTICE: i32 = 2;

/// Rows kept above and below the viewport so a wheel notch never shows a gap.
const OVERSCAN: f32 = 76.0;
/// Decoded images kept for the current DPI and appearance.
const IMAGE_CACHE_MAX: usize = 96;

pub struct VisibleList {
    pub rows: Vec<PaletteRow>,
    pub content_height: f32,
}

/// Decoded shell icons reused between refreshes; cleared when the DPI or the
/// appearance changes, because the cache key lives in `IconCache`.
pub struct RowImages {
    map: HashMap<String, slint::Image>,
    dpi: u32,
    appearance: u8,
}

impl Default for RowImages {
    fn default() -> Self {
        Self::new()
    }
}

impl RowImages {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            dpi: 0,
            appearance: 0,
        }
    }

    fn image(
        &mut self,
        source: &str,
        cache: &mut IconCache,
        dpi: u32,
        appearance: u8,
    ) -> Option<slint::Image> {
        if self.dpi != dpi || self.appearance != appearance {
            self.dpi = dpi;
            self.appearance = appearance;
            self.map.clear();
        }
        if let Some(image) = self.map.get(source) {
            return Some(image.clone());
        }
        let image = rgba_image(cache.get_or_load(source, dpi, appearance)?)?;
        if self.map.len() >= IMAGE_CACHE_MAX {
            self.map.clear();
        }
        self.map.insert(source.to_string(), image.clone());
        Some(image)
    }
}

/// The rows between `scroll` and `scroll + view`, with one row of overscan.
pub fn visible_rows(
    items: &[PaintItem],
    scroll: f32,
    view: f32,
    icons: &mut IconCache,
    images: &mut RowImages,
    dpi: u32,
    appearance: u8,
) -> VisibleList {
    let slots = slots_of(items);
    let content_height = slots.iter().copied().map(slot_height).sum();
    let top = (scroll - OVERSCAN).max(0.0);
    let bottom = scroll + view + OVERSCAN;
    let mut rows = Vec::new();
    let mut y = 0.0;
    for (item, slot) in items.iter().zip(slots.iter()) {
        let height = slot_height(*slot);
        if y + height >= top && y <= bottom {
            rows.push(to_row(item, y, height, icons, images, dpi, appearance));
        }
        y += height;
        if y > bottom {
            break;
        }
    }
    VisibleList {
        rows,
        content_height,
    }
}

fn to_row(
    item: &PaintItem,
    top: f32,
    height: f32,
    icons: &mut IconCache,
    images: &mut RowImages,
    dpi: u32,
    appearance: u8,
) -> PaletteRow {
    match item {
        PaintItem::Header { title } => {
            let mut row = base(KIND_HEADER, top, height);
            row.title = title.as_str().into();
            row
        }
        PaintItem::Row {
            title,
            alias,
            trailing,
            keycap,
            icon_source,
            symbol,
            checkbox,
            selected,
        } => {
            let mut row = base(KIND_ROW, top, height);
            row.title = title.as_str().into();
            // The alias is its own capsule after the title, never the trailing column.
            row.alias = alias.as_deref().unwrap_or_default().into();
            row.trailing = trailing.as_str().into();
            row.keycap = keycap.as_deref().unwrap_or_default().into();
            row.symbol = glyph(symbol.as_deref());
            row.checkbox = i32::from(*checkbox);
            if *checkbox == CHECKBOX_LOCKED {
                row.symbol = glyph(Some("lock.fill"));
            }
            row.selected = *selected;
            if let Some(source) = icon_source.as_deref() {
                if let Some(image) = images.image(source, icons, dpi, appearance) {
                    row.icon = image;
                    row.icon_ready = true;
                }
            }
            row
        }
        PaintItem::Calc {
            expression,
            display,
            source_badge,
            target_badge,
            selected,
            is_error,
        } => {
            let mut row = base(KIND_CALC, top, height);
            row.expression = expression.as_str().into();
            row.title = display.as_str().into();
            row.badge_left = source_badge.as_deref().unwrap_or_default().into();
            row.badge_right = target_badge.as_deref().unwrap_or_default().into();
            row.is_error = *is_error;
            if *is_error {
                row.symbol = glyph(Some("exclamationmark.triangle"));
            }
            row.selected = *selected;
            row
        }
        PaintItem::EmojiRow {
            glyphs,
            columns,
            start,
            selected,
        } => {
            let mut row = base(KIND_EMOJI, top, height);
            let cells: Vec<SharedString> = glyphs.iter().map(|g| g.as_str().into()).collect();
            row.glyphs = ModelRc::new(VecModel::from(cells));
            row.columns = *columns as i32;
            row.emoji_start = *start as i32;
            row.emoji_selected = *selected as i32;
            row
        }
    }
}

/// The AI transcript as fixed blocks. Tops and heights come from the same
/// `layout::list` functions `AppCore` clamps the scroll with, so the wheel can
/// always reach the last row and never scroll past it. The assistant's markdown
/// is flattened here, not in the surface: the renderer only wraps plain text.
pub fn chat_rows(messages: &[ChatMessage], notice: Option<&str>) -> Vec<ChatRow> {
    let panel_w = theme::size::PANEL_WIDTH;
    let gap = chat_layout::chat_message_gap();
    let mut rows = Vec::new();
    let mut top = chat_layout::chat_pad_top();
    let mut any = false;
    for message in messages {
        if any {
            top += gap;
        }
        any = true;
        let is_user = message.role == ChatRole::User;
        let text = if is_user {
            message.text.clone()
        } else {
            crate::features::ai::ui::screen::assistant_plain(&message.text)
        };
        let height = chat_layout::chat_message_height(is_user, &message.text, panel_w);
        rows.push(ChatRow {
            role: if is_user { CHAT_USER } else { CHAT_ASSISTANT },
            text: text.into(),
            failed: message.state == ChatState::Failed,
            streaming: message.state == ChatState::Streaming,
            top,
            height,
        });
        top += height;
    }
    if let Some(notice) = notice.filter(|s| !s.is_empty()) {
        if any {
            top += gap;
        }
        rows.push(ChatRow {
            role: CHAT_NOTICE,
            text: notice.into(),
            failed: false,
            streaming: false,
            top,
            height: chat_layout::CHAT_NOTICE_HEIGHT,
        });
    }
    rows
}

/// The icon's logical name, as the surface's GlyphIcon set knows it.
fn glyph(symbol: Option<&str>) -> SharedString {
    symbol.map(SharedString::from).unwrap_or_default()
}

fn base(kind: i32, top: f32, height: f32) -> PaletteRow {
    PaletteRow {
        kind,
        title: Default::default(),
        trailing: Default::default(),
        keycap: Default::default(),
        symbol: Default::default(),
        alias: Default::default(),
        expression: Default::default(),
        badge_left: Default::default(),
        badge_right: Default::default(),
        is_error: false,
        glyphs: Default::default(),
        columns: 0,
        emoji_start: 0,
        emoji_selected: 0,
        checkbox: 0,
        icon: Default::default(),
        icon_ready: false,
        selected: false,
        top,
        height,
    }
}

/// BGRA premultiplied from `list.rs::premultiply` to Slint's RGBA.
fn rgba_image(pixels: &IconPixels) -> Option<slint::Image> {
    if pixels.width == 0 || pixels.height == 0 {
        return None;
    }
    let mut buffer =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(pixels.width, pixels.height);
    let bytes = buffer.make_mut_slice();
    if pixels.bgra.len() != bytes.len() * 4 {
        return None;
    }
    for (dst, src) in bytes.iter_mut().zip(pixels.bgra.chunks_exact(4)) {
        dst.r = src[2];
        dst.g = src[1];
        dst.b = src[0];
        dst.a = src[3];
    }
    Some(slint::Image::from_rgba8_premultiplied(buffer))
}

/// Panel DIP height for the current expansion state.
pub fn panel_height(expanded: bool) -> f32 {
    if expanded {
        theme::size::PANEL_HEIGHT
    } else {
        theme::size::COMPACT_HEIGHT
    }
}
