/// 20 steps of 5%, so every level is round and the presets sit on the grid.
pub const STEPS: u32 = 20;
const TOLERANCE: f32 = 1e-6;

pub fn clamped(level: f32) -> f32 {
    level.clamp(0.0, 1.0)
}

/// To the next grid line, not past it; a level already on the grid moves a full step.
pub fn volume_step(current: f32, up: bool) -> f32 {
    let exact = clamped(current) * STEPS as f32;
    let line = if up {
        (exact + TOLERANCE).floor() + 1.0
    } else {
        (exact - TOLERANCE).ceil() - 1.0
    };
    clamped(line / STEPS as f32)
}

pub fn percentage(level: f32) -> String {
    format!("{}%", (clamped(level) * 100.0).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_grid_snaps() {
        assert!((volume_step(0.37, true) - 0.40).abs() < 1e-6);
        assert!((volume_step(0.37, false) - 0.35).abs() < 1e-6);
    }

    #[test]
    fn on_grid_moves_a_full_step() {
        assert!((volume_step(0.40, true) - 0.45).abs() < 1e-6);
        assert!((volume_step(0.40, false) - 0.35).abs() < 1e-6);
        assert!((volume_step(0.0, false) - 0.0).abs() < 1e-6);
        assert!((volume_step(1.0, true) - 1.0).abs() < 1e-6);
        assert_eq!(percentage(0.37), "37%");
        assert_eq!(percentage(1.0), "100%");
    }
}
