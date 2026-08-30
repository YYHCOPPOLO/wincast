use tinycast_pure::window_command::{WindowCommandId, WindowKind};
use windows::Win32::Foundation::HWND;

use crate::features::window_management::service::desktops;
use crate::features::window_management::service::mover::WindowMover;

pub const SPACE_UNAVAILABLE: &str = desktops::SPACE_UNAVAILABLE;

pub fn enabled_guard(window_management_enabled: bool) -> bool {
    window_management_enabled
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
