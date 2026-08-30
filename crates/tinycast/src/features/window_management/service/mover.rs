//! Apply placements with SetWindowPos. DIP math stays in tinycast-pure; pixels only here.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::window_action_memory::WindowActionMemory;
use tinycast_pure::window_command::{WindowCommandId, WindowKind};
use tinycast_pure::window_layout::{
    placement_for, screen_containing, LayoutQuery, Placement, Screen,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, GetWindowRect, IsIconic, IsWindow, IsWindowVisible, IsZoomed, SetWindowPos,
    ShowWindow, GWL_STYLE, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SW_RESTORE, WINDOW_STYLE, WS_SIZEBOX, WS_THICKFRAME,
};

use crate::platform::screens::{dip_to_px_with_dpi, screens_px, ScreenPx};

pub struct WindowMover {
    memory: WindowActionMemory<u64>,
}

impl WindowMover {
    pub fn new() -> Self {
        Self {
            memory: WindowActionMemory::new(),
        }
    }

    pub fn perform(
        &mut self,
        hwnd: HWND,
        command: WindowCommandId,
        gap: f32,
        cycle: bool,
    ) -> bool {
        if hwnd.is_invalid() || !unsafe { IsWindow(hwnd).as_bool() } {
            return false;
        }
        if command.kind() == WindowKind::Space {
            return false;
        }
        if command.kind() == WindowKind::Fullscreen {
            return toggle_fullscreen(hwnd);
        }
        if unsafe { IsIconic(hwnd).as_bool() } {
            return false;
        }
        if unsafe { IsZoomed(hwnd).as_bool() } {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
        }
        let Some(current_px) = window_bounds(hwnd) else {
            return false;
        };
        let screens_px = screens_px();
        if screens_px.is_empty() {
            return false;
        }
        let dip_screens: Vec<Screen> = screens_px
            .iter()
            .enumerate()
            .map(|(i, s)| Screen {
                id: i as i32,
                frame: px_to_dip(s.frame, s.dpi),
                visible: px_to_dip(s.work, s.dpi),
            })
            .collect();
        let host_px = host_screen(hwnd, &screens_px);
        let window_dip = px_to_dip(current_px, host_px.dpi);
        let key = hwnd.0 as usize as u64;
        let host = screen_containing(window_dip, &dip_screens);
        let screen_id = host.map(|s| s.id).unwrap_or(0);
        let now = unix_ms();
        if command == WindowCommandId::Restore {
            let decision = self
                .memory
                .decide(&key, command, window_dip, screen_id, cycle, now);
            if !decision.can_restore {
                return false;
            }
            let Some(placement) = placement_for(LayoutQuery {
                command,
                window: window_dip,
                screens: &dip_screens,
                gap,
                step: 0,
                restore: Some(decision.restore),
                last_tile: decision.last_tile,
            }) else {
                return false;
            };
            return self.apply(hwnd, command, placement, decision, &screens_px, now);
        }
        let decision = self
            .memory
            .decide(&key, command, window_dip, screen_id, cycle, now);
        let Some(placement) = placement_for(LayoutQuery {
            command,
            window: window_dip,
            screens: &dip_screens,
            gap,
            step: decision.step,
            restore: Some(decision.restore),
            last_tile: decision.last_tile,
        }) else {
            return false;
        };
        self.apply(hwnd, command, placement, decision, &screens_px, now)
    }

    fn apply(
        &mut self,
        hwnd: HWND,
        command: WindowCommandId,
        placement: Placement,
        decision: tinycast_pure::window_action_memory::Decision,
        screens_px: &[ScreenPx],
        now: i64,
    ) -> bool {
        let dest = screens_px
            .get(placement.screen_id as usize)
            .copied()
            .or_else(|| screens_px.first().copied());
        let Some(dest) = dest else {
            return false;
        };
        let target = dip_to_px_with_dpi(placement.frame, dest.dpi);
        let original = match window_bounds(hwnd) {
            Some(r) => r,
            None => return false,
        };
        let can_pos = position_settable(hwnd);
        if !can_pos {
            return false;
        }
        let can_size = placement.resizes && size_settable(hwnd);
        let write = if can_size {
            target
        } else {
            let size = DipRect {
                x: 0.0,
                y: 0.0,
                w: (original.right - original.left) as f32,
                h: (original.bottom - original.top) as f32,
            };
            let slot = px_to_dip(target, dest.dpi);
            let placed = placement.anchor.place(size.w, size.h, slot);
            dip_to_px_with_dpi(placed, dest.dpi)
        };
        if can_size {
            if set_rect(hwnd, write, true, false).is_err() {
                return false;
            }
        }
        if set_rect(hwnd, write, false, true).is_err() {
            if can_size {
                let _ = set_rect(hwnd, original, true, false);
            }
            return false;
        }
        if can_size {
            let _ = set_rect(hwnd, write, true, false);
        }
        let applied_px = window_bounds(hwnd).unwrap_or(write);
        let applied = px_to_dip(applied_px, dest.dpi);
        self.memory.commit(
            hwnd.0 as usize as u64,
            command,
            decision,
            applied,
            placement.screen_id,
            now,
        );
        true
    }
}

fn toggle_fullscreen(hwnd: HWND) -> bool {
    unsafe {
        if IsZoomed(hwnd).as_bool() {
            ShowWindow(hwnd, SW_RESTORE).as_bool()
        } else {
            ShowWindow(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE,
            )
            .as_bool()
        }
    }
}

fn position_settable(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
}

fn size_settable(hwnd: HWND) -> bool {
    unsafe {
        let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
        style.contains(WS_THICKFRAME) || style.contains(WS_SIZEBOX)
    }
}

fn set_rect(hwnd: HWND, r: RECT, size: bool, pos: bool) -> windows::core::Result<()> {
    let mut flags = SWP_NOZORDER | SWP_NOACTIVATE;
    if !size {
        flags |= SWP_NOSIZE;
    }
    if !pos {
        flags |= SWP_NOMOVE;
    }
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOP,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            flags,
        )
    }
}

fn window_bounds(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let mut ext = RECT::default();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut ext as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            && ext.right > ext.left
        {
            return Some(ext);
        }
        let mut rc = RECT::default();
        GetWindowRect(hwnd, &mut rc).ok()?;
        Some(rc)
    }
}

fn host_screen(hwnd: HWND, screens: &[ScreenPx]) -> ScreenPx {
    let _ = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let bounds = window_bounds(hwnd);
    screens
        .iter()
        .copied()
        .find(|s| bounds.map(|b| contains(s.frame, center(b))).unwrap_or(false))
        .or_else(|| screens.iter().copied().find(|s| s.origin_is_primary))
        .or_else(|| screens.first().copied())
        .unwrap_or(ScreenPx {
            frame: RECT::default(),
            work: RECT::default(),
            dpi: 96,
            origin_is_primary: true,
        })
}

fn center(r: RECT) -> (i32, i32) {
    (
        r.left + (r.right - r.left) / 2,
        r.top + (r.bottom - r.top) / 2,
    )
}

fn contains(r: RECT, p: (i32, i32)) -> bool {
    p.0 >= r.left && p.0 < r.right && p.1 >= r.top && p.1 < r.bottom
}

fn px_to_dip(r: RECT, dpi: u32) -> DipRect {
    let s = 96.0 / (if dpi == 0 { 96.0 } else { dpi as f32 });
    DipRect {
        x: r.left as f32 * s,
        y: r.top as f32 * s,
        w: (r.right - r.left) as f32 * s,
        h: (r.bottom - r.top) as f32 * s,
    }
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::position_settable;
    use windows::Win32::Foundation::HWND;

    #[test]
    fn invalid_hwnd_is_not_settable() {
        assert!(!position_settable(HWND::default()));
    }
}
