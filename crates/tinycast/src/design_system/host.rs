//! HWND Direct2D host. HWND targets present opaque; layered UpdateLayeredWindow keeps scrim alpha.

use tinycast_pure::motion::Visual;
use windows::Win32::Foundation::{
    COLORREF, D2DERR_RECREATE_TARGET, HANDLE, HWND, POINT, RECT, SIZE,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
};
use windows::Win32::Graphics::DirectWrite::{DWriteCreateFactory, DWRITE_FACTORY_TYPE_SHARED};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC,
    HGDIOBJ,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowLongPtrW, SetWindowLongPtrW, UpdateLayeredWindow, GWL_EXSTYLE,
    ULW_ALPHA, WS_EX_LAYERED,
};

use super::motion::{identity_transform, scale_about_center};
use super::Fonts;

#[derive(Clone, Copy, Debug)]
pub struct Present {
    pub alpha: u8,
    pub scale: f32,
}

impl Present {
    pub const OPAQUE: Self = Self {
        alpha: 255,
        scale: 1.0,
    };

    pub fn from_visual(visual: Visual) -> Self {
        Self {
            alpha: (visual.alpha * 255.0).round().clamp(0.0, 255.0) as u8,
            scale: visual.scale,
        }
    }
}

pub struct OverlayPainter {
    factory: ID2D1Factory,
    fonts: Fonts,
    hwnd_target: Option<ID2D1HwndRenderTarget>,
    layered_surface: Option<LayeredSurface>,
    layered: bool,
    locale: String,
}

/// Reused DIB and DC render target. Creating either per frame is most of the
/// cost of an `UpdateLayeredWindow` animation, and it shows up as hitching.
pub struct LayeredSurface {
    target: Option<ID2D1DCRenderTarget>,
    dc: HDC,
    dib: HBITMAP,
    previous: HGDIOBJ,
    width: u32,
    height: u32,
    dpi: f32,
}

impl Drop for LayeredSurface {
    fn drop(&mut self) {
        // Release Direct2D before the DC it is bound to.
        self.target.take();
        unsafe {
            if !self.previous.is_invalid() {
                let _ = SelectObject(self.dc, self.previous);
            }
            let _ = DeleteObject(self.dib);
            let _ = DeleteDC(self.dc);
        }
    }
}

impl LayeredSurface {
    fn create(
        factory: &ID2D1Factory,
        width: u32,
        height: u32,
        dpi: f32,
    ) -> windows::core::Result<Self> {
        unsafe {
            let dc = CreateCompatibleDC(HDC::default());
            if dc.is_invalid() {
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
            let dib =
                match CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0) {
                    Ok(dib) => dib,
                    Err(err) => {
                        let _ = DeleteDC(dc);
                        return Err(err);
                    }
                };
            let previous = SelectObject(dc, dib);
            let target = match factory.CreateDCRenderTarget(&target_properties(dpi)) {
                Ok(target) => target,
                Err(err) => {
                    if !previous.is_invalid() {
                        let _ = SelectObject(dc, previous);
                    }
                    let _ = DeleteObject(dib);
                    let _ = DeleteDC(dc);
                    return Err(err);
                }
            };
            Ok(Self {
                target: Some(target),
                dc,
                dib,
                previous,
                width,
                height,
                dpi,
            })
        }
    }
}

pub fn present_layered(
    slot: &mut Option<LayeredSurface>,
    factory: &ID2D1Factory,
    hwnd: HWND,
    alpha: u8,
    paint: impl FnOnce(&ID2D1RenderTarget) -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    let (width, height) = client_size(hwnd);
    if width == 0 || height == 0 {
        return Ok(());
    }
    let dpi = dpi_of(hwnd);
    let reuse = slot.as_ref().is_some_and(|surface| {
        surface.width == width && surface.height == height && (surface.dpi - dpi).abs() < 0.5
    });
    if !reuse {
        *slot = Some(LayeredSurface::create(factory, width, height, dpi)?);
    }
    let Some(surface) = slot.as_mut() else {
        return Ok(());
    };
    unsafe {
        let bind = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        let Some(target) = surface.target.as_ref() else {
            return Err(windows::core::Error::from_win32());
        };
        target.BindDC(surface.dc, &bind)?;
        paint(target)?;
        let src = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: width as i32,
            cy: height as i32,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: alpha,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            hwnd,
            HDC::default(),
            None,
            Some(&size),
            surface.dc,
            Some(&src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
    }
}

impl OverlayPainter {
    pub fn new() -> windows::core::Result<Self> {
        Self::with_locale("zh-CN")
    }

    pub fn with_locale(locale: &str) -> windows::core::Result<Self> {
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let fonts = Fonts::with_locale(&dwrite, locale)?;
        Ok(Self {
            factory,
            fonts,
            hwnd_target: None,
            layered_surface: None,
            layered: false,
            locale: locale.to_string(),
        })
    }

    pub fn set_locale(&mut self, locale: &str) {
        if self.locale == locale {
            return;
        }
        if let Ok(fonts) = Fonts::with_locale(&self.fonts.dwrite, locale) {
            self.fonts = fonts;
            self.locale = locale.to_string();
        }
    }

    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    pub fn prefer_layered(&mut self, hwnd: HWND) {
        self.set_layered(hwnd, true);
    }

    pub fn set_layered(&mut self, hwnd: HWND, layered: bool) {
        self.layered = layered;
        if layered {
            self.hwnd_target = None;
            unsafe {
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED.0 as isize);
            }
        } else {
            self.layered_surface = None;
        }
    }

    pub fn is_layered(&self) -> bool {
        self.layered
    }

    pub fn paint<F>(&mut self, hwnd: HWND, scene: F) -> windows::core::Result<()>
    where
        F: FnMut(&ID2D1RenderTarget, &Fonts) -> windows::core::Result<()>,
    {
        self.paint_present(hwnd, Present::OPAQUE, scene)
    }

    pub fn paint_visual<F>(
        &mut self,
        hwnd: HWND,
        visual: Visual,
        scene: F,
    ) -> windows::core::Result<()>
    where
        F: FnMut(&ID2D1RenderTarget, &Fonts) -> windows::core::Result<()>,
    {
        self.paint_present(hwnd, Present::from_visual(visual), scene)
    }

    pub fn paint_present<F>(
        &mut self,
        hwnd: HWND,
        present: Present,
        mut scene: F,
    ) -> windows::core::Result<()>
    where
        F: FnMut(&ID2D1RenderTarget, &Fonts) -> windows::core::Result<()>,
    {
        if self.layered {
            if self.paint_layered(hwnd, present, &mut scene).is_ok() {
                return Ok(());
            }
            self.layered = false;
        }
        self.paint_hwnd(hwnd, present, &mut scene)
    }

    fn paint_hwnd<F>(
        &mut self,
        hwnd: HWND,
        present: Present,
        scene: &mut F,
    ) -> windows::core::Result<()>
    where
        F: FnMut(&ID2D1RenderTarget, &Fonts) -> windows::core::Result<()>,
    {
        let (width, height) = client_size(hwnd);
        if width == 0 || height == 0 {
            return Ok(());
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
                Err(_) => {
                    self.set_layered(hwnd, true);
                    return self.paint_layered(hwnd, present, scene);
                }
            }
        }
        let Some(target) = &self.hwnd_target else {
            return Ok(());
        };
        if let Err(err) = paint_scene(target, &self.fonts, present.scale, scene) {
            if err.code() == D2DERR_RECREATE_TARGET {
                self.hwnd_target = None;
            }
            return Err(err);
        }
        Ok(())
    }

    fn paint_layered<F>(
        &mut self,
        hwnd: HWND,
        present: Present,
        scene: &mut F,
    ) -> windows::core::Result<()>
    where
        F: FnMut(&ID2D1RenderTarget, &Fonts) -> windows::core::Result<()>,
    {
        if !self.layered {
            self.set_layered(hwnd, true);
        }
        let scale = present.scale;
        let alpha = present.alpha;
        let OverlayPainter {
            factory,
            fonts,
            layered_surface,
            ..
        } = self;
        present_layered(layered_surface, factory, hwnd, alpha, |target| {
            paint_scene(target, fonts, scale, scene)
        })
    }
}

fn paint_scene<F>(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    scale: f32,
    scene: &mut F,
) -> windows::core::Result<()>
where
    F: FnMut(&ID2D1RenderTarget, &Fonts) -> windows::core::Result<()>,
{
    unsafe {
        target.BeginDraw();
        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        target.Clear(Some(&clear));
        let scaled = (scale - 1.0).abs() > 0.0005;
        if scaled {
            let size = target.GetSize();
            target.SetTransform(&scale_about_center(scale, size.width, size.height));
        }
        let result = scene(target, fonts);
        if scaled {
            target.SetTransform(&identity_transform());
        }
        target.EndDraw(None, None)?;
        result
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
