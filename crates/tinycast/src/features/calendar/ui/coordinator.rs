use tinycast_pure::meeting::{MeetingEvent, MeetingLink, UpcomingWindow};
use tinycast_pure::palette_mode::PaletteMode;

use crate::features::launcher::ui::coordinator::{execute, LaunchSpec};
use crate::surfaces::dialog::{self, ConfirmPrompt};

pub const CONSENT_TITLE: &str = "Turn on Calendar?";
pub const CONSENT_MESSAGE: &str =
    "Tinycast will read your calendar to show upcoming meetings and join links.";
pub const CONSENT_ACTION: &str = "Continue";
pub const NOTHING_TO_JOIN: &str = "Nothing to join right now";

pub fn consent() -> bool {
    dialog::confirm(&ConfirmPrompt {
        title: CONSENT_TITLE.into(),
        message: CONSENT_MESSAGE.into(),
        accept: CONSENT_ACTION.into(),
        cancel: "Cancel".into(),
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

/// Camera preview is optional. Deny is not fatal — joining still proceeds.
pub fn camera_preview_optional(enabled: bool) -> bool {
    if !enabled {
        return true;
    }
    try_camera()
}

fn try_camera() -> bool {
    match windows::Media::Capture::MediaCapture::new() {
        Ok(_cap) => true,
        Err(_) => true,
    }
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
        assert!(camera_preview_optional(false));
        assert!(camera_preview_optional(true));
    }
}
