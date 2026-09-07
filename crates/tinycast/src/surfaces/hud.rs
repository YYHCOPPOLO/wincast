//! Shared non-activating click-through message HUD.

use tinycast_pure::dialog::DialogTone;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, IsWindow, KillTimer,
    LoadCursorW, RegisterClassW, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_HREDRAW,
    CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE,
    WM_DESTROY, WM_ERASEBKGND, WM_NCDESTROY, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::design_system::host::OverlayPainter;
use crate::design_system::hud::{self as ds, HudPaint};
use crate::platform::screens::{dip_scalar_to_px, screens_px};

const CLASS: windows::core::PCWSTR = w!("TinycastHud");
const HIDE_TIMER: usize = 1;

pub struct MessageHud {
    hwnd: HWND,
}

struct Inner {
    painter: OverlayPainter,
    kind: Kind,
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
                painter: OverlayPainter::new()?,
                kind: Kind::Message {
                    text: String::new(),
                    tone: DialogTone::Neutral,
                },
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
            (*inner).kind = kind;
            let volume = matches!((*inner).kind, Kind::Volume { .. });
            let (w, h) = size_of(&*inner);
            position(self.hwnd, w, h, volume);
            paint_d2d(self.hwnd);
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
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
                let _ = ShowWindow(hwnd, SW_HIDE);
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
        WM_DESTROY => DefWindowProcW(hwnd, msg, wparam, lparam),
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

fn paint_d2d(hwnd: HWND) {
    unsafe {
        let Some(inner) = inner_from(hwnd) else {
            return;
        };
        let inner = &mut *inner;
        let appearance = 0u8;
        match inner.kind {
            Kind::Message { ref text, tone } => {
                let text = text.clone();
                let _ = inner.painter.paint(hwnd, |target, fonts| {
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
            Kind::Volume { level, muted } => {
                let _ = inner.painter.paint(hwnd, |target, fonts| {
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
