//! Captioned HWND sizing: client is DIP, outer includes non-client chrome at the window DPI.

use windows::Win32::Foundation::{FALSE, HWND, LPARAM, RECT};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER, WINDOW_EX_STYLE,
    WINDOW_STYLE,
};

use super::screens::dip_scalar_to_px;

pub fn captioned_outer_px(
    client_dip: (f32, f32),
    dpi: u32,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
) -> (i32, i32) {
    let w = dip_scalar_to_px(client_dip.0, dpi);
    let h = dip_scalar_to_px(client_dip.1, dpi);
    outer_from_client_px(w, h, dpi, style, ex)
}

pub fn outer_from_client_px(
    client_w: i32,
    client_h: i32,
    dpi: u32,
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
) -> (i32, i32) {
    let dpi = if dpi == 0 { 96 } else { dpi };
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: client_w,
        bottom: client_h,
    };
    unsafe {
        let _ = AdjustWindowRectExForDpi(&mut rc, style, FALSE, ex, dpi);
    }
    (rc.right - rc.left, rc.bottom - rc.top)
}

pub fn apply_captioned_client_dip(
    hwnd: HWND,
    client_dip: (f32, f32),
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let (ow, oh) = captioned_outer_px(client_dip, dpi, style, ex);
    unsafe {
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            wr.left,
            wr.top,
            ow,
            oh,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

pub fn apply_dpi_changed_fixed(
    hwnd: HWND,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: LPARAM,
    client_dip: (f32, f32),
    style: WINDOW_STYLE,
    ex: WINDOW_EX_STYLE,
) {
    let suggested = lparam.0 as *const RECT;
    if suggested.is_null() {
        return;
    }
    let r = unsafe { *suggested };
    let dpi = ((wparam.0 as u32) >> 16).max(1);
    let (ow, oh) = captioned_outer_px(client_dip, dpi, style, ex);
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            r.left,
            r.top,
            ow,
            oh,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{WS_CAPTION, WS_OVERLAPPED, WS_SYSMENU};

    fn caption_style() -> WINDOW_STYLE {
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU
    }

    #[test]
    fn captioned_outer_is_taller_than_client_at_96() {
        let client = (460.0, 360.0);
        let (ow, oh) = captioned_outer_px(client, 96, caption_style(), WINDOW_EX_STYLE::default());
        assert!(ow >= 460, "outer width {ow}");
        assert!(
            oh > 360,
            "captioned outer must include title bar, got height {oh}"
        );
    }

    #[test]
    fn captioned_outer_scales_with_dpi() {
        let client = (520.0, 400.0);
        let style = caption_style();
        let ex = WINDOW_EX_STYLE::default();
        let at_96 = captioned_outer_px(client, 96, style, ex);
        let at_144 = captioned_outer_px(client, 144, style, ex);
        assert!(at_144.0 > at_96.0);
        assert!(at_144.1 > at_96.1);
        assert!(
            at_144.0 >= 780,
            "520 DIP client at 150% is 780 px, outer {0}",
            at_144.0
        );
    }
}
