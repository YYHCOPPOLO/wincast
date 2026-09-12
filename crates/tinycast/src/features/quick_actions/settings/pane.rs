//! Settings → Quick Actions. Off by default; excluded from backups.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickActionsHit {
    Enable,
    Language,
}

pub fn section_header() -> &'static str {
    "Quick Actions"
}

pub fn content_height() -> f32 {
    ds::form_origin() + ROW_H * 2.0 + ds::CARD_PAD + theme::spacing::SECTION_SPACING
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<QuickActionsHit> {
    let y = y + scroll;
    let origin = ds::form_origin();
    if y >= origin && y < origin + ROW_H {
        Some(QuickActionsHit::Enable)
    } else if y >= origin + ROW_H && y < origin + ROW_H * 2.0 {
        Some(QuickActionsHit::Language)
    } else {
        None
    }
}

pub fn cycle_language(current: &str) -> &'static str {
    match current {
        "Spanish" => "French",
        "French" => "German",
        "German" => "English",
        _ => "Spanish",
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    language: &str,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (section, _, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::QuickActions,
            formats.lang,
        ),
        false,
    );
    let mut section = section;
    section.body_h = ds::CARD_PAD * 2.0 + ROW_H * 2.0;
    ds::paint_grouped_section(
        target,
        formats.header,
        formats.caption,
        &section,
        appearance,
    )?;
    let y0 = ds::form_origin() - scroll;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::qa_enable_title(formats.lang),
        tinycast_pure::i18n::qa_enable_subtitle(formats.lang),
        y0,
        width,
        true,
        appearance,
        ds::RowTrailing::Toggle(enabled),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::qa_translate_into(formats.lang),
        "",
        y0 + ROW_H,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Label(tinycast_pure::i18n::qa_language_name(
            language,
            formats.lang,
        )),
    )?;
    Ok(())
}
