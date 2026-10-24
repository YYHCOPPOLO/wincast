use tinycast_pure::palette_placement::DipRect;

use crate::design_system::settings as ds;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboutHit {
    Support,
}

pub fn version_line() -> String {
    format!("Version {}", env!("CARGO_PKG_VERSION"))
}

pub fn copyright_line() -> &'static str {
    "© 2026 Abue Ammar · AGPL-3.0"
}

pub fn content_height() -> f32 {
    420.0
}

pub fn support_rect(width: f32, scroll: f32) -> DipRect {
    let pad = ds::content_pad();
    DipRect {
        x: pad,
        y: 248.0 - scroll,
        w: (width - pad * 2.0).max(80.0),
        h: ds::ROW_H,
    }
}

pub fn hit(x: f32, y: f32, scroll: f32, width: f32) -> Option<AboutHit> {
    let r = support_rect(width, scroll);
    if x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h {
        Some(AboutHit::Support)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_row_is_hittable() {
        let width = 420.0;
        let r = support_rect(width, 0.0);
        assert_eq!(
            hit(r.x + 4.0, r.y + 4.0, 0.0, width),
            Some(AboutHit::Support)
        );
        assert_eq!(hit(1.0, 1.0, 0.0, width), None);
        assert!(content_height() > r.y + r.h);
    }

    #[test]
    fn about_shows_version_support_and_copyright() {
        let ver = version_line();
        assert!(ver.contains(env!("CARGO_PKG_VERSION")));
        assert!(copyright_line().contains("AGPL-3.0"));
        assert!(copyright_line().contains("Abue Ammar"));
        assert_eq!(support_rect(420.0, 0.0).h, ds::ROW_H);
    }
}
