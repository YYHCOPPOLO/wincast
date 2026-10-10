//! Easing shared with upstream Tinycast (oracle `da2c4e44`).
//!
//! Window fades use `CAMediaTimingFunction`: ease-out
//! `cubic-bezier(0, 0, 0.58, 1)` on the way in, ease-in
//! `cubic-bezier(0.42, 0, 1, 1)` on the way out
//! (`DesignSystem/Interaction/PanelTransition.swift`). SwiftUI `.easeOut` /
//! `.easeIn` / `.easeInOut` use those same control points. Palette menus play
//! [`MenuReveal`] (`Theme.MenuMotion`); dialogs play [`DialogMotion`]. Scrollbar
//! knobs use the thin-scrollbar spring (`response: 0.28`, `dampingFraction: 0.85`).

use std::time::Instant;

use crate::theme;

/// Content scale at the start of a panel entrance, matching `PanelEntrance.entryScale`.
pub const ENTER_SCALE: f32 = 0.94;

/// Settings shortcut-recorder callout scale at the start of its transition.
/// Upstream scales that callout from 0.5 (`Features/HotKeys/UI/ShortcutRecorderPopover.swift:139`);
/// it is retuned with the Settings phase, not here.
pub const CALLOUT_SCALE: f32 = 0.96;

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

/// Samples a `CAMediaTimingFunction(controlPoints:)` tuple.
pub fn bezier(curve: (f32, f32, f32, f32), t: f32) -> f32 {
    cubic_bezier(curve.0, curve.1, curve.2, curve.3, t)
}

/// A palette menu's alpha and scale `elapsed` seconds into its reveal
/// (`Palette/MenuPanel.swift` `startReveal`). Returns `true` once settled.
pub fn menu_open_visual(elapsed_secs: f32) -> (Visual, bool) {
    use theme::menu_motion as m;
    let total = m::EXPANSION_SECS + m::SETTLE_SECS;
    let e = elapsed_secs.max(0.0);
    let alpha = bezier(m::EXPANSION_CURVE, e / total);
    let scale = if e < m::EXPANSION_SECS {
        lerp(
            m::ENTRY_SCALE,
            m::MAXIMUM_SCALE,
            bezier(m::EXPANSION_CURVE, e / m::EXPANSION_SECS),
        )
    } else {
        lerp(
            m::MAXIMUM_SCALE,
            1.0,
            bezier(m::SETTLE_CURVE, (e - m::EXPANSION_SECS) / m::SETTLE_SECS),
        )
    };
    let done = e >= total;
    if done {
        (Visual::rest(), true)
    } else {
        (Visual { scale, alpha }, false)
    }
}

/// A palette menu's dismissal from wherever it stood (`MenuPanel.swift`
/// `startDismissalAnimation`): scale drops by `exitScaleDelta`, never below
/// `entryScale − exitScaleDelta`, while alpha runs to 0, both on the exit curve.
pub fn menu_close_visual(from: Visual, elapsed_secs: f32) -> (Visual, bool) {
    use theme::menu_motion as m;
    let floor = m::ENTRY_SCALE - m::EXIT_SCALE_DELTA;
    let target = (from.scale - m::EXIT_SCALE_DELTA).max(floor);
    let t = (elapsed_secs.max(0.0) / m::EXIT_SECS).clamp(0.0, 1.0);
    let e = bezier(m::EXIT_CURVE, t);
    let done = t >= 1.0;
    (
        Visual {
            scale: lerp(from.scale, target, e),
            alpha: if done { 0.0 } else { lerp(from.alpha, 0.0, e) },
        },
        done,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MenuPhase {
    Hidden,
    Opening { start: Instant },
    Open,
    Closing { start: Instant, from: Visual },
}

/// The open/close state machine behind [`menu_open_visual`] and
/// [`menu_close_visual`]. Re-opening while closing restarts from the entry scale
/// at alpha 0, as `MenuPanel.show` resets `alphaValue` and the model scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuReveal {
    phase: MenuPhase,
}

impl Default for MenuReveal {
    fn default() -> Self {
        Self {
            phase: MenuPhase::Hidden,
        }
    }
}

impl MenuReveal {
    pub fn open(&mut self, now: Instant) {
        if matches!(self.phase, MenuPhase::Hidden | MenuPhase::Closing { .. }) {
            self.phase = MenuPhase::Opening { start: now };
        }
    }

    /// A different menu replaced the open one: play the reveal again.
    pub fn restart(&mut self, now: Instant) {
        self.phase = MenuPhase::Opening { start: now };
    }

    pub fn close(&mut self, now: Instant) {
        if matches!(self.phase, MenuPhase::Opening { .. } | MenuPhase::Open) {
            let from = self.sample(now);
            self.phase = MenuPhase::Closing { start: now, from };
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.phase,
            MenuPhase::Opening { .. } | MenuPhase::Closing { .. }
        )
    }

    pub fn is_hidden(&self) -> bool {
        self.phase == MenuPhase::Hidden
    }

    pub fn sample(&mut self, now: Instant) -> Visual {
        match self.phase {
            MenuPhase::Hidden => Visual {
                scale: theme::menu_motion::ENTRY_SCALE,
                alpha: 0.0,
            },
            MenuPhase::Open => Visual::rest(),
            MenuPhase::Opening { start } => {
                let (visual, done) = menu_open_visual(secs_since(start, now));
                if done {
                    self.phase = MenuPhase::Open;
                }
                visual
            }
            MenuPhase::Closing { start, from } => {
                let (visual, done) = menu_close_visual(from, secs_since(start, now));
                if done {
                    self.phase = MenuPhase::Hidden;
                }
                visual
            }
        }
    }
}

fn secs_since(start: Instant, now: Instant) -> f32 {
    now.checked_duration_since(start)
        .unwrap_or_default()
        .as_secs_f32()
}

/// `Theme.DialogMotion` + `Duration.dialogEnter/Exit`: a dialog rises 3pt from 8%
/// opacity on ease-out, and fades out on ease-in without moving
/// (`docs/ui.md` "Entrance and exit"). Returns (alpha, y offset in DIP, done).
pub struct DialogMotion;

impl DialogMotion {
    pub fn enter(elapsed_secs: f32) -> (f32, f32, bool) {
        use theme::dialog_motion as d;
        let t = (elapsed_secs.max(0.0) / theme::duration::DIALOG_ENTER_SECS).clamp(0.0, 1.0);
        let e = Ease::Out.apply(t);
        (
            lerp(d::INITIAL_OPACITY, 1.0, e),
            lerp(d::OFFSET, 0.0, e),
            t >= 1.0,
        )
    }

    pub fn exit(from_alpha: f32, elapsed_secs: f32) -> (f32, bool) {
        let t = (elapsed_secs.max(0.0) / theme::duration::DIALOG_EXIT_SECS).clamp(0.0, 1.0);
        (lerp(from_alpha, 0.0, Ease::In.apply(t)), t >= 1.0)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn menu_reveal_overshoots_then_settles() {
        let (start, done) = menu_open_visual(0.0);
        assert!(!done);
        assert!((start.scale - 0.94).abs() < 1e-4);
        assert!(start.alpha.abs() < 1e-4);
        let (peak, _) = menu_open_visual(theme::menu_motion::EXPANSION_SECS);
        assert!((peak.scale - 1.003).abs() < 1e-3, "peak {}", peak.scale);
        let (end, done) = menu_open_visual(0.15);
        assert!(done);
        assert_eq!(end, Visual::rest());
        // The expansion curve front-loads: most of the alpha arrives early.
        let (early, _) = menu_open_visual(0.03);
        assert!(early.alpha > 0.3, "alpha {}", early.alpha);
    }

    #[test]
    fn menu_close_drops_scale_by_delta_and_floors() {
        let (end, done) = menu_close_visual(Visual::rest(), theme::menu_motion::EXIT_SECS);
        assert!(done);
        assert!((end.scale - 0.96).abs() < 1e-4);
        assert_eq!(end.alpha, 0.0);
        let low = Visual {
            scale: 0.92,
            alpha: 0.5,
        };
        let (end, _) = menu_close_visual(low, 1.0);
        assert!((end.scale - 0.90).abs() < 1e-4);
    }

    #[test]
    fn menu_reveal_state_machine() {
        let t0 = Instant::now();
        let mut m = MenuReveal::default();
        assert!(m.is_hidden());
        m.open(t0);
        assert!(m.is_running());
        let v = m.sample(t0 + std::time::Duration::from_millis(200));
        assert_eq!(v, Visual::rest());
        assert!(!m.is_running());
        m.close(t0 + std::time::Duration::from_millis(200));
        assert!(m.is_running());
        let _ = m.sample(t0 + std::time::Duration::from_millis(400));
        assert!(m.is_hidden());
        // Re-open mid-close starts again from the entry scale.
        m.open(t0);
        m.close(t0 + std::time::Duration::from_millis(50));
        m.open(t0 + std::time::Duration::from_millis(60));
        let v = m.sample(t0 + std::time::Duration::from_millis(60));
        assert!((v.scale - 0.94).abs() < 1e-3);
    }

    #[test]
    fn dialog_rises_three_points_from_eight_percent() {
        let (a, y, done) = DialogMotion::enter(0.0);
        assert!((a - 0.08).abs() < 1e-4 && (y - 3.0).abs() < 1e-4 && !done);
        let (a, y, done) = DialogMotion::enter(theme::duration::DIALOG_ENTER_SECS);
        assert!((a - 1.0).abs() < 1e-4 && y.abs() < 1e-4 && done);
        let (a, done) = DialogMotion::exit(1.0, theme::duration::DIALOG_EXIT_SECS);
        assert!(a.abs() < 1e-4 && done);
    }

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
