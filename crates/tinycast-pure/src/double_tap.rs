use crate::hotkey::DoubleTapModifier;

const MAX_HOLD_MS: u64 = 250;
const MAX_GAP_MS: u64 = 300;

/// Recognizes a double-tapped lone modifier. Fires on the second **release**.
#[derive(Clone, Debug, Default)]
pub struct DoubleTapDetector {
    press: Option<(DoubleTapModifier, u64)>,
    pending: Option<(DoubleTapModifier, u64)>,
    held: bool,
}

impl DoubleTapDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// `down` is the edge for `modifier`. Other modifiers must be up for a tap.
    pub fn on_flags(&mut self, modifier: DoubleTapModifier, down: bool, now_ms: u64) -> bool {
        if down {
            if self.held {
                self.invalidate();
                return false;
            }
            self.held = true;
            self.press = Some((modifier, now_ms));
            false
        } else {
            self.held = false;
            let Some((pressed, started)) = self.press.take() else {
                self.invalidate();
                return false;
            };
            if pressed != modifier || now_ms.saturating_sub(started) > MAX_HOLD_MS {
                self.invalidate();
                return false;
            }
            if let Some((pending_mod, released_at)) = self.pending {
                if pending_mod == modifier && started.saturating_sub(released_at) <= MAX_GAP_MS {
                    self.pending = None;
                    return true;
                }
            }
            self.pending = Some((modifier, now_ms));
            false
        }
    }

    pub fn reset(&mut self) {
        self.held = false;
        self.invalidate();
    }

    fn invalidate(&mut self) {
        self.press = None;
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_tap_fires_on_second_release() {
        let mut d = DoubleTapDetector::new();
        let m = DoubleTapModifier::Control;
        assert!(!d.on_flags(m, true, 0));
        assert!(!d.on_flags(m, false, 100));
        assert!(!d.on_flags(m, true, 200));
        assert!(d.on_flags(m, false, 300));
    }

    #[test]
    fn slow_hold_is_not_a_tap() {
        let mut d = DoubleTapDetector::new();
        let m = DoubleTapModifier::Shift;
        assert!(!d.on_flags(m, true, 0));
        assert!(!d.on_flags(m, false, 300));
        assert!(!d.on_flags(m, true, 350));
        assert!(!d.on_flags(m, false, 400));
    }
}
