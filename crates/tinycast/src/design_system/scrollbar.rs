//! Thin overlay scrollbar from `ThinScrollbar.swift`.
//!
//! The thumb fades with ease-out over 0.18s, lingers 800ms after scrolling,
//! and widens on a spring while the pointer is in the trailing strip.

use std::time::{Duration, Instant};

use tinycast_pure::motion::{lerp, spring_unit, Ease, Reveal};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use super::fill_squircle;

const THIN: f32 = 6.0;
const EXPANDED: f32 = 10.0;
const INSET: f32 = 3.0;
const MIN_THUMB: f32 = 28.0;
const HOVER_ZONE: f32 = 16.0;

pub struct ScrollChrome {
    opacity: Reveal,
    width: f32,
    width_from: f32,
    width_to: f32,
    width_at: Option<Instant>,
    last_scroll: f32,
    primed: bool,
    idle_deadline: Option<Instant>,
    hover: bool,
    hover_deadline: Option<Instant>,
}

impl Default for ScrollChrome {
    fn default() -> Self {
        Self {
            opacity: Reveal::default(),
            width: THIN,
            width_from: THIN,
            width_to: THIN,
            width_at: None,
            last_scroll: 0.0,
            primed: false,
            idle_deadline: None,
            hover: false,
            hover_deadline: None,
        }
    }
}

impl ScrollChrome {
    pub fn hover_zone(x: f32, y: f32, right: f32, top: f32, bottom: f32) -> bool {
        x >= right - HOVER_ZONE && y >= top && y <= bottom
    }

    pub fn pointer(&mut self, inside: bool, now: Instant) {
        if inside {
            self.hover = true;
            self.hover_deadline = None;
        } else if self.hover && self.hover_deadline.is_none() {
            self.hover_deadline =
                Some(now + Duration::from_secs_f32(theme::duration::SCROLLBAR_HOVER_EXIT_SECS));
        }
    }

    /// `scrollable` is content taller than the viewport. Returns whether another
    /// frame is needed.
    pub fn sync(&mut self, scroll: f32, scrollable: bool, now: Instant) -> bool {
        if self.hover_deadline.is_some_and(|deadline| now >= deadline) {
            self.hover = false;
            self.hover_deadline = None;
        }
        if !self.primed {
            self.last_scroll = scroll;
            self.primed = true;
        } else if scrollable && (scroll - self.last_scroll).abs() > 0.5 {
            self.last_scroll = scroll;
            self.idle_deadline =
                Some(now + Duration::from_secs_f32(theme::duration::SCROLLBAR_IDLE_SECS));
        }
        let scrolling = scrollable && self.idle_deadline.is_some_and(|deadline| now < deadline);
        if !scrollable {
            self.hover = false;
            self.hover_deadline = None;
            self.idle_deadline = None;
        }
        let show = scrolling || self.hover;
        let _ = self.opacity.go(
            if show { 1.0 } else { 0.0 },
            theme::duration::SCROLLBAR_FADE_SECS,
            Ease::Out,
        );
        self.aim_width(if self.hover { EXPANDED } else { THIN }, now);
        self.tick_width(now);
        self.needs_frame(now)
    }

    pub fn opacity(&self) -> f32 {
        self.opacity.value
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn needs_frame(&self, now: Instant) -> bool {
        self.opacity.is_running()
            || self.width_at.is_some()
            || self.idle_deadline.is_some_and(|deadline| now < deadline)
            || self.hover_deadline.is_some_and(|deadline| now < deadline)
    }

    fn aim_width(&mut self, to: f32, now: Instant) {
        if (self.width_to - to).abs() < 0.05 {
            return;
        }
        self.width_from = self.width;
        self.width_to = to;
        self.width_at = Some(now);
    }

    fn tick_width(&mut self, now: Instant) {
        let Some(start) = self.width_at else {
            return;
        };
        let t = now.saturating_duration_since(start).as_secs_f32();
        let eased = spring_unit(
            t,
            theme::duration::SCROLLBAR_SPRING_RESPONSE,
            theme::duration::SCROLLBAR_SPRING_DAMPING,
        );
        self.width = lerp(self.width_from, self.width_to, eased.clamp(0.0, 1.0));
        if eased > 0.995 || t > 1.2 {
            self.width = self.width_to;
            self.width_at = None;
        }
    }
}

pub struct Thumb {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rail: DipRect,
    pub rail_alpha: f32,
    pub thumb_alpha: f32,
}

pub fn thumb_geom(
    right: f32,
    track_y: f32,
    track_h: f32,
    scroll: f32,
    content_h: f32,
    view_h: f32,
    thumb_w: f32,
    opacity: f32,
) -> Option<Thumb> {
    if opacity <= 0.01 || content_h <= view_h + 1.0 || track_h < MIN_THUMB {
        return None;
    }
    let thumb_w = thumb_w.clamp(THIN, EXPANDED);
    let thumb_h = track_h.min((track_h * view_h / content_h).max(MIN_THUMB));
    let max_scroll = (content_h - view_h).max(0.0);
    let fraction = if max_scroll <= 0.5 {
        0.0
    } else {
        (scroll / max_scroll).clamp(0.0, 1.0)
    };
    let slot_x = right - INSET - EXPANDED;
    let reveal = ((thumb_w - THIN) / (EXPANDED - THIN)).clamp(0.0, 1.0);
    Some(Thumb {
        x: slot_x + (EXPANDED - thumb_w) * 0.5,
        y: track_y + fraction * (track_h - thumb_h),
        w: thumb_w,
        h: thumb_h,
        rail: DipRect {
            x: slot_x,
            y: track_y,
            w: EXPANDED,
            h: track_h,
        },
        rail_alpha: 0.10 * reveal * opacity,
        thumb_alpha: (0.30 + 0.12 * reveal) * opacity,
    })
}

pub fn paint(
    target: &ID2D1RenderTarget,
    right: f32,
    track_y: f32,
    track_h: f32,
    scroll: f32,
    content_h: f32,
    view_h: f32,
    thumb_w: f32,
    opacity: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let Some(thumb) = thumb_geom(
        right, track_y, track_h, scroll, content_h, view_h, thumb_w, opacity,
    ) else {
        return Ok(());
    };
    if thumb.rail_alpha > 0.01 {
        let rail = theme::colors::ramp_rgba(appearance, thumb.rail_alpha, thumb.rail_alpha);
        fill_squircle(target, thumb.rail, EXPANDED * 0.5, rail)?;
    }
    let ink = theme::colors::ramp_rgba(appearance, thumb.thumb_alpha, thumb.thumb_alpha);
    fill_squircle(
        target,
        DipRect {
            x: thumb.x,
            y: thumb.y,
            w: thumb.w,
            h: thumb.h,
        },
        thumb.w * 0.5,
        ink,
    )
}
