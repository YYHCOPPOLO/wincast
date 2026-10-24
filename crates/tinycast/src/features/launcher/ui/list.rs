use std::collections::HashMap;

use tinycast_pure::app_entry::AppEntry;
use tinycast_pure::favorites::FavoritesStore;
use tinycast_pure::hotkey::{DoubleTapModifier, HotKeyBinding};
use tinycast_pure::i18n::{favorites_title, kind_label, kind_section_title, results_title, UiLang};
use tinycast_pure::launcher_results::{
    list_items, LauncherListItem, LauncherSection, LauncherSectionKind,
};
use tinycast_pure::theme;
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, SIZE};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
    SIIGBF_RESIZETOFIT,
};

use super::coordinator::icon_source;

pub const ROW_HEIGHT: f32 = tinycast_pure::layout::list::ROW_HEIGHT;
pub const SECTION_HEADER_HEIGHT: f32 = theme::size::SECTION_HEADER_HEIGHT;

const ICON_CACHE_MAX: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotKind {
    Header,
    Row,
    ImageRow,
    Calc,
    EmojiRow { cells: usize, columns: usize },
}

#[derive(Clone, Debug)]
pub enum PaintItem {
    Header {
        title: String,
    },
    Row {
        title: String,
        alias: Option<String>,
        trailing: String,
        keycap: Option<String>,
        icon_source: Option<String>,
        /// A raster image for the leading slot; makes the row the taller
        /// clipboard image slot.
        thumbnail: Option<String>,
        /// SF Symbol name for the leading slot when the row has no icon.
        symbol: Option<String>,
        /// 0 none, 1 off, 2 on, 3 locked (the uninstall list's 16 DIP box).
        checkbox: u8,
        selected: bool,
    },
    Calc {
        expression: String,
        display: String,
        source_badge: Option<String>,
        target_badge: Option<String>,
        selected: bool,
        is_error: bool,
    },
    EmojiRow {
        glyphs: Vec<String>,
        columns: usize,
        start: usize,
        selected: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct IconKey {
    pub source: String,
    pub mtime: i64,
    pub dpi: u32,
    pub appearance: u8,
}

#[derive(Clone, Debug)]
pub struct IconPixels {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Default)]
pub struct IconCache {
    map: HashMap<IconKey, Option<IconPixels>>,
    bytes: usize,
}

impl IconCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn drop_all(&mut self) {
        self.map.clear();
        self.bytes = 0;
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn get_or_load(&mut self, source: &str, dpi: u32, appearance: u8) -> Option<&IconPixels> {
        let mtime = file_mtime(source);
        let key = IconKey {
            source: source.to_string(),
            mtime,
            dpi,
            appearance,
        };
        if !self.map.contains_key(&key) {
            let px = (theme::size::RESULT_ROW_ICON * dpi as f32 / 96.0).round() as i32;
            let loaded = load_shell_icon_bgra(source, px).map(|(width, height, bgra)| IconPixels {
                width,
                height,
                bgra,
            });
            if let Some(pixels) = &loaded {
                if self.bytes + pixels.bgra.len() > ICON_CACHE_MAX {
                    self.drop_all();
                }
                self.bytes += pixels.bgra.len();
            }
            self.map.insert(key.clone(), loaded);
        }
        self.map.get(&key).and_then(|slot| slot.as_ref())
    }

    #[cfg(test)]
    fn insert_test(&mut self, key: IconKey, pixels: IconPixels) {
        self.bytes += pixels.bgra.len();
        self.map.insert(key, Some(pixels));
    }
}

pub fn list_top() -> f32 {
    tinycast_pure::layout::list::content_top()
}

pub fn list_bottom(panel_h: f32) -> f32 {
    panel_h.max(list_top())
}

pub fn slot_height(kind: SlotKind) -> f32 {
    match kind {
        SlotKind::Header => SECTION_HEADER_HEIGHT,
        SlotKind::Row => ROW_HEIGHT,
        SlotKind::ImageRow => theme::size::CLIPBOARD_IMAGE_ROW_HEIGHT,
        SlotKind::Calc => theme::size::CALC_CARD_HEIGHT,
        SlotKind::EmojiRow { .. } => tinycast_pure::emoji::CELL_DIP,
    }
}

pub fn slots_of(items: &[PaintItem]) -> Vec<SlotKind> {
    items
        .iter()
        .map(|item| match item {
            PaintItem::Header { .. } => SlotKind::Header,
            PaintItem::Row { thumbnail, .. } => {
                if thumbnail.is_some() {
                    SlotKind::ImageRow
                } else {
                    SlotKind::Row
                }
            }
            PaintItem::Calc { .. } => SlotKind::Calc,
            PaintItem::EmojiRow {
                glyphs, columns, ..
            } => SlotKind::EmojiRow {
                cells: glyphs.len(),
                columns: *columns,
            },
        })
        .collect()
}

pub fn content_height(slots: &[SlotKind]) -> f32 {
    slots.iter().copied().map(slot_height).sum()
}

pub fn row_y(slots: &[SlotKind], selectable: usize) -> Option<(f32, f32)> {
    let mut y = 0.0;
    let mut idx = 0usize;
    for slot in slots {
        let h = slot_height(*slot);
        match *slot {
            SlotKind::Row | SlotKind::ImageRow | SlotKind::Calc => {
                if idx == selectable {
                    return Some((y, h));
                }
                idx += 1;
            }
            SlotKind::EmojiRow { cells, .. } => {
                if selectable >= idx && selectable < idx + cells {
                    return Some((y, h));
                }
                idx += cells;
            }
            SlotKind::Header => {}
        }
        y += h;
    }
    None
}

pub fn clamp_scroll(scroll: f32, content_h: f32, view_h: f32) -> f32 {
    let max = (content_h - view_h).max(0.0);
    scroll.clamp(0.0, max)
}

pub fn ensure_visible(scroll: f32, row_top: f32, row_h: f32, view_h: f32) -> f32 {
    if view_h <= 0.0 {
        return scroll;
    }
    if row_top < scroll {
        row_top
    } else if row_top + row_h > scroll + view_h {
        row_top + row_h - view_h
    } else {
        scroll
    }
}

/// Headers are not selectable. `y` is panel DIP.
pub fn selectable_at_y(
    slots: &[SlotKind],
    y: f32,
    scroll: f32,
    top: f32,
    bottom: f32,
) -> Option<usize> {
    if y < top || y >= bottom {
        return None;
    }
    let mut cursor = top - scroll;
    let mut selectable = 0usize;
    for slot in slots {
        let h = slot_height(*slot);
        if y >= cursor && y < cursor + h {
            return match slot {
                SlotKind::Header => None,
                SlotKind::Row | SlotKind::ImageRow | SlotKind::Calc => Some(selectable),
                SlotKind::EmojiRow { cells, .. } => {
                    if *cells == 0 {
                        None
                    } else {
                        Some(selectable)
                    }
                }
            };
        }
        match slot {
            SlotKind::Row | SlotKind::ImageRow | SlotKind::Calc => selectable += 1,
            SlotKind::EmojiRow { cells, .. } => selectable += *cells,
            SlotKind::Header => {}
        }
        cursor += h;
    }
    None
}

pub fn paint_items(
    sections: &[LauncherSection],
    selection: usize,
    _favorites: &FavoritesStore,
    lang: UiLang,
) -> Vec<PaintItem> {
    let mut out = Vec::new();
    let mut sel = 0usize;
    for item in list_items(sections) {
        match item {
            LauncherListItem::Header(kind) => {
                let title = match kind {
                    tinycast_pure::launcher_results::LauncherSectionKind::Favorites => {
                        favorites_title(lang).to_string()
                    }
                    tinycast_pure::launcher_results::LauncherSectionKind::Results => {
                        results_title(lang).to_string()
                    }
                    tinycast_pure::launcher_results::LauncherSectionKind::Kind(k) => {
                        kind_section_title(k, lang).to_string()
                    }
                };
                out.push(PaintItem::Header { title });
            }
            LauncherListItem::Row(entry) => {
                let fav_slot = favorite_slot_for(sections, entry, sel);
                let source = icon_source(entry);
                out.push(PaintItem::Row {
                    title: entry.name.clone(),
                    alias: entry.fields.user_alias.clone(),
                    trailing: kind_label(entry.kind, lang).to_string(),
                    keycap: keycap_label(entry, fav_slot),
                    // The kind glyph is the standing fallback: it only shows
                    // while the icon is missing or still decoding, so no row
                    // ever ships an empty icon slot.
                    symbol: Some(kind_glyph(entry.kind).to_string()),
                    icon_source: source,
                    thumbnail: None,
                    checkbox: 0,
                    selected: sel == selection,
                });
                sel += 1;
            }
        }
    }
    out
}

/// The SF name a row shows when its icon source is missing; the snapshot
/// layer turns it into the Segoe Fluent code point.
fn kind_glyph(kind: tinycast_pure::app_entry::AppKind) -> &'static str {
    use tinycast_pure::app_entry::AppKind;
    match kind {
        AppKind::Application => "square.grid.2x2",
        AppKind::SystemSettings => "gearshape",
        AppKind::Quicklink => "link",
        AppKind::Snippet => "curlybraces",
        AppKind::SystemAction => "bolt",
        AppKind::WindowCommand => "macwindow",
        AppKind::CustomCommand => "terminal",
        AppKind::Command => "info.circle",
    }
}

fn favorite_slot_for(
    sections: &[LauncherSection],
    entry: &AppEntry,
    _selectable: usize,
) -> Option<u8> {
    let fav = sections
        .iter()
        .find(|s| s.kind == LauncherSectionKind::Favorites)?;
    let i = fav.rows.iter().position(|r| r.id == entry.id)?;
    match i {
        0..=8 => Some(i as u8 + 1),
        9 => Some(0),
        _ => None,
    }
}

fn keycap_label(entry: &AppEntry, fav_slot: Option<u8>) -> Option<String> {
    if let Some(binding) = &entry.hotkey {
        return Some(format_hotkey(binding));
    }
    fav_slot.map(|n| n.to_string())
}

fn format_hotkey(binding: &HotKeyBinding) -> String {
    match binding {
        HotKeyBinding::Combo(shortcut) => {
            let mut parts = Vec::new();
            if shortcut.modifiers.ctrl {
                parts.push("Ctrl");
            }
            if shortcut.modifiers.alt {
                parts.push("Alt");
            }
            if shortcut.modifiers.shift {
                parts.push("Shift");
            }
            if shortcut.modifiers.win {
                parts.push("Win");
            }
            let key = vk_label(shortcut.vk);
            parts.push(key.as_str());
            parts.join("+")
        }
        HotKeyBinding::DoubleTap(m) => {
            let name = match m {
                DoubleTapModifier::Control => "Ctrl",
                DoubleTapModifier::Option => "Alt",
                DoubleTapModifier::Shift => "Shift",
                DoubleTapModifier::Command => "Win",
            };
            format!("Double {name}")
        }
    }
}

fn vk_label(vk: u16) -> String {
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x1B => "Esc".into(),
        0x20 => "Space".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x70..=0x7B => format!("F{}", vk - 0x6F),
        _ => format!("{:02X}", vk),
    }
}

fn load_shell_icon_bgra(source: &str, px: i32) -> Option<(u32, u32, Vec<u8>)> {
    if source.is_empty() || px <= 0 {
        return None;
    }
    let wide: Vec<u16> = source.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let hbmp = factory
            .GetImage(
                SIZE { cx: px, cy: px },
                SIIGBF_ICONONLY | SIIGBF_RESIZETOFIT,
            )
            .ok()?;
        let pixels = hbitmap_premultiplied_bgra(hbmp);
        let _ = DeleteObject(hbmp);
        pixels
    }
}

fn file_mtime(source: &str) -> i64 {
    if source.starts_with("shell:") {
        return 0;
    }
    std::fs::metadata(source)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn hbitmap_premultiplied_bgra(hbmp: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let mut bmp = BITMAP::default();
        if GetObjectW(
            hbmp,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut BITMAP as *mut core::ffi::c_void),
        ) == 0
        {
            return None;
        }
        let w = bmp.bmWidth.max(1) as u32;
        let h = bmp.bmHeight.unsigned_abs();
        if h == 0 {
            return None;
        }
        let mut bits = vec![0u8; (w * h * 4) as usize];
        let hdc = GetDC(HWND::default());
        if hdc.is_invalid() {
            return None;
        }
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let copied = GetDIBits(
            hdc,
            hbmp,
            0,
            h,
            Some(bits.as_mut_ptr() as *mut core::ffi::c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = ReleaseDC(HWND::default(), hdc);
        if copied == 0 {
            return None;
        }
        premultiply(&mut bits);
        Some((w, h, bits))
    }
}

fn premultiply(bits: &mut [u8]) {
    for px in bits.chunks_exact_mut(4) {
        let a = u16::from(px[3]);
        px[0] = ((u16::from(px[0]) * a) / 255) as u8;
        px[1] = ((u16::from(px[1]) * a) / 255) as u8;
        px[2] = ((u16::from(px[2]) * a) / 255) as u8;
    }
}

pub fn client_point_to_dip(x: i32, y: i32, dpi: u32) -> (f32, f32) {
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
    (x as f32 / scale, y as f32 / scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::app_entry::{AppEntry, AppKind};
    use tinycast_pure::launcher_ranking::LauncherRankingStore;
    use tinycast_pure::launcher_results::ordered_results;
    use tinycast_pure::search_relevance::SearchFields;
    use tinycast_pure::visibility::VisibilityStore;

    fn entry(id: &str, kind: AppKind, name: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            kind,
            name: name.into(),
            fields: SearchFields {
                display_name: name.into(),
                ..Default::default()
            },
            hotkey: None,
        }
    }

    #[test]
    fn row_metrics_match_theme_tokens() {
        assert_eq!(theme::size::RESULT_ROW_ICON, 26.0);
        assert_eq!(theme::radius::ROW, 10.0);
        assert_eq!(ROW_HEIGHT, 38.0);
    }

    #[test]
    fn fade_constants_match_spec() {
        assert_eq!(tinycast_pure::layout::list::edge_dissolve_top_band(), 86.0);
        assert_eq!(
            tinycast_pure::layout::list::edge_dissolve_bottom_band(),
            80.0
        );
    }

    #[test]
    fn ensure_visible_keeps_selection_above_footer() {
        let view = tinycast_pure::layout::list::view_height(theme::size::PANEL_HEIGHT);
        assert!((view - 359.0).abs() < 0.01);
        let row_top = 380.0;
        assert_eq!(
            ensure_visible(0.0, row_top, ROW_HEIGHT, view),
            row_top + ROW_HEIGHT - view
        );
    }

    #[test]
    fn row_height_matches_pure_layout() {
        assert_eq!(ROW_HEIGHT, tinycast_pure::layout::list::ROW_HEIGHT);
        assert_eq!(ROW_HEIGHT, 38.0);
    }

    #[test]
    fn calc_card_height_is_96() {
        assert_eq!(tinycast_pure::theme::size::CALC_CARD_HEIGHT, 96.0);
        assert_eq!(
            crate::features::launcher::ui::list::slot_height(
                crate::features::launcher::ui::list::SlotKind::Calc
            ),
            96.0
        );
    }

    #[test]
    fn calc_card_uses_card_radius() {
        assert_eq!(tinycast_pure::theme::radius::CARD, 10.0);
    }

    #[test]
    fn emoji_row_slot_is_56_dip() {
        assert_eq!(
            slot_height(SlotKind::EmojiRow {
                cells: 8,
                columns: 8
            }),
            tinycast_pure::emoji::CELL_DIP
        );
        assert_eq!(tinycast_pure::emoji::CELL_DIP, 56.0);
    }

    #[test]
    fn header_click_is_not_selectable() {
        let slots = [
            SlotKind::Header,
            SlotKind::Row,
            SlotKind::Header,
            SlotKind::Row,
        ];
        let top = list_top();
        let bottom = list_bottom(theme::size::PANEL_HEIGHT);
        assert_eq!(selectable_at_y(&slots, top + 2.0, 0.0, top, bottom), None);
        assert_eq!(
            selectable_at_y(&slots, top + SECTION_HEADER_HEIGHT + 2.0, 0.0, top, bottom),
            Some(0)
        );
        assert_eq!(
            selectable_at_y(
                &slots,
                top + SECTION_HEADER_HEIGHT + ROW_HEIGHT + 2.0,
                0.0,
                top,
                bottom
            ),
            None
        );
        assert_eq!(
            selectable_at_y(
                &slots,
                top + SECTION_HEADER_HEIGHT * 2.0 + ROW_HEIGHT + 2.0,
                0.0,
                top,
                bottom
            ),
            Some(1)
        );
    }

    #[test]
    fn ensure_visible_scrolls_selected_row_into_view() {
        let view = 100.0;
        assert_eq!(ensure_visible(0.0, 0.0, ROW_HEIGHT, view), 0.0);
        assert_eq!(ensure_visible(0.0, 80.0, ROW_HEIGHT, view), 18.0);
        assert_eq!(ensure_visible(50.0, 0.0, ROW_HEIGHT, view), 0.0);
    }

    #[test]
    fn drop_all_clears_icon_cache() {
        let mut cache = IconCache::new();
        cache.insert_test(
            IconKey {
                source: "x".into(),
                mtime: 1,
                dpi: 96,
                appearance: 0,
            },
            IconPixels {
                width: 1,
                height: 1,
                bgra: vec![0, 0, 0, 255],
            },
        );
        assert!(!cache.is_empty());
        cache.drop_all();
        assert!(cache.is_empty());
    }

    #[test]
    fn paint_items_skip_results_header_and_mark_selection() {
        let entries = [
            entry("app:a", AppKind::Application, "Alpha"),
            entry("command:quit", AppKind::Command, "Quit Tinycast"),
        ];
        let rank = LauncherRankingStore::load(std::path::PathBuf::from(
            "Z:\\tinycast-does-not-exist\\launcher-ranking.json",
        ));
        let vis = VisibilityStore::default();
        let fav = FavoritesStore::default();
        let sections = ordered_results(&entries, "a", 0, &rank, &vis, &fav, false);
        let items = paint_items(&sections, 0, &fav, UiLang::En);
        assert!(matches!(items[0], PaintItem::Row { selected: true, .. }));
        if let PaintItem::Row {
            title, trailing, ..
        } = &items[0]
        {
            assert_eq!(title, "Alpha");
            assert_eq!(trailing, "Application");
        } else {
            panic!("row");
        }
    }

    #[test]
    fn favorite_section_gets_number_keycaps() {
        let entries = [entry("app:a", AppKind::Application, "Alpha")];
        let rank = LauncherRankingStore::load(std::path::PathBuf::from(
            "Z:\\tinycast-does-not-exist\\launcher-ranking.json",
        ));
        let vis = VisibilityStore::default();
        let mut fav = FavoritesStore::default();
        fav.toggle("app:a".into());
        let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
        let items = paint_items(&sections, 0, &fav, UiLang::En);
        let row = items
            .iter()
            .find_map(|i| match i {
                PaintItem::Row { keycap, title, .. } if title == "Alpha" => keycap.clone(),
                _ => None,
            })
            .expect("row");
        assert_eq!(row, "1");
    }
}
