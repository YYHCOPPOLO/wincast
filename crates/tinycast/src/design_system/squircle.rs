use tinycast_pure::palette_placement::DipRect;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D_POINT_2F,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Factory, ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
};

use super::appearance;

fn pt(x: f32, y: f32) -> D2D_POINT_2F {
    D2D_POINT_2F { x, y }
}

fn cubic(p1: D2D_POINT_2F, p2: D2D_POINT_2F, p3: D2D_POINT_2F) -> D2D1_BEZIER_SEGMENT {
    D2D1_BEZIER_SEGMENT {
        point1: p1,
        point2: p2,
        point3: p3,
    }
}

use windows::Win32::Graphics::Direct2D::ID2D1PathGeometry;

fn squircle_path(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    radius: f32,
) -> windows::core::Result<ID2D1PathGeometry> {
    let factory: ID2D1Factory = unsafe { target.GetFactory()? };
    let geometry = unsafe { factory.CreatePathGeometry()? };
    let sink = unsafe { geometry.Open()? };

    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.w;
    let bottom = rect.y + rect.h;
    let r = radius.min(rect.w * 0.5).min(rect.h * 0.5).max(0.0);
    let k = 0.46 * r;

    unsafe {
        sink.BeginFigure(pt(left + r, top), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(pt(right - r, top));
        let tr = cubic(
            pt(right - r + k, top),
            pt(right, top + r - k),
            pt(right, top + r),
        );
        sink.AddBezier(&tr);
        sink.AddLine(pt(right, bottom - r));
        let br = cubic(
            pt(right, bottom - r + k),
            pt(right - r + k, bottom),
            pt(right - r, bottom),
        );
        sink.AddBezier(&br);
        sink.AddLine(pt(left + r, bottom));
        let bl = cubic(
            pt(left + r - k, bottom),
            pt(left, bottom - r + k),
            pt(left, bottom - r),
        );
        sink.AddBezier(&bl);
        sink.AddLine(pt(left, top + r));
        let tl = cubic(
            pt(left, top + r - k),
            pt(left + r - k, top),
            pt(left + r, top),
        );
        sink.AddBezier(&tl);
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    }
    Ok(geometry)
}

pub fn fill_squircle(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    radius: f32,
    rgba: (f32, f32, f32, f32),
) -> windows::core::Result<()> {
    let geometry = squircle_path(target, rect, radius)?;
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(rgba), None)? };
    unsafe {
        target.FillGeometry(&geometry, &brush, None);
    }
    Ok(())
}

pub fn stroke_squircle(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    radius: f32,
    rgba: (f32, f32, f32, f32),
    width: f32,
) -> windows::core::Result<()> {
    let geometry = squircle_path(target, rect, radius)?;
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(rgba), None)? };
    unsafe {
        target.DrawGeometry(&geometry, &brush, width, None);
    }
    Ok(())
}
