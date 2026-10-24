//! The Slint HUD: a non-activating, click-through capsule that replaces the
//! old layered Direct2D `MessageHud`.
//!
//! The window is born off-screen like the settings window (winit only creates
//! the HWND once the event loop runs), then hidden and reused for every
//! message. `WS_EX_NOACTIVATE | WS_EX_TRANSPARENT` keep it from taking focus
//! and let clicks fall through; `WS_EX_TRANSPARENT` is the one hit-test
//! transparency this UI is allowed, and there is no `WS_EX_LAYERED` — the
//! surface is opaque. Showing re-applies winit's own extended style, so the
//! bits are corrected after every `show`.

use std::cell::Cell;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer, Weak};
use tinycast_pure::dialog::DialogTone;
use tinycast_pure::theme;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE,
    SWP_NOCOPYBITS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use crate::design_system::appearance;
use crate::platform::screens::{dip_scalar_to_px, screens_px};

type Surface = tinycast_ui::HudWindow;

/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be styled, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's skip_taskbar leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;

pub struct MessageHud {
    ui: Surface,
    host: HWND,
    visible: Cell<bool>,
    appearance: Cell<u8>,
    /// The dwell countdown: one restart per present, fired once.
    hide_timer: Timer,
    /// The exit fade; a present during the fade cancels it instead.
    exit_timer: Timer,
}

impl MessageHud {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        let ui = Surface::new().map_err(crate::ui_host::platform_error)?;
        tinycast_ui::apply_appearance(&ui, appearance::resolved());
        // No HWND exists until the event loop runs; park the window off-screen.
        ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        ui.show().map_err(crate::ui_host::platform_error)?;
        let hud = Self {
            ui,
            host,
            visible: Cell::new(false),
            appearance: Cell::new(u8::MAX),
            hide_timer: Timer::default(),
            exit_timer: Timer::default(),
        };
        let weak = hud.ui.as_weak();
        arm_init(weak);
        Ok(hud)
    }

    pub fn show(&self, message: &str) {
        self.show_message(message, DialogTone::Success);
    }

    pub fn show_message(&self, message: &str, tone: DialogTone) {
        self.apply_appearance();
        self.ui.set_message(message.into());
        self.ui.set_symbol(tone_symbol(tone).into());
        self.ui.set_volume(false);
        self.present(false);
    }

    pub fn show_volume(&self, level: f32, muted: bool) {
        self.apply_appearance();
        self.ui.set_volume(true);
        self.ui.set_level(tinycast_pure::volume::clamped(level));
        self.ui.set_muted(muted);
        self.ui
            .set_volume_readout(crate::design_system::hud::volume_readout(level, muted).into());
        self.ui.set_volume_symbol(
            if muted {
                "speaker.slash.fill"
            } else {
                "speaker.wave.2"
            }
            .into(),
        );
        self.present(true);
    }

    /// Locale only decides the copy the callers hand in; the surface just
    /// follows the appearance, which is refreshed here.
    pub fn set_locale(&self, _locale: &str) {
        self.apply_appearance();
    }

    /// Shows the already-configured surface. From hidden the entry animates;
    /// while visible (including mid-exit) the content snaps, like the old
    /// `snap_shown`, and a running exit fades back in.
    fn present(&self, volume: bool) {
        self.exit_timer.stop();
        self.ui.set_exit_fade(1.0);
        if !self.visible.get() {
            self.ui.set_presented(true);
        }
        if self.ui.show().is_err() {
            return;
        }
        if let Some(hwnd) = window_hwnd(&self.ui) {
            apply_hud_style(hwnd);
            self.place(hwnd, volume);
        }
        let dwell = Duration::from_secs_f32(if volume {
            theme::duration::VOLUME_HUD_SECS
        } else {
            theme::duration::MESSAGE_HUD_SECS
        });
        self.hide_timer.stop();
        let weak = self.ui.as_weak();
        self.hide_timer
            .start(slint::TimerMode::SingleShot, dwell, move || {
                begin_exit(&weak)
            });
        self.visible.set(true);
    }

    fn apply_appearance(&self) {
        let appearance = crate::ui_host::app_core(self.host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or_else(appearance::resolved);
        if self.appearance.get() != appearance {
            self.appearance.set(appearance);
            tinycast_ui::apply_appearance(&self.ui, appearance);
        }
    }

    /// Message: centred on the primary work area, `HUD_EDGE_OFFSET` above its
    /// bottom edge. Volume: centred, 12% of the work area down. Both match the
    /// old `position()`; the size is the surface's own measurement.
    fn place(&self, hwnd: HWND, volume: bool) {
        let dip_w = self.ui.get_card_width();
        let dip_h = self.ui.get_card_height();
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let dpi = if dpi == 0 { 96 } else { dpi };
        let w = dip_scalar_to_px(dip_w, dpi);
        let h = dip_scalar_to_px(dip_h, dpi);
        let screens = screens_px();
        let Some(s) = screens.first() else {
            return;
        };
        let work_w = s.work.right - s.work.left;
        let work_h = s.work.bottom - s.work.top;
        let x = s.work.left + (work_w - w) / 2;
        let y = if volume {
            s.work.top + (work_h as f32 * 0.12).round() as i32
        } else {
            s.work.bottom - h - dip_scalar_to_px(theme::size::HUD_EDGE_OFFSET, dpi)
        };
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_NOCOPYBITS,
            );
        }
    }
}

/// The dwell ran out: fade out, then hide. `presented` only drops once the
/// window is hidden, so the reveal snaps back invisibly and the next entry
/// animates from the start again.
fn begin_exit(weak: &Weak<Surface>) {
    let Some(ui) = weak.upgrade() else {
        return;
    };
    ui.set_exit_fade(0.0);
    let weak = ui.as_weak();
    let millis = (theme::duration::EXIT_SECS * 1000.0).round().max(0.0) as u64;
    Timer::single_shot(Duration::from_millis(millis), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let _ = ui.hide();
        ui.set_presented(false);
    });
}

impl Drop for MessageHud {
    fn drop(&mut self) {
        self.hide_timer.stop();
        self.exit_timer.stop();
        let _ = self.ui.hide();
    }
}

/// Polls for the HWND winit creates on the first event-loop turn, styles it,
/// then hides the parked window until the first real message.
fn arm_init(weak: Weak<Surface>) {
    Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(hwnd) = window_hwnd(&ui) else {
            let _ = ui.hide();
            arm_init(ui.as_weak());
            return;
        };
        apply_hud_style(hwnd);
        let _ = ui.hide();
    });
}

/// The style winit cannot express: tool window on top, never activated, and
/// hit-test transparent so clicks fall through to the window underneath.
fn apply_hud_style(hwnd: HWND) {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (before
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TRANSPARENT.0 as isize)
            & !WS_EX_APPWINDOW_BIT;
        if next != before {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
    }
}

fn window_hwnd(ui: &Surface) -> Option<HWND> {
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::Win32(win32) = handle.as_raw() else {
                return None;
            };
            let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
            crate::ui_host::clear_backdrop(hwnd);
            Some(hwnd)
        })
        .flatten()
}

fn tone_symbol(tone: DialogTone) -> &'static str {
    match tone {
        DialogTone::Neutral => "info.circle.fill",
        DialogTone::Success => "checkmark.circle.fill",
        DialogTone::Danger => "exclamationmark.circle.fill",
    }
}
