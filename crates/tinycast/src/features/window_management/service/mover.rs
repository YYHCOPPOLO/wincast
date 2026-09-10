//! Apply placements with SetWindowPos. DIP math stays in tinycast-pure; pixels only here.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::window_action_memory::WindowActionMemory;
use tinycast_pure::window_command::{WindowCommandId, WindowKind};
use tinycast_pure::window_layout::{
    placement_for, screen_containing, Anchor, LayoutQuery, Placement, Screen,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
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

    pub fn perform(&mut self, hwnd: HWND, command: WindowCommandId, gap: f32, cycle: bool) -> bool {
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
        let Some(frames) = window_frames(hwnd) else {
            return false;
        };
        let screens_px = screens_px();
        if screens_px.is_empty() {
            return false;
        }
        let dpi = primary_dpi(&screens_px);
        let dip_screens = screens_to_dip(&screens_px, dpi);
        let window_dip = px_to_dip(frames.visible, dpi);
        let key = hwnd.0 as usize as u64;
        let host = screen_containing(window_dip, &dip_screens);
        let screen_id = host.map(|s| s.id).unwrap_or(0);
        let now = unix_ms();
        let decision = self
            .memory
            .decide(&key, command, window_dip, screen_id, cycle, now);
        if command == WindowCommandId::Restore && !decision.can_restore {
            return false;
        }
        let Some(placement) = placement_for(LayoutQuery {
            command,
            window: window_dip,
            screens: &dip_screens,
            gap,
            step: if command == WindowCommandId::Restore {
                0
            } else {
                decision.step
            },
            restore: Some(decision.restore),
            last_tile: decision.last_tile,
        }) else {
            return false;
        };
        self.apply(hwnd, command, placement, decision, &screens_px, dpi, now)
    }

    fn apply(
        &mut self,
        hwnd: HWND,
        command: WindowCommandId,
        placement: Placement,
        decision: tinycast_pure::window_action_memory::Decision,
        screens_px: &[ScreenPx],
        dpi: u32,
        now: i64,
    ) -> bool {
        let dest = screens_px
            .get(placement.screen_id as usize)
            .copied()
            .or_else(|| screens_px.first().copied());
        let Some(_dest) = dest else {
            return false;
        };
        let target_visible = dip_to_px_with_dpi(placement.frame, dpi);
        let Some(frames) = window_frames(hwnd) else {
            return false;
        };
        let can_pos = position_settable(hwnd);
        if !can_pos {
            return false;
        }
        let can_size = placement.resizes && size_settable(hwnd);
        let write_visible = if can_size {
            target_visible
        } else {
            place_pixel_size(frames.visible, target_visible, placement.anchor)
        };
        let write = apply_offset(write_visible, frames.offset);
        let original = frames.window;
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
        let applied_px = window_frames(hwnd)
            .map(|f| f.visible)
            .unwrap_or(write_visible);
        let applied = px_to_dip(applied_px, dpi);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameOffset {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

struct WindowFrames {
    visible: RECT,
    window: RECT,
    offset: FrameOffset,
}

fn window_frames(hwnd: HWND) -> Option<WindowFrames> {
    unsafe {
        let mut window = RECT::default();
        GetWindowRect(hwnd, &mut window).ok()?;
        let mut visible = RECT::default();
        let dwm_ok = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut visible as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            && visible.right > visible.left;
        if !dwm_ok {
            visible = window;
        }
        Some(WindowFrames {
            offset: frame_offset(window, visible),
            visible,
            window,
        })
    }
}

fn frame_offset(window: RECT, visible: RECT) -> FrameOffset {
    FrameOffset {
        left: window.left - visible.left,
        top: window.top - visible.top,
        right: window.right - visible.right,
        bottom: window.bottom - visible.bottom,
    }
}

fn apply_offset(visible: RECT, offset: FrameOffset) -> RECT {
    RECT {
        left: visible.left + offset.left,
        top: visible.top + offset.top,
        right: visible.right + offset.right,
        bottom: visible.bottom + offset.bottom,
    }
}

fn place_pixel_size(current_visible: RECT, slot: RECT, anchor: Anchor) -> RECT {
    let w = (current_visible.right - current_visible.left) as f32;
    let h = (current_visible.bottom - current_visible.top) as f32;
    let slot_dip = DipRect {
        x: slot.left as f32,
        y: slot.top as f32,
        w: (slot.right - slot.left) as f32,
        h: (slot.bottom - slot.top) as f32,
    };
    let placed = anchor.place(w, h, slot_dip);
    RECT {
        left: placed.x.round() as i32,
        top: placed.y.round() as i32,
        right: (placed.x + placed.w).round() as i32,
        bottom: (placed.y + placed.h).round() as i32,
    }
}

fn primary_dpi(screens: &[ScreenPx]) -> u32 {
    screens
        .iter()
        .find(|s| s.origin_is_primary)
        .or_else(|| screens.first())
        .map(|s| if s.dpi == 0 { 96 } else { s.dpi })
        .unwrap_or(96)
}

fn screens_to_dip(screens: &[ScreenPx], dpi: u32) -> Vec<Screen> {
    screens
        .iter()
        .enumerate()
        .map(|(i, s)| Screen {
            id: i as i32,
            frame: px_to_dip(s.frame, dpi),
            visible: px_to_dip(s.work, dpi),
        })
        .collect()
}

fn toggle_fullscreen(hwnd: HWND) -> bool {
    unsafe {
        if IsZoomed(hwnd).as_bool() {
            ShowWindow(hwnd, SW_RESTORE).as_bool()
        } else {
            ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE).as_bool()
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
    use super::*;
    use windows::Win32::Foundation::HWND;

    fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
        RECT {
            left: l,
            top: t,
            right: r,
            bottom: b,
        }
    }

    #[test]
    fn invalid_hwnd_is_not_settable() {
        assert!(!position_settable(HWND::default()));
    }

    #[test]
    fn dwm_offset_is_applied_to_setwindowpos_rect() {
        let visible = rect(100, 100, 500, 400);
        let window = rect(93, 100, 507, 407);
        let off = frame_offset(window, visible);
        assert_eq!(off.left, -7);
        assert_eq!(off.right, 7);
        assert_eq!(off.bottom, 7);
        let target = rect(0, 0, 960, 1040);
        let write = apply_offset(target, off);
        assert_eq!(write.left, -7);
        assert_eq!(write.top, 0);
        assert_eq!(write.right, 967);
        assert_eq!(write.bottom, 1047);
    }

    #[test]
    fn mixed_dpi_screens_do_not_overlap_in_dip() {
        let screens = [
            ScreenPx {
                frame: rect(0, 0, 1920, 1080),
                work: rect(0, 0, 1920, 1040),
                dpi: 96,
                origin_is_primary: true,
            },
            ScreenPx {
                frame: rect(1920, 0, 3840, 1080),
                work: rect(1920, 0, 3840, 1080),
                dpi: 144,
                origin_is_primary: false,
            },
        ];
        let dpi = primary_dpi(&screens);
        assert_eq!(dpi, 96);
        let dip = screens_to_dip(&screens, dpi);
        assert!((dip[0].frame.w - 1920.0).abs() < 0.01);
        assert!((dip[1].frame.x - 1920.0).abs() < 0.01);
        assert!((dip[1].frame.w - 1920.0).abs() < 0.01);
        assert!(dip[1].frame.x >= dip[0].frame.x + dip[0].frame.w - 0.01);
    }

    #[test]
    fn size_not_settable_places_pixel_size_in_pixel_slot() {
        let current = rect(10, 10, 810, 610);
        let slot = rect(0, 0, 1000, 800);
        let placed = place_pixel_size(current, slot, Anchor::CENTERED);
        assert_eq!(placed.right - placed.left, 800);
        assert_eq!(placed.bottom - placed.top, 600);
        assert_eq!(placed.left, 100);
        assert_eq!(placed.top, 100);
    }
}
