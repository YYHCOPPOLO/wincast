//! UI text formats. Latin text is Segoe UI Variable at the optical size for its point
//! size, falling back to Segoe UI; CJK text is routed to Microsoft YaHei UI through a
//! DirectWrite font fallback chain (`tinycast_pure::typeface`). The system `EDIT`
//! controls (search field, Notes) still use a GDI YaHei UI font for the IME.

use std::cell::RefCell;
use std::collections::HashMap;

use tinycast_pure::{theme, typeface};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteFactory2, IDWriteFontCollection, IDWriteFontFallback, IDWriteTextFormat,
    IDWriteTextFormat1, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_METRICS,
    DWRITE_UNICODE_RANGE, DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, FW_NORMAL,
    HFONT, OUT_DEFAULT_PRECIS,
};

/// The Latin family body text asks for (before installation is checked).
pub fn ui_font_family() -> &'static str {
    typeface::LATIN_TEXT
}

/// The family the GDI `EDIT` controls use. Kept on YaHei UI so the IME composition and
/// candidate window keep working; retuned with the palette phase.
pub fn gdi_font_family() -> &'static str {
    typeface::CJK[0]
}

pub fn create_gdi_ui_font(height: i32) -> HFONT {
    for family in [
        gdi_font_family(),
        typeface::CJK[1],
        typeface::LATIN_FALLBACK,
    ] {
        let wide: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
        let font = unsafe {
            CreateFontW(
                height,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32,
                PCWSTR(wide.as_ptr()),
            )
        };
        if !font.is_invalid() {
            return font;
        }
    }
    HFONT::default()
}

pub struct Fonts {
    pub search: IDWriteTextFormat,
    pub row_title: IDWriteTextFormat,
    pub trailing: IDWriteTextFormat,
    pub section: IDWriteTextFormat,
    pub bar: IDWriteTextFormat,
    pub bar_center: IDWriteTextFormat,
    pub keycap: IDWriteTextFormat,
    pub headline: IDWriteTextFormat,
    pub headline_center: IDWriteTextFormat,
    pub wrap_callout: IDWriteTextFormat,
    pub wrap_body: IDWriteTextFormat,
    pub(crate) dwrite: IDWriteFactory,
}

impl Fonts {
    pub fn new(dwrite: &IDWriteFactory) -> windows::core::Result<Self> {
        Self::with_locale(dwrite, "zh-CN")
    }

    pub fn with_locale(dwrite: &IDWriteFactory, locale: &str) -> windows::core::Result<Self> {
        Ok(Self {
            search: make(
                dwrite,
                theme::typography::SEARCH_FIELD,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                locale,
            )?,
            row_title: make(
                dwrite,
                theme::typography::ROW_TITLE,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                locale,
            )?,
            trailing: make(
                dwrite,
                theme::typography::ROW_TRAILING,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                locale,
            )?,
            section: make(
                dwrite,
                theme::typography::SECTION_HEADER,
                DWRITE_FONT_WEIGHT_MEDIUM,
                false,
                locale,
            )?,
            bar: make(
                dwrite,
                theme::typography::BAR,
                DWRITE_FONT_WEIGHT_MEDIUM,
                false,
                locale,
            )?,
            bar_center: make(
                dwrite,
                theme::typography::BAR,
                DWRITE_FONT_WEIGHT_MEDIUM,
                true,
                locale,
            )?,
            keycap: make(
                dwrite,
                theme::typography::KEY_CAP,
                DWRITE_FONT_WEIGHT_REGULAR,
                true,
                locale,
            )?,
            headline: make(
                dwrite,
                theme::typography::PANEL_TITLE,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                false,
                locale,
            )?,
            headline_center: make(
                dwrite,
                theme::typography::PANEL_TITLE,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                true,
                locale,
            )?,
            wrap_callout: make_wrap(
                dwrite,
                theme::typography::ROW_TRAILING,
                DWRITE_FONT_WEIGHT_REGULAR,
                locale,
            )?,
            wrap_body: make_wrap(
                dwrite,
                theme::typography::ROW_TITLE,
                DWRITE_FONT_WEIGHT_REGULAR,
                locale,
            )?,
            dwrite: dwrite.clone(),
        })
    }

    pub fn measure(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        max_w: f32,
        max_h: f32,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, 0.0);
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let Ok(layout) =
                self.dwrite
                    .CreateTextLayout(&wide, format, max_w.max(1.0), max_h.max(1.0))
            else {
                return (0.0, 0.0);
            };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut metrics).is_err() {
                return (0.0, 0.0);
            }
            (metrics.width, metrics.height)
        }
    }
}

fn make(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    center: bool,
    locale: &str,
) -> windows::core::Result<IDWriteTextFormat> {
    let format = create_ui_format(dwrite, size, weight, locale)?;
    unsafe {
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        if center {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        } else {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
        }
    }
    Ok(format)
}

fn make_wrap(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    locale: &str,
) -> windows::core::Result<IDWriteTextFormat> {
    let format = create_ui_format(dwrite, size, weight, locale)?;
    unsafe {
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
    }
    Ok(format)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

thread_local! {
    static INSTALLED: RefCell<HashMap<&'static str, bool>> = RefCell::new(HashMap::new());
}

fn installed(dwrite: &IDWriteFactory, family: &'static str) -> bool {
    if let Some(hit) = INSTALLED.with(|c| c.borrow().get(family).copied()) {
        return hit;
    }
    let found = unsafe {
        let mut collection: Option<IDWriteFontCollection> = None;
        if dwrite
            .GetSystemFontCollection(&mut collection, BOOL(0))
            .is_err()
        {
            false
        } else if let Some(collection) = collection {
            let name = wide(family);
            let mut index = 0u32;
            let mut exists = BOOL(0);
            collection
                .FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists)
                .is_ok()
                && exists.as_bool()
        } else {
            false
        }
    };
    INSTALLED.with(|c| c.borrow_mut().insert(family, found));
    found
}

/// First installed Latin family for this size. `CreateTextFormat` never fails on a
/// missing family (it silently substitutes), so presence is checked up front.
pub fn resolve_latin_family(dwrite: &IDWriteFactory, size: f32) -> &'static str {
    typeface::latin_chain(size)
        .into_iter()
        .find(|family| installed(dwrite, family))
        .unwrap_or(typeface::LATIN_FALLBACK)
}

/// CJK ranges → YaHei UI, then everything the system fallback knows (emoji, symbols,
/// other scripts). `None` before Windows 8.1, where `IDWriteFactory2` is missing.
pub fn cjk_fallback(dwrite: &IDWriteFactory) -> Option<IDWriteFontFallback> {
    let factory2: IDWriteFactory2 = dwrite.cast().ok()?;
    unsafe {
        let builder = factory2.CreateFontFallbackBuilder().ok()?;
        let ranges: Vec<DWRITE_UNICODE_RANGE> = typeface::CJK_RANGES
            .iter()
            .map(|&(first, last)| DWRITE_UNICODE_RANGE { first, last })
            .collect();
        let names: Vec<Vec<u16>> = typeface::CJK
            .iter()
            .filter(|family| installed(dwrite, family))
            .map(|family| wide(family))
            .collect();
        if !names.is_empty() {
            let ptrs: Vec<*const u16> = names.iter().map(|n| n.as_ptr()).collect();
            builder
                .AddMapping(
                    &ranges,
                    &ptrs,
                    None::<&IDWriteFontCollection>,
                    PCWSTR::null(),
                    PCWSTR::null(),
                    1.0,
                )
                .ok()?;
        }
        if let Ok(system) = factory2.GetSystemFontFallback() {
            builder.AddMappings(&system).ok()?;
        }
        builder.CreateFontFallback().ok()
    }
}

fn create_ui_format(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    locale: &str,
) -> windows::core::Result<IDWriteTextFormat> {
    let locale_w = wide(locale);
    let family = wide(resolve_latin_family(dwrite, size));
    let format = unsafe {
        dwrite.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            PCWSTR(locale_w.as_ptr()),
        )?
    };
    if let (Some(fallback), Ok(format1)) =
        (cjk_fallback(dwrite), format.cast::<IDWriteTextFormat1>())
    {
        unsafe {
            let _ = format1.SetFontFallback(&fallback);
        }
    }
    Ok(format)
}

#[cfg(test)]
mod tests {
    #[test]
    fn latin_text_asks_for_segoe_ui_variable() {
        assert_eq!(super::ui_font_family(), "Segoe UI Variable Text");
        let src = include_str!("fonts.rs");
        assert!(src.contains("SetFontFallback"));
        assert!(src.contains("bar_center"));
    }

    #[test]
    fn edit_controls_keep_yahei_ui() {
        let family = concat!("Microsoft ", "YaHei", " UI");
        assert_eq!(super::gdi_font_family(), family);
    }
}
