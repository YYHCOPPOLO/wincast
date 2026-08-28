use tinycast_pure::palette_placement::{DipRect, ScreenDip};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, MONITORINFOF_PRIMARY};

pub fn screens_dip() -> Vec<ScreenDip> {
    let mut screens = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitors),
            LPARAM(&mut screens as *mut Vec<ScreenDip> as isize),
        );
    }
    screens
}

pub fn cursor_dip() -> (f32, f32) {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let dpi = monitor_dpi(MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST));
        let s = 96.0 / dpi as f32;
        (pt.x as f32 * s, pt.y as f32 * s)
    }
}

pub fn cursor_monitor_dpi() -> u32 {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        monitor_dpi(MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST))
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
    let screens = &mut *(lparam.0 as *mut Vec<ScreenDip>);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut info).as_bool() {
        let dpi = monitor_dpi(monitor);
        screens.push(ScreenDip {
            frame: rect_to_dip(info.rcMonitor, dpi),
            work: rect_to_dip(info.rcWork, dpi),
            origin_is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    TRUE
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
