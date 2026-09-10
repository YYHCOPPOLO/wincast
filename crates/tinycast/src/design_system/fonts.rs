use tinycast_pure::theme;
use windows::core::{w, PCWSTR};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, FW_NORMAL,
    HFONT, OUT_DEFAULT_PRECIS,
};

pub fn ui_font_family() -> &'static str {
    "Microsoft YaHei UI"
}

pub fn ui_text_format(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    locale: &str,
) -> windows::core::Result<IDWriteTextFormat> {
    create_ui_format(dwrite, size, weight, locale)
}

pub fn create_gdi_ui_font(height: i32) -> HFONT {
    for family in ["Microsoft YaHei UI", "Microsoft YaHei", "Segoe UI"] {
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

#[allow(dead_code)]
pub struct Fonts {
    pub search: IDWriteTextFormat,
    pub row_title: IDWriteTextFormat,
    pub trailing: IDWriteTextFormat,
    pub section: IDWriteTextFormat,
    pub bar: IDWriteTextFormat,
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

fn create_ui_format(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    locale: &str,
) -> windows::core::Result<IDWriteTextFormat> {
    let locale_w: Vec<u16> = locale.encode_utf16().chain(Some(0)).collect();
    let locale_p = PCWSTR(locale_w.as_ptr());
    for family in [w!("Microsoft YaHei UI"), w!("Microsoft YaHei")] {
        if let Ok(format) = unsafe {
            dwrite.CreateTextFormat(
                family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                locale_p,
            )
        } {
            return Ok(format);
        }
    }
    let segoe: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
    unsafe {
        dwrite.CreateTextFormat(
            PCWSTR(segoe.as_ptr()),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            locale_p,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fonts_rs_requests_yahei_ui() {
        let src = include_str!("fonts.rs");
        let family = concat!("Microsoft ", "YaHei", " UI");
        assert_eq!(super::ui_font_family(), family);
        assert!(src.contains(&format!("w!(\"{family}\")")));
        assert!(!src.contains("w!(\"Noto"));
        assert!(!src.contains("w!(\"Segoe UI\")"));
    }
}
