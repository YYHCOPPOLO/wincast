use tinycast_pure::window_command::{WindowCommandId, WindowKind};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::features::window_management::service::desktops;
use crate::features::window_management::service::mover::WindowMover;

pub const SPACE_UNAVAILABLE: &str = desktops::SPACE_UNAVAILABLE;

pub fn enabled_guard(window_management_enabled: bool) -> bool {
    window_management_enabled
}

/// Palette path keeps `previous`. Hotkeys with the palette hidden use the foreground window.
pub fn resolve_action_target(
    palette_visible: bool,
    previous: HWND,
    foreground: HWND,
    foreground_is_ours: bool,
) -> HWND {
    if palette_visible {
        return previous;
    }
    if foreground.is_invalid() || foreground_is_ours {
        return previous;
    }
    foreground
}

pub fn pid_belongs_to(window_pid: u32, our_pid: u32) -> bool {
    window_pid != 0 && window_pid == our_pid
}

/// Skip Tinycast host/palette/settings/HUD/dialog windows (same process).
pub fn hwnd_is_ours(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    pid_belongs_to(pid, unsafe { GetCurrentProcessId() })
}

pub fn run(
    mover: &mut WindowMover,
    id: WindowCommandId,
    target: HWND,
    enabled: bool,
    gap: f32,
    cycle: bool,
) -> Outcome {
    if !enabled_guard(enabled) {
        return Outcome::Disabled;
    }
    if id.kind() == WindowKind::Space {
        let _ = desktops::switch_space(target, id == WindowCommandId::NextSpace);
        return Outcome::Hud(SPACE_UNAVAILABLE);
    }
    if target.is_invalid() {
        return Outcome::Quiet;
    }
    if mover.perform(target, id, gap, cycle) {
        Outcome::Moved
    } else {
        Outcome::Quiet
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Disabled,
    Quiet,
    Moved,
    Hud(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::window_command::WindowKind;

    #[test]
    fn space_commands_are_not_geometry() {
        assert_eq!(WindowCommandId::NextSpace.kind(), WindowKind::Space);
        assert_eq!(WindowCommandId::Restore.kind(), WindowKind::Restore);
    }

    #[test]
    fn hidden_palette_uses_foreground_not_previous() {
        let previous = HWND(1 as *mut core::ffi::c_void);
        let foreground = HWND(2 as *mut core::ffi::c_void);
        assert_eq!(
            resolve_action_target(true, previous, foreground, false).0,
            previous.0
        );
        assert_eq!(
            resolve_action_target(false, previous, foreground, false).0,
            foreground.0
        );
        assert_eq!(
            resolve_action_target(false, previous, foreground, true).0,
            previous.0
        );
        assert_eq!(
            resolve_action_target(false, previous, HWND::default(), false).0,
            previous.0
        );
    }

    #[test]
    fn same_pid_windows_are_ours_invalid_are_not() {
        assert!(!pid_belongs_to(0, 4242));
        assert!(!pid_belongs_to(7, 4242));
        assert!(pid_belongs_to(4242, 4242));
        assert!(!hwnd_is_ours(HWND::default()));
    }

    #[test]
    fn coordinator_rechecks_enable_switch() {
        let mut mover = WindowMover::new();
        assert_eq!(
            run(
                &mut mover,
                WindowCommandId::LeftHalf,
                HWND::default(),
                false,
                0.0,
                false,
            ),
            Outcome::Disabled
        );
        assert_eq!(
            run(
                &mut mover,
                WindowCommandId::NextSpace,
                HWND::default(),
                true,
                0.0,
                false,
            ),
            Outcome::Hud(SPACE_UNAVAILABLE)
        );
    }
}
