use crate::palette_placement::DipRect;
use crate::window_command::{WindowCommandId, WindowKind};

const STEP_FRACTION: f32 = 0.05;
const ALMOST_MAXIMIZE: f32 = 0.9;
const REASONABLE: f32 = 0.6;
const REASONABLE_MAX_W: f32 = 1025.0;
const REASONABLE_MAX_H: f32 = 900.0;
const ONE_THIRD: f32 = 1.0 / 3.0;
const TWO_THIRDS: f32 = 2.0 / 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutInput {
    pub visible: DipRect,
    pub window: DipRect,
    pub gap: f32,
    pub step: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub id: i32,
    pub frame: DipRect,
    pub visible: DipRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    Min,
    Center,
    Max,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Anchor {
    pub horizontal: Axis,
    pub vertical: Axis,
}

impl Anchor {
    pub const TOP_LEADING: Self = Self {
        horizontal: Axis::Min,
        vertical: Axis::Min,
    };
    pub const CENTERED: Self = Self {
        horizontal: Axis::Center,
        vertical: Axis::Center,
    };

    pub fn place(self, w: f32, h: f32, slot: DipRect) -> DipRect {
        DipRect {
            x: origin(self.horizontal, slot.x, slot.w, w),
            y: origin(self.vertical, slot.y, slot.h, h),
            w,
            h,
        }
    }
}

fn origin(axis: Axis, min: f32, len: f32, size: f32) -> f32 {
    match axis {
        Axis::Min => min,
        Axis::Center => min + (len - size) / 2.0,
        Axis::Max => min + len - size,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub frame: DipRect,
    pub screen_id: i32,
    pub anchor: Anchor,
    pub resizes: bool,
}

pub fn placement(id: WindowCommandId, input: LayoutInput) -> Option<DipRect> {
    let screen = Screen {
        id: 0,
        frame: input.visible,
        visible: input.visible,
    };
    placement_for(LayoutQuery {
        command: id,
        window: input.window,
        screens: &[screen],
        gap: input.gap,
        step: input.step as i32,
        restore: None,
        last_tile: None,
    })
    .map(|p| p.frame)
}

pub struct LayoutQuery<'a> {
    pub command: WindowCommandId,
    pub window: DipRect,
    pub screens: &'a [Screen],
    pub gap: f32,
    pub step: i32,
    pub restore: Option<DipRect>,
    pub last_tile: Option<WindowCommandId>,
}

pub fn placement_for(input: LayoutQuery<'_>) -> Option<Placement> {
    let kind = input.command.kind();
    if kind != WindowKind::Geometry && kind != WindowKind::Restore {
        return None;
    }
    if input.screens.is_empty() {
        return None;
    }
    if kind == WindowKind::Restore {
        return restore_placement(&input);
    }
    let host = screen_containing(input.window, input.screens)?;
    if host.visible.w <= 0.0 || host.visible.h <= 0.0 {
        return None;
    }
    let gap = sanitized_gap(input.gap, host.visible);
    if matches!(
        input.command,
        WindowCommandId::NextDisplay | WindowCommandId::PreviousDisplay
    ) {
        return display_placement(&input, host, gap);
    }
    let step = if input.command.cycles_on_repeat() {
        normalized_step(input.step)
    } else {
        0
    };
    if let Some(frac) = tile_fractions(input.command, step) {
        let frame = tile(host.visible, frac.x0, frac.x1, frac.y0, frac.y1, gap);
        return Some(Placement {
            frame,
            screen_id: host.id,
            anchor: frac.anchor,
            resizes: true,
        });
    }
    let canvas = canvas(host.visible, gap);
    if canvas.w <= 0.0 || canvas.h <= 0.0 {
        return None;
    }
    let current = input.window;
    match input.command {
        WindowCommandId::Maximize => Some(Placement {
            frame: canvas,
            screen_id: host.id,
            anchor: Anchor::TOP_LEADING,
            resizes: true,
        }),
        WindowCommandId::AlmostMaximize => {
            let size_w = canvas.w * ALMOST_MAXIMIZE;
            let size_h = canvas.h * ALMOST_MAXIMIZE;
            Some(Placement {
                frame: rounded(Anchor::CENTERED.place(size_w, size_h, canvas)),
                screen_id: host.id,
                anchor: Anchor::CENTERED,
                resizes: true,
            })
        }
        WindowCommandId::ReasonableSize => {
            let size_w = (canvas.w * REASONABLE).min(REASONABLE_MAX_W);
            let size_h = (canvas.h * REASONABLE).min(REASONABLE_MAX_H);
            Some(Placement {
                frame: rounded(Anchor::CENTERED.place(size_w, size_h, canvas)),
                screen_id: host.id,
                anchor: Anchor::CENTERED,
                resizes: true,
            })
        }
        WindowCommandId::MaximizeHeight => {
            let frame = DipRect {
                x: current.x,
                y: canvas.y,
                w: current.w,
                h: canvas.h,
            };
            Some(Placement {
                frame: rounded(clamped(frame, canvas)),
                screen_id: host.id,
                anchor: Anchor::TOP_LEADING,
                resizes: true,
            })
        }
        WindowCommandId::MaximizeWidth => {
            let frame = DipRect {
                x: canvas.x,
                y: current.y,
                w: canvas.w,
                h: current.h,
            };
            Some(Placement {
                frame: rounded(clamped(frame, canvas)),
                screen_id: host.id,
                anchor: Anchor::TOP_LEADING,
                resizes: true,
            })
        }
        WindowCommandId::Center => {
            let w = current.w.min(canvas.w);
            let h = current.h.min(canvas.h);
            Some(Placement {
                frame: rounded(Anchor::CENTERED.place(w, h, canvas)),
                screen_id: host.id,
                anchor: Anchor::CENTERED,
                resizes: true,
            })
        }
        WindowCommandId::MakeLarger | WindowCommandId::MakeSmaller => Some(Placement {
            frame: resized(
                current,
                canvas,
                input.command == WindowCommandId::MakeLarger,
            ),
            screen_id: host.id,
            anchor: Anchor::CENTERED,
            resizes: true,
        }),
        WindowCommandId::MoveLeft
        | WindowCommandId::MoveRight
        | WindowCommandId::MoveUp
        | WindowCommandId::MoveDown => Some(Placement {
            frame: nudged(current, canvas, input.command),
            screen_id: host.id,
            anchor: Anchor::TOP_LEADING,
            resizes: false,
        }),
        _ => None,
    }
}

pub fn screen_containing(frame: DipRect, screens: &[Screen]) -> Option<Screen> {
    let mut best: Option<(Screen, f32)> = None;
    for screen in screens {
        let area = intersection_area(screen.frame, frame);
        if area > best.map(|(_, a)| a).unwrap_or(0.0) {
            best = Some((*screen, area));
        }
    }
    if let Some((screen, area)) = best {
        if area > 0.0 {
            return Some(screen);
        }
    }
    let cx = frame.x + frame.w / 2.0;
    let cy = frame.y + frame.h / 2.0;
    screens
        .iter()
        .copied()
        .find(|s| contains(s.frame, cx, cy))
        .or_else(|| screens.first().copied())
}

pub fn ordered(screens: &[Screen]) -> Vec<Screen> {
    let mut out = screens.to_vec();
    out.sort_by(|a, b| {
        a.frame
            .x
            .partial_cmp(&b.frame.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.frame
                    .y
                    .partial_cmp(&b.frame.y)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    out
}

fn display_placement(input: &LayoutQuery<'_>, host: Screen, gap: f32) -> Option<Placement> {
    let ordered_screens = ordered(input.screens);
    if ordered_screens.len() <= 1 {
        return None;
    }
    let index = ordered_screens.iter().position(|s| s.id == host.id)?;
    let offset = if input.command == WindowCommandId::NextDisplay {
        1
    } else {
        -1
    };
    let dest = ordered_screens
        [((index as i32 + offset + ordered_screens.len() as i32) as usize) % ordered_screens.len()];
    let frame = moved(input.window, host, dest, gap, input.last_tile);
    Some(Placement {
        frame,
        screen_id: dest.id,
        anchor: Anchor::CENTERED,
        resizes: true,
    })
}

fn moved(
    frame: DipRect,
    from: Screen,
    to: Screen,
    gap: f32,
    last_tile: Option<WindowCommandId>,
) -> DipRect {
    if let Some(last) = last_tile {
        if let Some(frac) = tile_fractions(last, 0) {
            return tile(
                to.visible,
                frac.x0,
                frac.x1,
                frac.y0,
                frac.y1,
                sanitized_gap(gap, to.visible),
            );
        }
    }
    let source = from.visible;
    let target = to.visible;
    if source.w <= 0.0 || source.h <= 0.0 {
        return frame;
    }
    let relative_x = (frame.x - source.x) / source.w;
    let relative_y = (frame.y - source.y) / source.h;
    let scaled = DipRect {
        x: target.x + relative_x * target.w,
        y: target.y + relative_y * target.h,
        w: target.w.min(frame.w / source.w * target.w),
        h: target.h.min(frame.h / source.h * target.h),
    };
    rounded(clamped(scaled, target))
}

fn restore_placement(input: &LayoutQuery<'_>) -> Option<Placement> {
    let restore = input.restore?;
    let host = screen_containing(restore, input.screens)?;
    let overlap = intersection(host.visible, restore);
    let stranded = overlap.w < 40.0 || overlap.h < 40.0;
    let frame = if stranded {
        rounded(clamped(
            Anchor::CENTERED.place(restore.w, restore.h, host.visible),
            host.visible,
        ))
    } else {
        restore
    };
    Some(Placement {
        frame,
        screen_id: host.id,
        anchor: Anchor::CENTERED,
        resizes: true,
    })
}

struct Fractions {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    anchor: Anchor,
}

fn tile_fractions(command: WindowCommandId, step: i32) -> Option<Fractions> {
    let cycle = [0.5, ONE_THIRD, TWO_THIRDS];
    let position = cycle[normalized_step(step) as usize];
    match command {
        WindowCommandId::LeftHalf => Some(Fractions {
            x0: 0.0,
            x1: position,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::TOP_LEADING,
        }),
        WindowCommandId::RightHalf => Some(Fractions {
            x0: 1.0 - position,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        }),
        WindowCommandId::TopHalf => Some(Fractions {
            x0: 0.0,
            x1: 1.0,
            y0: 0.0,
            y1: position,
            anchor: Anchor::TOP_LEADING,
        }),
        WindowCommandId::BottomHalf => Some(Fractions {
            x0: 0.0,
            x1: 1.0,
            y0: 1.0 - position,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Min,
                vertical: Axis::Max,
            },
        }),
        WindowCommandId::TopLeftQuarter => Some(Fractions {
            x0: 0.0,
            x1: 0.5,
            y0: 0.0,
            y1: 0.5,
            anchor: Anchor::TOP_LEADING,
        }),
        WindowCommandId::TopRightQuarter => Some(Fractions {
            x0: 0.5,
            x1: 1.0,
            y0: 0.0,
            y1: 0.5,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        }),
        WindowCommandId::BottomLeftQuarter => Some(Fractions {
            x0: 0.0,
            x1: 0.5,
            y0: 0.5,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Min,
                vertical: Axis::Max,
            },
        }),
        WindowCommandId::BottomRightQuarter => Some(Fractions {
            x0: 0.5,
            x1: 1.0,
            y0: 0.5,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Max,
            },
        }),
        WindowCommandId::FirstThreeFourths => Some(Fractions {
            x0: 0.0,
            x1: 0.75,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::TOP_LEADING,
        }),
        WindowCommandId::LastThreeFourths => Some(Fractions {
            x0: 0.25,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        }),
        WindowCommandId::FirstThird => Some(Fractions {
            x0: 0.0,
            x1: ONE_THIRD,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::TOP_LEADING,
        }),
        WindowCommandId::CenterThird => Some(Fractions {
            x0: ONE_THIRD,
            x1: TWO_THIRDS,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Center,
                vertical: Axis::Min,
            },
        }),
        WindowCommandId::LastThird => Some(Fractions {
            x0: TWO_THIRDS,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        }),
        WindowCommandId::FirstTwoThirds => Some(Fractions {
            x0: 0.0,
            x1: TWO_THIRDS,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::TOP_LEADING,
        }),
        WindowCommandId::LastTwoThirds => Some(Fractions {
            x0: ONE_THIRD,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        }),
        WindowCommandId::CenterHalf => Some(Fractions {
            x0: 0.25,
            x1: 0.75,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Center,
                vertical: Axis::Min,
            },
        }),
        _ => None,
    }
}

pub fn is_tile_command(command: WindowCommandId) -> bool {
    tile_fractions(command, 0).is_some()
}

pub fn tile(visible: DipRect, x0: f32, x1: f32, y0: f32, y1: f32, gap: f32) -> DipRect {
    let left = visible.x + x0 * visible.w + if x0 == 0.0 { gap } else { gap / 2.0 };
    let right = visible.x + x1 * visible.w - if x1 == 1.0 { gap } else { gap / 2.0 };
    let top = visible.y + y0 * visible.h + if y0 == 0.0 { gap } else { gap / 2.0 };
    let bottom = visible.y + y1 * visible.h - if y1 == 1.0 { gap } else { gap / 2.0 };
    rounded(DipRect {
        x: left,
        y: top,
        w: (right - left).max(1.0),
        h: (bottom - top).max(1.0),
    })
}

pub fn canvas(visible: DipRect, gap: f32) -> DipRect {
    rounded(inset(visible, gap, gap))
}

fn minimum_size(canvas: DipRect) -> (f32, f32) {
    (
        canvas.w.min((canvas.w * 0.15).max(200.0)),
        canvas.h.min((canvas.h * 0.15).max(150.0)),
    )
}

fn even_step(dimension: f32) -> f32 {
    ((dimension * STEP_FRACTION / 2.0).round() * 2.0).max(2.0)
}

fn resized(frame: DipRect, canvas: DipRect, larger: bool) -> DipRect {
    let direction = if larger { 1.0 } else { -1.0 };
    let (floor_w, floor_h) = minimum_size(canvas);
    let width = canvas
        .w
        .min((frame.w + direction * even_step(canvas.w)).max(floor_w));
    let height = canvas
        .h
        .min((frame.h + direction * even_step(canvas.h)).max(floor_h));
    let centred = DipRect {
        x: frame.x - (width - frame.w) / 2.0,
        y: frame.y - (height - frame.h) / 2.0,
        w: width,
        h: height,
    };
    rounded(clamped(centred, canvas))
}

fn nudged(frame: DipRect, canvas: DipRect, command: WindowCommandId) -> DipRect {
    let dx = (canvas.w * STEP_FRACTION).round();
    let dy = (canvas.h * STEP_FRACTION).round();
    let mut moved = frame;
    match command {
        WindowCommandId::MoveLeft => moved.x -= dx,
        WindowCommandId::MoveRight => moved.x += dx,
        WindowCommandId::MoveUp => moved.y -= dy,
        WindowCommandId::MoveDown => moved.y += dy,
        _ => {}
    }
    rounded(clamped(moved, canvas))
}

pub fn rounded(rect: DipRect) -> DipRect {
    let min_x = rect.x.round();
    let min_y = rect.y.round();
    let max_x = (rect.x + rect.w).round();
    let max_y = (rect.y + rect.h).round();
    DipRect {
        x: min_x,
        y: min_y,
        w: (max_x - min_x).max(0.0),
        h: (max_y - min_y).max(0.0),
    }
}

pub fn clamped(frame: DipRect, box_r: DipRect) -> DipRect {
    let x = frame
        .x
        .max(box_r.x)
        .min(box_r.x.max(box_r.x + box_r.w - frame.w));
    let y = frame
        .y
        .max(box_r.y)
        .min(box_r.y.max(box_r.y + box_r.h - frame.h));
    DipRect {
        x,
        y,
        w: frame.w,
        h: frame.h,
    }
}

pub fn sanitized_gap(gap: f32, visible: DipRect) -> f32 {
    if !gap.is_finite() || gap <= 0.0 || visible.w <= 0.0 || visible.h <= 0.0 {
        0.0
    } else {
        gap.min(visible.w.min(visible.h) / 10.0)
    }
}

fn normalized_step(step: i32) -> i32 {
    ((step % 3) + 3) % 3
}

fn inset(r: DipRect, dx: f32, dy: f32) -> DipRect {
    DipRect {
        x: r.x + dx,
        y: r.y + dy,
        w: (r.w - dx * 2.0).max(0.0),
        h: (r.h - dy * 2.0).max(0.0),
    }
}

fn intersection(a: DipRect, b: DipRect) -> DipRect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    DipRect {
        x: x0,
        y: y0,
        w: (x1 - x0).max(0.0),
        h: (y1 - y0).max(0.0),
    }
}

fn intersection_area(a: DipRect, b: DipRect) -> f32 {
    let i = intersection(a, b);
    i.w * i.h
}

fn contains(r: DipRect, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_command::{WindowCommandId, WindowKind};

    #[test]
    fn top_half_min_y_is_visible_min_y() {
        let vis = DipRect {
            x: 0.0,
            y: 40.0,
            w: 1000.0,
            h: 800.0,
        };
        let r = placement(
            WindowCommandId::TopHalf,
            LayoutInput {
                visible: vis,
                window: vis,
                gap: 0.0,
                step: 0,
            },
        )
        .unwrap();
        assert!((r.y - vis.y).abs() < 0.01);
    }

    #[test]
    fn fuzz_finite_rects() {
        let vis = DipRect {
            x: 0.0,
            y: 0.0,
            w: 1440.0,
            h: 900.0,
        };
        for id in WindowCommandId::all() {
            if matches!(id.kind(), WindowKind::Space | WindowKind::Fullscreen) {
                continue;
            }
            let Some(r) = placement(
                *id,
                LayoutInput {
                    visible: vis,
                    window: vis,
                    gap: 8.0,
                    step: 0,
                },
            ) else {
                continue;
            };
            assert!(r.w.is_finite() && r.h.is_finite());
            assert!(r.w >= 0.0 && r.h >= 0.0);
        }
    }
}
