use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenDip {
    pub frame: DipRect,          // full display in DIP, global origin
    pub work: DipRect,           // work area (taskbar excluded)
    pub origin_is_primary: bool, // frame.origin == (0,0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaletteAnchor {
    pub left: f32,
    pub top: f32,
}

pub fn compact_size() -> (f32, f32) {
    (theme::size::PANEL_WIDTH, theme::size::COMPACT_HEIGHT)
}
pub fn expanded_size() -> (f32, f32) {
    (theme::size::PANEL_WIDTH, theme::size::PANEL_HEIGHT)
}

/// Default top-left of the compact bar on `screen`.
/// Top = work.y + work.h * PALETTE_TOP_MARGIN_FRACTION.
/// Horizontal: centered in work.
pub fn default_anchor(screen: ScreenDip) -> PaletteAnchor {
    let (panel_width, _) = compact_size();
    PaletteAnchor {
        left: screen.work.x + (screen.work.w - panel_width) / 2.0,
        top: screen.work.y + screen.work.h * theme::size::PALETTE_TOP_MARGIN_FRACTION,
    }
}

/// Pick the screen: if `open_on_cursor` then the screen containing `cursor`,
/// else the primary (`origin_is_primary`). Never “the focused window’s screen”.
pub fn target_screen<'a>(
    screens: &'a [ScreenDip],
    cursor: (f32, f32),
    open_on_cursor: bool,
) -> Option<&'a ScreenDip> {
    if open_on_cursor {
        if let Some(hit) = screens.iter().find(|s| contains(s.frame, cursor)) {
            return Some(hit);
        }
    }
    screens.iter().find(|s| s.origin_is_primary)
}

pub fn frame_for(anchor: PaletteAnchor, expanded: bool) -> DipRect {
    let (w, h) = if expanded {
        expanded_size()
    } else {
        compact_size()
    };
    DipRect {
        x: anchor.left,
        y: anchor.top,
        w,
        h,
    }
}

fn contains(r: DipRect, (x, y): (f32, f32)) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_anchor_is_18_percent_down_work_area_centered() {
        let screen = ScreenDip {
            frame: DipRect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0,
            },
            work: DipRect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1040.0,
            },
            origin_is_primary: true,
        };
        let a = default_anchor(screen);
        assert!((a.top - 1040.0 * 0.18).abs() < 0.01);
        assert!((a.left - (1920.0 - 750.0) / 2.0).abs() < 0.01);
    }

    #[test]
    fn follow_cursor_uses_containing_screen_not_primary() {
        let screens = [
            ScreenDip {
                frame: DipRect {
                    x: 0.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0,
                },
                work: DipRect {
                    x: 0.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1040.0,
                },
                origin_is_primary: true,
            },
            ScreenDip {
                frame: DipRect {
                    x: 1920.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0,
                },
                work: DipRect {
                    x: 1920.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0,
                },
                origin_is_primary: false,
            },
        ];
        let s = target_screen(&screens, (2000.0, 10.0), true).unwrap();
        assert!(!s.origin_is_primary);
        let primary = target_screen(&screens, (2000.0, 10.0), false).unwrap();
        assert!(primary.origin_is_primary);
    }

    #[test]
    fn expanded_frame_keeps_top_left_anchor() {
        let a = PaletteAnchor {
            left: 100.0,
            top: 80.0,
        };
        let c = frame_for(a, false);
        let e = frame_for(a, true);
        assert_eq!(c.x, e.x);
        assert_eq!(c.y, e.y);
        assert_eq!(e.h, 475.0);
        assert_eq!(c.h, 64.0);
    }

    #[test]
    fn cursor_miss_falls_back_to_primary() {
        let screens = [
            ScreenDip {
                frame: DipRect {
                    x: 0.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0,
                },
                work: DipRect {
                    x: 0.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1040.0,
                },
                origin_is_primary: true,
            },
            ScreenDip {
                frame: DipRect {
                    x: 1920.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0,
                },
                work: DipRect {
                    x: 1920.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0,
                },
                origin_is_primary: false,
            },
        ];
        let s = target_screen(&screens, (-10.0, -10.0), true).unwrap();
        assert!(s.origin_is_primary);
    }
}
