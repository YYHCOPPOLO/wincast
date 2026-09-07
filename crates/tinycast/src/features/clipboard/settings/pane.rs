//! Clipboard settings: retention, disabled apps, clear history.

use std::path::Path;

use tinycast_pure::theme;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 36.0;

const RETENTION_DAYS: [i64; 7] = [1, 7, 30, 90, 180, 365, -1];

pub fn section_header() -> &'static str {
    "Clipboard"
}

pub const ADD_APPLICATION_TITLE: &str = "Add Application…";
pub const CLEAR_HISTORY_TITLE: &str = "Clear history";
pub const CLEAR_CONFIRM_TITLE: &str = "Clear clipboard history?";
pub const CLEAR_CONFIRM_MESSAGE: &str = "This can't be undone.";
pub const CLEAR_CONFIRM_ACTION: &str = "Clear History";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardHit {
    Retention,
    Clear,
    RemoveApp(usize),
    AddApp,
}

pub fn retention_title(days: i64) -> &'static str {
    match days {
        1 => "1 Day",
        7 => "1 Week",
        30 => "1 Month",
        90 => "3 Months",
        180 => "6 Months",
        365 => "1 Year",
        _ => "Forever",
    }
}

pub fn cycle_retention(days: i64) -> i64 {
    let idx = RETENTION_DAYS.iter().position(|d| *d == days).unwrap_or(3);
    RETENTION_DAYS[(idx + 1) % RETENTION_DAYS.len()]
}

struct ClipboardLayout {
    retention: f32,
    apps: Vec<f32>,
    add_app: f32,
    clear: f32,
}

fn layout(disabled_len: usize) -> ClipboardLayout {
    let mut y = ds::switch_section_next_y(false);
    let retention = y;
    y += ROW_H + theme::spacing::XL + 24.0;
    let mut apps = Vec::new();
    y += ROW_H;
    for _ in 0..disabled_len {
        apps.push(y);
        y += ROW_H;
    }
    let add_app = y;
    y += ROW_H + theme::spacing::XL + 24.0;
    let clear = y;
    ClipboardLayout {
        retention,
        apps,
        add_app,
        clear,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    retention_days: i64,
    disabled: &[String],
    detail_w: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let lang = formats.lang;
    let (section, _, _) = ds::feature_switch_section(
        detail_w,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Clipboard,
            lang,
        ),
        false,
    );
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, appearance)?;
    let origin = -scroll;
    let inset = theme::spacing::XXL;
    let rows = layout(disabled.len());
    draw_header(
        target,
        formats.header,
        inset,
        rows.retention + origin - 24.0,
        detail_w,
        tinycast_pure::i18n::clipboard_history(lang),
        appearance,
    )?;
    draw_row(
        target,
        formats,
        inset,
        rows.retention + origin,
        detail_w,
        tinycast_pure::i18n::clipboard_keep_for(lang),
        tinycast_pure::i18n::clipboard_retention(retention_days, lang),
        appearance,
    )?;
    draw_header(
        target,
        formats.header,
        inset,
        rows.retention + ROW_H + theme::spacing::XL + origin,
        detail_w,
        tinycast_pure::i18n::clipboard_disabled_apps(lang),
        appearance,
    )?;
    draw_caption(
        target,
        formats.caption,
        inset,
        rows.retention + ROW_H + theme::spacing::XL + 24.0 + origin,
        detail_w,
        tinycast_pure::i18n::clipboard_disabled_caption(lang),
        appearance,
    )?;
    for (i, name) in disabled.iter().enumerate() {
        draw_row(
            target,
            formats,
            inset,
            rows.apps[i] + origin,
            detail_w,
            name,
            tinycast_pure::i18n::remove_label(lang),
            appearance,
        )?;
    }
    draw_row(
        target,
        formats,
        inset,
        rows.add_app + origin,
        detail_w,
        tinycast_pure::i18n::clipboard_add_application(lang),
        tinycast_pure::i18n::clipboard_add(lang),
        appearance,
    )?;
    draw_header(
        target,
        formats.header,
        inset,
        rows.add_app + ROW_H + theme::spacing::XL + origin,
        detail_w,
        tinycast_pure::i18n::clipboard_clear_section(lang),
        appearance,
    )?;
    draw_row(
        target,
        formats,
        inset,
        rows.clear + origin,
        detail_w,
        tinycast_pure::i18n::clipboard_clear_history(lang),
        tinycast_pure::i18n::clipboard_clear_ellipsis(lang),
        appearance,
    )?;
    Ok(())
}

pub fn hit(x: f32, y: f32, scroll: f32, disabled_len: usize) -> Option<ClipboardHit> {
    let y = y + scroll;
    let rows = layout(disabled_len);
    if in_row(y, rows.retention) && x > theme::spacing::XXL {
        return Some(ClipboardHit::Retention);
    }
    for (i, row_y) in rows.apps.iter().enumerate() {
        if in_row(y, *row_y) {
            return Some(ClipboardHit::RemoveApp(i));
        }
    }
    if in_row(y, rows.add_app) {
        return Some(ClipboardHit::AddApp);
    }
    if in_row(y, rows.clear) {
        return Some(ClipboardHit::Clear);
    }
    let _ = x;
    None
}

pub fn pick_application_stem(owner: HWND) -> Option<String> {
    let mut file = [0u16; 1024];
    let mut filter: Vec<u16> = "Applications\0*.exe\0All Files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Add Application"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut ofn = OPENFILENAMEW::default();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = PCWSTR(filter.as_mut_ptr());
    ofn.lpstrFile = PWSTR(file.as_mut_ptr());
    ofn.nMaxFile = file.len() as u32;
    ofn.lpstrTitle = PCWSTR(title.as_ptr());
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    let ok = unsafe { GetOpenFileNameW(&mut ofn) };
    if !ok.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    let path = String::from_utf16_lossy(&file[..len]);
    if path.is_empty() {
        return None;
    }
    Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

fn in_row(y: f32, row_y: f32) -> bool {
    y >= row_y && y < row_y + ROW_H
}

fn draw_header(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    x: f32,
    y: f32,
    w: f32,
    text: &str,
    appearance: u8,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&ds::tertiary_ink(appearance), None)? };
    draw_text(
        target,
        format,
        &brush,
        D2D_RECT_F {
            left: x,
            top: y,
            right: w - theme::spacing::XXL,
            bottom: y + 20.0,
        },
        text,
    )
}

fn draw_caption(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    x: f32,
    y: f32,
    w: f32,
    text: &str,
    appearance: u8,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&ds::tertiary_ink(appearance), None)? };
    draw_text(
        target,
        format,
        &brush,
        D2D_RECT_F {
            left: x,
            top: y,
            right: w - theme::spacing::XXL,
            bottom: y + 20.0,
        },
        text,
    )
}

fn draw_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    x: f32,
    y: f32,
    w: f32,
    title: &str,
    trailing: &str,
    appearance: u8,
) -> windows::core::Result<()> {
    let pill = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x - 4.0,
            top: y,
            right: w - theme::spacing::XL,
            bottom: y + ROW_H,
        },
        radiusX: theme::radius::ROW,
        radiusY: theme::radius::ROW,
    };
    let fill = unsafe {
        target.CreateSolidColorBrush(
            &crate::design_system::appearance::color(ds::card_fill(appearance)),
            None,
        )?
    };
    unsafe {
        target.FillRoundedRectangle(&pill, &fill);
    }
    let title_brush = unsafe { target.CreateSolidColorBrush(&ds::primary_ink(appearance), None)? };
    draw_text(
        target,
        formats.body,
        &title_brush,
        D2D_RECT_F {
            left: x + theme::spacing::SM,
            top: y,
            right: w - 140.0,
            bottom: y + ROW_H,
        },
        title,
    )?;
    let trail = unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    draw_text(
        target,
        formats.caption,
        &trail,
        D2D_RECT_F {
            left: w - 130.0,
            top: y,
            right: w - theme::spacing::XXL,
            bottom: y + ROW_H,
        },
        trailing,
    )
}

fn draw_text(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    text: &str,
) -> windows::core::Result<()> {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_cycles_through_presets() {
        assert_eq!(cycle_retention(90), 180);
        assert_eq!(retention_title(90), "3 Months");
        assert_eq!(retention_title(-1), "Forever");
    }

    #[test]
    fn add_application_and_clear_are_hittable() {
        let rows = layout(2);
        assert_eq!(
            hit(theme::spacing::XXL + 1.0, rows.add_app + 1.0, 0.0, 2),
            Some(ClipboardHit::AddApp)
        );
        assert_eq!(
            hit(theme::spacing::XXL + 1.0, rows.clear + 1.0, 0.0, 2),
            Some(ClipboardHit::Clear)
        );
        assert_eq!(
            hit(theme::spacing::XXL + 1.0, rows.apps[1] + 1.0, 0.0, 2),
            Some(ClipboardHit::RemoveApp(1))
        );
        let empty = layout(0);
        assert_eq!(
            hit(theme::spacing::XXL + 1.0, empty.add_app + 1.0, 0.0, 0),
            Some(ClipboardHit::AddApp)
        );
        assert!(rows.retention >= ds::switch_section_next_y(false) - 0.001);
        assert_eq!(ADD_APPLICATION_TITLE, "Add Application…");
        assert_eq!(CLEAR_CONFIRM_ACTION, "Clear History");
    }
}
