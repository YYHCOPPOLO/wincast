use std::collections::HashMap;

use tinycast_pure::app_entry::AppEntry;
use tinycast_pure::favorites::FavoritesStore;
use tinycast_pure::i18n::{favorites_title, kind_label, kind_section_title, results_title, UiLang};
use tinycast_pure::hotkey::{DoubleTapModifier, HotKeyBinding};
use tinycast_pure::launcher_results::{
    list_items, LauncherListItem, LauncherSection, LauncherSectionKind,
};
use tinycast_pure::theme;
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, SIZE};
use windows::Win32::Graphics::Direct2D::Common::D2D1_ALPHA_MODE_PREMULTIPLIED;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_GRADIENT_STOP, D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_PROPERTIES, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_METRICS, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
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
pub const SECTION_HEADER_HEIGHT: f32 = 22.0;

fn fade_top() -> f32 {
    tinycast_pure::layout::list::edge_dissolve_top_band()
}

fn fade_bottom() -> f32 {
    tinycast_pure::layout::list::edge_dissolve_bottom_band()
}

#[cfg(test)]
pub(crate) fn fade_top_for_test() -> f32 {
    fade_top()
}

fn fade_visible(panel_h: f32) -> f32 {
    tinycast_pure::layout::list::view_height(panel_h)
}

#[cfg(test)]
pub(crate) fn fade_visible_for_test(panel_h: f32) -> f32 {
    fade_visible(panel_h)
}
const ICON_CACHE_MAX: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotKind {
    Header,
    Row,
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
            let px = (theme::size::ROW_ICON * dpi as f32 / 96.0).round() as i32;
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

pub struct ListFonts {
    pub title: IDWriteTextFormat,
    pub trailing: IDWriteTextFormat,
    pub header: IDWriteTextFormat,
    pub chip: IDWriteTextFormat,
    pub keycap: IDWriteTextFormat,
    pub calc_result: IDWriteTextFormat,
    pub calc_badge: IDWriteTextFormat,
    pub emoji: IDWriteTextFormat,
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
        SlotKind::Calc => theme::size::CALC_CARD_HEIGHT,
        SlotKind::EmojiRow { .. } => tinycast_pure::emoji::CELL_DIP,
    }
}

pub fn slots_of(items: &[PaintItem]) -> Vec<SlotKind> {
    items
        .iter()
        .map(|item| match item {
            PaintItem::Header { .. } => SlotKind::Header,
            PaintItem::Row { .. } => SlotKind::Row,
            PaintItem::Calc { .. } => SlotKind::Calc,
            PaintItem::EmojiRow { glyphs, columns, .. } => SlotKind::EmojiRow {
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
            SlotKind::Row | SlotKind::Calc => {
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
                SlotKind::Row | SlotKind::Calc => Some(selectable),
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
            SlotKind::Row | SlotKind::Calc => selectable += 1,
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
                out.push(PaintItem::Row {
                    title: entry.name.clone(),
                    alias: entry.fields.user_alias.clone(),
                    trailing: kind_label(entry.kind, lang).to_string(),
                    keycap: keycap_label(entry, fav_slot),
                    icon_source: icon_source(entry),
                    selected: sel == selection,
                });
                sel += 1;
            }
        }
    }
    out
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

pub fn paint(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    items: &[PaintItem],
    scroll: f32,
    panel_w: f32,
    panel_h: f32,
    cache: &mut IconCache,
    dpi: f32,
    appearance: u8,
    empty_results: Option<&str>,
) -> windows::core::Result<()> {
    let origin = list_top();
    let clip_top = tinycast_pure::layout::list::paint_clip_top();
    let bottom = list_bottom(panel_h);
    if bottom - origin < ROW_HEIGHT {
        return Ok(());
    }
    if items.is_empty() {
        if let Some(text) = empty_results {
            paint_empty_results(target, dwrite, text, panel_w, panel_h, appearance)?;
        }
        return Ok(());
    }
    let clip = D2D_RECT_F {
        left: 0.0,
        top: clip_top,
        right: panel_w,
        bottom,
    };
    unsafe {
        target.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    }
    let mut y = origin - scroll;
    for item in items {
        match item {
            PaintItem::Header { title } => {
                let h = SECTION_HEADER_HEIGHT;
                if y + h > clip_top && y < bottom {
                    paint_header(target, fonts, title, y, panel_w, h)?;
                }
                y += h;
            }
            PaintItem::Row {
                title,
                alias,
                trailing,
                keycap,
                icon_source,
                selected,
            } => {
                let h = ROW_HEIGHT;
                if y + h > clip_top && y < bottom {
                    paint_row(
                        target,
                        dwrite,
                        fonts,
                        title,
                        alias.as_deref(),
                        trailing,
                        keycap.as_deref(),
                        icon_source.as_deref(),
                        *selected,
                        y,
                        panel_w,
                        h,
                        cache,
                        dpi,
                        appearance,
                    )?;
                }
                y += h;
            }
            PaintItem::Calc {
                expression,
                display,
                source_badge,
                target_badge,
                selected,
                is_error,
            } => {
                let h = theme::size::CALC_CARD_HEIGHT;
                if y + h > clip_top && y < bottom {
                    crate::features::calculator::ui::card::paint(
                        target,
                        dwrite,
                        fonts,
                        expression,
                        display,
                        source_badge.as_deref(),
                        target_badge.as_deref(),
                        *selected,
                        *is_error,
                        y,
                        panel_w,
                        h,
                        appearance,
                    )?;
                }
                y += h;
            }
            PaintItem::EmojiRow {
                glyphs,
                columns,
                start,
                selected,
            } => {
                let h = tinycast_pure::emoji::CELL_DIP;
                if y + h > clip_top && y < bottom {
                    paint_emoji_row(
                        target, fonts, glyphs, *columns, *start, *selected, y, panel_w, h,
                    )?;
                }
                y += h;
            }
        }
        if y > bottom + tinycast_pure::emoji::CELL_DIP {
            break;
        }
    }
    unsafe {
        target.PopAxisAlignedClip();
    }
    let content_h = content_height(&slots_of(items));
    let visible = fade_visible(panel_h);
    paint_fade(
        target,
        panel_w,
        clip_top,
        bottom,
        content_h,
        visible,
        appearance,
    )?;
    Ok(())
}

fn paint_empty_results(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    text: &str,
    panel_w: f32,
    panel_h: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let block = tinycast_pure::layout::list::empty_results_center(panel_w, panel_h);
    let glyph_size = 32.0;
    let glyph = tinycast_pure::palette_placement::DipRect {
        x: (panel_w - glyph_size) / 2.0,
        y: block.y,
        w: glyph_size,
        h: glyph_size,
    };
    let tertiary = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    );
    crate::design_system::symbols::paint_fluent_in(
        target,
        dwrite,
        "magnifyingglass",
        glyph,
        tertiary,
    )?;
    let format = unsafe {
        dwrite.CreateTextFormat(
            w!("Microsoft YaHei UI"),
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            theme::typography::ROW_TITLE,
            w!("en-US"),
        )?
    };
    unsafe {
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
    }
    let secondary = theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    );
    let brush = unsafe {
        target.CreateSolidColorBrush(
            &crate::design_system::appearance::color(secondary),
            None,
        )?
    };
    let wide: Vec<u16> = text.encode_utf16().collect();
    let text_top = glyph.y + glyph.h + theme::spacing::MD;
    unsafe {
        target.DrawText(
            &wide,
            &format,
            &D2D_RECT_F {
                left: 0.0,
                top: text_top,
                right: panel_w,
                bottom: text_top + theme::typography::ROW_TITLE + theme::spacing::SM,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_emoji_row(
    target: &ID2D1RenderTarget,
    fonts: &ListFonts,
    glyphs: &[String],
    columns: usize,
    start: usize,
    selected: usize,
    y: f32,
    panel_w: f32,
    h: f32,
) -> windows::core::Result<()> {
    let cols = columns.max(1);
    let cell_w = panel_w / cols as f32;
    for (col, glyph) in glyphs.iter().enumerate() {
        let x = col as f32 * cell_w;
        let index = start + col;
        if index == selected {
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x + 2.0,
                    top: y + 2.0,
                    right: x + cell_w - 2.0,
                    bottom: y + h - 2.0,
                },
                radiusX: theme::radius::ROW,
                radiusY: theme::radius::ROW,
            };
            let brush = unsafe { target.CreateSolidColorBrush(&selection_color(), None)? };
            unsafe {
                target.FillRoundedRectangle(&pill, &brush);
            }
        }
        let rect = D2D_RECT_F {
            left: x,
            top: y,
            right: x + cell_w,
            bottom: y + h,
        };
        let brush = unsafe {
            target.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.95,
                },
                None,
            )?
        };
        let wide: Vec<u16> = glyph.encode_utf16().collect();
        unsafe {
            target.DrawText(
                &wide,
                &fonts.emoji,
                &rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
    Ok(())
}

fn paint_header(
    target: &ID2D1RenderTarget,
    fonts: &ListFonts,
    title: &str,
    y: f32,
    panel_w: f32,
    h: f32,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&muted_color(theme::colors::TEXT_SECONDARY_ALPHA), None)? };
    let rect = D2D_RECT_F {
        left: theme::spacing::MD,
        top: y,
        right: panel_w - theme::spacing::MD,
        bottom: y + h,
    };
    draw_text(target, &fonts.header, &brush, rect, title)
}

fn paint_row(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    title: &str,
    alias: Option<&str>,
    trailing: &str,
    keycap: Option<&str>,
    icon: Option<&str>,
    selected: bool,
    y: f32,
    panel_w: f32,
    h: f32,
    cache: &mut IconCache,
    dpi: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let inset = theme::spacing::MD;
    if selected {
        crate::design_system::paint_row_fill(target, panel_w, y, true, false, appearance)?;
    }

    let icon_rect = tinycast_pure::layout::list::row_icon_rect(y);
    let icon_size = icon_rect.w;
    let icon_x = icon_rect.x;
    let icon_y = icon_rect.y;
    if let Some(source) = icon {
        if let Some(pixels) = cache.get_or_load(source, dpi.round() as u32, appearance) {
            let _ = draw_icon(target, pixels, icon_x, icon_y, icon_size);
        } else {
            paint_icon_placeholder(target, icon_x, icon_y, icon_size)?;
        }
    } else {
        paint_icon_placeholder(target, icon_x, icon_y, icon_size)?;
    }

    let mut right = panel_w - inset;
    let chrome = unsafe { target.CreateSolidColorBrush(&muted_color(theme::colors::TEXT_SECONDARY_ALPHA), None)? };
    if let Some(cap) = keycap {
        let ds = crate::design_system::Fonts::new(dwrite)?;
        let cap_w = crate::design_system::paint_keycap(
            target, &ds, cap, right, y, h, true, appearance,
        )?;
        right -= cap_w + theme::spacing::SM;
    }

    if !trailing.is_empty() {
        let tw = text_width(dwrite, &fonts.trailing, trailing, 180.0, h) + theme::spacing::XS;
        let rect = D2D_RECT_F {
            left: (right - tw).max(inset),
            top: y,
            right,
            bottom: y + h,
        };
        draw_text(target, &fonts.trailing, &chrome, rect, trailing)?;
        right = rect.left - theme::spacing::SM;
    }

    let mut text_left = icon_x + icon_size + theme::spacing::LG;
    let title_brush = unsafe { target.CreateSolidColorBrush(&title_color(), None)? };
    let chip_w = alias
        .map(|a| text_width(dwrite, &fonts.chip, a, 160.0, h) + theme::spacing::MD * 2.0)
        .unwrap_or(0.0);
    let title_right = if chip_w > 0.0 {
        (right - chip_w - theme::spacing::SM).max(text_left + 8.0)
    } else {
        right
    };
    draw_trimmed(
        target,
        dwrite,
        &fonts.title,
        &title_brush,
        text_left,
        y,
        (title_right - text_left).max(8.0),
        h,
        title,
    )?;
    if let Some(alias) = alias {
        text_left = title_right + theme::spacing::SM;
        if text_left + chip_w <= right + 0.5 {
            let chip_h = 18.0;
            let chip_y = y + (h - chip_h) / 2.0;
            let rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: text_left,
                    top: chip_y,
                    right: text_left + chip_w,
                    bottom: chip_y + chip_h,
                },
                radiusX: theme::radius::MENU,
                radiusY: theme::radius::MENU,
            };
            unsafe {
                target.FillRoundedRectangle(&rounded, &chrome);
            }
            draw_text(target, &fonts.chip, &title_brush, rounded.rect, alias)?;
        }
    }
    Ok(())
}

pub fn paint_icon_at(
    target: &ID2D1RenderTarget,
    cache: &mut IconCache,
    source: Option<&str>,
    x: f32,
    y: f32,
    size: f32,
    dpi: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    if let Some(source) = source {
        if let Some(pixels) = cache.get_or_load(source, dpi.round() as u32, appearance) {
            let _ = draw_icon(target, pixels, x, y, size);
            return Ok(());
        }
    }
    paint_icon_placeholder(target, x, y, size)
}

fn paint_icon_placeholder(
    target: &ID2D1RenderTarget,
    x: f32,
    y: f32,
    size: f32,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&muted_color(0.18), None)? };
    let rounded = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x,
            top: y,
            right: x + size,
            bottom: y + size,
        },
        radiusX: theme::radius::THUMBNAIL,
        radiusY: theme::radius::THUMBNAIL,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &brush);
    }
    Ok(())
}

fn draw_icon(
    target: &ID2D1RenderTarget,
    pixels: &IconPixels,
    x: f32,
    y: f32,
    size: f32,
) -> windows::core::Result<()> {
    let props = D2D1_BITMAP_PROPERTIES {
        pixelFormat: windows::Win32::Graphics::Direct2D::Common::D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
    };
    let bitmap = unsafe {
        target.CreateBitmap(
            D2D_SIZE_U {
                width: pixels.width,
                height: pixels.height,
            },
            Some(pixels.bgra.as_ptr() as *const core::ffi::c_void),
            pixels.width * 4,
            &props,
        )?
    };
    let dest = D2D_RECT_F {
        left: x,
        top: y,
        right: x + size,
        bottom: y + size,
    };
    unsafe {
        target.DrawBitmap(
            &bitmap,
            Some(&dest),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            None,
        );
    }
    Ok(())
}

fn paint_fade(
    target: &ID2D1RenderTarget,
    panel_w: f32,
    clip_top: f32,
    bottom: f32,
    content_h: f32,
    visible: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    if content_h <= visible + 0.5 {
        return Ok(());
    }
    let top_band = fade_top();
    let bot_band = fade_bottom();
    if top_band <= 1.0 && bot_band <= 1.0 {
        return Ok(());
    }
    let (sr, sg, sb, _) = theme::colors::scrim_rgba(appearance);
    let top_floor = 0.15;
    let bot_floor = 0.25;
    let top_outer = D2D1_COLOR_F {
        r: sr,
        g: sg,
        b: sb,
        a: 1.0 - top_floor,
    };
    let bot_outer = D2D1_COLOR_F {
        r: sr,
        g: sg,
        b: sb,
        a: 1.0 - bot_floor,
    };
    let clear = D2D1_COLOR_F {
        r: sr,
        g: sg,
        b: sb,
        a: 0.0,
    };
    if top_band > 1.0 {
        fill_fade(
            target,
            panel_w,
            clip_top,
            clip_top + top_band,
            top_outer,
            clear,
        )?;
    }
    if bot_band > 1.0 {
        fill_fade(
            target,
            panel_w,
            (bottom - bot_band).max(clip_top),
            bottom,
            clear,
            bot_outer,
        )?;
    }
    Ok(())
}

fn fill_fade(
    target: &ID2D1RenderTarget,
    panel_w: f32,
    y0: f32,
    y1: f32,
    c0: D2D1_COLOR_F,
    c1: D2D1_COLOR_F,
) -> windows::core::Result<()> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: c0,
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: c1,
        },
    ];
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: D2D_POINT_2F { x: 0.0, y: y0 },
            endPoint: D2D_POINT_2F { x: 0.0, y: y1 },
        };
        let brush = target.CreateLinearGradientBrush(&props, None, &collection)?;
        target.FillRectangle(
            &D2D_RECT_F {
                left: 0.0,
                top: y0,
                right: panel_w,
                bottom: y1,
            },
            &brush,
        );
    }
    Ok(())
}

fn draw_text(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    text: &str,
) -> windows::core::Result<()> {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn draw_trimmed(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: &str,
) -> windows::core::Result<()> {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let layout = dwrite.CreateTextLayout(&wide, format, w.max(1.0), h)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        let _ = layout.SetTrimming(&trimming, None);
        target.DrawTextLayout(
            D2D_POINT_2F { x, y },
            &layout,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
    }
    Ok(())
}

fn text_width(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &str,
    max_w: f32,
    h: f32,
) -> f32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let Ok(layout) = dwrite.CreateTextLayout(&wide, format, max_w, h) else {
            return 0.0;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        if layout.GetMetrics(&mut metrics).is_err() {
            return 0.0;
        }
        metrics.width
    }
}

fn selection_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: theme::colors::SELECTION_DARK_ALPHA,
    }
}

fn title_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    }
}

fn muted_color(a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
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
        assert_eq!(theme::size::ROW_ICON, 24.0);
        assert_eq!(theme::radius::ROW, 10.0);
        assert_eq!(ROW_HEIGHT, 36.0);
    }

    #[test]
    fn fade_constants_match_spec() {
        assert_eq!(tinycast_pure::layout::list::edge_dissolve_top_band(), 86.0);
        assert_eq!(tinycast_pure::layout::list::edge_dissolve_bottom_band(), 80.0);
    }

    #[test]
    fn list_fade_uses_dissolve_bands() {
        assert!((crate::features::launcher::ui::list::fade_top_for_test() - 86.0).abs() < 0.01);
    }

    #[test]
    fn fade_visible_is_between_bars() {
        assert!(
            (crate::features::launcher::ui::list::fade_visible_for_test(475.0) - 359.0).abs()
                < 0.01
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
        assert_eq!(ROW_HEIGHT, 36.0);
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
        assert_eq!(ensure_visible(0.0, 80.0, ROW_HEIGHT, view), 16.0);
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
