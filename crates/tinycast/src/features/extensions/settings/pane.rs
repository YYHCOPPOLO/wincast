//! Settings → Extensions: original switch, no JS runtime.

use tinycast_pure::extensions::RUNTIME_ABSENT;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const NOTICE_H: f32 = 40.0;

pub const ENABLE_TITLE: &str = "Enable extensions";
pub const ENABLE_SUBTITLE: &str =
    "Run Raycast extensions natively. A running command holds a JavaScript engine in memory until you leave it.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const LAUNCHER_SUBTITLE: &str = "List every extension's commands in launcher search.";
pub const RUNTIME_NOTICE: &str = RUNTIME_ABSENT;

pub fn section_header() -> &'static str {
    "Extensions"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionsHit {
    Enable,
    ShowInLauncher,
}

pub fn content_height() -> f32 {
    ds::switch_section_next_y(true) + NOTICE_H + 24.0
}

fn row_y(i: usize) -> f32 {
    ds::form_origin() + i as f32 * ROW_H
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<ExtensionsHit> {
    let y = y + scroll;
    if y >= row_y(0) && y < row_y(0) + ROW_H {
        return Some(ExtensionsHit::Enable);
    }
    if y >= row_y(1) && y < row_y(1) + ROW_H {
        return Some(ExtensionsHit::ShowInLauncher);
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
    let lang = formats.lang;
    let (section, _, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Extensions,
            lang,
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
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::extensions_enable_title(lang),
        tinycast_pure::i18n::extensions_enable_subtitle(lang),
        row_y(0) - scroll,
        width,
        true,
        appearance,
        ds::RowTrailing::Toggle(enabled),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::show_in_launcher(lang),
        tinycast_pure::i18n::extensions_show_subtitle(lang),
        row_y(1) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Toggle(show_in_launcher),
    )?;
    let notice_y = ds::switch_section_next_y(true) - scroll;
    let brush = unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    let wide: Vec<u16> = tinycast_pure::i18n::extensions_runtime_notice(lang)
        .encode_utf16()
        .collect();
    let pad = ds::content_pad();
    unsafe {
        target.DrawText(
            &wide,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: notice_y,
                right: width - pad,
                bottom: notice_y + NOTICE_H,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_switch_is_visible_and_runtime_copy_matches_reject() {
        assert_eq!(ENABLE_TITLE, "Enable extensions");
        assert_eq!(SHOW_IN_LAUNCHER, "Show in launcher");
        assert_eq!(RUNTIME_NOTICE, RUNTIME_ABSENT);
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0),
            Some(ExtensionsHit::Enable)
        );
        assert_eq!(
            hit(20.0, row_y(1) + 4.0, 0.0),
            Some(ExtensionsHit::ShowInLauncher)
        );
        assert!(content_height() > row_y(1) + ROW_H);
        let (_, _, show) =
            ds::feature_switch_section(420.0, ds::CARD_INSET, section_header(), true);
        assert_eq!(
            hit(20.0, show.unwrap().y + 4.0, 0.0),
            Some(ExtensionsHit::ShowInLauncher)
        );
    }
}
