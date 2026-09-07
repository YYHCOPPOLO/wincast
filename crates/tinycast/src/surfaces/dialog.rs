//! Tinycast-owned Continue/Cancel dialog. One at a time; Escape cancels.

use std::sync::atomic::{AtomicBool, Ordering};

use tinycast_pure::dialog::{DialogRole, DialogTone};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, IsWindow, LoadCursorW, PeekMessageW, RegisterClassW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST,
    IDC_ARROW, MSG, PM_REMOVE, SW_SHOW, SWP_NOACTIVATE, WM_ACTIVATE, WM_CLOSE, WM_DESTROY,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY, WM_PAINT,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::design_system::dialog::{self as ds, DialogContent, DialogHits};
use crate::design_system::host::OverlayPainter;
use crate::platform::screens::dip_scalar_to_px;

const CLASS: windows::core::PCWSTR = w!("TinycastDialog");

static DIALOG_UP: AtomicBool = AtomicBool::new(false);
static LAST_VOLUME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static LAST_RESULT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmPrompt {
    pub title: String,
    pub message: String,
    pub accept: String,
    pub cancel: String,
}

pub fn begin() -> bool {
    DIALOG_UP
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn end() {
    DIALOG_UP.store(false, Ordering::SeqCst);
}

pub fn is_up() -> bool {
    DIALOG_UP.load(Ordering::SeqCst)
}

/// Continue / Cancel. Escape and clicking outside the card cancel.
pub fn confirm(prompt: &ConfirmPrompt) -> bool {
    run(prompt, true)
}

/// Set Volume slider: ←/→ walk the 5% grid, Enter accepts, Escape cancels.
pub fn pick_volume(current: f32) -> Option<f32> {
    if !begin() {
        return None;
    }
    LAST_VOLUME.store(current.to_bits(), Ordering::SeqCst);
    let prompt = ConfirmPrompt {
        title: tinycast_pure::i18n::dialog_set_volume(tinycast_pure::i18n::UiLang::default()).into(),
        message: tinycast_pure::i18n::dialog_set_volume_message(tinycast_pure::i18n::UiLang::default())
            .into(),
        accept: tinycast_pure::i18n::dialog_set_volume(tinycast_pure::i18n::UiLang::default()).into(),
        cancel: tinycast_pure::i18n::chrome(
            tinycast_pure::i18n::Chrome::Cancel,
            tinycast_pure::i18n::UiLang::default(),
        )
        .into(),
    };
    let accepted = run_volume(&prompt, current);
    end();
    if accepted {
        Some(f32::from_bits(LAST_VOLUME.load(Ordering::SeqCst)))
    } else {
        None
    }
}

/// Single Continue button for failure reports.
pub fn alert(title: &str, message: &str) {
    let prompt = ConfirmPrompt {
        title: title.to_string(),
        message: message.to_string(),
        accept: tinycast_pure::i18n::onboarding_continue(tinycast_pure::i18n::UiLang::default())
            .into(),
        cancel: String::new(),
    };
    if !begin() {
        return;
    }
    let _ = run(&prompt, false);
    end();
}

fn run(prompt: &ConfirmPrompt, has_cancel: bool) -> bool {
    let hwnd = match create_window(prompt, has_cancel, None) {
        Ok(h) => h,
        Err(_) => return false,
    };
    pump(hwnd)
}

fn run_volume(prompt: &ConfirmPrompt, current: f32) -> bool {
    let hwnd = match create_window(prompt, true, Some(current)) {
        Ok(h) => h,
        Err(_) => return false,
    };
    pump(hwnd)
}

fn pump(hwnd: HWND) -> bool {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
        arm(hwnd);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() {
            let ok = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if !ok.as_bool() {
                break;
            }
            if msg.message == WM_KEYDOWN {
                let vk = msg.wParam.0 as u16;
                if vk == 0x1B {
                    set_result(hwnd, false);
                    let _ = DestroyWindow(hwnd);
                    continue;
                }
                if vk == 0x0D {
                    set_result(hwnd, true);
                    let _ = DestroyWindow(hwnd);
                    continue;
                }
                if vk == 0x25 || vk == 0x27 {
                    nudge_volume(hwnd, vk == 0x27);
                    continue;
                }
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_PAINT || msg.message == 0x0012 {
                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        take_result()
    }
}

struct Inner {
    prompt: ConfirmPrompt,
    has_cancel: bool,
    result: bool,
    chosen: bool,
    armed: bool,
    continue_rect: DipRect,
    cancel_rect: Option<DipRect>,
    volume_rect: Option<DipRect>,
    volume: Option<f32>,
    dragging: bool,
    painter: OverlayPainter,
}

fn create_window(
    prompt: &ConfirmPrompt,
    has_cancel: bool,
    volume: Option<f32>,
) -> windows::core::Result<HWND> {
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
        let painter = OverlayPainter::new()?;
        let content = content_of(prompt, has_cancel, volume);
        let height = ds::measure_height(painter.fonts(), &content);
        let inner = Box::new(Inner {
            prompt: prompt.clone(),
            has_cancel,
            result: false,
            chosen: false,
            armed: false,
            continue_rect: DipRect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            cancel_rect: None,
            volume_rect: None,
            volume,
            dragging: false,
            painter,
        });
        let ptr = Box::into_raw(inner);
        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED,
            CLASS,
            w!("Tinycast"),
            WS_POPUP,
            0,
            0,
            420,
            height.round() as i32,
            HWND::default(),
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
        position(hwnd, height);
        paint_d2d(hwnd);
        Ok(hwnd)
    }
}

fn content_of<'a>(
    prompt: &'a ConfirmPrompt,
    has_cancel: bool,
    volume: Option<f32>,
) -> DialogContent<'a> {
    DialogContent {
        title: &prompt.title,
        message: &prompt.message,
        accept: &prompt.accept,
        cancel: if has_cancel && !prompt.cancel.is_empty() {
            Some(prompt.cancel.as_str())
        } else {
            None
        },
        volume,
        appearance: 0,
        tone: DialogTone::Neutral,
        symbol: if volume.is_some() {
            "speaker.wave.2"
        } else {
            "info.circle"
        },
        accept_role: if prompt.accept == "Uninstall" {
            DialogRole::Destructive
        } else {
            DialogRole::Standard
        },
    }
}

fn position(hwnd: HWND, height_dip: f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let w = dip_scalar_to_px(theme::size::DIALOG_WIDTH, dpi);
    let h = dip_scalar_to_px(height_dip, dpi);
    let screens = crate::platform::screens::screens_px();
    let screen = screens
        .iter()
        .find(|s| s.origin_is_primary)
        .or(screens.first());
    let (x, y) = if let Some(s) = screen {
        let sw = s.work.right - s.work.left;
        let sh = s.work.bottom - s.work.top;
        (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 3)
    } else {
        (100, 100)
    };
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    }
}

fn nudge_volume(hwnd: HWND, up: bool) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        if let Some(level) = (*ptr).volume {
            let next = tinycast_pure::volume::volume_step(level, up);
            (*ptr).volume = Some(next);
            LAST_VOLUME.store(next.to_bits(), Ordering::SeqCst);
            paint_d2d(hwnd);
        }
    }
}

fn set_volume_from_x(hwnd: HWND, x_dip: f32) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        let Some(rect) = (*ptr).volume_rect else {
            return;
        };
        let next = ds::volume_level_at(rect, x_dip);
        (*ptr).volume = Some(next);
        LAST_VOLUME.store(next.to_bits(), Ordering::SeqCst);
        paint_d2d(hwnd);
    }
}

fn set_result(hwnd: HWND, value: bool) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() || (*ptr).chosen {
            return;
        }
        (*ptr).chosen = true;
        (*ptr).result = value;
        LAST_RESULT.store(if value { 1 } else { 0 }, Ordering::SeqCst);
        if let Some(level) = (*ptr).volume {
            LAST_VOLUME.store(level.to_bits(), Ordering::SeqCst);
        }
    }
}

fn take_result() -> bool {
    LAST_RESULT.swap(0, Ordering::SeqCst) == 1
}

fn arm(hwnd: HWND) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if !ptr.is_null() {
            (*ptr).armed = true;
        }
    }
}

fn client_dip(hwnd: HWND, lparam: LPARAM) -> (f32, f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    let x = (lparam.0 as u32 & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    let y = ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    (x, y)
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
        WM_LBUTTONDOWN => {
            hit(hwnd, lparam);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            unsafe_drag(hwnd, lparam);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            unsafe {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
                if !ptr.is_null() && (*ptr).dragging {
                    (*ptr).dragging = false;
                    let _ = ReleaseCapture();
                }
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 as u16) == 0 {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
                if !ptr.is_null() && (*ptr).armed && !(*ptr).chosen {
                    set_result(hwnd, false);
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            if vk == 0x1B {
                set_result(hwnd, false);
                let _ = DestroyWindow(hwnd);
            } else if vk == 0x0D {
                set_result(hwnd, true);
                let _ = DestroyWindow(hwnd);
            } else if vk == 0x25 || vk == 0x27 {
                nudge_volume(hwnd, vk == 0x27);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            set_result(hwnd, false);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ptr.is_null() {
                LAST_RESULT.store(if (*ptr).result { 1 } else { 0 }, Ordering::SeqCst);
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn unsafe_drag(hwnd: HWND, lparam: LPARAM) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() || !(*ptr).dragging {
            return;
        }
        let (x, _) = client_dip(hwnd, lparam);
        set_volume_from_x(hwnd, x);
    }
}

fn paint_d2d(hwnd: HWND) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        let inner = &mut *ptr;
        let prompt = inner.prompt.clone();
        let has_cancel = inner.has_cancel;
        let volume = inner.volume;
        let content = content_of(&prompt, has_cancel, volume);
        let mut hits: Option<DialogHits> = None;
        let _ = inner.painter.paint(hwnd, |target, fonts| {
            hits = Some(ds::paint(target, fonts, &content)?);
            Ok(())
        });
        if let Some(h) = hits {
            inner.continue_rect = h.accept;
            inner.cancel_rect = h.cancel;
            inner.volume_rect = h.volume;
        }
    }
}

fn hit(hwnd: HWND, lparam: LPARAM) {
    let (x, y) = client_dip(hwnd, lparam);
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        let inner = &*ptr;
        if ds::contains(inner.continue_rect, x, y) {
            set_result(hwnd, true);
            let _ = DestroyWindow(hwnd);
            return;
        }
        if let Some(cancel) = inner.cancel_rect {
            if ds::contains(cancel, x, y) {
                set_result(hwnd, false);
                let _ = DestroyWindow(hwnd);
                return;
            }
        }
        if let Some(volume) = inner.volume_rect {
            if ds::contains(volume, x, y) {
                (*ptr).dragging = true;
                let _ = SetCapture(hwnd);
                set_volume_from_x(hwnd, x);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_dialog_is_refused_while_one_is_up() {
        assert!(begin());
        assert!(is_up());
        assert!(!begin());
        end();
        assert!(!is_up());
        assert!(begin());
        end();
    }

    #[test]
    fn dialog_width_is_420() {
        assert_eq!(tinycast_pure::layout::dialog::frame().0, 420.0);
        assert!(tinycast_pure::layout::dialog::cancel_is_leading());
    }

    #[test]
    fn dialog_paint_is_not_gdi_marker() {
        let src = include_str!("dialog.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(
            !impl_src.contains(&marker),
            "dialog must not paint with GDI {marker}"
        );
    }
}
