//! Settings → Commands: Custom Commands switches, list, and New.

use tinycast_pure::custom_command::CustomCommand;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const ITEM_H: f32 = 36.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

pub fn section_header() -> &'static str {
    "Commands"
}

pub const ENABLE_TITLE: &str = "Custom Commands";
pub const ENABLE_SUBTITLE: &str = "Run your own commands from the launcher. Off by default.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const NEW_LABEL: &str = "New command";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomCommandsHit {
    Enable,
    ShowInLauncher,
    New,
    Item(usize),
}

pub fn content_height(count: usize) -> f32 {
    ds::switch_section_next_y(true)
        + ITEM_H
        + theme::spacing::SM
        + ITEM_H * count as f32
        + theme::spacing::SECTION_SPACING
}

pub fn hit(x: f32, y: f32, scroll: f32, count: usize) -> Option<CustomCommandsHit> {
    let y = y + scroll;
    let mut row = ds::form_origin();
    if y >= row && y < row + ROW_H {
        return Some(CustomCommandsHit::Enable);
    }
    row += ROW_H;
    if y >= row && y < row + ROW_H {
        return Some(CustomCommandsHit::ShowInLauncher);
    }
    row = ds::switch_section_next_y(true);
    if y >= row && y < row + ITEM_H {
        return Some(CustomCommandsHit::New);
    }
    row += ITEM_H + theme::spacing::SM;
    for i in 0..count {
        if y >= row && y < row + ITEM_H {
            let _ = x;
            return Some(CustomCommandsHit::Item(i));
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
    commands: &[CustomCommand],
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let (section, _, _) =
        ds::feature_switch_section(width, ds::CARD_INSET - scroll, section_header(), true);
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, appearance)?;
    let origin = -scroll;
    let mut y = ds::form_origin() + origin;
    paint_toggle_row(
        target,
        formats,
        ENABLE_TITLE,
        ENABLE_SUBTITLE,
        enabled,
        y,
        width,
        appearance,
    )?;
    y += ROW_H;
    paint_toggle_row(
        target,
        formats,
        SHOW_IN_LAUNCHER,
        "Hide the Custom Commands section without turning shortcuts off.",
        show_in_launcher,
        y,
        width,
        appearance,
    )?;
    y = ds::switch_section_next_y(true) + origin;
    paint_button(target, formats, NEW_LABEL, y, width, appearance)?;
    y += ITEM_H + theme::spacing::SM;
    for cmd in commands {
        paint_item(target, formats, &cmd.name, &cmd.command, y, width, appearance)?;
        y += ITEM_H;
    }
    Ok(())
}

fn paint_toggle_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    y: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let text_w = width - pad * 3.0 - TOGGLE_W;
    draw_text(
        target,
        formats.body,
        title,
        pad,
        y + 8.0,
        pad + text_w,
        y + 28.0,
        appearance,
        0.92,
    )?;
    draw_text(
        target,
        formats.caption,
        subtitle,
        pad,
        y + 28.0,
        pad + text_w,
        y + ROW_H - 4.0,
        appearance,
        0.55,
    )?;
    paint_toggle(
        target,
        width - pad - TOGGLE_W,
        y + (ROW_H - TOGGLE_H) / 2.0,
        on,
        appearance,
    )
}

fn paint_button(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    label: &str,
    y: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let rect = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: pad,
            top: y,
            right: (pad + 140.0).min(width - pad),
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
        rect.rect.left + 12.0,
        y,
        rect.rect.right - 12.0,
        y + ITEM_H,
        appearance,
        1.0,
    )
}

fn paint_item(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    name: &str,
    command: &str,
    y: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    draw_text(
        target,
        formats.body,
        name,
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
        command,
        pad,
        y + 18.0,
        width - pad,
        y + ITEM_H,
        appearance,
        0.5,
    )
}

fn paint_toggle(
    target: &ID2D1RenderTarget,
    x: f32,
    y: f32,
    on: bool,
    appearance: u8,
) -> windows::core::Result<()> {
    let toggle = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x,
            top: y,
            right: x + TOGGLE_W,
            bottom: y + TOGGLE_H,
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
    fn custom_commands_settings_exposes_feature_switch() {
        assert_eq!(ENABLE_TITLE, "Custom Commands");
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0, 0),
            Some(CustomCommandsHit::Enable)
        );
        assert_eq!(
            hit(20.0, ds::form_origin() + ROW_H + 4.0, 0.0, 0),
            Some(CustomCommandsHit::ShowInLauncher)
        );
        assert_eq!(
            hit(
                20.0,
                ds::switch_section_next_y(true) + 4.0,
                0.0,
                0
            ),
            Some(CustomCommandsHit::New)
        );
    }
}
