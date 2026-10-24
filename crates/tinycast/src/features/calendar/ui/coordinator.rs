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

/// Registers the settings write and shows the consent card. Turning the
/// calendar on is not destructive, so the action keeps the default focus.
pub fn consent(lang: UiLang, on_accept: impl FnOnce() + 'static) -> bool {
    dialog::confirm(
        &ConfirmPrompt {
            title: tinycast_pure::i18n::calendar_consent_title(lang).into(),
            message: tinycast_pure::i18n::calendar_consent_message(lang).into(),
            accept: tinycast_pure::i18n::onboarding_continue(lang).into(),
            cancel: chrome(Chrome::Cancel, lang).into(),
            destructive: false,
        },
        on_accept,
    )
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

/// Camera preview card is the confirmation. Deny of the camera is not fatal;
/// a busy slot or a failed surface returns false and the caller joins
/// directly, like the old create failure did.
pub fn present_preview(
    enabled: bool,
    title: &str,
    lang: UiLang,
    on_join: Box<dyn FnOnce()>,
) -> bool {
    if !enabled {
        return false;
    }
    super::preview::present(title, lang, on_join)
}

pub fn create_event(owner: windows::Win32::Foundation::HWND, lang: UiLang) {
    // The event sheet is the Slint editor now; the appointment lands in its
    // accept continuation, and a cancelled sheet adds nothing.
    let request = crate::ui_host::editor::EditorRequest {
        title: "Create Event".into(),
        name_label: crate::features::custom_commands::ui::editor::field_acc_names(
            "Duration minutes",
            lang,
        )
        .0
        .to_string(),
        value_label: "Duration minutes".into(),
        confirm_label: String::new(),
        accept_label: tinycast_pure::i18n::editor_save(lang).to_string(),
        cancel_label: tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang)
            .to_string(),
        show_confirm: false,
        name_hidden: false,
        initial_name: "Meeting".into(),
        initial_value: "30".into(),
        initial_confirm: false,
    };
    let _ = crate::ui_host::editor::open(
        owner,
        request,
        Box::new(|draft| {
            let minutes: i64 = draft.command.trim().parse().unwrap_or(30).max(1);
            if show_add_appointment(&draft.name, minutes).is_err() {
                let _ = execute(&LaunchSpec::Uri("outlookcal:".into()));
            }
        }),
        Box::new(|| {}),
    );
}

fn show_add_appointment(title: &str, minutes: i64) -> windows::core::Result<()> {
    use windows::core::HSTRING;
    use windows::ApplicationModel::Appointments::{Appointment, AppointmentManager};
    use windows::Foundation::{DateTime, Rect, TimeSpan};
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
        assert!(!present_preview(
            false,
            "Standup",
            tinycast_pure::i18n::UiLang::En,
            Box::new(|| {})
        ));
    }
}
