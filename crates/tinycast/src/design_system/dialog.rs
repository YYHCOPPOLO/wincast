use tinycast_pure::dialog::{DialogRole, DialogTone};
use tinycast_pure::layout::dialog as dlg;
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use super::fonts::Fonts;
use super::panel::paint_scrim;
use super::squircle::fill_squircle;
use super::symbols;
use super::text;

pub struct DialogContent<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub accept: &'a str,
    pub cancel: Option<&'a str>,
    pub volume: Option<f32>,
    pub appearance: u8,
    pub tone: DialogTone,
    pub symbol: &'a str,
    pub accept_role: DialogRole,
}

#[derive(Clone, Copy, Debug)]
pub struct DialogHits {
    pub accept: DipRect,
    pub cancel: Option<DipRect>,
    pub volume: Option<DipRect>,
}

pub fn measure_height(fonts: &Fonts, content: &DialogContent<'_>) -> f32 {
    layout(fonts, content).0
}

pub fn paint(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    content: &DialogContent<'_>,
) -> windows::core::Result<DialogHits> {
    let width = theme::size::DIALOG_WIDTH;
    let (height, hits) = layout(fonts, content);
    paint_scrim(
        target,
        width,
        height,
        theme::radius::DIALOG,
        content.appearance,
    )?;

    let pad = theme::spacing::XXL;
    let icon = dlg::icon_rect();
    symbols::paint_fluent_in(
        target,
        &fonts.dwrite,
        content.symbol,
        icon,
        tone_rgba(content.tone, content.appearance),
    )?;

    let text_x = icon.x + icon.w + theme::spacing::LG;
    let text_w = (width - text_x - pad).max(40.0);
    let title_h = fonts
        .measure(&fonts.headline, content.title, text_w, 40.0)
        .1
        .max(18.0);
    text::draw(
        target,
        &fonts.headline,
        content.title,
        DipRect {
            x: text_x,
            y: pad,
            w: text_w,
            h: title_h,
        },
        text::primary_ink(content.appearance),
    )?;
    if !content.message.is_empty() {
        let msg_h = fonts
            .measure(&fonts.wrap_callout, content.message, text_w, 200.0)
            .1
            .max(16.0);
        text::draw(
            target,
            &fonts.wrap_callout,
            content.message,
            DipRect {
                x: text_x,
                y: pad + title_h + theme::spacing::XS,
                w: text_w,
                h: msg_h,
            },
            text::secondary_ink(content.appearance),
        )?;
    }

    if let (Some(level), Some(rect)) = (content.volume, hits.volume) {
        paint_volume(target, fonts, rect, level, content.appearance)?;
    }

    if let Some(cancel) = hits.cancel {
        if let Some(label) = content.cancel {
            paint_button(
                target,
                fonts,
                cancel,
                label,
                DialogRole::Cancel,
                content.appearance,
            )?;
        }
    }
    paint_button(
        target,
        fonts,
        hits.accept,
        content.accept,
        content.accept_role,
        content.appearance,
    )?;
    Ok(hits)
}

pub fn volume_level_at(rect: DipRect, x: f32) -> f32 {
    let icon = theme::size::MENU_ICON;
    let readout = theme::size::VOLUME_READOUT;
    let knob = theme::size::VOLUME_KNOB;
    let track_x = rect.x + icon + theme::spacing::LG;
    let track_w = (rect.w - icon - theme::spacing::LG - theme::spacing::MD - readout).max(knob);
    let travel = (track_w - knob).max(1.0);
    ((x - track_x - knob / 2.0) / travel).clamp(0.0, 1.0)
}

pub fn contains(rect: DipRect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
}

fn layout(fonts: &Fonts, content: &DialogContent<'_>) -> (f32, DialogHits) {
    let width = theme::size::DIALOG_WIDTH;
    let pad = theme::spacing::XXL;
    let icon = dlg::icon_rect();
    let text_x = icon.x + icon.w + theme::spacing::LG;
    let text_w = (width - text_x - pad).max(40.0);
    let title_h = fonts
        .measure(&fonts.headline, content.title, text_w, 40.0)
        .1
        .max(18.0);
    let msg_h = if content.message.is_empty() {
        0.0
    } else {
        fonts
            .measure(&fonts.wrap_callout, content.message, text_w, 200.0)
            .1
            .max(16.0)
    };
    let text_h = title_h
        + if msg_h > 0.0 {
            theme::spacing::XS + msg_h
        } else {
            0.0
        };
    let header_h = icon.h.max(text_h);
    let mut y = pad + header_h;
    let volume = if content.volume.is_some() {
        y += theme::spacing::XXL;
        let rect = DipRect {
            x: pad,
            y,
            w: width - pad * 2.0,
            h: theme::size::VOLUME_KNOB,
        };
        y += rect.h;
        Some(rect)
    } else {
        None
    };
    y += theme::spacing::XXL;
    let btn_h = theme::size::MENU_BUTTON;
    let accept_w = button_width(fonts, content.accept);
    let mut right = width - pad;
    let accept = DipRect {
        x: right - accept_w,
        y,
        w: accept_w,
        h: btn_h,
    };
    right = accept.x;
    let mut cancel = None;
    if let Some(label) = content.cancel {
        if !label.is_empty() {
            let w = button_width(fonts, label);
            right -= theme::spacing::MD;
            cancel = Some(DipRect {
                x: right - w,
                y,
                w,
                h: btn_h,
            });
        }
    }
    let height = (y + btn_h + pad).max(dlg::frame().1);
    (
        height,
        DialogHits {
            accept,
            cancel,
            volume,
        },
    )
}

fn button_width(fonts: &Fonts, label: &str) -> f32 {
    let tw = fonts
        .measure(&fonts.bar, label, 240.0, theme::size::MENU_BUTTON)
        .0;
    (tw + theme::spacing::XL * 2.0).max(72.0)
}

fn paint_button(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    rect: DipRect,
    label: &str,
    role: DialogRole,
    appearance: u8,
) -> windows::core::Result<()> {
    fill_squircle(
        target,
        rect,
        rect.h / 2.0,
        text::control_surface(appearance),
    )?;
    let ink = match role {
        DialogRole::Cancel => text::secondary_ink(appearance),
        DialogRole::Destructive => text::DESTRUCTIVE,
        DialogRole::Standard => text::primary_ink(appearance),
    };
    text::draw(target, &fonts.bar, label, rect, ink)
}

fn paint_volume(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    rect: DipRect,
    level: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let icon = theme::size::MENU_ICON;
    symbols::paint_fluent_in(
        target,
        &fonts.dwrite,
        "speaker.wave.2",
        DipRect {
            x: rect.x,
            y: rect.y + (rect.h - icon) / 2.0,
            w: icon,
            h: icon,
        },
        text::secondary_ink(appearance),
    )?;
    let readout = theme::size::VOLUME_READOUT;
    let knob = theme::size::VOLUME_KNOB;
    let track_h = theme::size::VOLUME_TRACK_HEIGHT;
    let track_x = rect.x + icon + theme::spacing::LG;
    let track_w = (rect.w - icon - theme::spacing::LG - theme::spacing::MD - readout).max(knob);
    let track_y = rect.y + (rect.h - track_h) / 2.0;
    let fill = tinycast_pure::volume::clamped(level);
    fill_squircle(
        target,
        DipRect {
            x: track_x,
            y: track_y,
            w: track_w,
            h: track_h,
        },
        track_h / 2.0,
        text::control_surface(appearance),
    )?;
    let travel = (track_w - knob).max(1.0);
    fill_squircle(
        target,
        DipRect {
            x: track_x,
            y: track_y,
            w: (knob / 2.0 + fill * travel).min(track_w),
            h: track_h,
        },
        track_h / 2.0,
        {
            let (r, g, b, a) = text::primary_ink(appearance);
            (r, g, b, a * 0.85)
        },
    )?;
    fill_squircle(
        target,
        DipRect {
            x: track_x + fill * travel,
            y: rect.y + (rect.h - knob) / 2.0,
            w: knob,
            h: knob,
        },
        knob / 2.0,
        text::primary_ink(appearance),
    )?;
    let label = tinycast_pure::volume::percentage(level);
    text::draw(
        target,
        &fonts.trailing,
        &label,
        DipRect {
            x: rect.x + rect.w - readout,
            y: rect.y,
            w: readout,
            h: rect.h,
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
    use windows::Win32::Graphics::DirectWrite::{DWriteCreateFactory, DWRITE_FACTORY_TYPE_SHARED};

    #[test]
    fn dialog_width_is_420() {
        assert_eq!(tinycast_pure::layout::dialog::frame().0, 420.0);
        assert!(tinycast_pure::layout::dialog::cancel_is_leading());
    }

    #[test]
    fn cancel_is_left_of_accept() {
        let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.expect("dwrite");
        let fonts = Fonts::new(&dwrite).expect("fonts");
        let content = DialogContent {
            title: "Restart",
            message: "Restart this PC now?",
            accept: "Restart",
            cancel: Some("Cancel"),
            volume: None,
            appearance: 0,
            tone: DialogTone::Neutral,
            symbol: "info.circle",
            accept_role: DialogRole::Standard,
        };
        let (h, hits) = layout(&fonts, &content);
        assert!(h >= 120.0);
        let cancel = hits.cancel.expect("cancel");
        assert!(cancel.x + cancel.w <= hits.accept.x + 0.5);
        assert_eq!(
            hits.accept.x + hits.accept.w,
            theme::size::DIALOG_WIDTH - theme::spacing::XXL
        );
    }

    #[test]
    fn dark_dialog_center_is_scrim() {
        let (w, h, bits) = crate::design_system::test_render::with_offscreen(420, 180, |target| {
            let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
            let fonts = Fonts::new(&dwrite)?;
            let content = DialogContent {
                title: "Confirm",
                message: "Continue?",
                accept: "Continue",
                cancel: Some("Cancel"),
                volume: None,
                appearance: 0,
                tone: DialogTone::Neutral,
                symbol: "info.circle",
                accept_role: DialogRole::Standard,
            };
            let _ = paint(target, &fonts, &content)?;
            Ok(())
        })
        .expect("offscreen");
        assert_eq!(w, 420);
        let i = ((h / 2) * w + (w / 2)) * 4;
        let a = bits[i + 3] as f32 / 255.0;
        assert!(a > 0.30 && a < 0.50, "alpha {a}");
        assert_eq!(bits[3], 0, "outside squircle must be transparent");
    }
}
