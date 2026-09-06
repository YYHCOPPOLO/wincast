use crate::palette_placement::DipRect;
use crate::theme;

pub fn sidebar_rect(window_h: f32) -> DipRect {
    DipRect {
        x: 0.0,
        y: 0.0,
        w: theme::size::SETTINGS_SIDEBAR,
        h: window_h,
    }
}

pub fn detail_rect(window_w: f32, window_h: f32) -> DipRect {
    let x = theme::size::SETTINGS_SIDEBAR;
    DipRect {
        x,
        y: 0.0,
        w: (window_w - x).max(0.0),
        h: window_h,
    }
}

pub fn columns_overlap(a: DipRect, b: DipRect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w
}

pub fn x_to_detail_local(x: f32) -> Option<f32> {
    let side = crate::theme::size::SETTINGS_SIDEBAR;
    if x < side {
        None
    } else {
        Some(x - side)
    }
}

pub fn recorder_well() -> DipRect {
    DipRect {
        x: 0.0,
        y: 0.0,
        w: theme::size::SHORTCUT_RECORDER,
        h: 24.0,
    }
}

pub fn recorder_callout_above(well: DipRect) -> DipRect {
    let (w, h) = theme::size::SHORTCUT_POPOVER;
    DipRect {
        x: well.x + (well.w - w) / 2.0,
        y: well.y - theme::spacing::SM - h,
        w,
        h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_detail_starts_after_sidebar() {
        let side = sidebar_rect(700.0);
        let detail = detail_rect(860.0, 700.0);
        assert_eq!(side.w, 215.0);
        assert_eq!(detail.x, 215.0);
        assert!(!columns_overlap(side, detail));
        assert_eq!(side.x + side.w, detail.x);
    }

    #[test]
    fn shortcut_popover_is_132_wide() {
        assert_eq!(crate::theme::size::SHORTCUT_POPOVER.0, 132.0);
        assert_eq!(crate::theme::size::SHORTCUT_RECORDER, 120.0);
    }

    #[test]
    fn recorder_well_is_120() {
        let r = recorder_well();
        assert_eq!(r.w, 120.0);
    }

    #[test]
    fn detail_hit_is_local_to_sidebar() {
        let detail = crate::layout::settings::detail_rect(860.0, 700.0);
        assert_eq!(detail.x, 215.0);
        let local = x_to_detail_local(100.0);
        assert!(local.is_none());
        assert_eq!(x_to_detail_local(215.0), Some(0.0));
        assert_eq!(x_to_detail_local(315.0), Some(100.0));
    }
}
