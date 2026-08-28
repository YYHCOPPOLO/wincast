use tinycast_pure::palette_placement::{compact_size, expanded_size};
use tinycast_pure::palette_state::should_draw_placeholder;
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
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DOWN;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, IsWindow, LoadCursorW,
    RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, EN_CHANGE, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_CHAR, WM_COMMAND, WM_CTLCOLOREDIT, WM_DESTROY,
    WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SIZE,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::d2d::Renderer;
use super::edit::SearchEdit;
use super::physical;
use crate::app_core::AppCore;
use crate::platform::screens::dip_scalar_to_px;

const CLASS: windows::core::PCWSTR = w!("TinycastPalette");

pub struct PaletteWindow {
    pub hwnd: HWND,
}

struct PaletteInner {
    host: HWND,
    renderer: Renderer,
    edit: Option<SearchEdit>,
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
            Ok(Self { hwnd })
        }
    }

    pub fn show_at(&self, frame_px: physical::Rect) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                frame_px.x,
                frame_px.y,
                frame_px.w,
                frame_px.h,
                SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(self.hwnd);
            let _ = InvalidateRect(self.hwnd, None, FALSE);
            self.layout_search();
            self.focus_search();
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
            let _ = ShowWindow(self.hwnd, SW_HIDE);
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
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                anchor_px.x,
                anchor_px.y,
                dip_scalar_to_px(w_dip, dpi),
                dip_scalar_to_px(h_dip, dpi),
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
    (*inner).renderer.paint(hwnd, placeholder);
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
            if wparam.0 as u16 == VK_DOWN.0 {
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
        WM_DESTROY => LRESULT(0),
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
