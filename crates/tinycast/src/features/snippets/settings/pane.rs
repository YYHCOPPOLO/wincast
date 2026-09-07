//! Settings → Snippets: feature switch (keyword-expansion consent) and launcher visibility.

use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use crate::design_system::settings::{self as ds, RowTrailing};
use crate::features::launcher::settings::items::{ConfirmCopy, Formats};

pub const ENABLE_TITLE: &str = "Snippets";
pub const ENABLE_SUBTITLE: &str =
    "Keyword expansion requires listening to keystrokes. Keystrokes stay on this PC.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const ENABLE_CONFIRM_TITLE: &str = "Enable snippets?";
pub const ENABLE_CONFIRM_MESSAGE: &str =
    "Keyword expansion requires the Accessibility permission. Keystrokes stay on this PC.";
pub const ENABLE_CONFIRM_ACTION: &str = "Continue";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnippetsHit {
    Enable,
    ShowInLauncher,
}

pub fn section_header() -> &'static str {
    "Snippets"
}

pub fn enable_copy() -> ConfirmCopy {
    enable_copy_lang(tinycast_pure::i18n::UiLang::En)
}

pub fn enable_copy_lang(lang: tinycast_pure::i18n::UiLang) -> ConfirmCopy {
    ConfirmCopy {
        title: tinycast_pure::i18n::snippets_confirm_title(lang),
        message: tinycast_pure::i18n::snippets_confirm_message(lang),
        accept: tinycast_pure::i18n::snippets_confirm_action(lang),
        cancel: tinycast_pure::i18n::cancel_label(lang),
    }
}

pub fn content_height() -> f32 {
    let (section, _, _) =
        ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), true);
    section.next_y() + ds::CARD_INSET
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<SnippetsHit> {
    let (_, enable, show) =
        ds::feature_switch_section(420.0, ds::CARD_INSET - scroll, section_header(), true);
    if y >= enable.y && y < enable.y + enable.h {
        return Some(SnippetsHit::Enable);
    }
    if let Some(show) = show {
        if y >= show.y && y < show.y + show.h {
            return Some(SnippetsHit::ShowInLauncher);
        }
    }
    None
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    show_in_launcher: bool,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (section, enable, show) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Snippets,
            formats.lang,
        ),
        true,
    );
    ds::paint_grouped_section(
        target,
        formats.header,
        formats.caption,
        &section,
        appearance,
    )?;
    ds::paint_settings_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::snippets_enable_title(formats.lang),
        tinycast_pure::i18n::snippets_enable_subtitle(formats.lang),
        enable.y,
        width,
        enable.x + ds::CARD_PAD,
        true,
        appearance,
        RowTrailing::Toggle(enabled),
    )?;
    if let Some(show) = show {
        ds::paint_settings_row(
            target,
            formats.body,
            formats.caption,
            tinycast_pure::i18n::show_in_launcher(formats.lang),
            tinycast_pure::i18n::snippets_show_subtitle(formats.lang),
            show.y,
            width,
            show.x + ds::CARD_PAD,
            enabled,
            appearance,
            RowTrailing::Toggle(show_in_launcher),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_copy_matches_oracle_title() {
        assert_eq!(enable_copy().title, "Enable snippets?");
        let (_, enable, show) =
            ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), true);
        assert_eq!(hit(20.0, enable.y + 4.0, 0.0), Some(SnippetsHit::Enable));
        assert_eq!(
            hit(20.0, show.unwrap().y + 4.0, 0.0),
            Some(SnippetsHit::ShowInLauncher)
        );
    }
}
