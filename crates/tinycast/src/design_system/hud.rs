use tinycast_pure::dialog::DialogTone;
use tinycast_pure::layout::hud as hud_layout;
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use super::fonts::Fonts;
use super::panel::paint_scrim;
use super::squircle::fill_squircle;
use super::symbols;
use super::text;

pub enum HudPaint<'a> {
    Message {
        text: &'a str,
        tone: DialogTone,
        appearance: u8,
    },
    Volume {
        level: f32,
        muted: bool,
        appearance: u8,
    },
}

pub fn volume_readout(level: f32, muted: bool) -> String {
    if muted {
        "Muted".into()
    } else {
        tinycast_pure::volume::percentage(level)
    }
}

pub fn message_size(fonts: &Fonts, text: &str) -> (f32, f32) {
    let max_w = hud_layout::message_max_width();
    let pad_x = theme::spacing::XL;
    let pad_y = theme::spacing::LG;
    let icon = theme::size::MENU_ICON;
    let gap = theme::spacing::MD;
    let text_max = (max_w - pad_x * 2.0 - gap - icon).max(40.0);
    let (tw, th) = fonts.measure(&fonts.bar, text, text_max, 40.0);
    let w = (pad_x * 2.0 + tw + gap + icon).clamp(64.0, max_w);
    let h = (icon.max(th) + pad_y * 2.0).max(36.0);
    (w, h)
}

pub fn volume_size() -> (f32, f32) {
    hud_layout::volume_size()
}

pub fn paint(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    paint: &HudPaint<'_>,
) -> windows::core::Result<()> {
    match paint {
        HudPaint::Message {
            text,
            tone,
            appearance,
        } => paint_message(target, fonts, text, *tone, *appearance),
        HudPaint::Volume {
            level,
            muted,
            appearance,
        } => paint_volume(target, fonts, *level, *muted, *appearance),
    }
}

fn paint_message(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    text: &str,
    tone: DialogTone,
    appearance: u8,
) -> windows::core::Result<()> {
    let (w, h) = message_size(fonts, text);
    paint_scrim(target, w, h, h / 2.0, appearance)?;
    let pad_x = theme::spacing::XL;
    let icon = theme::size::MENU_ICON;
    let gap = theme::spacing::MD;
    let text_w = (w - pad_x * 2.0 - gap - icon).max(8.0);
    text::draw(
        target,
        &fonts.bar,
        text,
        DipRect {
            x: pad_x,
            y: 0.0,
            w: text_w,
            h,
        },
        text::primary_ink(appearance),
    )?;
    let symbol = match tone {
        DialogTone::Neutral => "info.circle.fill",
        DialogTone::Success => "checkmark.circle.fill",
        DialogTone::Danger => "exclamationmark.circle.fill",
    };
    symbols::paint_fluent_in(
        target,
        &fonts.dwrite,
        symbol,
        DipRect {
            x: w - pad_x - icon,
            y: (h - icon) / 2.0,
            w: icon,
            h: icon,
        },
        tone_rgba(tone, appearance),
    )
}

fn paint_volume(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    level: f32,
    muted: bool,
    appearance: u8,
) -> windows::core::Result<()> {
    let (w, h) = volume_size();
    paint_scrim(target, w, h, theme::radius::DIALOG, appearance)?;
    let pad_v = theme::spacing::XXL;
    let pad_h = theme::spacing::XL;
    let icon = theme::size::DIALOG_ICON;
    let symbol = if muted {
        "speaker.slash.fill"
    } else {
        "speaker.wave.2"
    };
    symbols::paint_fluent_in(
        target,
        &fonts.dwrite,
        symbol,
        DipRect {
            x: (w - icon) / 2.0,
            y: pad_v,
            w: icon,
            h: icon,
        },
        text::primary_ink(appearance),
    )?;
    let row_y = pad_v + icon + theme::spacing::LG;
    let readout = theme::size::VOLUME_READOUT;
    let track_h = theme::size::VOLUME_TRACK_HEIGHT;
    let track_x = pad_h;
    let track_w = (w - pad_h * 2.0 - theme::spacing::MD - readout).max(8.0);
    let fill = if muted {
        0.0
    } else {
        tinycast_pure::volume::clamped(level)
    };
    fill_squircle(
        target,
        DipRect {
            x: track_x,
            y: row_y + (theme::size::VOLUME_KNOB - track_h) / 2.0,
            w: track_w,
            h: track_h,
        },
        track_h / 2.0,
        text::control_surface(appearance),
    )?;
    if fill > 0.0 {
        let (r, g, b, a) = text::primary_ink(appearance);
        fill_squircle(
            target,
            DipRect {
                x: track_x,
                y: row_y + (theme::size::VOLUME_KNOB - track_h) / 2.0,
                w: track_w * fill,
                h: track_h,
            },
            track_h / 2.0,
            (r, g, b, a * 0.85),
        )?;
    }
    let label = volume_readout(level, muted);
    text::draw(
        target,
        &fonts.trailing,
        &label,
        DipRect {
            x: w - pad_h - readout,
            y: row_y,
            w: readout,
            h: theme::size::VOLUME_KNOB,
        },
        text::secondary_ink(appearance),
    )?;
    Ok(())
}

fn tone_rgba(tone: DialogTone, appearance: u8) -> (f32, f32, f32, f32) {
    match tone {
        DialogTone::Neutral => text::secondary_ink(appearance),
        DialogTone::Success => text::SUCCESS,
        DialogTone::Danger => text::DESTRUCTIVE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_prints_word_not_percent() {
        assert_eq!(volume_readout(0.0, true), "Muted");
        assert_ne!(volume_readout(0.0, true), "0%");
        assert_eq!(volume_readout(0.4, false), "40%");
    }

    #[test]
    fn hud_timers_split() {
        assert_eq!(theme::duration::MESSAGE_HUD_SECS, 2.4);
        assert_eq!(theme::duration::VOLUME_HUD_SECS, 1.6);
    }
}
