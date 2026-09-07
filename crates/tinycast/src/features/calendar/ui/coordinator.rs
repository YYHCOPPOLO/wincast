use tinycast_pure::i18n::{chrome, Chrome, UiLang};
use tinycast_pure::meeting::{MeetingEvent, MeetingLink, UpcomingWindow};
use tinycast_pure::palette_mode::PaletteMode;

use crate::features::launcher::ui::coordinator::{execute, LaunchSpec};
use crate::surfaces::dialog::{self, ConfirmPrompt};

pub const CONSENT_TITLE: &str = "Turn on Calendar?";
pub const CONSENT_MESSAGE: &str =
    "Tinycast will read your calendar to show upcoming meetings and join links.";
pub const CONSENT_ACTION: &str = "Continue";
pub const NOTHING_TO_JOIN: &str = "Nothing to join right now";

pub fn consent(lang: UiLang) -> bool {
    dialog::confirm(&ConfirmPrompt {
        title: tinycast_pure::i18n::calendar_consent_title(lang).into(),
        message: tinycast_pure::i18n::calendar_consent_message(lang).into(),
        accept: tinycast_pure::i18n::onboarding_continue(lang).into(),
        cancel: chrome(Chrome::Cancel, lang).into(),
    })
}

pub fn window(lead_minutes: i64) -> UpcomingWindow {
    UpcomingWindow { lead_minutes }
}

pub fn join_url(event: &MeetingEvent) -> Option<String> {
    let link = event.link.as_ref()?;
    Some(link.app_url().unwrap_or_else(|| link.url.clone()))
}

pub fn copy_url(event: &MeetingEvent) -> Option<String> {
    event.link.as_ref().map(|l| l.url.clone())
}

pub fn open_link(link: &MeetingLink) -> windows::core::Result<()> {
    let url = link.app_url().unwrap_or_else(|| link.url.clone());
    execute(&LaunchSpec::Uri(url)).or_else(|_| execute(&LaunchSpec::Uri(link.url.clone())))
}

pub fn should_show_card(mode: PaletteMode, query: &str, enabled: bool) -> bool {
    enabled && mode == PaletteMode::Launcher && query.is_empty()
}

/// Camera preview HWND is the confirmation. Deny of the camera is not fatal.
pub fn camera_preview_optional(enabled: bool, title: &str, lang: UiLang) -> bool {
    if !enabled {
        return true;
    }
    super::preview::present(title, lang)
}

pub fn create_event(owner: windows::Win32::Foundation::HWND, lang: UiLang) {
    use crate::features::custom_commands::ui::editor::{edit_with, CommandDraft, EditorLabels};
    let Some(draft) = edit_with(
        owner,
        Some(&CommandDraft {
            name: "Meeting".into(),
            command: "30".into(),
            confirm: false,
        }),
        EditorLabels {
            title: "Create Event",
            value_label: "Duration minutes",
            show_confirm: false,
        },
        lang,
    ) else {
        return;
    };
    let minutes: i64 = draft.command.trim().parse().unwrap_or(30).max(1);
    if show_add_appointment(&draft.name, minutes).is_err() {
        let _ = execute(&LaunchSpec::Uri("outlookcal:".into()));
    }
}

fn show_add_appointment(title: &str, minutes: i64) -> windows::core::Result<()> {
    use windows::ApplicationModel::Appointments::{Appointment, AppointmentManager};
    use windows::Foundation::{DateTime, Rect, TimeSpan};
    use windows::core::HSTRING;
    let appt = Appointment::new()?;
    appt.SetSubject(&HSTRING::from(title))?;
    let now = crate::platform::clock::unix_now();
    appt.SetStartTime(DateTime {
        UniversalTime: (now + 11_644_473_600) * 10_000_000,
    })?;
    appt.SetDuration(TimeSpan {
        Duration: minutes.saturating_mul(60) * 10_000_000,
    })?;
    let rect = Rect {
        X: 200.0,
        Y: 160.0,
        Width: 320.0,
        Height: 240.0,
    };
    let _ = AppointmentManager::ShowAddAppointmentAsync(&appt, rect)?.get()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::meeting::Provider;

    #[test]
    fn card_only_on_empty_launcher_query() {
        assert!(should_show_card(PaletteMode::Launcher, "", true));
        assert!(!should_show_card(PaletteMode::Launcher, "meet", true));
        assert!(!should_show_card(PaletteMode::Clipboard, "", true));
        assert!(!should_show_card(PaletteMode::Launcher, "", false));
    }

    #[test]
    fn join_prefers_app_url() {
        let event = MeetingEvent {
            id: "1".into(),
            title: "Z".into(),
            start: 0,
            end: 1,
            is_all_day: false,
            is_declined: false,
            calendar_id: String::new(),
            calendar_name: String::new(),
            calendar_item_id: String::new(),
            link: Some(MeetingLink {
                provider: Provider::Zoom,
                url: "https://zoom.us/j/123".into(),
            }),
        };
        let url = join_url(&event).unwrap();
        assert!(url.starts_with("zoommtg:"));
    }

    #[test]
    fn camera_deny_is_not_fatal() {
        assert!(crate::features::calendar::ui::preview::probe_camera_nonfatal());
        assert!(camera_preview_optional(
            false,
            "Standup",
            tinycast_pure::i18n::UiLang::En
        ));
    }
}
