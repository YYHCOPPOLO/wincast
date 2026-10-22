use tinycast_pure::custom_command::CustomCommand;
use tinycast_pure::i18n::{chrome, Chrome, UiLang};
use windows::Win32::Foundation::HWND;

use crate::features::custom_commands::service::runner;
use crate::surfaces::dialog::{self, ConfirmPrompt};

pub const CONTINUE: &str = "Continue";
pub const CANCEL: &str = "Cancel";

pub enum RunRequest {
    Confirm,
    Execute,
}

pub fn request_run(command: &CustomCommand) -> RunRequest {
    if command.confirm {
        RunRequest::Confirm
    } else {
        RunRequest::Execute
    }
}

pub fn confirm_prompt(command: &CustomCommand) -> ConfirmPrompt {
    confirm_prompt_lang(command, UiLang::En)
}

pub fn confirm_prompt_lang(command: &CustomCommand, lang: UiLang) -> ConfirmPrompt {
    ConfirmPrompt {
        title: command.name.clone(),
        message: command.command.clone(),
        accept: tinycast_pure::i18n::onboarding_continue(lang).into(),
        cancel: chrome(Chrome::Cancel, lang).into(),
        // The user asked to confirm before this command runs: Cancel leads.
        destructive: true,
    }
}

/// Registers the runner call and shows the card. Returns whether it opened;
/// `on_accept` runs later, and only if the user accepted.
pub fn confirm(command: &CustomCommand, on_accept: impl FnOnce() + 'static) -> bool {
    confirm_lang(command, UiLang::En, on_accept)
}

pub fn confirm_lang(
    command: &CustomCommand,
    lang: UiLang,
    on_accept: impl FnOnce() + 'static,
) -> bool {
    dialog::confirm(&confirm_prompt_lang(command, lang), on_accept)
}

pub fn start(command: &CustomCommand, host: HWND) {
    runner::start(&command.command, host);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(confirm: bool) -> CustomCommand {
        CustomCommand {
            id: "1".into(),
            name: "Format Disk".into(),
            command: "format C:".into(),
            confirm,
        }
    }

    #[test]
    fn confirmation_gate_is_in_coordinator() {
        assert!(matches!(request_run(&cmd(true)), RunRequest::Confirm));
        assert!(matches!(request_run(&cmd(false)), RunRequest::Execute));
    }

    #[test]
    fn confirm_copy_includes_name_and_command() {
        let p = confirm_prompt(&cmd(true));
        assert_eq!(p.title, "Format Disk");
        assert_eq!(p.message, "format C:");
        assert_eq!(p.accept, "Continue");
        assert_eq!(p.cancel, "Cancel");
    }

    #[test]
    fn confirm_returns_false_when_a_dialog_is_already_up() {
        assert!(dialog::begin());
        assert!(
            !confirm(&cmd(true), || {}),
            "a stacked confirm must cancel rather than run"
        );
        dialog::end();
    }

    #[test]
    fn confirm_is_not_an_always_true_hud() {
        assert_ne!(CONTINUE, "");
        assert_ne!(CANCEL, "");
        assert!(dialog::begin());
        assert!(!confirm(&cmd(true), || {}));
        dialog::end();
    }
}
