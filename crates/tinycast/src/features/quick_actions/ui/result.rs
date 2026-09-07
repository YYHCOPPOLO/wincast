//! Quick Action result panel: 520 DIP wide, Replace / Copy / Esc.

use std::sync::atomic::{AtomicIsize, Ordering};

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, GetAncestor, GetClientRect, GetWindowLongPtrW,
    GetWindowRect, IsWindowVisible, LoadCursorW, PostMessageW, RegisterClassW, SetWindowLongPtrW,
    SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, CS_HREDRAW, CS_VREDRAW,
    GA_ROOTOWNER, GWLP_USERDATA, HHOOK, HWND_TOPMOST, IDC_ARROW, KBDLLHOOKSTRUCT, SWP_NOACTIVATE,
    SW_HIDE, SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::design_system::host::OverlayPainter;
use crate::design_system::panel::paint_scrim;
use crate::design_system::squircle::fill_squircle;
use crate::design_system::text;
use crate::design_system::Fonts;
use crate::platform::screens::{dip_scalar_to_px, screens_px};

pub const PANEL_WIDTH_DIP: f32 = theme::size::QUICK_ACTION_PANEL;
const CLASS: windows::core::PCWSTR = w!("TinycastQuickAction");
const BAR_H: i32 = 40;
const HEADER_ICON: f32 = theme::size::QUICK_ACTION_HEADER_ICON;

static HOOK_PANEL: AtomicIsize = AtomicIsize::new(0);
static KEY_HOOK: AtomicIsize = AtomicIsize::new(0);

pub struct ResultPanel {
    hwnd: HWND,
}

struct Inner {
    host: HWND,
    title: String,
    body: String,
    painter: OverlayPainter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelHit {
    Replace,
    Copy,
    Dismiss,
}

pub fn hit(x: i32, y: i32, width: i32, height: i32) -> PanelHit {
    hit_dip(x as f32, y as f32, width as f32, height as f32)
}

pub fn hit_from_px(x: i32, y: i32, width: i32, height: i32, dpi: u32) -> PanelHit {
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    let to_dip = |v: i32| v as f32 * 96.0 / dpi;
    hit_dip(to_dip(x), to_dip(y), to_dip(width), to_dip(height))
}

pub fn hit_dip(x: f32, y: f32, width: f32, height: f32) -> PanelHit {
    let footer = BAR_H as f32;
    if y >= height - footer && y < height && x >= 0.0 && x < width {
        if x < width / 2.0 {
            PanelHit::Replace
        } else {
            PanelHit::Copy
        }
    } else {
        PanelHit::Dismiss
    }
}

impl ResultPanel {
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
            let painter = OverlayPainter::new()?;
            let inner = Box::new(Inner {
                host,
                title: String::new(),
                body: String::new(),
                painter,
            });
            let ptr = Box::into_raw(inner);
            let hwnd = match CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                CLASS,
                w!(""),
                WS_POPUP,
                0,
                0,
                PANEL_WIDTH_DIP.round() as i32,
                240,
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

    pub fn show(&self, title: &str, body: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).title = title.to_string();
                (*inner).body = body.to_string();
                (*inner).painter.prefer_layered(self.hwnd);
                place(self.hwnd, (*inner).painter.fonts(), body, true);
            }
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            install_key_hook(self.hwnd);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, false);
        }
    }

    pub fn set_body(&self, body: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).body = body.to_string();
                place(self.hwnd, (*inner).painter.fonts(), body, false);
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, false);
        }
    }

    pub fn hide(&self) {
        unsafe {
            remove_key_hook(self.hwnd);
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn set_locale(&self, locale: &str) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                (*inner).painter.set_locale(locale);
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, false);
        }
    }
}

fn header_height() -> f32 {
    theme::spacing::XL + HEADER_ICON.max(18.0) + theme::spacing::LG
}

fn footer_height() -> f32 {
    BAR_H as f32
}

fn panel_height(fonts: &Fonts, body: &str) -> f32 {
    let pad = theme::spacing::XXL;
    let text_w = (PANEL_WIDTH_DIP - pad * 2.0).max(40.0);
    let shown = if body.is_empty() {
        tinycast_pure::i18n::qa_working(tinycast_pure::i18n::UiLang::ZhHans)
    } else {
        body
    };
    let body_h = fonts
        .measure(
            &fonts.wrap_body,
            shown,
            text_w,
            theme::size::QUICK_ACTION_PANEL_BODY,
        )
        .1
        .clamp(
            theme::size::QUICK_ACTION_PANEL_MIN_BODY,
            theme::size::QUICK_ACTION_PANEL_BODY,
        );
    header_height() + body_h + footer_height()
}

fn place(hwnd: HWND, fonts: &Fonts, body: &str, recenter: bool) {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let w = dip_scalar_to_px(PANEL_WIDTH_DIP, dpi);
        let h = dip_scalar_to_px(panel_height(fonts, body), dpi);
        let (x, y) = if recenter {
            let screens = screens_px();
            let screen = screens
                .iter()
                .find(|s| s.origin_is_primary)
                .or_else(|| screens.first());
            if let Some(s) = screen {
                let work_w = s.work.right - s.work.left;
                let work_h = s.work.bottom - s.work.top;
                (
                    s.work.left + (work_w - w) / 2,
                    s.work.top + (work_h - h) / 3,
                )
            } else {
                (80, 80)
            }
        } else {
            let mut rc = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rc);
            (rc.left, rc.top)
        };
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    }
}

fn install_key_hook(hwnd: HWND) {
    HOOK_PANEL.store(hwnd.0 as isize, Ordering::SeqCst);
    if KEY_HOOK.load(Ordering::SeqCst) != 0 {
        return;
    }
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), hinstance, 0);
        if let Ok(hook) = hook {
            KEY_HOOK.store(hook.0 as isize, Ordering::SeqCst);
        }
    }
}

fn remove_key_hook(hwnd: HWND) {
    let current = HOOK_PANEL.load(Ordering::SeqCst);
    if current != hwnd.0 as isize {
        return;
    }
    HOOK_PANEL.store(0, Ordering::SeqCst);
    let bits = KEY_HOOK.swap(0, Ordering::SeqCst);
    if bits != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(bits as *mut core::ffi::c_void));
        }
    }
}

unsafe extern "system" fn key_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && (wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == 0x0104) {
        let panel = HWND(HOOK_PANEL.load(Ordering::SeqCst) as *mut core::ffi::c_void);
        if !panel.is_invalid() && IsWindowVisible(panel).as_bool() {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;
            let owner = GetAncestor(panel, GA_ROOTOWNER);
            if vk == 0x1B {
                let _ = PostMessageW(
                    owner,
                    crate::platform::messages::WM_QA_DISMISS,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
            if vk == 0x0D {
                let _ = PostMessageW(
                    owner,
                    crate::platform::messages::WM_QA_APPLY,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
            if vk == 0x43 && GetAsyncKeyState(0x11) < 0 {
                let _ = PostMessageW(
                    owner,
                    crate::platform::messages::WM_QA_COPY,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

unsafe fn lang_of(inner: *mut Inner) -> tinycast_pure::i18n::UiLang {
    let ptr = GetWindowLongPtrW((*inner).host, GWLP_USERDATA) as *mut crate::app_core::AppCore;
    if ptr.is_null() {
        tinycast_pure::i18n::UiLang::ZhHans
    } else {
        (*ptr).ui_lang()
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
        windows::Win32::UI::WindowsAndMessaging::WM_NCCREATE => {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            paint(hwnd);
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let dpi = GetDpiForWindow(hwnd);
            let x = (lparam.0 as u32 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as i32;
            let owner = GetAncestor(hwnd, GA_ROOTOWNER);
            match hit_from_px(x, y, rc.right, rc.bottom, dpi) {
                PanelHit::Replace => {
                    let _ = PostMessageW(
                        owner,
                        crate::platform::messages::WM_QA_APPLY,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
                PanelHit::Copy => {
                    let _ = PostMessageW(
                        owner,
                        crate::platform::messages::WM_QA_COPY,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
                PanelHit::Dismiss => {
                    let _ = PostMessageW(
                        owner,
                        crate::platform::messages::WM_QA_DISMISS,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
            return LRESULT(0);
        }
        WM_CLOSE => {
            remove_key_hook(hwnd);
            let _ = ShowWindow(hwnd, SW_HIDE);
            return LRESULT(0);
        }
        WM_DESTROY => {
            remove_key_hook(hwnd);
        }
        WM_NCDESTROY => {
            remove_key_hook(hwnd);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                drop(Box::from_raw(ptr as *mut Inner));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
        if hdc.is_invalid() {
            return;
        }
        if let Some(inner) = inner_from(hwnd) {
            let title = (*inner).title.clone();
            let body = (*inner).body.clone();
            let lang = lang_of(inner);
            (*inner).painter.set_locale(lang.dwrite_locale());
            let _ = (*inner).painter.paint(hwnd, |target, fonts| {
                let width = PANEL_WIDTH_DIP;
                let size = target.GetSize();
                let height = size.height;
                paint_scrim(target, width, height, theme::radius::DIALOG, 0)?;
                let pad = theme::spacing::XXL;
                let icon = HEADER_ICON;
                let header_y = theme::spacing::XL;
                crate::design_system::symbols::paint_fluent_in(
                    target,
                    &fonts.dwrite,
                    "sparkles",
                    DipRect {
                        x: pad,
                        y: header_y,
                        w: icon,
                        h: icon,
                    },
                    text::secondary_ink(0),
                )?;
                text::draw(
                    target,
                    &fonts.headline,
                    &title,
                    DipRect {
                        x: pad + icon + theme::spacing::SM,
                        y: header_y - 2.0,
                        w: width - pad * 2.0 - icon - theme::spacing::SM,
                        h: 20.0,
                    },
                    text::primary_ink(0),
                )?;
                let body_top = header_height();
                let footer = footer_height();
                let body_h = (height - body_top - footer).min(theme::size::QUICK_ACTION_PANEL_BODY);
                let working = tinycast_pure::i18n::qa_working(lang);
                let shown = if body.is_empty() {
                    working
                } else {
                    body.as_str()
                };
                text::draw(
                    target,
                    &fonts.wrap_body,
                    shown,
                    DipRect {
                        x: pad,
                        y: body_top,
                        w: width - pad * 2.0,
                        h: body_h.max(theme::size::QUICK_ACTION_PANEL_MIN_BODY),
                    },
                    text::primary_ink(0),
                )?;
                let btn_h = theme::size::MENU_BUTTON;
                let btn_y = height - footer + (footer - btn_h) / 2.0;
                let half = (width - pad * 2.0 - theme::spacing::MD) / 2.0;
                let replace = DipRect {
                    x: pad,
                    y: btn_y,
                    w: half,
                    h: btn_h,
                };
                let copy = DipRect {
                    x: pad + half + theme::spacing::MD,
                    y: btn_y,
                    w: half,
                    h: btn_h,
                };
                fill_squircle(target, replace, btn_h / 2.0, text::control_surface(0))?;
                fill_squircle(target, copy, btn_h / 2.0, text::control_surface(0))?;
                text::draw(
                    target,
                    &fonts.bar,
                    tinycast_pure::i18n::qa_replace(lang),
                    replace,
                    text::primary_ink(0),
                )?;
                text::draw(
                    target,
                    &fonts.bar,
                    tinycast_pure::i18n::qa_copy(lang),
                    copy,
                    text::secondary_ink(0),
                )?;
                Ok(())
            });
        }
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_is_520_dip() {
        assert_eq!(PANEL_WIDTH_DIP, 520.0);
        let _ = theme::spacing::XL;
    }

    #[test]
    fn hits_split_replace_copy_and_dismiss() {
        assert_eq!(hit(10, 270, 520, 280), PanelHit::Replace);
        assert_eq!(hit(400, 270, 520, 280), PanelHit::Copy);
        assert_eq!(hit(10, 40, 520, 280), PanelHit::Dismiss);
        assert_ne!(hit(10, 40, 520, 280), PanelHit::Replace);
    }

    #[test]
    fn high_dpi_footer_click_is_replace() {
        let dpi = 144u32;
        let w_px = dip_scalar_to_px(520.0, dpi);
        let h_px = dip_scalar_to_px(280.0, dpi);
        let y_px = h_px - dip_scalar_to_px(30.0, dpi);
        let x_px = dip_scalar_to_px(10.0, dpi);
        assert_eq!(hit_from_px(x_px, y_px, w_px, h_px, dpi), PanelHit::Replace);
        let y_body = h_px - dip_scalar_to_px(80.0, dpi);
        assert_eq!(
            hit_from_px(x_px, y_body, w_px, h_px, dpi),
            PanelHit::Dismiss
        );
    }

    #[test]
    fn qa_result_has_no_textout() {
        let src = include_str!("result.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
