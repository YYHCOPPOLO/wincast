//! Shared non-activating click-through message HUD.

use tinycast_pure::dialog::DialogTone;
use tinycast_pure::motion::{Ease, ScalarMotion};
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, IsWindow, IsWindowVisible,
    KillTimer, LoadCursorW, RegisterClassW, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SW_HIDE,
    SW_SHOWNOACTIVATE, WM_DESTROY, WM_ERASEBKGND, WM_NCDESTROY, WM_PAINT, WM_TIMER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::design_system::host::OverlayPainter;
use crate::design_system::hud::{self as ds, HudPaint};
use crate::design_system::motion::{self, OverlayAnim};
use crate::platform::screens::{dip_scalar_to_px, screens_px};

const CLASS: windows::core::PCWSTR = w!("TinycastHud");
const HIDE_TIMER: usize = 1;
const ANIM_TIMER: usize = 2;

pub struct MessageHud {
    hwnd: HWND,
}

struct Inner {
    host: HWND,
    painter: OverlayPainter,
    kind: Kind,
    anim: OverlayAnim,
    level: Option<ScalarMotion>,
    level_start: std::time::Instant,
    display_level: f32,
}

enum Kind {
    Message { text: String, tone: DialogTone },
    Volume { level: f32, muted: bool },
}

impl MessageHud {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            let atom = RegisterClassW(&class);
            if atom == 0 {
                let last = windows::Win32::Foundation::GetLastError();
                if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                    return Err(last.into());
                }
            }
            let inner = Box::new(Inner {
                host,
                painter: OverlayPainter::new()?,
                kind: Kind::Message {
                    text: String::new(),
                    tone: DialogTone::Neutral,
                },
                anim: OverlayAnim::default(),
                level: None,
                level_start: std::time::Instant::now(),
                display_level: 0.0,
            });
            let ptr = Box::into_raw(inner);
            let hwnd = match CreateWindowExW(
                WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TRANSPARENT
                    | WS_EX_LAYERED,
                CLASS,
                w!(""),
                WS_POPUP,
                0,
                0,
                200,
                48,
                host,
                None,
                hinstance,
                Some(ptr as *const core::ffi::c_void),
            ) {
                Ok(h) => h,
                Err(err) => {
                    drop(Box::from_raw(ptr));
                    return Err(err);
                }
            };
            (*ptr).painter.prefer_layered(hwnd);
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self, message: &str) {
        self.show_message(message, DialogTone::Success);
    }

    pub fn show_message(&self, message: &str, tone: DialogTone) {
        self.present(Kind::Message {
            text: message.to_string(),
            tone,
        });
    }

    pub fn show_volume(&self, level: f32, muted: bool) {
        self.present(Kind::Volume { level, muted });
    }

    pub fn set_locale(&self, locale: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).painter.set_locale(locale);
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, false);
        }
    }

    fn present(&self, kind: Kind) {
        unsafe {
            let Some(inner) = inner_from(self.hwnd) else {
                return;
            };
            let visible = IsWindowVisible(self.hwnd).as_bool();
            let was_volume = matches!((*inner).kind, Kind::Volume { .. });
            let showing_volume = matches!(kind, Kind::Volume { .. });
            if let Kind::Volume { level, muted } = &kind {
                let to = if *muted {
                    0.0
                } else {
                    tinycast_pure::volume::clamped(*level)
                };
                let from = if was_volume && (visible || (*inner).anim.is_running()) {
                    display_level(&*inner)
                } else {
                    to
                };
                if (from - to).abs() < 0.004 {
                    (*inner).level = None;
                    (*inner).display_level = to;
                } else {
                    (*inner).level = Some(ScalarMotion {
                        from,
                        to,
                        duration_secs: theme::duration::EXIT_SECS,
                        ease: Ease::Out,
                    });
                    (*inner).level_start = std::time::Instant::now();
                    (*inner).display_level = from;
                }
            } else {
                (*inner).level = None;
            }
            (*inner).kind = kind;
            let volume = matches!((*inner).kind, Kind::Volume { .. });
            let (w, h) = size_of(&*inner);
            position(self.hwnd, w, h, volume);
            let continue_enter = visible
                && showing_volume
                && was_volume
                && (*inner).anim.is_running()
                && !(*inner).anim.is_hiding();
            if !continue_enter {
                if visible {
                    (*inner).anim.snap_shown();
                } else if showing_volume {
                    (*inner).anim.present(false);
                } else {
                    (*inner).anim.present_fade(false);
                }
            }
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            paint_d2d(self.hwnd);
            if !motion::arm_timer(self.hwnd, ANIM_TIMER) {
                (*inner).anim.snap_end();
                (*inner).level = None;
                paint_d2d(self.hwnd);
            }
            let _ = KillTimer(self.hwnd, HIDE_TIMER);
            let _ = SetTimer(self.hwnd, HIDE_TIMER, dwell_ms(volume), None);
        }
    }
}

impl Drop for MessageHud {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

fn dwell_ms(volume: bool) -> u32 {
    let secs = if volume {
        theme::duration::VOLUME_HUD_SECS
    } else {
        theme::duration::MESSAGE_HUD_SECS
    };
    (secs * 1000.0).round() as u32
}

fn size_of(inner: &Inner) -> (f32, f32) {
    match &inner.kind {
        Kind::Message { text, .. } => ds::message_size(inner.painter.fonts(), text),
        Kind::Volume { .. } => ds::volume_size(),
    }
}

fn position(hwnd: HWND, w_dip: f32, h_dip: f32, volume: bool) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let w = dip_scalar_to_px(w_dip, dpi);
    let h = dip_scalar_to_px(h_dip, dpi);
    let screens = screens_px();
    let (x, y) = if let Some(s) = screens.first() {
        let work = s.work;
        let work_h = work.bottom - work.top;
        let x = work.left + (work.right - work.left - w) / 2;
        let y = if volume {
            work.top + (work_h as f32 * 0.12).round() as i32
        } else {
            work.bottom - h - dip_scalar_to_px(theme::size::HUD_EDGE_OFFSET, dpi)
        };
        (x, y)
    } else {
        (80, 80)
    };
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    }
}

unsafe fn inner_from(hwnd: HWND) -> Option<*mut Inner> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        0x0081 => {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_TIMER => {
            if wparam.0 == HIDE_TIMER {
                let _ = KillTimer(hwnd, HIDE_TIMER);
                if let Some(inner) = inner_from(hwnd) {
                    (*inner).anim.dismiss();
                    if !motion::arm_timer(hwnd, ANIM_TIMER) {
                        (*inner).anim.snap_end();
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    } else {
                        paint_d2d(hwnd);
                    }
                } else {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            } else if wparam.0 == ANIM_TIMER {
                tick_hud(hwnd);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                paint_d2d(hwnd);
                let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            motion::kill_timer(hwnd, ANIM_TIMER);
            motion::kill_timer(hwnd, HIDE_TIMER);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn display_level(inner: &Inner) -> f32 {
    if let Some(motion) = inner.level {
        let (value, _) = motion.sample(inner.level_start.elapsed().as_secs_f32());
        value
    } else if let Kind::Volume { level, .. } = inner.kind {
        level
    } else {
        inner.display_level
    }
}

unsafe fn tick_hud(hwnd: HWND) {
    let _frame = motion::frame_guard(hwnd);
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let overlay = (*inner).anim.tick();
    if let Some(motion) = (*inner).level {
        let (value, done) = motion.sample((*inner).level_start.elapsed().as_secs_f32());
        (*inner).display_level = value;
        if done {
            (*inner).level = None;
        }
    }
    if overlay.hide {
        motion::kill_timer(hwnd, ANIM_TIMER);
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    }
    paint_d2d(hwnd);
    if !overlay.running && (*inner).level.is_none() {
        motion::kill_timer(hwnd, ANIM_TIMER);
    }
}

fn appearance_of(host: HWND) -> u8 {
    unsafe {
        let ptr = GetWindowLongPtrW(host, GWLP_USERDATA) as *mut crate::app_core::AppCore;
        if ptr.is_null() {
            crate::design_system::appearance::resolved()
        } else {
            (*ptr).appearance_key()
        }
    }
}

fn paint_d2d(hwnd: HWND) {
    unsafe {
        let Some(inner) = inner_from(hwnd) else {
            return;
        };
        let inner = &mut *inner;
        let appearance = appearance_of(inner.host);
        let visual = inner.anim.visual();
        match inner.kind {
            Kind::Message { ref text, tone } => {
                let text = text.clone();
                let _ = inner.painter.paint_visual(hwnd, visual, |target, fonts| {
                    ds::paint(
                        target,
                        fonts,
                        &HudPaint::Message {
                            text: &text,
                            tone,
                            appearance,
                        },
                    )
                });
            }
            Kind::Volume { muted, .. } => {
                let level = display_level(inner);
                let _ = inner.painter.paint_visual(hwnd, visual, |target, fonts| {
                    ds::paint(
                        target,
                        fonts,
                        &HudPaint::Volume {
                            level,
                            muted,
                            appearance,
                        },
                    )
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hud_file_has_no_textout() {
        let src = include_str!("hud.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }

    #[test]
    fn hud_timers_split() {
        assert_eq!(tinycast_pure::theme::duration::MESSAGE_HUD_SECS, 2.4);
        assert_eq!(tinycast_pure::theme::duration::VOLUME_HUD_SECS, 1.6);
    }
}
