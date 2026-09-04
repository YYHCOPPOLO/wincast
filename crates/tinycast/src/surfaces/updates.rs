//! Check for Updates: empty feed is a HUD, never a crash.

use tinycast_pure::update::{readiness, UpdateActivity, EMPTY_FEED_HUD};

use crate::features::updates::service::feed::{check_now, command_visible, FeedOutcome};

pub fn check_for_updates(activity: UpdateActivity) -> UpdateUi {
    if !command_visible() {
        return UpdateUi::Hidden;
    }
    if readiness(activity).is_some() {
        return UpdateUi::Deferred;
    }
    match check_now() {
        FeedOutcome::Empty => UpdateUi::Hud(EMPTY_FEED_HUD),
        FeedOutcome::Current => UpdateUi::Hud("You're up to date."),
        FeedOutcome::Available { version, notes, .. } => UpdateUi::Offer {
            version: version.to_string(),
            notes,
        },
        FeedOutcome::Failed(_) => UpdateUi::Hud("The update check failed."),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateUi {
    Hidden,
    Deferred,
    Hud(&'static str),
    Offer { version: String, notes: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_feed_is_hud() {
        let ui = check_for_updates(UpdateActivity::default());
        assert_eq!(ui, UpdateUi::Hud("No updates configured."));
    }

    #[test]
    fn defers_while_palette_is_open() {
        let ui = check_for_updates(UpdateActivity {
            palette_visible: true,
            ..UpdateActivity::default()
        });
        assert_eq!(ui, UpdateUi::Deferred);
    }
}
