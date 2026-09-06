use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// Premultiplied D2D brush color from straight-alpha `(r, g, b, a)`.
pub fn color(rgba: (f32, f32, f32, f32)) -> D2D1_COLOR_F {
    let (r, g, b, a) = rgba;
    D2D1_COLOR_F {
        r: r * a,
        g: g * a,
        b: b * a,
        a,
    }
}
