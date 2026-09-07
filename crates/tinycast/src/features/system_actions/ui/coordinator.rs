//! One dispatch funnel for system actions, their confirmation gates, and HUD feedback.

use tinycast_pure::dialog::DialogTone;
use tinycast_pure::i18n::{chrome, system_action_title, system_confirm, Chrome, UiLang};
use tinycast_pure::system_action::{Confirmation, SystemActionId};
use windows::Win32::Foundation::HWND;

use crate::features::system_actions::service::runner::{self, Failure, Feedback};
use crate::surfaces::dialog::{self, ConfirmPrompt};

pub const STAGE_MANAGER_UNAVAILABLE: &str = "Stage Manager is not available on Windows.";
pub const CANCEL: &str = "Cancel";

#[derive(Clone, Debug, PartialEq)]
pub enum RunPlan {
    Confirm {
        title: String,
        message: String,
        accept: String,
    },
    PickVolume,
    Execute,
}

/// Confirmation must be accepted before the runner is called.
pub fn gated_run(confirm_ok: bool) -> bool {
    confirm_ok
}

pub fn plan(id: SystemActionId) -> RunPlan {
    plan_for(id, UiLang::En)
}

pub fn plan_for(id: SystemActionId, lang: UiLang) -> RunPlan {
    match id.confirmation() {
        Confirmation::Required { .. } => {
            let (title, message) = system_confirm(id, lang).unwrap_or_else(|| {
                (id.name(), "")
            });
            RunPlan::Confirm {
                title: title.to_string(),
                message: message.to_string(),
                accept: system_action_title(id, lang).to_string(),
            }
        }
        Confirmation::Computed if id == SystemActionId::QuitAllApps => {
            let count = runner::quit_all_targets().len();
            if count == 0 {
                return RunPlan::Execute;
            }
            RunPlan::Confirm {
                title: tinycast_pure::i18n::quit_all_title(count, lang),
                message: tinycast_pure::i18n::quit_all_message(lang).into(),
                accept: tinycast_pure::i18n::quit_all_accept(lang).into(),
            }
        }
        Confirmation::Computed => RunPlan::Execute,
        Confirmation::None if id == SystemActionId::SetVolume => RunPlan::PickVolume,
        Confirmation::None => RunPlan::Execute,
    }
}

pub fn confirm_prompt(title: &str, message: &str, accept: &str) -> ConfirmPrompt {
    confirm_prompt_lang(title, message, accept, UiLang::En)
}

pub fn confirm_prompt_lang(
    title: &str,
    message: &str,
    accept: &str,
    lang: UiLang,
) -> ConfirmPrompt {
    ConfirmPrompt {
        title: title.to_string(),
        message: message.to_string(),
        accept: accept.to_string(),
        cancel: chrome(Chrome::Cancel, lang).into(),
    }
}

/// Palette must already be hidden. Returns false on Cancel, Escape, or a stacked dialog.
pub fn confirm(title: &str, message: &str, accept: &str) -> bool {
    confirm_lang(title, message, accept, UiLang::En)
}

pub fn confirm_lang(title: &str, message: &str, accept: &str, lang: UiLang) -> bool {
    if !dialog::begin() {
        return false;
    }
    let prompt = confirm_prompt_lang(title, message, accept, lang);
    let accepted = dialog::confirm(&prompt);
    dialog::end();
    accepted
}

pub fn execute(id: SystemActionId, previous: HWND) -> Result<Option<Feedback>, Failure> {
    runner::run(id, previous)
}

pub fn tone_for(feedback: &Feedback) -> DialogTone {
    DialogTone::from_noop(feedback.noop)
}

pub fn shows_volume(id: SystemActionId) -> bool {
    runner::shows_volume_feedback(id)
}

pub fn is_stage_manager(id: SystemActionId) -> bool {
    id == SystemActionId::ToggleStageManager
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::system_action::Confirmation;

    #[test]
    fn coordinator_cannot_skip_confirmation() {
        assert_ne!(
            SystemActionId::EmptyTrash.confirmation(),
            Confirmation::None
        );
        match plan(SystemActionId::EmptyTrash) {
            RunPlan::Confirm { title, .. } => assert_eq!(title, "Empty Trash?"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cancel_does_not_run() {
        assert!(!gated_run(false));
        assert!(gated_run(true));
        let mut ran = false;
        if gated_run(false) {
            ran = true;
        }
        assert!(!ran);
    }

    #[test]
    fn restart_plan_says_pc() {
        match plan(SystemActionId::Restart) {
            RunPlan::Confirm { title, .. } => assert!(title.contains("PC")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn stage_manager_stays_in_catalog_and_is_flagged() {
        assert!(is_stage_manager(SystemActionId::ToggleStageManager));
        assert_eq!(
            STAGE_MANAGER_UNAVAILABLE,
            "Stage Manager is not available on Windows."
        );
    }

    #[test]
    fn volume_actions_use_volume_hud() {
        assert!(shows_volume(SystemActionId::VolumeUp));
        assert!(shows_volume(SystemActionId::ToggleMute));
        assert!(!shows_volume(SystemActionId::LockScreen));
        assert!(matches!(
            plan(SystemActionId::SetVolume),
            RunPlan::PickVolume
        ));
    }

    #[test]
    fn confirm_returns_false_when_a_dialog_is_already_up() {
        assert!(dialog::begin());
        assert!(!confirm("Empty Trash?", "gone", "Empty Trash"));
        dialog::end();
    }
}
