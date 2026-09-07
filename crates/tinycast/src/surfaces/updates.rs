//! Check for Updates: empty feed is a HUD, never a crash.

use tinycast_pure::i18n::UiLang;
use tinycast_pure::update::{readiness, UpdateActivity};

use crate::features::updates::service::feed::{check_now, command_visible, FeedOutcome};

pub fn check_for_updates(activity: UpdateActivity, lang: UiLang) -> UpdateUi {
    if !command_visible() {
        return UpdateUi::Hidden;
    }
    if readiness(activity).is_some() {
        return UpdateUi::Deferred;
    }
    match check_now() {
        FeedOutcome::Empty => UpdateUi::Hud(tinycast_pure::i18n::update_none_configured(lang)),
        FeedOutcome::Current => UpdateUi::Hud(tinycast_pure::i18n::update_up_to_date(lang)),
        FeedOutcome::Available { version, notes, .. } => UpdateUi::Offer {
            version: version.to_string(),
            notes,
        },
        FeedOutcome::Failed(_) => UpdateUi::Hud(tinycast_pure::i18n::update_check_failed(lang)),
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
        let ui = check_for_updates(UpdateActivity::default(), UiLang::En);
        assert_eq!(
            ui,
            UpdateUi::Hud(tinycast_pure::i18n::update_none_configured(UiLang::En))
        );
    }

    #[test]
    fn defers_while_palette_is_open() {
        let ui = check_for_updates(
            UpdateActivity {
                palette_visible: true,
                ..UpdateActivity::default()
            },
            UiLang::En,
        );
        assert_eq!(ui, UpdateUi::Deferred);
    }
}
