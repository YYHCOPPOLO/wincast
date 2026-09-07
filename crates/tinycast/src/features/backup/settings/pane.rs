//! Settings → Backup: export, import, Raycast import.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 44.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupHit {
    Export,
    Import,
    Raycast,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H * 3.0 + theme::spacing::XL * 2.0
}

fn row_y(i: usize) -> f32 {
    24.0 + i as f32 * (ROW_H + theme::spacing::XL)
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<BackupHit> {
    let y = y + scroll;
    let hits = [BackupHit::Export, BackupHit::Import, BackupHit::Raycast];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    None
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    row(
        target,
        formats,
        tinycast_pure::i18n::backup_export(formats.lang),
        row_y(0) - scroll,
        width,
        appearance,
    )?;
    row(
        target,
        formats,
        tinycast_pure::i18n::backup_import(formats.lang),
        row_y(1) - scroll,
        width,
        appearance,
    )?;
    row(
        target,
        formats,
        tinycast_pure::i18n::backup_raycast(formats.lang),
        row_y(2) - scroll,
        width,
        appearance,
    )?;
    Ok(())
}

fn row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    y: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&ds::primary_ink(appearance), None)? };
    let t: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &t,
            formats.body,
            &D2D_RECT_F {
                left: theme::spacing::XL,
                top: y + 10.0,
                right: width - theme::spacing::XL,
                bottom: y + ROW_H - 6.0,
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
    fn export_is_first_row() {
        assert_eq!(hit(20.0, 30.0, 0.0), Some(BackupHit::Export));
    }
}
