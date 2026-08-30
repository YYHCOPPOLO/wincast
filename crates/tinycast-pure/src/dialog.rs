/// How serious a dialog or HUD is; it tints the glyph, never picks one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DialogTone {
    Neutral,
    Success,
    Danger,
}

impl DialogTone {
    pub fn from_noop(noop: bool) -> Self {
        if noop {
            DialogTone::Neutral
        } else {
            DialogTone::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DialogTone;
    use crate::system_action::{Confirmation, SystemActionId};

    #[test]
    fn coordinator_cannot_skip_confirmation() {
        assert_ne!(
            SystemActionId::EmptyTrash.confirmation(),
            Confirmation::None
        );
    }

    #[test]
    fn noop_feedback_is_neutral() {
        assert_eq!(DialogTone::from_noop(true), DialogTone::Neutral);
        assert_eq!(DialogTone::from_noop(false), DialogTone::Success);
    }
}
