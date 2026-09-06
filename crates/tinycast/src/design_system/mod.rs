pub mod appearance;
pub mod chat;
pub mod dialog;
pub mod fonts;
pub mod host;
pub mod hud;
pub mod keycap;
pub mod panel;
pub mod row;
pub mod settings;
pub mod squircle;
pub mod symbols;
pub mod text;

pub use fonts::Fonts;
pub use keycap::paint_keycap;
pub use panel::paint_panel_scrim;
pub use row::paint_row_fill;
pub use squircle::fill_squircle;

#[cfg(test)]
pub mod test_render {
    use windows::Win32::Foundation::{HANDLE, RECT};
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1RenderTarget,
        D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
        D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC,
    };

    pub fn panel_scrim(
        w: u32,
        h: u32,
        appearance: u8,
    ) -> windows::core::Result<(usize, usize, Vec<u8>)> {
        with_offscreen(w, h, |target| {
            crate::design_system::paint_panel_scrim(target, w as f32, h as f32, appearance)
        })
    }

    pub fn with_offscreen(
        w: u32,
        h: u32,
        paint: impl FnOnce(&ID2D1RenderTarget) -> windows::core::Result<()>,
    ) -> windows::core::Result<(usize, usize, Vec<u8>)> {
        let dpi = 96.0;
        unsafe {
            let factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let mem_dc = CreateCompatibleDC(HDC::default());
            if mem_dc.is_invalid() {
                return Err(windows::core::Error::from_win32());
            }
            let mut bits = std::ptr::null_mut();
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            };
            let dib = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0)?;
            let prev = SelectObject(mem_dc, dib);
            let result = (|| {
                let props = D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: dpi,
                    dpiY: dpi,
                    usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
                    minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                };
                let target: ID2D1DCRenderTarget = factory.CreateDCRenderTarget(&props)?;
                let bind = RECT {
                    left: 0,
                    top: 0,
                    right: w as i32,
                    bottom: h as i32,
                };
                target.BindDC(mem_dc, &bind)?;
                target.BeginDraw();
                paint(&target)?;
                target.EndDraw(None, None)?;
                let byte_count = (w as usize) * (h as usize) * 4;
                let slice = std::slice::from_raw_parts(bits as *const u8, byte_count);
                Ok((w as usize, h as usize, slice.to_vec()))
            })();
            SelectObject(mem_dc, prev);
            let _ = DeleteObject(dib);
            let _ = DeleteDC(mem_dc);
            result
        }
    }
}
