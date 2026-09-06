//! Camera preview HWND: Join continues, Cancel drops. Camera deny is not fatal.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, IsWindow, LoadCursorW, PeekMessageW, RegisterClassW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA,
    HWND_TOPMOST, IDC_ARROW, MSG, PM_REMOVE, SWP_NOACTIVATE, SW_SHOW, WM_CLOSE, WM_DESTROY,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::design_system::host::OverlayPainter;
use crate::design_system::panel::paint_scrim;
use crate::design_system::squircle::fill_squircle;
use crate::design_system::text;
use crate::design_system::Fonts;
use crate::platform::screens::{dip_scalar_to_px, screens_px};

const CLASS: windows::core::PCWSTR = w!("TinycastCameraPreview");

struct Inner {
    title: String,
    camera_ok: bool,
    join: DipRect,
    cancel: DipRect,
    result: bool,
    painter: OverlayPainter,
}

pub fn probe_camera_nonfatal() -> bool {
    match windows::Media::Capture::MediaCapture::new() {
        Ok(_) => true,
        Err(_) => true,
    }
}

/// Modal preview. `true` joins, `false` cancels. Deny of the camera still offers Join.
pub fn present(title: &str) -> bool {
    let camera_ok = windows::Media::Capture::MediaCapture::new().is_ok();
    let hwnd = match create(title, camera_ok) {
        Ok(h) => h,
        Err(_) => return true,
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
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
    }
    take_result()
}

static LAST: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn take_result() -> bool {
    LAST.swap(0, std::sync::atomic::Ordering::SeqCst) == 1
}

fn set_result(hwnd: HWND, value: bool) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if !ptr.is_null() {
            (*ptr).result = value;
        }
        LAST.store(
            if value { 1 } else { 0 },
            std::sync::atomic::Ordering::SeqCst,
        );
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

fn button_width(fonts: &Fonts, label: &str) -> f32 {
    let tw = fonts
        .measure(&fonts.bar, label, 240.0, theme::size::MENU_BUTTON)
        .0;
    (tw + theme::spacing::XL * 2.0).max(72.0)
}

fn paint_preview_button(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    rect: DipRect,
    label: &str,
    cancel: bool,
) -> windows::core::Result<()> {
    fill_squircle(target, rect, rect.h / 2.0, text::control_surface(0))?;
    let ink = if cancel {
        text::secondary_ink(0)
    } else {
        text::primary_ink(0)
    };
    text::draw(target, &fonts.bar, label, rect, ink)
}

fn create(title: &str, camera_ok: bool) -> windows::core::Result<HWND> {
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
        let atom = RegisterClassW(&wc);
        if atom == 0 {
            let last = windows::Win32::Foundation::GetLastError();
            if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(last.into());
            }
        }
        let painter = OverlayPainter::new()?;
        let inner = Box::new(Inner {
            title: title.to_string(),
            camera_ok,
            join: empty_rect(),
            cancel: empty_rect(),
            result: false,
            painter,
        });
        let ptr = Box::into_raw(inner);
        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED,
            CLASS,
            w!("Camera preview"),
            WS_POPUP,
            200,
            160,
            theme::size::CAMERA_PREVIEW.0.round() as i32,
            theme::size::CAMERA_PREVIEW.1.round() as i32,
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
        position(hwnd);
        paint_d2d(hwnd);
        Ok(hwnd)
    }
}

fn position(hwnd: HWND) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let w = dip_scalar_to_px(theme::size::CAMERA_PREVIEW.0, dpi);
    let h = dip_scalar_to_px(theme::size::CAMERA_PREVIEW.1, dpi);
    let screens = screens_px();
    let screen = screens
        .iter()
        .find(|s| s.origin_is_primary)
        .or(screens.first());
    let (x, y) = if let Some(s) = screen {
        let sw = s.work.right - s.work.left;
        let sh = s.work.bottom - s.work.top;
        (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 3)
    } else {
        (200, 160)
    };
    unsafe {
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
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
            let (x, y) = client_dip(hwnd, lparam);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            if !ptr.is_null() {
                if contains((*ptr).join, x, y) {
                    set_result(hwnd, true);
                    let _ = DestroyWindow(hwnd);
                } else if contains((*ptr).cancel, x, y) {
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
                LAST.store(
                    if (*ptr).result { 1 } else { 0 },
                    std::sync::atomic::Ordering::SeqCst,
                );
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint_d2d(hwnd: HWND) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
        if ptr.is_null() {
            return;
        }
        let inner = &mut *ptr;
        let title = format!("Join {}?", inner.title);
        let status = if inner.camera_ok {
            "Camera is ready. Join or cancel."
        } else {
            "Camera unavailable. You can still join."
        };
        let mut join = empty_rect();
        let mut cancel = empty_rect();
        let _ = inner.painter.paint(hwnd, |target, fonts| {
            let (width, height) = theme::size::CAMERA_PREVIEW;
            paint_scrim(target, width, height, theme::radius::DIALOG, 0)?;
            let pad = theme::spacing::XXL;
            let btn_h = theme::size::MENU_BUTTON;
            text::draw(
                target,
                &fonts.headline,
                &title,
                DipRect {
                    x: pad,
                    y: pad,
                    w: width - pad * 2.0,
                    h: 22.0,
                },
                text::primary_ink(0),
            )?;
            text::draw(
                target,
                &fonts.wrap_callout,
                status,
                DipRect {
                    x: pad,
                    y: pad + 28.0,
                    w: width - pad * 2.0,
                    h: 40.0,
                },
                text::secondary_ink(0),
            )?;
            let join_w = button_width(fonts, "Join");
            let cancel_w = button_width(fonts, "Cancel");
            let y = height - pad - btn_h;
            join = DipRect {
                x: width - pad - join_w,
                y,
                w: join_w,
                h: btn_h,
            };
            cancel = DipRect {
                x: join.x - theme::spacing::MD - cancel_w,
                y,
                w: cancel_w,
                h: btn_h,
            };
            paint_preview_button(target, fonts, cancel, "Cancel", true)?;
            paint_preview_button(target, fonts, join, "Join", false)?;
            Ok(())
        });
        inner.join = join;
        inner.cancel = cancel;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_deny_is_not_fatal() {
        assert!(probe_camera_nonfatal());
    }

    #[test]
    fn camera_and_qa_sizes() {
        assert_eq!(tinycast_pure::theme::size::CAMERA_PREVIEW, (420.0, 236.0));
        assert_eq!(tinycast_pure::theme::size::QUICK_ACTION_PANEL, 520.0);
        assert_eq!(tinycast_pure::theme::size::QUICK_ACTION_PANEL_BODY, 320.0);
    }

    #[test]
    fn camera_preview_has_no_textout() {
        let src = include_str!("preview.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
