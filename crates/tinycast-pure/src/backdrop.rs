//! Experimental live blur behind the palette (upstream's panel surface is a scrim over
//! `NSVisualEffectView` blur, `docs/ui.md` "Panel surface"). Off unless
//! `TINYCAST_EXPERIMENTAL_BLUR=1` is set; the scrim alone remains the default surface.

/// Environment variable that turns the blur spike on.
pub const ENV_FLAG: &str = "TINYCAST_EXPERIMENTAL_BLUR";

/// Squircle corners (k = 0.46 r Béziers) bulge past a circular arc of the same radius:
/// at the 45° point the squircle sits 0.3275 r in from the corner versus 0.2929 R for a
/// circle. A circular clip of `R = 1.12 r` keeps the blur inside the painted squircle so
/// no blurred sliver shows outside the scrim's corners.
pub const SQUIRCLE_TO_CIRCLE: f32 = 1.12;

pub fn flag_enabled(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim),
        Some("1") | Some("true") | Some("on") | Some("yes")
    )
}

/// Circular clip radius in physical pixels for a squircle of `radius_dip` at `dpi`.
pub fn clip_radius_px(radius_dip: f32, dpi: u32) -> f32 {
    radius_dip * SQUIRCLE_TO_CIRCLE * dpi as f32 / 96.0
}

/// Whether Windows "Transparency effects" is on (`EnableTransparency` DWORD). A missing
/// value means on, the Windows default.
pub fn transparency_enabled(value: Option<u32>) -> bool {
    value != Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_is_off_unless_explicit() {
        assert!(!flag_enabled(None));
        assert!(!flag_enabled(Some("")));
        assert!(!flag_enabled(Some("0")));
        assert!(flag_enabled(Some("1")));
        assert!(flag_enabled(Some(" true ")));
    }

    #[test]
    fn circular_clip_stays_inside_the_squircle() {
        // Midpoint inset of the circle must not be smaller than the squircle's.
        let r = 26.0_f32;
        let circle = clip_radius_px(r, 96) * (1.0 - std::f32::consts::FRAC_1_SQRT_2);
        assert!(circle >= 0.3275 * r - 0.01, "{circle}");
        assert!((clip_radius_px(26.0, 144) - 43.68).abs() < 1e-3);
    }

    #[test]
    fn transparency_defaults_on() {
        assert!(transparency_enabled(None));
        assert!(transparency_enabled(Some(1)));
        assert!(!transparency_enabled(Some(0)));
    }
}
