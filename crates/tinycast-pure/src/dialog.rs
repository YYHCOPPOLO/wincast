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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DialogRole {
    Cancel,
    Standard,
    Destructive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DialogAction {
    pub label: &'static str,
    pub role: DialogRole,
}

/// Cancel always leads so Escape and the left-hand action agree.
pub fn ordered_actions(primary: &'static str, destructive: bool) -> Vec<DialogAction> {
    vec![
        DialogAction {
            label: "Cancel",
            role: DialogRole::Cancel,
        },
        DialogAction {
            label: primary,
            role: if destructive {
                DialogRole::Destructive
            } else {
                DialogRole::Standard
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{ordered_actions, DialogRole, DialogTone};
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

    #[test]
    fn ordered_actions_cancel_is_leading() {
        let buttons = ordered_actions("Restart", false);
        assert_eq!(buttons[0].label, "Cancel");
        assert_eq!(buttons[0].role, DialogRole::Cancel);
        assert_eq!(buttons[1].label, "Restart");
        assert_eq!(buttons[1].role, DialogRole::Standard);
        let destructive = ordered_actions("Uninstall", true);
        assert_eq!(destructive[0].label, "Cancel");
        assert_eq!(destructive[1].role, DialogRole::Destructive);
    }
}
