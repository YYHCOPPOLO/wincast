//! Settings → Quick Actions. Off by default; excluded from backups.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

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
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, appearance)?;
    let y0 = ds::form_origin() - scroll;
    paint_toggle(
        target,
        formats,
        tinycast_pure::i18n::qa_enable_title(formats.lang),
        tinycast_pure::i18n::qa_enable_subtitle(formats.lang),
        enabled,
        y0,
        width,
        appearance,
    )?;
    paint_row(
        target,
        formats,
        tinycast_pure::i18n::qa_translate_into(formats.lang),
        tinycast_pure::i18n::qa_language_name(language, formats.lang),
        y0 + ROW_H,
        width,
        appearance,
    )?;
    Ok(())
}

fn paint_toggle(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    y: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    paint_row(target, formats, title, subtitle, y, width, appearance)?;
    let pad = theme::spacing::XL;
    let toggle = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: width - pad - TOGGLE_W,
            top: y + (ROW_H - TOGGLE_H) / 2.0,
            right: width - pad,
            bottom: y + (ROW_H - TOGGLE_H) / 2.0 + TOGGLE_H,
        },
        radiusX: TOGGLE_H / 2.0,
        radiusY: TOGGLE_H / 2.0,
    };
    let fill = if on {
        D2D1_COLOR_F {
            r: 0.2,
            g: 0.55,
            b: 1.0,
            a: 1.0,
        }
    } else {
        ds::ramp_color(appearance, 0.18)
    };
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe { target.FillRoundedRectangle(&toggle, &brush) };
    Ok(())
}

fn paint_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    y: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let brush = unsafe { target.CreateSolidColorBrush(&ds::primary_ink(appearance), None)? };
    let title_w: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title_w,
            formats.body,
            &D2D_RECT_F {
                left: pad,
                top: y + 8.0,
                right: width - pad - TOGGLE_W - 8.0,
                bottom: y + 28.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let muted_brush = unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    let sub: Vec<u16> = subtitle.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &sub,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: y + 28.0,
                right: width - pad - TOGGLE_W - 8.0,
                bottom: y + ROW_H - 4.0,
            },
            &muted_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}
