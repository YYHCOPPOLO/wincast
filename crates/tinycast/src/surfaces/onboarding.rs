//! First-launch window: record `togglePalette` with the Settings recorder.

use std::time::{SystemTime, UNIX_EPOCH};

use tinycast_pure::hotkey::{CaptureOutcome, Modifiers};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, IsWindow,
    LoadCursorW, RegisterClassW, SetWindowLongPtrW, ShowWindow, CREATESTRUCTW, CS_HREDRAW,
    CS_VREDRAW, GWLP_USERDATA, GWLP_WNDPROC, IDC_ARROW, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP,
    WM_LBUTTONDOWN, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_OVERLAPPED, WS_SYSMENU,
};

use crate::app_core::AppCore;
use crate::design_system::host::OverlayPainter;
use crate::design_system::panel::paint_sheen;
use crate::design_system::squircle::fill_squircle;
use crate::design_system::text;
use crate::features::hotkeys::ui::recorder::{self, Recorder};

const CLASS: windows::core::PCWSTR = w!("TinycastOnboarding");
const ID_CONTINUE: usize = 1;
const ID_RECORD: usize = 2;
pub const WINDOW: (f32, f32) = (520.0, 400.0);

const STEPS: [&str; 4] = [
    "Welcome to Tinycast",
    "Enable Pasting",
    "Import from Raycast",
    "You're all set",
];

const SUBTITLES: [&str; 4] = [
    "Set a shortcut to summon the launcher from anywhere.",
    "Let Tinycast paste items back into the app you were using.",
    "Bring your shortcuts, favorites, and clipboard history along.",
    "Tinycast is ready. Press your shortcut anytime to start.",
];

pub struct OnboardingWindow {
    pub hwnd: HWND,
}

struct Inner {
    host: HWND,
    recorder: Recorder,
    record_btn: HWND,
    record_prev: Option<windows::Win32::UI::WindowsAndMessaging::WNDPROC>,
    step: usize,
    painter: Option<OverlayPainter>,
    continue_rect: DipRect,
    record_rect: DipRect,
}

impl OnboardingWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS,
                w!("Welcome to Tinycast"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                220,
                160,
                WINDOW.0.round() as i32,
                WINDOW.1.round() as i32,
                host,
                None,
                hinstance,
                Some(host.0 as *const core::ffi::c_void),
            )?;
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(self.hwnd).as_bool() }
    }
}

impl Drop for OnboardingWindow {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
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

unsafe fn core_from_host(host: HWND) -> Option<*mut AppCore> {
    let ptr = GetWindowLongPtrW(host, GWLP_USERDATA) as *mut AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn current_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            win: GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0,
        }
    }
}

fn empty_rect() -> DipRect {
    DipRect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    }
}

fn contains(rect: DipRect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
}

fn client_dip(hwnd: HWND, lparam: LPARAM) -> (f32, f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    let x = (lparam.0 as u32 & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    let y = ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    (x, y)
}

unsafe fn apply_capture(hwnd: HWND, outcome: CaptureOutcome) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    match outcome {
        CaptureOutcome::Ignore => {}
        CaptureOutcome::Cancel | CaptureOutcome::Clear => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).resume_global_hotkeys();
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                hwnd,
                None,
                windows::Win32::Foundation::FALSE,
            );
        }
        CaptureOutcome::Commit(binding) => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).set_hotkey("hotkey.togglePalette", Some(binding));
                (*core).resume_global_hotkeys();
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                hwnd,
                None,
                windows::Win32::Foundation::FALSE,
            );
        }
    }
}

fn continue_clicked(hwnd: HWND) {
    unsafe {
        let Some(inner) = inner_from(hwnd) else {
            return;
        };
        if (*inner).step + 1 < STEPS.len() {
            (*inner).step += 1;
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                hwnd,
                None,
                windows::Win32::Foundation::FALSE,
            );
            return;
        }
        if let Some(core) = core_from_host((*inner).host) {
            (*core).finish_onboarding();
        }
    }
}

fn record_clicked(hwnd: HWND) {
    unsafe {
        if let Some(inner) = inner_from(hwnd) {
            (*inner).recorder.begin("hotkey.togglePalette".into());
            if let Some(core) = core_from_host((*inner).host) {
                (*core).pause_global_hotkeys();
            }
            let _ = SetFocus(hwnd);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                hwnd,
                None,
                windows::Win32::Foundation::FALSE,
            );
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let record_btn = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Record shortcut"),
                WINDOW_STYLE(WS_CHILD.0),
                40,
                140,
                160,
                32,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_RECORD as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let continue_btn = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Continue"),
                WINDOW_STYLE(WS_CHILD.0 | 1),
                220,
                140,
                120,
                32,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CONTINUE as *mut core::ffi::c_void),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let _ = ShowWindow(record_btn, SW_HIDE);
            let _ = ShowWindow(continue_btn, SW_HIDE);
            let inner = Box::new(Inner {
                host: HWND(cs.lpCreateParams),
                recorder: Recorder::new(),
                record_btn,
                record_prev: None,
                step: 0,
                painter: OverlayPainter::new().ok(),
                continue_rect: empty_rect(),
                record_rect: empty_rect(),
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
            if let Some(inner) = inner_from(hwnd) {
                let prev = SetWindowLongPtrW(
                    (*inner).record_btn,
                    GWLP_WNDPROC,
                    record_subclass as usize as isize,
                );
                (*inner).record_prev = Some(std::mem::transmute(prev));
            }
            LRESULT(1)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = client_dip(hwnd, lparam);
            if let Some(inner) = inner_from(hwnd) {
                if contains((*inner).continue_rect, x, y) {
                    continue_clicked(hwnd);
                } else if (*inner).step == 0 && contains((*inner).record_rect, x, y) {
                    record_clicked(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            if id == ID_RECORD {
                record_clicked(hwnd);
            }
            if id == ID_CONTINUE {
                continue_clicked(hwnd);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if inner_from(hwnd)
                .map(|i| (*i).recorder.is_recording())
                .unwrap_or(false)
            {
                if let Some(inner) = inner_from(hwnd) {
                    let outcome =
                        (*inner)
                            .recorder
                            .on_keydown(wparam.0 as u16, current_modifiers(), now_ms());
                    apply_capture(hwnd, outcome);
                }
            }
            LRESULT(0)
        }
        WM_KEYUP => {
            if inner_from(hwnd)
                .map(|i| (*i).recorder.is_recording())
                .unwrap_or(false)
            {
                if let Some(inner) = inner_from(hwnd) {
                    let outcome = (*inner).recorder.on_keyup(wparam.0 as u16, now_ms());
                    apply_capture(hwnd, outcome);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(core) = inner_from(hwnd).and_then(|i| core_from_host((*i).host)) {
                (*core).finish_onboarding();
            }
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                drop(Box::from_raw(ptr as *mut Inner));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return;
        }
        let Some(inner) = inner_from(hwnd) else {
            let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            return;
        };
        let step = (*inner).step.min(3);
        let recording = (*inner).recorder.is_recording();
        let mut continue_rect = empty_rect();
        let mut record_rect = empty_rect();
        if let Some(painter) = (*inner).painter.as_mut() {
            let _ = painter.paint(hwnd, |target, fonts| {
                let size = target.GetSize();
                fill_squircle(
                    target,
                    DipRect {
                        x: 0.0,
                        y: 0.0,
                        w: size.width,
                        h: size.height,
                    },
                    theme::radius::PANEL,
                    theme::colors::scrim_rgba(0),
                )?;
                paint_sheen(target, size.width, size.height, 0)?;
                let pad = theme::spacing::XXL;
                let hero = 60.0;
                let hero_rect = DipRect {
                    x: (size.width - hero) / 2.0,
                    y: pad,
                    w: hero,
                    h: hero,
                };
                fill_squircle(target, hero_rect, 16.0, text::control_surface(0))?;
                let glyph = match step {
                    1 => "lock.shield",
                    2 => "wand.and.sparkles",
                    3 => "checkmark",
                    _ => "magnifyingglass",
                };
                crate::design_system::symbols::paint_fluent_in(
                    target,
                    &fonts.dwrite,
                    glyph,
                    DipRect {
                        x: hero_rect.x + 14.0,
                        y: hero_rect.y + 14.0,
                        w: 32.0,
                        h: 32.0,
                    },
                    text::primary_ink(0),
                )?;
                text::draw(
                    target,
                    &fonts.headline_center,
                    STEPS[step],
                    DipRect {
                        x: pad,
                        y: pad + hero + theme::spacing::MD,
                        w: size.width - pad * 2.0,
                        h: 24.0,
                    },
                    text::primary_ink(0),
                )?;
                text::draw(
                    target,
                    &fonts.wrap_callout,
                    SUBTITLES[step],
                    DipRect {
                        x: pad,
                        y: pad + hero + 36.0,
                        w: size.width - pad * 2.0,
                        h: 48.0,
                    },
                    text::secondary_ink(0),
                )?;
                if step == 0 {
                    let well_w = theme::size::SHORTCUT_RECORDER;
                    record_rect = DipRect {
                        x: (size.width - well_w) / 2.0,
                        y: 210.0,
                        w: well_w,
                        h: 28.0,
                    };
                    fill_squircle(
                        target,
                        record_rect,
                        theme::radius::MENU,
                        text::control_surface(0),
                    )?;
                    let caption = recorder::well_caption(None, recording);
                    text::draw(
                        target,
                        &fonts.bar,
                        &caption,
                        record_rect,
                        text::secondary_ink(0),
                    )?;
                }
                let btn_h = theme::size::MENU_BUTTON;
                continue_rect = DipRect {
                    x: (size.width - 140.0) / 2.0,
                    y: size.height - pad - btn_h,
                    w: 140.0,
                    h: btn_h,
                };
                fill_squircle(
                    target,
                    continue_rect,
                    btn_h / 2.0,
                    text::control_surface(0),
                )?;
                let label = if step + 1 == STEPS.len() {
                    "Get Started"
                } else {
                    "Continue"
                };
                text::draw(
                    target,
                    &fonts.bar,
                    label,
                    continue_rect,
                    text::primary_ink(0),
                )?;
                let mut dot_x = size.width / 2.0 - 18.0;
                let dot_y = continue_rect.y - 18.0;
                for i in 0..STEPS.len() {
                    let on = i == step;
                    fill_squircle(
                        target,
                        DipRect {
                            x: dot_x,
                            y: dot_y,
                            w: 7.0,
                            h: 7.0,
                        },
                        3.5,
                        if on {
                            text::primary_ink(0)
                        } else {
                            text::tertiary_ink(0)
                        },
                    )?;
                    dot_x += 12.0;
                }
                Ok(())
            });
            (*inner).continue_rect = continue_rect;
            (*inner).record_rect = record_rect;
        }
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn record_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd).unwrap_or_default();
    if msg == WM_KEYDOWN || msg == WM_KEYUP {
        return wndproc(parent, msg, wparam, lparam);
    }
    let prev = inner_from(parent).and_then(|i| (*i).record_prev);
    if let Some(prev) = prev {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_default_summon_is_alt_space() {
        let b = tinycast_pure::hotkey::default_toggle_palette();
        match b {
            tinycast_pure::hotkey::HotKeyBinding::Combo(s) => {
                assert_eq!(s.vk, 0x20);
                assert!(s.modifiers.alt);
            }
            _ => panic!("expected combo"),
        }
    }

    #[test]
    fn recorder_is_used_for_onboarding_capture() {
        let mut r = Recorder::new();
        assert!(!r.is_recording());
        r.begin("hotkey.togglePalette".into());
        assert!(r.is_recording());
        let _ = now_ms();
        let _ = current_modifiers();
    }

    #[test]
    fn onboarding_size_is_520x400() {
        assert_eq!(crate::surfaces::onboarding::WINDOW, (520.0, 400.0));
    }

    #[test]
    fn onboarding_has_no_textout() {
        let src = include_str!("onboarding.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
