use tinycast_pure::palette_placement::{compact_size, expanded_size};
use tinycast_pure::palette_state::should_draw_placeholder;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, FALSE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
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
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor, GetClientRect,
    GetForegroundWindow, GetWindowLongPtrW, HideCaret, IsWindow, IsWindowVisible, KillTimer,
    LoadCursorW, PostMessageW, RegisterClassW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, ShowCaret, ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, EN_CHANGE, GA_ROOT,
    GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW, MA_NOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
    WA_INACTIVE, WM_ACTIVATE, WM_CHAR, WM_COMMAND, WM_CTLCOLOREDIT, WM_DESTROY, WM_DPICHANGED,
    WM_ERASEBKGND, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SIZE, WM_TIMER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::d2d::{PaintParams, Renderer, LAYERED_SOURCE_CONSTANT_ALPHA};
use super::edit::SearchEdit;
use super::physical;
use crate::app_core::AppCore;
use crate::features::launcher::ui::list::{client_point_to_dip, IconCache};
use crate::platform::hotkey::{self, TOGGLE_PALETTE_ID};
use crate::platform::messages::WM_RESIGN_PALETTE;
use crate::platform::screens::dip_scalar_to_px;

const CLASS: windows::core::PCWSTR = w!("TinycastPalette");
const ANIM_TIMER_ID: usize = 1;
const RESIGN_TIMER_ID: usize = 2;
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
    icons: IconCache,
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
                icons: IconCache::new(),
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
        self.set_search_text("");
    }

    pub fn set_search_text(&self, text: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                if let Some(edit) = (*inner).edit.as_ref() {
                    edit.set_text(text);
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

    pub fn set_search_caret_visible(&self, visible: bool) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                if let Some(edit) = (*inner).edit.as_ref() {
                    if visible {
                        let _ = ShowCaret(edit.hwnd);
                    } else {
                        let _ = HideCaret(edit.hwnd);
                    }
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
                let trailing = core_from_host((*inner).host)
                    .map(|c| (*c).search_trailing_width())
                    .unwrap_or(0.0);
                layout_edit(self.hwnd, inner, trailing);
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
                (*inner).icons.drop_all();
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
    let inner = &mut *inner;
    let Some(core) = core_from_host(inner.host) else {
        let params = PaintParams {
            placeholder: false,
            placeholder_text: "",
            items: &[],
            scroll: 0.0,
            cache: &mut inner.icons,
            appearance: 0,
            footer: super::menu::FooterPaint {
                show_action_group: false,
                primary_label: "",
            },
            menu: None,
            clipboard_preview: None,
            tab_hint: None,
            clipboard_filter: None,
        };
        inner.renderer.paint(hwnd, params, inner.present_alpha);
        return;
    };
    let placeholder = should_draw_placeholder(&(*core).palette);
    let placeholder_owned = (*core).search_placeholder();
    let items = (*core).launcher_paint_items();
    let scroll = (*core).list_scroll();
    let appearance = (*core).appearance_key();
    let footer = (*core).footer_paint();
    let menu = (*core).menu_paint();
    let preview = (*core).clipboard_preview();
    let tab_hint = (*core).tab_hint();
    let filter = (*core).clipboard_filter_paint();
    layout_edit(hwnd, inner, (*core).search_trailing_width());
    let params = PaintParams {
        placeholder,
        placeholder_text: placeholder_owned.as_str(),
        items: &items,
        scroll,
        cache: &mut inner.icons,
        appearance,
        footer,
        menu,
        clipboard_preview: preview.as_deref(),
        tab_hint,
        clipboard_filter: filter,
    };
    inner.renderer.paint(hwnd, params, inner.present_alpha);
}

fn layout_edit(hwnd: HWND, inner: *mut PaletteInner, trailing: f32) {
    unsafe {
        if let Some(edit) = (*inner).edit.as_mut() {
            edit.layout_with_trailing(hwnd, trailing);
        }
    }
}

fn client_dip_size(hwnd: HWND, dpi: u32) -> (f32, f32) {
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    client_point_to_dip(rc.right, rc.bottom, dpi)
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
            (*inner).icons.drop_all();
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

fn resign_should_hide(palette_visible: bool, foreground_is_self: bool) -> bool {
    palette_visible && !foreground_is_self
}

fn lparam_point(lp: LPARAM) -> (i32, i32) {
    let v = lp.0 as u32;
    let x = v as u16 as i16 as i32;
    let y = (v >> 16) as u16 as i16 as i32;
    (x, y)
}

fn foreground_is_palette_or_child(palette: HWND) -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return false;
    }
    if fg == palette {
        return true;
    }
    unsafe { GetAncestor(fg, GA_ROOT) == palette }
}

unsafe fn apply_resign(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let Some(core) = core_from_host((*inner).host) else {
        return;
    };
    if !resign_should_hide(
        (*core).palette_visible,
        foreground_is_palette_or_child(hwnd),
    ) {
        return;
    }
    (*core).hide_palette();
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
                let trailing = core_from_host((*inner).host)
                    .map(|c| (*c).search_trailing_width())
                    .unwrap_or(0.0);
                layout_edit(hwnd, inner, trailing);
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
                let trailing = core_from_host((*inner).host)
                    .map(|c| (*c).search_trailing_width())
                    .unwrap_or(0.0);
                layout_edit(hwnd, inner, trailing);
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).relayout_palette();
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if let Some(inner) = inner_from(hwnd) {
                if let Some(core) = core_from_host((*inner).host) {
                    let _ = (*core).handle_key(wparam.0 as u16);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            if let Some(inner) = inner_from(hwnd) {
                if let Some(core) = core_from_host((*inner).host) {
                    let dpi = GetDpiForWindow(hwnd);
                    let (x, y) = lparam_point(lparam);
                    let (x_dip, y_dip) = client_point_to_dip(x, y, dpi);
                    let (w_dip, h_dip) = client_dip_size(hwnd, dpi);
                    (*core).pointer_down(x_dip, y_dip, w_dip, h_dip, msg == WM_LBUTTONDBLCLK);
                    if let Some(edit) = (*inner).edit.as_ref() {
                        edit.focus();
                    }
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(inner) = inner_from(hwnd) {
                if let Some(core) = core_from_host((*inner).host) {
                    let dpi = GetDpiForWindow(hwnd);
                    let (x, y) = lparam_point(lparam);
                    let (x_dip, y_dip) = client_point_to_dip(x, y, dpi);
                    let (w_dip, h_dip) = client_dip_size(hwnd, dpi);
                    (*core).pointer_move(x_dip, y_dip, w_dip, h_dip);
                }
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => {
            let menu_open = inner_from(hwnd)
                .and_then(|inner| core_from_host(unsafe { (*inner).host }))
                .map(|core| unsafe { (*core).menu_is_open() })
                .unwrap_or(false);
            if menu_open {
                LRESULT(MA_NOACTIVATE as isize)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_MOUSEWHEEL => {
            if let Some(inner) = inner_from(hwnd) {
                if let Some(core) = core_from_host((*inner).host) {
                    let delta = ((wparam.0 as u32) >> 16) as i16;
                    (*core).scroll_list(delta);
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
                // Defer hide: tray click deactivates us before WM_TOGGLE_PALETTE.
                let _ = SetTimer(hwnd, RESIGN_TIMER_ID, 1, None);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == ANIM_TIMER_ID {
                tick_anim(hwnd);
            } else if wparam.0 == RESIGN_TIMER_ID {
                let _ = KillTimer(hwnd, RESIGN_TIMER_ID);
                let _ = PostMessageW(hwnd, WM_RESIGN_PALETTE, WPARAM(0), LPARAM(0));
            }
            LRESULT(0)
        }
        WM_RESIGN_PALETTE => {
            apply_resign(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            hotkey::unregister_toggle_palette(hwnd);
            let _ = KillTimer(hwnd, ANIM_TIMER_ID);
            let _ = KillTimer(hwnd, RESIGN_TIMER_ID);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resign_hides_only_when_visible_and_focus_left() {
        assert!(resign_should_hide(true, false));
        assert!(!resign_should_hide(true, true));
        assert!(!resign_should_hide(false, false));
        assert!(!resign_should_hide(false, true));
    }
}
