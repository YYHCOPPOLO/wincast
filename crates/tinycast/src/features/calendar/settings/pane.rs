//! Settings → Calendar: enable (consent), auto-join, join window, camera preview.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarHit {
    Enable,
    AutoJoin,
    Camera,
    JoinWindow,
}

pub fn section_header() -> &'static str {
    "Calendar"
}

pub fn content_height() -> f32 {
    ds::form_origin() + ROW_H * 4.0 + ds::CARD_PAD + theme::spacing::SECTION_SPACING
}

fn row_y(i: usize) -> f32 {
    ds::form_origin() + i as f32 * ROW_H
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<CalendarHit> {
    let y = y + scroll;
    let hits = [
        CalendarHit::Enable,
        CalendarHit::AutoJoin,
        CalendarHit::Camera,
        CalendarHit::JoinWindow,
    ];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    None
}

pub fn join_window_title(minutes: i64) -> &'static str {
    match minutes {
        1 => "1 minute",
        2 => "2 minutes",
        10 => "10 minutes",
        15 => "15 minutes",
        _ => "5 minutes",
    }
}

pub fn cycle_join_window(minutes: i64) -> i64 {
    match minutes {
        1 => 2,
        2 => 5,
        5 => 10,
        10 => 15,
        _ => 1,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    auto_join: bool,
    camera: bool,
    join_minutes: i64,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (section, _, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Calendar,
            formats.lang,
        ),
        false,
    );
    let mut section = section;
    section.body_h = ds::CARD_PAD * 2.0 + ROW_H * 4.0;
    ds::paint_grouped_section(
        target,
        formats.header,
        formats.caption,
        &section,
        appearance,
    )?;
    paint_toggle(
        target,
        formats,
        tinycast_pure::i18n::calendar_enable_title(formats.lang),
        tinycast_pure::i18n::calendar_enable_subtitle(formats.lang),
        enabled,
        row_y(0) - scroll,
        width,
        appearance,
    )?;
    paint_toggle(
        target,
        formats,
        tinycast_pure::i18n::calendar_auto_join(formats.lang),
        tinycast_pure::i18n::calendar_auto_join_sub(formats.lang),
        auto_join,
        row_y(1) - scroll,
        width,
        appearance,
    )?;
    paint_toggle(
        target,
        formats,
        tinycast_pure::i18n::calendar_camera(formats.lang),
        tinycast_pure::i18n::calendar_camera_sub(formats.lang),
        camera,
        row_y(2) - scroll,
        width,
        appearance,
    )?;
    paint_row(
        target,
        formats,
        tinycast_pure::i18n::calendar_join_window(formats.lang),
        tinycast_pure::i18n::calendar_join_minutes(join_minutes, formats.lang),
        row_y(3) - scroll,
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
    unsafe {
        target.FillRoundedRectangle(&toggle, &brush);
    }
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
    let t: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &t,
            formats.body,
            &D2D_RECT_F {
                left: pad,
                top: y,
                right: width - pad * 2.0 - TOGGLE_W,
                bottom: y + 28.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let muted_brush =
        unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    let s: Vec<u16> = subtitle.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &s,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: y + 28.0,
                right: width - pad * 2.0 - TOGGLE_W,
                bottom: y + ROW_H - 4.0,
            },
            &muted_brush,
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
    fn enable_is_first_row() {
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0),
            Some(CalendarHit::Enable)
        );
        assert_eq!(cycle_join_window(5), 10);
        assert_eq!(hit(20.0, row_y(1) + 4.0, 0.0), Some(CalendarHit::AutoJoin));
        assert!((row_y(1) - row_y(0) - ROW_H).abs() < 0.001);
    }
}
