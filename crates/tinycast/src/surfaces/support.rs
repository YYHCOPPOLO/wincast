//! Support and About windows. Every `showSupport` route lands here so the reminder anchor moves once.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowLongPtrW, IsWindow,
    LoadCursorW, MoveWindow, RegisterClassW, SendMessageW, SetWindowLongPtrW, ShowWindow,
    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, SW_HIDE, SW_SHOW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SIZE, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

const BS_AUTOCHECKBOX: WINDOW_STYLE = WINDOW_STYLE(0x00000003);
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;

use crate::app_core::AppCore;
use crate::design_system::host::OverlayPainter;
use crate::design_system::panel::paint_sheen;
use crate::design_system::squircle::{fill_squircle, stroke_squircle};
use crate::design_system::text;
use crate::features::launcher::ui::coordinator::{execute, LaunchSpec};
use crate::platform::screens::dip_scalar_to_px;
use crate::platform::window::{apply_captioned_client_dip, apply_dpi_changed_fixed};

fn overlay_caption_style() -> WINDOW_STYLE {
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU
}

fn overlay_caption_ex() -> WINDOW_EX_STYLE {
    WINDOW_EX_STYLE::default()
}

pub const CHECKOUT: &str =
    "https://buy.polar.sh/polar_cl_NDVFC20DKQpLcNawsh97QzbARBXD3WNn8v35R0mbJmT";
pub const WIDTH: f32 = 460.0;
pub const HEIGHT: f32 = 360.0;
pub const ICON: f32 = 76.0;

const SUPPORT_CLASS: windows::core::PCWSTR = w!("TinycastSupport");
const ABOUT_CLASS: windows::core::PCWSTR = w!("TinycastAbout");
const ID_SUPPORT: usize = 1;
const ID_REMINDERS: usize = 2;

pub struct SupportWindow {
    pub hwnd: HWND,
}

pub struct AboutWindow {
    pub hwnd: HWND,
}

struct Inner {
    host: HWND,
    kind: Kind,
    reminders: HWND,
    painter: Option<OverlayPainter>,
    support_rect: DipRect,
    remind_rect: DipRect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Support,
    About,
}

impl SupportWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        Ok(Self {
            hwnd: create(host, Kind::Support)?,
        })
    }

    pub fn show(&self) {
        unsafe {
            sync_reminders_checkbox(self.hwnd);
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);
        }
    }

    pub fn set_locale(&self, locale: &str) {
        unsafe {
            apply_locale(self.hwnd, locale);
        }
    }
}

impl AboutWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        Ok(Self {
            hwnd: create(host, Kind::About)?,
        })
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(self.hwnd);
        }
    }

    pub fn set_locale(&self, locale: &str) {
        unsafe {
            apply_locale(self.hwnd, locale);
        }
    }
}

impl Drop for SupportWindow {
    fn drop(&mut self) {
        destroy(self.hwnd);
        self.hwnd = HWND::default();
    }
}

impl Drop for AboutWindow {
    fn drop(&mut self) {
        destroy(self.hwnd);
        self.hwnd = HWND::default();
    }
}

fn destroy(hwnd: HWND) {
    unsafe {
        if IsWindow(hwnd).as_bool() {
            let _ = DestroyWindow(hwnd);
        }
    }
}

struct SupportChrome {
    support: DipRect,
    polar_bottom: f32,
    remind: DipRect,
}

fn support_chrome(width: f32, height: f32) -> SupportChrome {
    let pad = theme::spacing::XXL;
    let icon_y = theme::spacing::MD;
    let btn_h = 46.0;
    let support = DipRect {
        x: pad,
        y: icon_y + ICON + 96.0,
        w: (width - pad * 2.0).max(40.0),
        h: btn_h,
    };
    let polar_bottom = support.y + btn_h + theme::spacing::MD + 18.0;
    let remind_h = 24.0;
    let y = (polar_bottom + theme::spacing::MD).min((height - pad - remind_h).max(polar_bottom));
    SupportChrome {
        support,
        polar_bottom,
        remind: DipRect {
            x: pad,
            y,
            w: 280.0,
            h: remind_h,
        },
    }
}

fn paint_remind_row(
    target: &windows::Win32::Graphics::Direct2D::ID2D1RenderTarget,
    fonts: &crate::design_system::Fonts,
    row: DipRect,
    on: bool,
    lang: tinycast_pure::i18n::UiLang,
) -> windows::core::Result<()> {
    let box_s = theme::size::CHECKBOX;
    let box_rect = DipRect {
        x: row.x,
        y: row.y + (row.h - box_s) / 2.0,
        w: box_s,
        h: box_s,
    };
    let fill = if on {
        text::BRAND
    } else {
        text::control_surface(0)
    };
    fill_squircle(target, box_rect, theme::radius::RECORDER_KEY_CAP, fill)?;
    stroke_squircle(
        target,
        box_rect,
        theme::radius::RECORDER_KEY_CAP,
        theme::colors::ramp_rgba(0, theme::colors::BORDER_DARK_ALPHA, theme::colors::BORDER_LIGHT_ALPHA),
        theme::size::HAIRLINE,
    )?;
    if on {
        crate::design_system::symbols::paint_fluent_in(
            target,
            &fonts.dwrite,
            "checkmark",
            DipRect {
                x: box_rect.x + 1.0,
                y: box_rect.y + 1.0,
                w: box_s - 2.0,
                h: box_s - 2.0,
            },
            (1.0, 1.0, 1.0, 1.0),
        )?;
    }
    text::draw(
        target,
        &fonts.row_title,
        tinycast_pure::i18n::support_remind_later(lang),
        DipRect {
            x: box_rect.x + box_s + theme::spacing::MD,
            y: row.y,
            w: (row.w - box_s - theme::spacing::MD).max(8.0),
            h: row.h,
        },
        text::primary_ink(0),
    )?;
    Ok(())
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

fn client_dip_size(hwnd: HWND) -> (f32, f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    (rc.right as f32 * 96.0 / dpi, rc.bottom as f32 * 96.0 / dpi)
}

unsafe fn layout_reminders(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    if (*inner).reminders.is_invalid() {
        return;
    }
    let dpi = GetDpiForWindow(hwnd);
    let (w, h) = client_dip_size(hwnd);
    let chrome = support_chrome(w, h);
    let r = chrome.remind;
    let _ = MoveWindow(
        (*inner).reminders,
        dip_scalar_to_px(r.x, dpi),
        dip_scalar_to_px(r.y, dpi),
        dip_scalar_to_px(r.w, dpi),
        dip_scalar_to_px(r.h, dpi),
        true,
    );
}

fn client_dip(hwnd: HWND, lparam: LPARAM) -> (f32, f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    let x = (lparam.0 as u32 & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    let y = ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as f32 * 96.0 / dpi;
    (x, y)
}

fn create(host: HWND, kind: Kind) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = match kind {
            Kind::Support => SUPPORT_CLASS,
            Kind::About => ABOUT_CLASS,
        };
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            overlay_caption_ex(),
            class,
            windows::core::PCWSTR::null(),
            overlay_caption_style(),
            240,
            180,
            WIDTH.round() as i32,
            HEIGHT.round() as i32,
            host,
            None,
            hinstance,
            Some(host.0 as *const core::ffi::c_void),
        )?;
        if let Some(inner) = inner_from(hwnd) {
            (*inner).kind = kind;
        }
        apply_captioned_client_dip(
            hwnd,
            (WIDTH, HEIGHT),
            overlay_caption_style(),
            overlay_caption_ex(),
        );
        let support_btn = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!(""),
            WINDOW_STYLE(WS_CHILD.0 | 1),
            40,
            200,
            200,
            36,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_SUPPORT as *mut core::ffi::c_void),
            hinstance,
            None,
        )
        .unwrap_or_default();
        let _ = ShowWindow(support_btn, SW_HIDE);
        if matches!(kind, Kind::Support) {
            let box_hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX,
                0,
                0,
                80,
                24,
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(
                    ID_REMINDERS as *mut core::ffi::c_void,
                ),
                hinstance,
                None,
            )
            .unwrap_or_default();
            let _ = ShowWindow(box_hwnd, SW_HIDE);
            if let Some(inner) = inner_from(hwnd) {
                (*inner).reminders = box_hwnd;
            }
            layout_reminders(hwnd);
        }
        if let Some(inner) = inner_from(hwnd) {
            apply_chrome_titles(hwnd, inner);
        }
        Ok(hwnd)
    }
}

unsafe fn sync_reminders_checkbox(hwnd: HWND) {
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    if (*inner).reminders.is_invalid() {
        return;
    }
    let on = core_from_host((*inner).host)
        .map(|c| (*c).settings.support_reminders)
        .unwrap_or(true);
    let _ = SendMessageW(
        (*inner).reminders,
        BM_SETCHECK,
        WPARAM(if on {
            BST_CHECKED.0 as usize
        } else {
            BST_UNCHECKED.0 as usize
        }),
        LPARAM(0),
    );
}

unsafe fn inner_from(hwnd: HWND) -> Option<*mut Inner> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn lang_of(inner: *mut Inner) -> tinycast_pure::i18n::UiLang {
    core_from_host((*inner).host)
        .map(|c| (*c).ui_lang())
        .unwrap_or(tinycast_pure::i18n::UiLang::ZhHans)
}

unsafe fn apply_locale(hwnd: HWND, locale: &str) {
    if let Some(inner) = inner_from(hwnd) {
        if let Some(painter) = (*inner).painter.as_mut() {
            painter.set_locale(locale);
        }
        apply_chrome_titles(hwnd, inner);
    }
    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
}

unsafe fn apply_chrome_titles(hwnd: HWND, inner: *mut Inner) {
    let lang = lang_of(inner);
    let title = match (*inner).kind {
        Kind::Support => tinycast_pure::i18n::support_title(lang),
        Kind::About => tinycast_pure::i18n::about_window_title(lang),
    };
    let mut wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
        hwnd,
        windows::core::PCWSTR(wide.as_mut_ptr()),
    );
    if !(*inner).reminders.is_invalid() {
        let mut remind: Vec<u16> = tinycast_pure::i18n::support_remind_later(lang)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(
            (*inner).reminders,
            windows::core::PCWSTR(remind.as_mut_ptr()),
        );
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

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let inner = Box::new(Inner {
                host: HWND(cs.lpCreateParams),
                kind: Kind::Support,
                reminders: HWND::default(),
                painter: OverlayPainter::new().ok(),
                support_rect: empty_rect(),
                remind_rect: empty_rect(),
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
            LRESULT(1)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => {
            layout_reminders(hwnd);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            apply_dpi_changed_fixed(
                hwnd,
                wparam,
                lparam,
                (WIDTH, HEIGHT),
                overlay_caption_style(),
                overlay_caption_ex(),
            );
            layout_reminders(hwnd);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            if id == ID_SUPPORT {
                let _ = execute(&LaunchSpec::Uri(CHECKOUT.into()));
            }
            if id == ID_REMINDERS {
                if let Some(inner) = inner_from(hwnd) {
                    let checked =
                        SendMessageW((*inner).reminders, BM_GETCHECK, WPARAM(0), LPARAM(0)).0
                            == BST_CHECKED.0 as isize;
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).set_support_reminders(checked);
                    }
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = client_dip(hwnd, lparam);
            if let Some(inner) = inner_from(hwnd) {
                if contains((*inner).support_rect, x, y) {
                    let _ = execute(&LaunchSpec::Uri(CHECKOUT.into()));
                }
                if (*inner).kind == Kind::Support && contains((*inner).remind_rect, x, y) {
                    if let Some(core) = core_from_host((*inner).host) {
                        let next = !(*core).settings.support_reminders;
                        (*core).set_support_reminders(next);
                    }
                    sync_reminders_checkbox(hwnd);
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
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
        let kind = (*inner).kind;
        let lang = lang_of(inner);
        let mut support_rect = empty_rect();
        let mut remind_rect = empty_rect();
        let reminders_on = core_from_host((*inner).host)
            .map(|c| (*c).settings.support_reminders)
            .unwrap_or(true);
        if let Some(painter) = (*inner).painter.as_mut() {
            painter.set_locale(lang.dwrite_locale());
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
                let icon = ICON;
                let chrome = support_chrome(size.width, size.height);
                let icon_rect = DipRect {
                    x: (size.width - icon) / 2.0,
                    y: theme::spacing::MD,
                    w: icon,
                    h: icon,
                };
                fill_squircle(target, icon_rect, 18.0, text::BRAND)?;
                crate::design_system::symbols::paint_fluent_in(
                    target,
                    &fonts.dwrite,
                    "heart",
                    DipRect {
                        x: icon_rect.x + 22.0,
                        y: icon_rect.y + 22.0,
                        w: 32.0,
                        h: 32.0,
                    },
                    (1.0, 1.0, 1.0, 1.0),
                )?;
                let lang = lang_of(inner);
                let (title, subtitle) = match kind {
                    Kind::Support => (
                        tinycast_pure::i18n::support_title(lang),
                        tinycast_pure::i18n::support_built_with_love(lang),
                    ),
                    Kind::About => (
                        tinycast_pure::i18n::about_product(lang),
                        tinycast_pure::i18n::support_keeps_independent(lang),
                    ),
                };
                text::draw(
                    target,
                    &fonts.headline_center,
                    title,
                    DipRect {
                        x: pad,
                        y: icon_rect.y + icon + theme::spacing::XL,
                        w: size.width - pad * 2.0,
                        h: 28.0,
                    },
                    text::primary_ink(0),
                )?;
                text::draw(
                    target,
                    &fonts.wrap_callout,
                    subtitle,
                    DipRect {
                        x: pad,
                        y: icon_rect.y + icon + 44.0,
                        w: size.width - pad * 2.0,
                        h: 40.0,
                    },
                    text::secondary_ink(0),
                )?;
                let btn_h = chrome.support.h;
                support_rect = chrome.support;
                fill_squircle(target, support_rect, 12.0, text::BRAND)?;
                text::draw(
                    target,
                    &fonts.bar,
                    tinycast_pure::i18n::support_title(lang),
                    support_rect,
                    (1.0, 1.0, 1.0, 1.0),
                )?;
                text::draw(
                    target,
                    &fonts.section,
                    tinycast_pure::i18n::support_checkout(lang),
                    DipRect {
                        x: pad,
                        y: support_rect.y + btn_h + theme::spacing::MD,
                        w: size.width - pad * 2.0,
                        h: 18.0,
                    },
                    text::tertiary_ink(0),
                )?;
                if matches!(kind, Kind::Support) {
                    remind_rect = chrome.remind;
                    paint_remind_row(target, fonts, remind_rect, reminders_on, lang)?;
                }
                Ok(())
            });
            (*inner).support_rect = support_rect;
            (*inner).remind_rect = remind_rect;
        }
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_is_the_one_polar_link() {
        assert!(CHECKOUT.contains("polar.sh"));
    }

    #[test]
    fn support_width_is_460() {
        assert_eq!(crate::surfaces::support::WIDTH, 460.0);
        assert_eq!(crate::surfaces::support::HEIGHT, 360.0);
        assert_eq!(crate::surfaces::support::ICON, 76.0);
    }

    #[test]
    fn reminders_sit_below_polar_and_inside_client() {
        let chrome = support_chrome(WIDTH, HEIGHT);
        assert!(
            chrome.remind.y >= chrome.polar_bottom,
            "remind y {} polar {}",
            chrome.remind.y,
            chrome.polar_bottom
        );
        assert!(
            chrome.remind.y < chrome.polar_bottom + 40.0,
            "remind y {} should follow polar {}",
            chrome.remind.y,
            chrome.polar_bottom
        );
        assert!(chrome.remind.y + chrome.remind.h <= HEIGHT - theme::spacing::MD);
        let _ = chrome.support;
    }

    #[test]
    fn support_has_no_textout() {
        let src = include_str!("support.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
