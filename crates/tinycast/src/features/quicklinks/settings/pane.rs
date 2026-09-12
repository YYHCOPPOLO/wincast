//! Settings → Quicklinks: feature switch, show-in-launcher, Create / Import / Export.

use std::path::Path;

use tinycast_pure::quicklink::Quicklink;
use tinycast_pure::theme;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
    OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const ITEM_H: f32 = 36.0;
const BTN_W: f32 = 120.0;

pub fn section_header() -> &'static str {
    "Quicklinks"
}

pub const ENABLE_TITLE: &str = "Quicklinks";
pub const ENABLE_SUBTITLE: &str =
    "Open URLs, files, and searches from the launcher. Off by default.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const CREATE_LABEL: &str = "Create";
pub const IMPORT_LABEL: &str = "Import";
pub const EXPORT_LABEL: &str = "Export";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuicklinksHit {
    Enable,
    ShowInLauncher,
    Create,
    Import,
    Export,
    Item(usize),
}

pub fn content_height(count: usize) -> f32 {
    ds::switch_section_next_y(true) + ITEM_H + theme::spacing::SM + ITEM_H * count as f32 + 24.0
}

pub fn hit(x: f32, y: f32, scroll: f32, count: usize) -> Option<QuicklinksHit> {
    let y = y + scroll;
    let mut row = ds::form_origin();
    if y >= row && y < row + ROW_H {
        return Some(QuicklinksHit::Enable);
    }
    row += ROW_H;
    if y >= row && y < row + ROW_H {
        return Some(QuicklinksHit::ShowInLauncher);
    }
    row = ds::switch_section_next_y(true);
    let pad = ds::content_pad();
    if y >= row && y < row + ITEM_H {
        if x >= pad && x < pad + BTN_W {
            return Some(QuicklinksHit::Create);
        }
        if x >= pad + BTN_W + 8.0 && x < pad + BTN_W * 2.0 + 8.0 {
            return Some(QuicklinksHit::Import);
        }
        if x >= pad + BTN_W * 2.0 + 16.0 && x < pad + BTN_W * 3.0 + 16.0 {
            return Some(QuicklinksHit::Export);
        }
    }
    row += ITEM_H + theme::spacing::SM;
    for i in 0..count {
        if y >= row && y < row + ITEM_H {
            return Some(QuicklinksHit::Item(i));
        }
        row += ITEM_H;
    }
    None
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    show_in_launcher: bool,
    links: &[Quicklink],
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let lang = formats.lang;
    let (section, _, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(
            tinycast_pure::settings_tab::SettingsTab::Quicklinks,
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
    let origin = -scroll;
    let mut y = ds::form_origin() + origin;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::quicklinks_enable_title(lang),
        tinycast_pure::i18n::quicklinks_enable_subtitle(lang),
        y,
        width,
        true,
        appearance,
        ds::RowTrailing::Toggle(enabled),
    )?;
    y += ROW_H;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::show_in_launcher(lang),
        tinycast_pure::i18n::quicklinks_show_subtitle(lang),
        y,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Toggle(show_in_launcher),
    )?;
    y = ds::switch_section_next_y(true) + origin;
    let pad = ds::content_pad();
    paint_btn(
        target,
        formats,
        tinycast_pure::i18n::quicklinks_create(lang),
        pad,
        y,
        appearance,
    )?;
    paint_btn(
        target,
        formats,
        tinycast_pure::i18n::quicklinks_import(lang),
        pad + BTN_W + 8.0,
        y,
        appearance,
    )?;
    paint_btn(
        target,
        formats,
        tinycast_pure::i18n::quicklinks_export(lang),
        pad + BTN_W * 2.0 + 16.0,
        y,
        appearance,
    )?;
    y += ITEM_H + theme::spacing::SM;
    for link in links {
        draw_text(
            target,
            formats.body,
            &link.name,
            pad,
            y,
            width - pad,
            y + 20.0,
            appearance,
            0.92,
        )?;
        draw_text(
            target,
            formats.caption,
            &link.destination,
            pad,
            y + 18.0,
            width - pad,
            y + ITEM_H,
            appearance,
            0.5,
        )?;
        y += ITEM_H;
    }
    Ok(())
}

pub fn pick_open_json(owner: HWND) -> Option<std::path::PathBuf> {
    pick_json(owner, false)
}

pub fn pick_save_json(owner: HWND) -> Option<std::path::PathBuf> {
    pick_json(owner, true)
}

fn pick_json(owner: HWND, save: bool) -> Option<std::path::PathBuf> {
    let mut file = [0u16; 1024];
    let mut filter: Vec<u16> = "JSON\0*.json\0All Files\0*.*\0\0".encode_utf16().collect();
    let title: Vec<u16> = if save {
        "Export Quicklinks"
    } else {
        "Import Quicklinks"
    }
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
    ofn.Flags = if save {
        OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR
    } else {
        OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR
    };
    let ok = unsafe {
        if save {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        }
    };
    if !ok.as_bool() {
        return None;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    let path = String::from_utf16_lossy(&file[..len]);
    if path.is_empty() {
        return None;
    }
    Some(Path::new(&path).to_path_buf())
}

fn paint_btn(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    label: &str,
    x: f32,
    y: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let rect = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x,
            top: y,
            right: x + BTN_W,
            bottom: y + ITEM_H,
        },
        radiusX: 6.0,
        radiusY: 6.0,
    };
    let fill = D2D1_COLOR_F {
        r: 0.2,
        g: 0.55,
        b: 1.0,
        a: 1.0,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(&rect, &brush);
    }
    draw_text(
        target,
        formats.caption,
        label,
        x + 12.0,
        y,
        x + BTN_W - 12.0,
        y + ITEM_H,
        appearance,
        1.0,
    )
}

fn draw_text(
    target: &ID2D1RenderTarget,
    format: &windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
    text: &str,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    appearance: u8,
    alpha: f32,
) -> windows::core::Result<()> {
    let color = ds::ramp_color(appearance, alpha);
    let brush = unsafe { target.CreateSolidColorBrush(&color, None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &D2D_RECT_F {
                left,
                top,
                right,
                bottom,
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
    fn quicklinks_settings_exposes_feature_switch() {
        assert_eq!(ENABLE_TITLE, "Quicklinks");
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0, 0),
            Some(QuicklinksHit::Enable)
        );
        assert_eq!(
            hit(20.0, ds::form_origin() + ROW_H + 4.0, 0.0, 0),
            Some(QuicklinksHit::ShowInLauncher)
        );
        let y = ds::switch_section_next_y(true) + 4.0;
        let pad = ds::content_pad();
        assert_eq!(hit(pad + 4.0, y, 0.0, 0), Some(QuicklinksHit::Create));
        assert_eq!(
            hit(pad + BTN_W + 12.0, y, 0.0, 0),
            Some(QuicklinksHit::Import)
        );
        assert_eq!(
            hit(pad + BTN_W * 2.0 + 20.0, y, 0.0, 0),
            Some(QuicklinksHit::Export)
        );
    }
}
