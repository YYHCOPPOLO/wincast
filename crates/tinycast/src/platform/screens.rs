use tinycast_pure::palette_placement::{DipRect, ScreenDip};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONULL,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, MONITORINFOF_PRIMARY};

#[derive(Clone, Copy, Debug)]
pub struct ScreenPx {
    pub frame: RECT,
    pub work: RECT,
    pub dpi: u32,
    pub origin_is_primary: bool,
}

/// All monitors in DIP. Placement hit-tests in pixels via `cursor_target_screen`.
#[allow(dead_code)]
pub fn screens_dip() -> Vec<ScreenDip> {
    screens_px().into_iter().map(screen_dip_from_px).collect()
}

pub fn screens_px() -> Vec<ScreenPx> {
    let mut screens = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitors),
            LPARAM(&mut screens as *mut Vec<ScreenPx> as isize),
        );
    }
    screens
}

/// Hit-test `cursor` in physical pixels, then convert only that monitor with its DPI.
pub fn target_screen_from_cursor_px(cursor: (i32, i32), screens: &[ScreenPx]) -> Option<ScreenPx> {
    screens
        .iter()
        .copied()
        .find(|s| contains_px(s.frame, cursor.0, cursor.1))
        .or_else(|| screens.iter().copied().find(|s| s.origin_is_primary))
}

pub fn screen_dip_from_px(screen: ScreenPx) -> ScreenDip {
    ScreenDip {
        frame: rect_to_dip(screen.frame, screen.dpi),
        work: rect_to_dip(screen.work, screen.dpi),
        origin_is_primary: screen.origin_is_primary,
    }
}

/// Cursor monitor in DIP, chosen in pixel space (`MonitorFromPoint`).
pub fn cursor_target_screen() -> Option<(ScreenDip, u32)> {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONULL);
        let chosen = if !mon.is_invalid() {
            screen_px_from_hmonitor(mon)
        } else {
            None
        };
        let chosen = chosen.or_else(|| {
            let screens = screens_px();
            target_screen_from_cursor_px((pt.x, pt.y), &screens)
        })?;
        Some((screen_dip_from_px(chosen), chosen.dpi))
    }
}

pub fn dip_to_px(hwnd: HWND, r: DipRect) -> RECT {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    dip_to_px_with_dpi(r, dpi)
}

pub fn dip_to_px_with_dpi(r: DipRect, dpi: u32) -> RECT {
    let s = scale_from_dpi(dpi);
    RECT {
        left: (r.x * s).round() as i32,
        top: (r.y * s).round() as i32,
        right: ((r.x + r.w) * s).round() as i32,
        bottom: ((r.y + r.h) * s).round() as i32,
    }
}

pub fn dip_scalar_to_px(dip: f32, dpi: u32) -> i32 {
    (dip * scale_from_dpi(dpi)).round() as i32
}

fn contains_px(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

fn screen_px_from_hmonitor(monitor: HMONITOR) -> Option<ScreenPx> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    Some(ScreenPx {
        frame: info.rcMonitor,
        work: info.rcWork,
        dpi: monitor_dpi(monitor),
        origin_is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

fn scale_from_dpi(dpi: u32) -> f32 {
    let dpi = if dpi == 0 { 96 } else { dpi };
    dpi as f32 / 96.0
}

fn monitor_dpi(monitor: HMONITOR) -> u32 {
    let mut dpix = 96u32;
    let mut dpiy = 96u32;
    unsafe {
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpix, &mut dpiy);
    }
    if dpix == 0 {
        96
    } else {
        dpix
    }
}

fn rect_to_dip(r: RECT, dpi: u32) -> DipRect {
    let s = 1.0 / scale_from_dpi(dpi);
    DipRect {
        x: r.left as f32 * s,
        y: r.top as f32 * s,
        w: (r.right - r.left) as f32 * s,
        h: (r.bottom - r.top) as f32 * s,
    }
}

unsafe extern "system" fn enum_monitors(
    monitor: HMONITOR,
    _hdc: HDC,
    _lprc: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let screens = &mut *(lparam.0 as *mut Vec<ScreenPx>);
    if let Some(screen) = screen_px_from_hmonitor(monitor) {
        screens.push(screen);
    }
    TRUE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_dip_enumerates_at_least_one() {
        assert!(!screens_dip().is_empty());
    }

    #[test]
    fn dip_to_px_is_dip_times_dpi_over_96() {
        let r = DipRect {
            x: 100.0,
            y: 80.0,
            w: 750.0,
            h: 64.0,
        };
        let px = dip_to_px_with_dpi(r, 96);
        assert_eq!(px.left, 100);
        assert_eq!(px.top, 80);
        assert_eq!(px.right, 850);
        assert_eq!(px.bottom, 144);

        let px = dip_to_px_with_dpi(r, 144);
        assert_eq!(px.left, 150);
        assert_eq!(px.top, 120);
        assert_eq!(px.right, 1275);
        assert_eq!(px.bottom, 216);
        assert_eq!(dip_scalar_to_px(26.0, 96), 26);
        assert_eq!(dip_scalar_to_px(26.0, 144), 39);
    }

    #[test]
    fn mixed_dpi_pixel_hit_picks_secondary_even_when_dip_frames_overlap() {
        let primary = ScreenPx {
            frame: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            work: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
            dpi: 96,
            origin_is_primary: true,
        };
        let secondary = ScreenPx {
            frame: RECT {
                left: 1920,
                top: 0,
                right: 3840,
                bottom: 1080,
            },
            work: RECT {
                left: 1920,
                top: 0,
                right: 3840,
                bottom: 1080,
            },
            dpi: 144,
            origin_is_primary: false,
        };
        let primary_dip = screen_dip_from_px(primary);
        let secondary_dip = screen_dip_from_px(secondary);
        assert!(secondary_dip.frame.x > 0.0 && secondary_dip.frame.x < primary_dip.frame.w);

        let picked = target_screen_from_cursor_px((2000, 10), &[primary, secondary]).unwrap();
        assert!(!picked.origin_is_primary);
        let picked_dip = screen_dip_from_px(picked);
        assert!((picked_dip.frame.x - 1280.0).abs() < 0.01);
        assert!((picked_dip.frame.w - 1280.0).abs() < 0.01);
        assert_eq!(picked.dpi, 144);

        let miss = target_screen_from_cursor_px((-10, -10), &[primary, secondary]).unwrap();
        assert!(miss.origin_is_primary);
    }
}
