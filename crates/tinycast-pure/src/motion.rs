//! Easing shared with Tinycast v0.10.2.
//!
//! Window fades use `CAMediaTimingFunction`: ease-out
//! `cubic-bezier(0, 0, 0.58, 1)` on the way in, ease-in
//! `cubic-bezier(0.42, 0, 1, 1)` on the way out. SwiftUI `.easeOut` /
//! `.easeIn` / `.easeInOut` use those same control points. A menu or callout
//! also scales from [`MENU_SCALE`]. Scrollbar knobs use the thin-scrollbar
//! spring (`response: 0.28`, `dampingFraction: 0.85`).

use std::time::Instant;

use crate::theme;

/// Content scale at the start of a panel entrance, matching `PanelEntrance.entryScale`.
pub const ENTER_SCALE: f32 = 0.94;

/// Menu and recorder-callout scale at the start of their transition.
pub const MENU_SCALE: f32 = 0.96;

/// Floor so a reverse near the end of a clip does not vanish in one frame.
const MIN_DURATION_SECS: f32 = 0.06;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    Linear,
    /// `CAMediaTimingFunction.easeIn` / SwiftUI `.easeIn`.
    In,
    /// `CAMediaTimingFunction.easeOut` / SwiftUI `.easeOut`.
    Out,
    /// `CAMediaTimingFunction.easeInEaseOut` / SwiftUI `.easeInOut`.
    InOut,
}

impl Ease {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::In => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
            Self::Out => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
            Self::InOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
        }
    }
}

/// CSS/Fluent cubic-bezier with endpoints fixed at (0, 0) and (1, 1).
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let t = if x1 == 0.0 && x2 == 0.0 {
        x.cbrt()
    } else if x1 == 1.0 && x2 == 1.0 {
        1.0 - (1.0 - x).cbrt()
    } else {
        solve_bezier_x(x, x1, x2)
    };
    sample_bezier(t, y1, y2)
}

fn sample_bezier(t: f32, a: f32, b: f32) -> f32 {
    let c = 1.0 - t;
    3.0 * c * c * t * a + 3.0 * c * t * t * b + t * t * t
}

fn sample_bezier_slope(t: f32, a: f32, b: f32) -> f32 {
    let c = 1.0 - t;
    3.0 * c * c * a + 6.0 * c * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

fn solve_bezier_x(x: f32, x1: f32, x2: f32) -> f32 {
    let mut t = x;
    for _ in 0..8 {
        let err = sample_bezier(t, x1, x2) - x;
        if err.abs() < 1.0e-5 {
            return t;
        }
        let slope = sample_bezier_slope(t, x1, x2);
        if slope.abs() < 1.0e-6 {
            break;
        }
        t = (t - err / slope).clamp(0.0, 1.0);
    }
    let mut lo = 0.0;
    let mut hi = 1.0;
    t = x;
    for _ in 0..14 {
        let err = sample_bezier(t, x1, x2) - x;
        if err.abs() < 1.0e-5 {
            break;
        }
        if err > 0.0 {
            hi = t;
        } else {
            lo = t;
        }
        t = (lo + hi) * 0.5;
    }
    t
}

pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - (u * u * u) / 2.0
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Unit spring from 0 to 1, released from rest. Matches SwiftUI
/// `.spring(response:dampingFraction:)` with mass 1.
pub fn spring_unit(t: f32, response: f32, damping: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if response <= 0.001 {
        return 1.0;
    }
    let omega = std::f32::consts::TAU / response;
    let zeta = damping.max(0.0);
    if (zeta - 1.0).abs() < 0.001 {
        let decay = (-omega * t).exp();
        return 1.0 - decay * (1.0 + omega * t);
    }
    if zeta < 1.0 {
        let wd = omega * (1.0 - zeta * zeta).sqrt();
        let decay = (-zeta * omega * t).exp();
        let beta = zeta * omega;
        let value = 1.0 - decay * ((wd * t).cos() + (beta / wd) * (wd * t).sin());
        return value.clamp(0.0, 1.08);
    }
    let disc = (zeta * zeta - 1.0).sqrt();
    let r1 = omega * (zeta - disc);
    let r2 = omega * (zeta + disc);
    let a = r2 / (r2 - r1);
    let b = -r1 / (r2 - r1);
    (1.0 - a * (-r1 * t).exp() - b * (-r2 * t).exp()).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub scale: f32,
    pub alpha: f32,
}

impl Visual {
    pub const fn hidden() -> Self {
        Self {
            scale: ENTER_SCALE,
            alpha: 0.0,
        }
    }

    pub const fn rest() -> Self {
        Self {
            scale: 1.0,
            alpha: 1.0,
        }
    }

    pub fn is_rest(self) -> bool {
        self.alpha >= 0.995 && (self.scale - 1.0).abs() < 0.002
    }

    pub fn is_hidden(self) -> bool {
        self.alpha <= 0.005
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub from: Visual,
    pub to: Visual,
    pub duration_secs: f32,
    pub ease: Ease,
}

impl Motion {
    pub fn enter() -> Self {
        Self {
            from: Visual::hidden(),
            to: Visual::rest(),
            duration_secs: theme::duration::ENTER_SECS,
            ease: Ease::Out,
        }
    }

    /// Alpha only. The original fade-out does not scale the content back down.
    pub fn exit() -> Self {
        Self::exit_from(Visual::rest())
    }

    pub fn exit_from(from: Visual) -> Self {
        Self {
            from,
            to: Visual {
                scale: from.scale,
                alpha: 0.0,
            },
            duration_secs: theme::duration::EXIT_SECS,
            ease: Ease::In,
        }
    }

    pub fn retarget(from: Visual, to: Visual, duration_secs: f32, ease: Ease) -> Self {
        Self {
            from,
            to,
            duration_secs: duration_secs.max(MIN_DURATION_SECS),
            ease,
        }
    }

    pub fn sample(self, elapsed_secs: f32) -> (Visual, bool) {
        if self.duration_secs <= 0.0 {
            return (self.to, true);
        }
        let done = elapsed_secs >= self.duration_secs;
        let t = if done {
            1.0
        } else {
            (elapsed_secs / self.duration_secs).clamp(0.0, 1.0)
        };
        let e = self.ease.apply(t);
        (
            Visual {
                scale: lerp(self.from.scale, self.to.scale, e),
                alpha: lerp(self.from.alpha, self.to.alpha, e),
            },
            done,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalarMotion {
    pub from: f32,
    pub to: f32,
    pub duration_secs: f32,
    pub ease: Ease,
}

impl ScalarMotion {
    pub fn sample(self, elapsed_secs: f32) -> (f32, bool) {
        if self.duration_secs <= 0.0 || (self.from - self.to).abs() < 0.01 {
            return (self.to, true);
        }
        let done = elapsed_secs >= self.duration_secs;
        let t = if done {
            1.0
        } else {
            (elapsed_secs / self.duration_secs).clamp(0.0, 1.0)
        };
        (lerp(self.from, self.to, self.ease.apply(t)), done)
    }
}

/// A scalar that runs the full duration on every retarget, the way
/// `withAnimation` and `NSAnimationContext` do.
#[derive(Clone, Debug)]
pub struct Reveal {
    pub value: f32,
    motion: Option<ScalarMotion>,
    start: Instant,
}

impl Default for Reveal {
    fn default() -> Self {
        Self::at(0.0)
    }
}

impl Reveal {
    pub fn at(value: f32) -> Self {
        Self {
            value,
            motion: None,
            start: Instant::now(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.motion.is_some()
    }

    /// Returns whether a clip is in flight after the call.
    pub fn go(&mut self, to: f32, duration_secs: f32, ease: Ease) -> bool {
        let current = self.sample_now();
        if self.motion.is_none() && (current - to).abs() < 0.001 {
            self.value = to;
            return false;
        }
        if let Some(motion) = self.motion {
            if (motion.to - to).abs() < 0.001 {
                return true;
            }
        }
        self.motion = Some(ScalarMotion {
            from: current,
            to,
            duration_secs,
            ease,
        });
        self.start = Instant::now();
        true
    }

    pub fn sample_now(&mut self) -> f32 {
        self.sample_at(Instant::now())
    }

    pub fn sample_at(&mut self, now: Instant) -> f32 {
        let Some(motion) = self.motion else {
            return self.value;
        };
        let elapsed = now
            .checked_duration_since(self.start)
            .unwrap_or_default()
            .as_secs_f32();
        let (value, done) = motion.sample(elapsed);
        self.value = value;
        if done {
            self.motion = None;
            self.value = motion.to;
        }
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_eases_pin_endpoints() {
        assert_eq!(ease_in_cubic(0.0), 0.0);
        assert_eq!(ease_in_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_in_out_cubic(0.0), 0.0);
        assert_eq!(ease_in_out_cubic(1.0), 1.0);
    }

    #[test]
    fn ease_out_leads_linear_ease_in_lags() {
        let mid = 0.5;
        assert!(ease_out_cubic(mid) > mid);
        assert!(ease_in_cubic(mid) < mid);
        assert!((ease_in_out_cubic(mid) - mid).abs() < 0.001);
    }

    #[test]
    fn apple_ease_curves_match_core_animation() {
        assert_eq!(Ease::Out.apply(0.0), 0.0);
        assert_eq!(Ease::Out.apply(1.0), 1.0);
        assert_eq!(Ease::In.apply(0.0), 0.0);
        assert_eq!(Ease::In.apply(1.0), 1.0);
        let out = Ease::Out.apply(0.5);
        let inn = Ease::In.apply(0.5);
        let both = Ease::InOut.apply(0.5);
        assert!(out > 0.62 && out < 0.78, "ease-out at halfway is {out}");
        assert!(inn < 0.40, "ease-in at halfway is {inn}");
        assert!(
            (both - 0.5).abs() < 0.08,
            "ease-in-out at halfway is {both}"
        );
        let mut prev = -1.0;
        for i in 0..=20 {
            let y = Ease::Out.apply(i as f32 / 20.0);
            assert!(y + 0.0001 >= prev);
            prev = y;
        }
    }

    #[test]
    fn spring_starts_at_rest_and_settles() {
        assert!(spring_unit(0.0, 0.28, 0.85).abs() < 0.001);
        let late = spring_unit(0.8, 0.28, 0.85);
        assert!(late > 0.95 && late < 1.05, "settled spring is {late}");
        assert!(spring_unit(0.1, 0.28, 0.85) > 0.2);
    }

    #[test]
    fn eases_are_monotonic() {
        let mut prev_out = -1.0;
        let mut prev_in = -1.0;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let out = ease_out_cubic(t);
            let inn = ease_in_cubic(t);
            assert!(out >= prev_out);
            assert!(inn >= prev_in);
            prev_out = out;
            prev_in = inn;
        }
    }

    #[test]
    fn linear_lerp_is_stiff_midpoint() {
        assert!((lerp(0.0, 1.0, 0.5) - 0.5).abs() < f32::EPSILON);
        assert_eq!(lerp(2.0, 4.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 4.0, 1.0), 4.0);
    }

    #[test]
    fn enter_starts_hidden_and_settles_at_rest() {
        let m = Motion::enter();
        let (start, done) = m.sample(0.0);
        assert!(!done);
        assert_eq!(start, Visual::hidden());
        let (end, done) = m.sample(theme::duration::ENTER_SECS);
        assert!(done);
        assert_eq!(end, Visual::rest());
        let (mid, _) = m.sample(theme::duration::ENTER_SECS * 0.5);
        assert!(mid.alpha > 0.5);
        assert!(mid.scale > 0.97);
    }

    #[test]
    fn exit_fades_alpha_and_keeps_scale() {
        let m = Motion::exit();
        let (mid, _) = m.sample(theme::duration::EXIT_SECS * 0.5);
        assert!(mid.alpha > 0.5, "ease-in still mostly opaque at halfway");
        assert!((mid.scale - 1.0).abs() < 0.001);
        let (end, done) = m.sample(theme::duration::EXIT_SECS);
        assert!(done);
        assert!(end.is_hidden());
        assert!((end.scale - 1.0).abs() < 0.001);
    }

    #[test]
    fn exit_from_mid_enter_freezes_scale() {
        let enter = Motion::enter();
        let (almost, _) = enter.sample(theme::duration::ENTER_SECS * 0.5);
        let exit = Motion::exit_from(almost);
        assert!((exit.duration_secs - theme::duration::EXIT_SECS).abs() < 0.001);
        let (at_zero, _) = exit.sample(0.0);
        assert!((at_zero.alpha - almost.alpha).abs() < 0.001);
        assert!((at_zero.scale - almost.scale).abs() < 0.001);
        let (end, _) = exit.sample(theme::duration::EXIT_SECS);
        assert!(end.alpha < 0.001);
        assert!((end.scale - almost.scale).abs() < 0.001);
    }

    #[test]
    fn scalar_motion_eases_height() {
        let m = ScalarMotion {
            from: 64.0,
            to: 475.0,
            duration_secs: theme::duration::ENTER_SECS,
            ease: Ease::Out,
        };
        let (mid, done) = m.sample(theme::duration::ENTER_SECS * 0.5);
        assert!(!done);
        assert!(mid > lerp(64.0, 475.0, 0.5));
        let (end, done) = m.sample(theme::duration::ENTER_SECS);
        assert!(done);
        assert!((end - 475.0).abs() < 0.01);
    }

    #[test]
    fn rest_and_hidden_predicates() {
        assert!(Visual::rest().is_rest());
        assert!(!Visual::rest().is_hidden());
        assert!(Visual::hidden().is_hidden());
        assert!(!Visual::hidden().is_rest());
    }
}
