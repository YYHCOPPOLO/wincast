//! Overlay fade/scale on top of `tinycast_pure::motion`.
//!
//! Frames are paced to the DWM vblank. `SetTimer(16)` drifts against the
//! refresh and lands between vsyncs, which is what makes a Win32 animation
//! look like it stutters next to a WinUI flyout.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, Once};
use std::time::Instant;

use tinycast_pure::motion::{Ease, Motion, Visual};
use tinycast_pure::theme;
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS,
};
use windows::Win32::Graphics::Dwm::{DwmGetCompositionTimingInfo, DWM_TIMING_INFO};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{
    CreateWaitableTimerExW, SetWaitableTimer, WaitForSingleObject,
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, PostMessageW, WM_TIMER};

pub const TICK_MS: u32 = 16;
pub const TIMER_ID: usize = 41;

struct Slot {
    hwnd: isize,
    id: usize,
    /// A frame message is already queued. Cleared when that frame finishes.
    pending: AtomicBool,
}

struct Clock {
    slots: Mutex<Vec<Slot>>,
    wake: Condvar,
}

fn clock() -> &'static Clock {
    static CLOCK: std::sync::OnceLock<Clock> = std::sync::OnceLock::new();
    CLOCK.get_or_init(|| Clock {
        slots: Mutex::new(Vec::new()),
        wake: Condvar::new(),
    })
}

fn slot_lock(clock: &Clock) -> std::sync::MutexGuard<'_, Vec<Slot>> {
    clock.slots.lock().unwrap_or_else(|err| err.into_inner())
}

/// Clears the queued-frame flag when the tick handler returns, including
/// early returns, so the next vblank can post exactly one more frame.
pub struct FrameGuard {
    hwnd: isize,
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        let clock = clock();
        let slots = slot_lock(clock);
        for slot in slots.iter() {
            if slot.hwnd == self.hwnd {
                slot.pending.store(false, Ordering::Release);
            }
        }
        drop(slots);
        clock.wake.notify_one();
    }
}

pub fn frame_guard(hwnd: HWND) -> FrameGuard {
    FrameGuard {
        hwnd: hwnd.0 as isize,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    pub running: bool,
    pub hide: bool,
}

pub struct OverlayAnim {
    motion: Option<Motion>,
    start: Instant,
    visual: Visual,
    hiding: bool,
}

impl Default for OverlayAnim {
    fn default() -> Self {
        Self {
            motion: None,
            start: Instant::now(),
            visual: Visual::hidden(),
            hiding: false,
        }
    }
}

impl OverlayAnim {
    pub fn visual(&self) -> Visual {
        self.visual
    }

    pub fn is_hiding(&self) -> bool {
        self.hiding
    }

    pub fn is_running(&self) -> bool {
        self.motion.is_some()
    }

    /// Fade and scale in. A full `Theme.Duration.enter`, even when reversing.
    pub fn present(&mut self, already_visible: bool) {
        self.begin(already_visible, true);
    }

    /// Window fade only. The message pill has no `panelEntrance`.
    pub fn present_fade(&mut self, already_visible: bool) {
        self.begin(already_visible, false);
    }

    /// `cancelFade`: the panel is already on screen, so opacity snaps back.
    pub fn snap_shown(&mut self) {
        self.motion = None;
        self.visual = Visual::rest();
        self.hiding = false;
    }

    fn begin(&mut self, already_visible: bool, scale_in: bool) {
        if already_visible && !self.hiding && !self.is_running() {
            self.visual = Visual::rest();
            return;
        }
        let from = if self.motion.is_some() || already_visible {
            self.visual
        } else if scale_in {
            Visual::hidden()
        } else {
            Visual {
                scale: 1.0,
                alpha: 0.0,
            }
        };
        self.motion = Some(Motion::retarget(
            from,
            Visual::rest(),
            theme::duration::ENTER_SECS,
            Ease::Out,
        ));
        self.start = Instant::now();
        self.hiding = false;
        self.visual = from;
    }

    /// Fade out. Alpha only, over the full exit duration.
    pub fn dismiss(&mut self) {
        if self.hiding && self.motion.is_some() {
            return;
        }
        if self.visual.is_hidden() && self.motion.is_none() {
            self.hiding = true;
            return;
        }
        let from = self.visual;
        self.motion = Some(Motion::exit_from(from));
        self.start = Instant::now();
        self.hiding = true;
    }

    pub fn snap_end(&mut self) {
        if let Some(motion) = self.motion.take() {
            self.visual = motion.to;
        }
    }

    pub fn tick(&mut self) -> Tick {
        self.tick_at(Instant::now())
    }

    pub fn tick_at(&mut self, now: Instant) -> Tick {
        let Some(motion) = self.motion else {
            return Tick {
                running: false,
                hide: self.hiding && self.visual.is_hidden(),
            };
        };
        let elapsed = now
            .checked_duration_since(self.start)
            .unwrap_or_default()
            .as_secs_f32();
        let (visual, done) = motion.sample(elapsed);
        self.visual = visual;
        if done {
            self.motion = None;
            self.visual = motion.to;
        }
        Tick {
            running: self.motion.is_some(),
            hide: self.hiding && self.motion.is_none() && self.visual.is_hidden(),
        }
    }
}

pub fn scale_about(scale: f32, anchor_x: f32, anchor_y: f32) -> Matrix3x2 {
    Matrix3x2 {
        M11: scale,
        M12: 0.0,
        M21: 0.0,
        M22: scale,
        M31: anchor_x * (1.0 - scale),
        M32: anchor_y * (1.0 - scale),
    }
}

/// Draws `paint` with opacity and a local-space transform, then restores the
/// transform that was current. Opacity of 1 and an identity local transform
/// skip the layer.
pub fn paint_in_layer(
    target: &ID2D1RenderTarget,
    opacity: f32,
    local: Matrix3x2,
    paint: impl FnOnce() -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    if opacity <= 0.001 {
        return Ok(());
    }
    let mut previous = identity_transform();
    unsafe { target.GetTransform(&mut previous) };
    let identity = (local.M11 - 1.0).abs() < 0.001
        && local.M12.abs() < 0.001
        && local.M21.abs() < 0.001
        && (local.M22 - 1.0).abs() < 0.001
        && local.M31.abs() < 0.01
        && local.M32.abs() < 0.01;
    if opacity >= 0.999 && identity {
        return paint();
    }
    let layer = unsafe { target.CreateLayer(None)? };
    let size = unsafe { target.GetSize() };
    let params = D2D1_LAYER_PARAMETERS {
        contentBounds: D2D_RECT_F {
            left: -size.width,
            top: -size.height,
            right: size.width * 2.0,
            bottom: size.height * 2.0,
        },
        geometricMask: std::mem::ManuallyDrop::new(None),
        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        maskTransform: identity_transform(),
        opacity,
        opacityBrush: std::mem::ManuallyDrop::new(None),
        layerOptions: D2D1_LAYER_OPTIONS_NONE,
    };
    unsafe {
        target.SetTransform(&compose(local, previous));
        target.PushLayer(&params, &layer);
    }
    let result = paint();
    unsafe {
        target.PopLayer();
        target.SetTransform(&previous);
    }
    result
}

pub fn scale_about_center(scale: f32, width: f32, height: f32) -> Matrix3x2 {
    Matrix3x2 {
        M11: scale,
        M12: 0.0,
        M21: 0.0,
        M22: scale,
        M31: width * 0.5 * (1.0 - scale),
        M32: height * 0.5 * (1.0 - scale),
    }
}

pub fn compose(first: Matrix3x2, then: Matrix3x2) -> Matrix3x2 {
    Matrix3x2 {
        M11: first.M11 * then.M11 + first.M12 * then.M21,
        M12: first.M11 * then.M12 + first.M12 * then.M22,
        M21: first.M21 * then.M11 + first.M22 * then.M21,
        M22: first.M21 * then.M12 + first.M22 * then.M22,
        M31: first.M31 * then.M11 + first.M32 * then.M21 + then.M31,
        M32: first.M31 * then.M12 + first.M32 * then.M22 + then.M32,
    }
}

pub fn identity_transform() -> Matrix3x2 {
    Matrix3x2 {
        M11: 1.0,
        M12: 0.0,
        M21: 0.0,
        M22: 1.0,
        M31: 0.0,
        M32: 0.0,
    }
}

pub fn arm_timer(hwnd: HWND, id: usize) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    let clock = clock();
    {
        let mut slots = slot_lock(clock);
        if let Some(slot) = slots
            .iter()
            .find(|slot| slot.hwnd == hwnd.0 as isize && slot.id == id)
        {
            slot.pending.store(false, Ordering::Release);
        } else {
            slots.push(Slot {
                hwnd: hwnd.0 as isize,
                id,
                pending: AtomicBool::new(false),
            });
        }
    }
    clock.wake.notify_one();
    static START: Once = Once::new();
    START.call_once(|| {
        std::thread::Builder::new()
            .name("tinycast-vsync".into())
            .spawn(vsync_pump)
            .expect("vsync pump");
    });
    true
}

pub fn kill_timer(hwnd: HWND, id: usize) {
    let clock = clock();
    {
        let mut slots = slot_lock(clock);
        slots.retain(|slot| !(slot.hwnd == hwnd.0 as isize && slot.id == id));
    }
    unsafe {
        let _ = KillTimer(hwnd, id);
    }
}

fn vsync_pump() {
    let timer = unsafe {
        CreateWaitableTimerExW(
            None,
            None,
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ALL_ACCESS.0,
        )
        .or_else(|_| CreateWaitableTimerExW(None, None, Default::default(), TIMER_ALL_ACCESS.0))
        .ok()
    };
    let clock = clock();
    let mut slots = slot_lock(clock);
    loop {
        let batch: Vec<(isize, usize)> = slots
            .iter()
            .filter(|slot| !slot.pending.load(Ordering::Acquire))
            .map(|slot| (slot.hwnd, slot.id))
            .collect();
        if batch.is_empty() {
            slots = clock
                .wake
                .wait(slots)
                .unwrap_or_else(|err| err.into_inner());
            continue;
        }
        for slot in slots.iter() {
            if batch
                .iter()
                .any(|(hwnd, id)| slot.hwnd == *hwnd && slot.id == *id)
            {
                slot.pending.store(true, Ordering::Release);
            }
        }
        drop(slots);
        for (raw, id) in batch {
            let hwnd = HWND(raw as *mut core::ffi::c_void);
            if unsafe { PostMessageW(hwnd, WM_TIMER, WPARAM(id), LPARAM(0)) }.is_err() {
                kill_timer(hwnd, id);
            }
        }
        wait_one_frame(timer);
        slots = slot_lock(clock);
    }
}

fn wait_one_frame(timer: Option<windows::Win32::Foundation::HANDLE>) {
    let delay_100ns = next_vblank_delay_100ns().clamp(1_000, 40_000 * 10);
    let Some(timer) = timer else {
        std::thread::sleep(std::time::Duration::from_millis(8));
        return;
    };
    let due = -(delay_100ns as i64);
    unsafe {
        if SetWaitableTimer(timer, &due, 0, None, None, false).is_ok() {
            let _ = WaitForSingleObject(timer, 40);
        } else {
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
    }
}

/// Relative delay, in 100ns units, until the next DWM vblank.
fn next_vblank_delay_100ns() -> i64 {
    unsafe {
        let mut info: DWM_TIMING_INFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<DWM_TIMING_INFO>() as u32;
        if DwmGetCompositionTimingInfo(HWND::default(), &mut info).is_err() {
            return 8_000 * 10;
        }
        let period = info.qpcRefreshPeriod.max(1);
        let mut now = 0i64;
        if QueryPerformanceCounter(&mut now).is_err() {
            return 8_000 * 10;
        }
        let mut freq = 0i64;
        if QueryPerformanceFrequency(&mut freq).is_err() || freq <= 0 {
            return 8_000 * 10;
        }
        let now = now.max(0) as u64;
        let since = now.saturating_sub(info.qpcVBlank);
        let into = since % period;
        let mut remain = period - into;
        if remain < period / 8 {
            remain += period;
        }
        (remain as i128 * 10_000_000 / freq as i128).clamp(1, 40_000 * 10) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tinycast_pure::theme;

    #[test]
    fn present_from_hidden_reaches_rest() {
        let mut anim = OverlayAnim::default();
        anim.present(false);
        let start = anim.start;
        let tick = anim.tick_at(start + Duration::from_secs_f32(theme::duration::ENTER_SECS));
        assert!(!tick.running);
        assert!(!tick.hide);
        assert!(anim.visual().is_rest());
    }

    #[test]
    fn dismiss_from_rest_reaches_hidden() {
        let mut anim = OverlayAnim::default();
        anim.present(false);
        let t0 = anim.start;
        let _ = anim.tick_at(t0 + Duration::from_secs_f32(theme::duration::ENTER_SECS));
        anim.dismiss();
        let t1 = anim.start;
        let tick = anim.tick_at(t1 + Duration::from_secs_f32(theme::duration::EXIT_SECS));
        assert!(!tick.running);
        assert!(tick.hide);
        assert!(anim.visual().is_hidden());
    }

    #[test]
    fn reverse_exit_resumes_enter_from_live_visual() {
        let mut anim = OverlayAnim::default();
        anim.present(false);
        let t0 = anim.start;
        let _ = anim.tick_at(t0 + Duration::from_millis(40));
        let mid = anim.visual();
        anim.dismiss();
        let at_reverse = anim.visual();
        assert!((at_reverse.alpha - mid.alpha).abs() < 0.001);
        assert!(anim.is_hiding());
        anim.present(true);
        assert!(!anim.is_hiding());
        let live = anim.visual();
        assert!((live.alpha - mid.alpha).abs() < 0.05);
    }

    #[test]
    fn present_while_already_resting_does_not_replay() {
        let mut anim = OverlayAnim::default();
        anim.present(false);
        let t0 = anim.start;
        let _ = anim.tick_at(t0 + Duration::from_secs_f32(theme::duration::ENTER_SECS));
        anim.present(true);
        assert!(!anim.is_running());
        assert!(anim.visual().is_rest());
    }

    #[test]
    fn scale_about_center_is_identity_at_one() {
        let m = scale_about_center(1.0, 750.0, 475.0);
        assert_eq!(m.M11, 1.0);
        assert_eq!(m.M22, 1.0);
        assert_eq!(m.M31, 0.0);
        assert_eq!(m.M32, 0.0);
    }

    #[test]
    fn scale_about_center_translates_to_keep_midpoint() {
        let m = scale_about_center(0.5, 100.0, 40.0);
        assert_eq!(m.M11, 0.5);
        assert_eq!(m.M31, 25.0);
        assert_eq!(m.M32, 10.0);
    }
}
