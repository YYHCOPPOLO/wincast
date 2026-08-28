use tinycast_pure::palette_placement::{compact_size, expanded_size};
use tinycast_pure::palette_state::should_draw_placeholder;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, FALSE, HWND, LPARAM, LRESULT, TRUE, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_TRANSIENTWINDOW,
    DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetStockObject, InvalidateRect, SetBkMode, SetTextColor, HDC, NULL_BRUSH,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, IsWindow, IsWindowVisible,
    KillTimer, LoadCursorW, RegisterClassW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, EN_CHANGE, GWLP_USERDATA,
    HWND_TOPMOST, IDC_ARROW, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WA_INACTIVE, WM_ACTIVATE,
    WM_CHAR, WM_COMMAND, WM_CTLCOLOREDIT, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_HOTKEY,
    WM_KEYDOWN, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SIZE, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::d2d::{Renderer, LAYERED_SOURCE_CONSTANT_ALPHA};
use super::edit::SearchEdit;
use super::physical;
use crate::app_core::AppCore;
use crate::platform::hotkey::{self, TOGGLE_PALETTE_ID};
use crate::platform::screens::dip_scalar_to_px;

const CLASS: windows::core::PCWSTR = w!("TinycastPalette");
const ANIM_TIMER_ID: usize = 1;
const ANIM_TICK_MS: u32 = 16;
const ENTER_SCALE: f32 = 0.94;

pub struct PaletteWindow {
    pub hwnd: HWND,
}

struct PaletteInner {
    host: HWND,
    renderer: Renderer,
    edit: Option<SearchEdit>,
    rest_frame: physical::Rect,
    anim: Option<PaletteAnim>,
    present_alpha: u8,
}

#[derive(Clone, Copy)]
struct PaletteAnim {
    kind: PaletteAnimKind,
    start: std::time::Instant,
}

#[derive(Clone, Copy)]
enum PaletteAnimKind {
    Enter,
    Exit,
}

impl PaletteWindow {
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

            let inner = Box::new(PaletteInner {
                host,
                renderer: Renderer::new()?,
                edit: None,
                rest_frame: physical::Rect {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                },
                anim: None,
                present_alpha: LAYERED_SOURCE_CONSTANT_ALPHA,
            });
            let ptr = Box::into_raw(inner);
            let hwnd = match CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_LAYERED,
                CLASS,
                w!("Tinycast"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                host,
                None,
                hinstance,
                Some(ptr as *const core::ffi::c_void),
            ) {
                Ok(hwnd) => hwnd,
                Err(err) => {
                    drop(Box::from_raw(ptr));
                    return Err(err);
                }
            };
            let _ = apply_dwm(hwnd);
            // HWND Direct2D presents opaque and covers the DWM backdrop;
            // per-pixel alpha via UpdateLayeredWindow keeps the 0.40 scrim
            // and 26 DIP corners without a solid gray slab.
            (*ptr).renderer.set_layered(hwnd, true);
            match SearchEdit::create(hwnd, host) {
                Ok(edit) => (*ptr).edit = Some(edit),
                Err(err) => {
                    let _ = DestroyWindow(hwnd);
                    return Err(err);
                }
            }
            if let Err(err) = hotkey::register_toggle_palette(hwnd) {
                // Combo may already be taken (ERROR_HOTKEY_ALREADY_REGISTERED); tray still toggles.
                eprintln!("{err}");
            }
            Ok(Self { hwnd })
        }
    }

    pub fn show_at(&self, frame_px: physical::Rect) {
        unsafe {
            let Some(inner) = inner_from(self.hwnd) else {
                return;
            };
            (*inner).rest_frame = frame_px;
            (*inner).anim = Some(PaletteAnim {
                kind: PaletteAnimKind::Enter,
                start: std::time::Instant::now(),
            });
            let (scale, alpha) = scale_alpha(PaletteAnimKind::Enter, 0.0);
            (*inner).present_alpha = alpha;
            let frame = scaled_rect(frame_px, scale);
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                frame.x,
                frame.y,
                frame.w,
                frame.h,
                SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(self.hwnd);
            self.layout_search();
            self.focus_search();
            if SetTimer(self.hwnd, ANIM_TIMER_ID, ANIM_TICK_MS, None) == 0 {
                finish_anim(self.hwnd, inner);
            }
        }
    }

    pub fn reset_search(&self) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                if let Some(edit) = (*inner).edit.as_ref() {
                    edit.set_text("");
                }
            }
        }
    }

    pub fn focus_search(&self) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                if let Some(edit) = (*inner).edit.as_ref() {
                    edit.focus();
                }
            }
        }
    }

    pub fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, FALSE);
        }
    }

    fn layout_search(&self) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                if let Some(edit) = (*inner).edit.as_mut() {
                    edit.layout(self.hwnd);
                }
            }
        }
    }

    pub fn hide(&self) {
        unsafe {
            let Some(inner) = inner_from(self.hwnd) else {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                return;
            };
            if let Some(PaletteAnim {
                kind: PaletteAnimKind::Exit,
                ..
            }) = (*inner).anim
            {
                return;
            }
            if !IsWindowVisible(self.hwnd).as_bool() && (*inner).anim.is_none() {
                return;
            }
            if (*inner).rest_frame.w <= 0 || (*inner).rest_frame.h <= 0 {
                (*inner).anim = None;
                (*inner).present_alpha = LAYERED_SOURCE_CONSTANT_ALPHA;
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                return;
            }
            (*inner).anim = Some(PaletteAnim {
                kind: PaletteAnimKind::Exit,
                start: std::time::Instant::now(),
            });
            if SetTimer(self.hwnd, ANIM_TIMER_ID, ANIM_TICK_MS, None) == 0 {
                finish_anim(self.hwnd, inner);
            } else {
                apply_anim_frame(self.hwnd, inner);
            }
        }
    }

    pub fn set_expanded(&self, expanded: bool, anchor_px: physical::Point) {
        unsafe {
            let dpi = GetDpiForWindow(self.hwnd);
            let (w_dip, h_dip) = if expanded {
                expanded_size()
            } else {
                compact_size()
            };
            let rest = physical::Rect {
                x: anchor_px.x,
                y: anchor_px.y,
                w: dip_scalar_to_px(w_dip, dpi),
                h: dip_scalar_to_px(h_dip, dpi),
            };
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).rest_frame = rest;
                if (*inner).anim.is_some() {
                    apply_anim_frame(self.hwnd, inner);
                    return;
                }
            }
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rest.x,
                rest.y,
                rest.w,
                rest.h,
                SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(self.hwnd, None, FALSE);
        }
    }
}

impl Drop for PaletteWindow {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                hotkey::unregister_toggle_palette(self.hwnd);
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

unsafe fn apply_dwm(hwnd: HWND) -> bool {
    let backdrop = DWMSBT_TRANSIENTWINDOW;
    let acrylic = DwmSetWindowAttribute(
        hwnd,
        DWMWA_SYSTEMBACKDROP_TYPE,
        &backdrop as *const _ as *const core::ffi::c_void,
        std::mem::size_of_val(&backdrop) as u32,
    )
    .is_ok();

    let corners = DWMWCP_ROUND;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corners as *const _ as *const core::ffi::c_void,
        std::mem::size_of_val(&corners) as u32,
    );

    let dark = TRUE;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark as *const _ as *const core::ffi::c_void,
        std::mem::size_of_val(&dark) as u32,
    );

    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    acrylic
}

unsafe fn inner_from(hwnd: HWND) -> Option<*mut PaletteInner> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PaletteInner;
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

unsafe fn paint_palette(hwnd: HWND, inner: *mut PaletteInner) {
    let placeholder = core_from_host((*inner).host)
        .map(|core| should_draw_placeholder(&(*core).palette))
        .unwrap_or(false);
    (*inner)
        .renderer
        .paint(hwnd, placeholder, (*inner).present_alpha);
}

fn scaled_rect(rest: physical::Rect, scale: f32) -> physical::Rect {
    let w = ((rest.w as f32) * scale).round().max(1.0) as i32;
    let h = ((rest.h as f32) * scale).round().max(1.0) as i32;
    physical::Rect {
        x: rest.x + (rest.w - w) / 2,
        y: rest.y + (rest.h - h) / 2,
        w,
        h,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn scale_alpha(kind: PaletteAnimKind, t: f32) -> (f32, u8) {
    let rest_alpha = LAYERED_SOURCE_CONSTANT_ALPHA as f32;
    match kind {
        PaletteAnimKind::Enter => (
            lerp(ENTER_SCALE, 1.0, t),
            lerp(0.0, rest_alpha, t).round() as u8,
        ),
        PaletteAnimKind::Exit => (
            lerp(1.0, ENTER_SCALE, t),
            lerp(rest_alpha, 0.0, t).round() as u8,
        ),
    }
}

fn anim_t(anim: &PaletteAnim) -> (f32, bool) {
    let dur = std::time::Duration::from_secs_f32(match anim.kind {
        PaletteAnimKind::Enter => theme::duration::ENTER_SECS,
        PaletteAnimKind::Exit => theme::duration::EXIT_SECS,
    });
    let elapsed = anim.start.elapsed();
    if elapsed >= dur {
        (1.0, true)
    } else {
        (elapsed.as_secs_f32() / dur.as_secs_f32(), false)
    }
}

unsafe fn apply_anim_frame(hwnd: HWND, inner: *mut PaletteInner) {
    let Some(anim) = (*inner).anim else {
        return;
    };
    let (t, _) = anim_t(&anim);
    let (scale, alpha) = scale_alpha(anim.kind, t);
    (*inner).present_alpha = alpha;
    let frame = scaled_rect((*inner).rest_frame, scale);
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        frame.x,
        frame.y,
        frame.w,
        frame.h,
        SWP_NOACTIVATE,
    );
}

unsafe fn finish_anim(hwnd: HWND, inner: *mut PaletteInner) {
    let Some(anim) = (*inner).anim.take() else {
        return;
    };
    let _ = KillTimer(hwnd, ANIM_TIMER_ID);
    (*inner).present_alpha = LAYERED_SOURCE_CONSTANT_ALPHA;
    match anim.kind {
        PaletteAnimKind::Enter => {
            let rest = (*inner).rest_frame;
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                rest.x,
                rest.y,
                rest.w,
                rest.h,
                SWP_NOACTIVATE,
            );
        }
        PaletteAnimKind::Exit => {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

unsafe fn tick_anim(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let Some(anim) = (*inner).anim else {
        return;
    };
    let (_, done) = anim_t(&anim);
    if done {
        finish_anim(hwnd, inner);
    } else {
        apply_anim_frame(hwnd, inner);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => {
            if let Some(inner) = inner_from(hwnd) {
                let width = (lparam.0 as u32) & 0xffff;
                let height = ((lparam.0 as u32) >> 16) & 0xffff;
                (*inner).renderer.resize(hwnd, width, height);
                if let Some(edit) = (*inner).edit.as_mut() {
                    edit.layout(hwnd);
                }
                if (*inner).renderer.is_layered() {
                    paint_palette(hwnd, inner);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let _hdc = BeginPaint(hwnd, &mut ps);
            if let Some(inner) = inner_from(hwnd) {
                paint_palette(hwnd, inner);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            unsafe {
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00FFFFFF));
            }
            LRESULT(unsafe { GetStockObject(NULL_BRUSH) }.0 as isize)
        }
        WM_COMMAND => {
            let notify = ((wparam.0 as u32) >> 16) & 0xffff;
            if notify == EN_CHANGE {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        let edit = HWND(lparam.0 as *mut core::ffi::c_void);
                        (*core).set_query(super::edit::window_text(edit));
                    }
                }
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            if let Some(inner) = inner_from(hwnd) {
                (*inner).renderer.discard_target();
                if let Some(edit) = (*inner).edit.as_mut() {
                    edit.layout(hwnd);
                }
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).relayout_palette();
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u16 == VK_ESCAPE.0 {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).handle_escape();
                    }
                }
            } else if wparam.0 as u16 == VK_DOWN.0 {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).expand_select_first();
                    }
                }
            }
            LRESULT(0)
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam.0 as u32) {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).append_query_char(ch);
                    }
                }
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if wparam.0 as i32 == TOGGLE_PALETTE_ID {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).toggle_palette();
                    }
                }
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 as u32) & 0xffff == WA_INACTIVE {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        if (*core).palette_visible {
                            (*core).hide_palette();
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == ANIM_TIMER_ID {
                tick_anim(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            hotkey::unregister_toggle_palette(hwnd);
            let _ = KillTimer(hwnd, ANIM_TIMER_ID);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PaletteInner;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
