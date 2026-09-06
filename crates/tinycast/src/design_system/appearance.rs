use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

/// Straight-alpha `D2D1_COLOR_F`. Direct2D premultiplies into the target.
pub fn color(rgba: (f32, f32, f32, f32)) -> D2D1_COLOR_F {
    let (r, g, b, a) = rgba;
    D2D1_COLOR_F { r, g, b, a }
}
