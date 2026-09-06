use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, D2DERR_RECREATE_TARGET, HANDLE, HWND, POINT, RECT, SIZE,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HDC,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowLongPtrW, SetWindowLongPtrW, UpdateLayeredWindow, GWL_EXSTYLE,
    ULW_ALPHA, WS_EX_LAYERED,
};

use super::menu::{self, FooterPaint, MenuPaint};
use crate::features::launcher::ui::list::{self, IconCache, ListFonts, PaintItem};

pub struct Renderer {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
    text_format: IDWriteTextFormat,
    list_fonts: ListFonts,
    hwnd_target: Option<ID2D1HwndRenderTarget>,
    layered: bool,
}

pub struct PaintParams<'a> {
    pub placeholder: bool,
    pub placeholder_text: &'a str,
    pub header_symbol: &'a str,
    pub items: &'a [PaintItem],
    pub scroll: f32,
    pub cache: &'a mut IconCache,
    pub appearance: u8,
    pub footer: FooterPaint<'a>,
    pub menu: Option<MenuPaint<'a>>,
    pub clipboard_preview: Option<&'a str>,
    pub tab_hint: Option<&'a str>,
    pub clipboard_filter: Option<FilterButtonPaint>,
    pub compact_favorite_icons: &'a [Option<String>],
    pub empty_results: Option<&'a str>,
}

pub struct FilterButtonPaint {
    pub title: String,
    pub open: bool,
    pub rect: tinycast_pure::palette_placement::DipRect,
}

impl Renderer {
    pub fn new() -> windows::core::Result<Self> {
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let text_format = make_text_format(
            &dwrite,
            w!("Segoe UI"),
            super::edit::SEARCH_FONT_DIP,
            DWRITE_FONT_WEIGHT_REGULAR,
            false,
            false,
        )?;
        let list_fonts = ListFonts {
            title: make_text_format(&dwrite, w!("Segoe UI"), 15.0, DWRITE_FONT_WEIGHT_REGULAR, false, false)?,
            trailing: make_text_format(&dwrite, w!("Segoe UI"), 12.0, DWRITE_FONT_WEIGHT_REGULAR, true, false)?,
            header: make_text_format(&dwrite, w!("Segoe UI"), 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, false, false)?,
            chip: make_text_format(&dwrite, w!("Segoe UI"), 11.0, DWRITE_FONT_WEIGHT_REGULAR, false, true)?,
            keycap: make_text_format(&dwrite, w!("Segoe UI"), 11.0, DWRITE_FONT_WEIGHT_REGULAR, false, true)?,
            calc_result: make_text_format(
                &dwrite,
                w!("Segoe UI"),
                theme::typography::CALC_RESULT,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                false,
                true,
            )?,
            calc_badge: make_text_format(
                &dwrite,
                w!("Segoe UI"),
                theme::typography::CALC_BADGE,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                true,
            )?,
            emoji: make_text_format(
                &dwrite,
                w!("Segoe UI Emoji"),
                32.0,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                true,
            )?,
        };
        Ok(Self {
            factory,
            dwrite,
            text_format,
            list_fonts,
            hwnd_target: None,
            layered: false,
        })
    }

    pub fn set_layered(&mut self, hwnd: HWND, layered: bool) {
        self.layered = layered;
        if layered {
            self.hwnd_target = None;
            unsafe {
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED.0 as isize);
            }
        }
    }

    pub fn is_layered(&self) -> bool {
        self.layered
    }

    pub fn discard_target(&mut self) {
        self.hwnd_target = None;
    }

    pub fn resize(&mut self, hwnd: HWND, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.layered {
            return;
        }
        if let Some(target) = &self.hwnd_target {
            let size = D2D_SIZE_U { width, height };
            if unsafe { target.Resize(&size) }.is_err() {
                self.hwnd_target = None;
            }
        }
        if self.hwnd_target.is_none() {
            match create_hwnd_target(&self.factory, hwnd, width, height) {
                Ok(target) => self.hwnd_target = Some(target),
                Err(_) => self.set_layered(hwnd, true),
            }
        }
    }

    pub fn paint(&mut self, hwnd: HWND, params: PaintParams<'_>, source_alpha: u8) {
        if self.layered {
            let _ = self.paint_layered(hwnd, params, source_alpha);
            return;
        }
        if self.hwnd_target.is_none() {
            let (w, h) = client_size(hwnd);
            self.resize(hwnd, w, h);
        }
        let Some(target) = &self.hwnd_target else {
            let _ = self.paint_layered(hwnd, params, source_alpha);
            return;
        };
        if let Err(err) = paint_scene(
            target,
            &self.dwrite,
            &self.text_format,
            &self.list_fonts,
            params,
            dpi_of(hwnd),
        ) {
            if err.code() == D2DERR_RECREATE_TARGET {
                self.hwnd_target = None;
            }
        }
    }

    fn paint_layered(
        &mut self,
        hwnd: HWND,
        params: PaintParams<'_>,
        source_alpha: u8,
    ) -> windows::core::Result<()> {
        if !self.layered {
            self.set_layered(hwnd, true);
        }
        let (width, height) = client_size(hwnd);
        if width == 0 || height == 0 {
            return Ok(());
        }
        let dpi = dpi_of(hwnd);
        unsafe {
            let mem_dc = CreateCompatibleDC(HDC::default());
            if mem_dc.is_invalid() {
                return Err(windows::core::Error::from_win32());
            }
            let mut bits = std::ptr::null_mut();
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            };
            let dib = CreateDIBSection(
                mem_dc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                HANDLE::default(),
                0,
            )?;
            let prev = SelectObject(mem_dc, dib);
            let result = (|| {
                let target: ID2D1DCRenderTarget =
                    self.factory.CreateDCRenderTarget(&target_properties(dpi))?;
                let bind = RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                };
                target.BindDC(mem_dc, &bind)?;
                paint_scene(
                    &target,
                    &self.dwrite,
                    &self.text_format,
                    &self.list_fonts,
                    params,
                    dpi,
                )?;
                let src = POINT { x: 0, y: 0 };
                let size = SIZE {
                    cx: width as i32,
                    cy: height as i32,
                };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: source_alpha,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                UpdateLayeredWindow(
                    hwnd,
                    HDC::default(),
                    None,
                    Some(&size),
                    mem_dc,
                    Some(&src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                )
            })();
            SelectObject(mem_dc, prev);
            let _ = DeleteObject(dib);
            let _ = DeleteDC(mem_dc);
            result
        }
    }
}

fn dpi_of(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        96.0
    } else {
        dpi as f32
    }
}

fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    (
        (rc.right - rc.left).max(0) as u32,
        (rc.bottom - rc.top).max(0) as u32,
    )
}

fn target_properties(dpi: f32) -> D2D1_RENDER_TARGET_PROPERTIES {
    D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    }
}

fn create_hwnd_target(
    factory: &ID2D1Factory,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> windows::core::Result<ID2D1HwndRenderTarget> {
    let dpi = dpi_of(hwnd);
    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: D2D_SIZE_U { width, height },
        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
    };
    unsafe { factory.CreateHwndRenderTarget(&target_properties(dpi), &hwnd_props) }
}

fn make_text_format(
    dwrite: &IDWriteFactory,
    family: windows::core::PCWSTR,
    size: f32,
    weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    trailing: bool,
    center: bool,
) -> windows::core::Result<IDWriteTextFormat> {
    let format = unsafe {
        dwrite.CreateTextFormat(
            family,
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
        if trailing {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
        } else if center {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        }
    }
    Ok(format)
}

fn paint_scene(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    text_format: &IDWriteTextFormat,
    list_fonts: &ListFonts,
    params: PaintParams<'_>,
    dpi: f32,
) -> windows::core::Result<()> {
    unsafe {
        target.BeginDraw();
        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        target.Clear(Some(&clear));
        paint_layers(target, dwrite, text_format, list_fonts, params, dpi)?;
        target.EndDraw(None, None)
    }
}

fn paint_layers(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    text_format: &IDWriteTextFormat,
    list_fonts: &ListFonts,
    params: PaintParams<'_>,
    dpi: f32,
) -> windows::core::Result<()> {
    unsafe {
        let size = target.GetSize();
        let radius = theme::radius::PANEL;
        let rounded = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: size.width,
                bottom: size.height,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let color = scrim_color();
        let brush = target.CreateSolidColorBrush(&color, None)?;
        target.FillRoundedRectangle(&rounded, &brush);
        let split = params.clipboard_preview.is_some() && !params.items.is_empty();
        let list_w = if split {
            tinycast_pure::layout::list::clipboard_columns(size.width).0.w
        } else {
            size.width
        };
        let _ = list::paint(
            target,
            dwrite,
            list_fonts,
            params.items,
            params.scroll,
            list_w,
            size.height,
            params.cache,
            dpi,
            params.appearance,
        );
        if split {
            if let Some(preview) = params.clipboard_preview {
                let _ = paint_clipboard_preview(
                    target,
                    list_fonts,
                    preview,
                    list_w,
                    size.width,
                    size.height,
                    params.appearance,
                );
            }
        } else if let Some(text) = params.empty_results {
            let _ = paint_empty_results(
                target,
                dwrite,
                text,
                size.width,
                size.height,
                params.appearance,
            );
        }
        let _ = crate::design_system::symbols::paint_header_glyph(
            target,
            dwrite,
            params.header_symbol,
            params.appearance,
        );
        if !params.compact_favorite_icons.is_empty() {
            let _ = paint_compact_favorites(
                target,
                params.compact_favorite_icons,
                params.cache,
                dpi,
                params.appearance,
            );
        }
        if params.placeholder {
            let _ = paint_placeholder(
                target,
                text_format,
                params.placeholder_text,
                params.appearance,
            );
        }
        if let Some(hint) = params.tab_hint {
            let trailing = params
                .clipboard_filter
                .as_ref()
                .map(|_| crate::features::clipboard::ui::screen::filter_trailing_width())
                .unwrap_or(0.0);
            let _ = paint_tab_hint(target, dwrite, list_fonts, hint, trailing, params.appearance);
        }
        if let Some(filter) = &params.clipboard_filter {
            let _ = paint_filter_button(target, list_fonts, filter);
        }
        let _ = menu::paint_footer(
            target,
            dwrite,
            list_fonts,
            size.width,
            size.height,
            &params.footer,
        );
        if let Some(open) = &params.menu {
            let _ = menu::paint_menu(target, dwrite, list_fonts, size.width, size.height, open);
        }
        Ok(())
    }
}

fn paint_clipboard_preview(
    target: &ID2D1RenderTarget,
    fonts: &ListFonts,
    preview: &str,
    list_w: f32,
    panel_w: f32,
    panel_h: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let top = theme::size::COMPACT_HEIGHT;
    let bottom = (panel_h - theme::size::BOTTOM_BAR_HEIGHT).max(top);
    let sep = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::SEPARATOR_DARK_ALPHA,
        theme::colors::SEPARATOR_LIGHT_ALPHA,
    ));
    let line = unsafe { target.CreateSolidColorBrush(&sep, None)? };
    unsafe {
        target.DrawLine(
            windows::Win32::Graphics::Direct2D::Common::D2D_POINT_2F { x: list_w, y: top },
            windows::Win32::Graphics::Direct2D::Common::D2D_POINT_2F {
                x: list_w,
                y: bottom,
            },
            &line,
            theme::size::HAIRLINE,
            None,
        );
    }
    let ink = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_PRIMARY_ALPHA,
        theme::colors::TEXT_PRIMARY_ALPHA,
    ));
    let text_brush = unsafe { target.CreateSolidColorBrush(&ink, None)? };
    let pad = theme::spacing::XL;
    let rect = D2D_RECT_F {
        left: list_w + pad,
        top: top + pad,
        right: panel_w - pad,
        bottom: bottom - pad,
    };
    let wide: Vec<u16> = preview
        .chars()
        .take(2000)
        .collect::<String>()
        .encode_utf16()
        .collect();
    unsafe {
        target.DrawText(
            &wide,
            &fonts.title,
            &rect,
            &text_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
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
    let top = tinycast_pure::layout::list::content_top();
    let bottom = (panel_h - theme::size::BOTTOM_BAR_HEIGHT).max(top);
    let format = make_text_format(
        dwrite,
        w!("Segoe UI"),
        theme::typography::ROW_TITLE,
        DWRITE_FONT_WEIGHT_REGULAR,
        false,
        true,
    )?;
    let ink = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    ));
    let brush = unsafe { target.CreateSolidColorBrush(&ink, None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            &format,
            &D2D_RECT_F {
                left: 0.0,
                top,
                right: panel_w,
                bottom,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_filter_button(
    target: &ID2D1RenderTarget,
    fonts: &ListFonts,
    filter: &FilterButtonPaint,
) -> windows::core::Result<()> {
    let rect = filter.rect;
    let fill = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: if filter.open {
            theme::colors::SELECTION_DARK_ALPHA
        } else {
            0.14
        },
    };
    let rounded = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.w,
            bottom: rect.y + rect.h,
        },
        radiusX: rect.h / 2.0,
        radiusY: rect.h / 2.0,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(&rounded, &brush);
    }
    let ink = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    };
    let text_brush = unsafe { target.CreateSolidColorBrush(&ink, None)? };
    let chevron = if filter.open { "▴" } else { "▾" };
    let label = format!("{} {chevron}", filter.title);
    let wide: Vec<u16> = label.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            &fonts.header,
            &D2D_RECT_F {
                left: rect.x + theme::spacing::SM,
                top: rect.y,
                right: rect.x + rect.w - theme::spacing::SM,
                bottom: rect.y + rect.h,
            },
            &text_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_tab_hint(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    hint: &str,
    trailing: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (x, y, w, h) = super::edit::search_field_dip_with_trailing(trailing);
    let right = x + w;
    let ds = crate::design_system::Fonts::new(dwrite)?;
    let cap_w = crate::design_system::paint_keycap(
        target,
        &ds,
        "⇥",
        right,
        y,
        h,
        true,
        appearance,
    )?;
    let cap_x = right - cap_w;
    let ink = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    ));
    let brush = unsafe { target.CreateSolidColorBrush(&ink, None)? };
    let label: Vec<u16> = hint.encode_utf16().collect();
    let label_rect = D2D_RECT_F {
        left: (cap_x - 90.0).max(x),
        top: y,
        right: cap_x - theme::spacing::SM,
        bottom: y + h,
    };
    unsafe {
        target.DrawText(
            &label,
            &fonts.header,
            &label_rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_compact_favorites(
    target: &ID2D1RenderTarget,
    icons: &[Option<String>],
    cache: &mut IconCache,
    dpi: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let trailing = tinycast_pure::layout::palette_chrome::compact_favorites_trailing(icons.len());
    let (x, _y, w, _h) = super::edit::search_field_dip_with_trailing(trailing);
    let search_right = x + w;
    for (index, source) in icons.iter().enumerate() {
        let rect = tinycast_pure::layout::palette_chrome::compact_favorite_slot(index, search_right);
        list::paint_icon_at(
            target,
            cache,
            source.as_deref(),
            rect.x,
            rect.y,
            rect.w,
            dpi,
            appearance,
        )?;
    }
    Ok(())
}

fn paint_placeholder(
    target: &ID2D1RenderTarget,
    text_format: &IDWriteTextFormat,
    label: &str,
    appearance: u8,
) -> windows::core::Result<()> {
    let (x, y, w, h) = super::edit::search_field_dip();
    let rect = D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let color = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_TERTIARY_DARK_ALPHA,
        theme::colors::TEXT_TERTIARY_LIGHT_ALPHA,
    ));
    let brush = unsafe { target.CreateSolidColorBrush(&color, None)? };
    let text: Vec<u16> = label.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &text,
            text_format,
            &rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

/// Brush alpha is the frozen dark scrim; later D2D content stays fully opaque.
pub(crate) const LAYERED_SOURCE_CONSTANT_ALPHA: u8 = 255;

pub(crate) fn scrim_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: theme::colors::PANEL_SCRIM_DARK_ALPHA,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrim_alpha_is_baked_into_brush_not_source_constant() {
        let c = scrim_color();
        assert_eq!(c.r, 0.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, theme::colors::PANEL_SCRIM_DARK_ALPHA);
        assert_eq!(LAYERED_SOURCE_CONSTANT_ALPHA, 255);
    }

    #[test]
    fn header_glyph_stays_opaque_over_dissolve() {
        let (w, _h, bits) = crate::design_system::test_render::with_offscreen(750, 475, |target| {
            let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
            let text_format = make_text_format(
                &dwrite,
                w!("Segoe UI"),
                crate::palette::edit::SEARCH_FONT_DIP,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                false,
            )?;
            let body = make_text_format(
                &dwrite,
                w!("Segoe UI"),
                13.0,
                DWRITE_FONT_WEIGHT_REGULAR,
                false,
                false,
            )?;
            let list_fonts = ListFonts {
                title: body.clone(),
                trailing: body.clone(),
                header: body.clone(),
                chip: body.clone(),
                keycap: body.clone(),
                calc_result: body.clone(),
                calc_badge: body.clone(),
                emoji: body,
            };
            let items: Vec<PaintItem> = (0..20)
                .map(|i| PaintItem::Row {
                    title: format!("Row {i}"),
                    alias: None,
                    trailing: String::new(),
                    keycap: None,
                    icon_source: None,
                    selected: i == 0,
                })
                .collect();
            let mut cache = IconCache::new();
            paint_layers(
                target,
                &dwrite,
                &text_format,
                &list_fonts,
                PaintParams {
                    placeholder: false,
                    placeholder_text: "",
                    header_symbol: "magnifyingglass",
                    items: &items,
                    scroll: 0.0,
                    cache: &mut cache,
                    appearance: 0,
                    footer: FooterPaint {
                        show_action_group: false,
                        primary_label: "",
                        primary_destructive: false,
                    },
                    menu: None,
                    clipboard_preview: None,
                    tab_hint: None,
                    clipboard_filter: None,
                    compact_favorite_icons: &[],
                    empty_results: None,
                },
                96.0,
            )
        })
        .expect("header over dissolve");
        let slot = tinycast_pure::layout::palette_chrome::header_icon_rect();
        let x0 = slot.x.round() as usize;
        let y0 = slot.y.round() as usize;
        let x1 = (slot.x + slot.w).round() as usize;
        let y1 = (slot.y + slot.h).round() as usize;
        let mut max_r = 0u8;
        for y in y0..y1.min(_h) {
            for x in x0..x1.min(w) {
                max_r = max_r.max(bits[(y * w + x) * 4 + 2]);
            }
        }
        assert!(
            max_r > 80,
            "header glyph must float above dissolve, max_r={max_r}"
        );
    }
}
