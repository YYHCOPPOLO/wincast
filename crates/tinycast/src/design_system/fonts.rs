use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP,
};

#[allow(dead_code)]
pub struct Fonts {
    pub search: IDWriteTextFormat,
    pub row_title: IDWriteTextFormat,
    pub trailing: IDWriteTextFormat,
    pub section: IDWriteTextFormat,
    pub bar: IDWriteTextFormat,
    pub keycap: IDWriteTextFormat,
    pub(crate) dwrite: IDWriteFactory,
}

impl Fonts {
    pub fn new(dwrite: &IDWriteFactory) -> windows::core::Result<Self> {
        Ok(Self {
            search: make(
                dwrite,
                theme::typography::SEARCH_FIELD,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
            )?,
            row_title: make(
                dwrite,
                theme::typography::ROW_TITLE,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
            )?,
            trailing: make(
                dwrite,
                theme::typography::ROW_TRAILING,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
            )?,
            section: make(
                dwrite,
                theme::typography::SECTION_HEADER,
                DWRITE_FONT_WEIGHT_MEDIUM,
                false,
            )?,
            bar: make(
                dwrite,
                theme::typography::BAR,
                DWRITE_FONT_WEIGHT_MEDIUM,
                false,
            )?,
            keycap: make(
                dwrite,
                theme::typography::KEY_CAP,
                DWRITE_FONT_WEIGHT_REGULAR,
                true,
            )?,
            dwrite: dwrite.clone(),
        })
    }
}

fn make(
    dwrite: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    center: bool,
) -> windows::core::Result<IDWriteTextFormat> {
    let format = unsafe {
        dwrite.CreateTextFormat(
            w!("Segoe UI"),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-US"),
        )?
    };
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
