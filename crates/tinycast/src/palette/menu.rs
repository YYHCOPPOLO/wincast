//! Footer and menu snapshots plus the DirectWrite measurement the footer's hit
//! testing shares with the Slint surface.

use tinycast_pure::palette_menu::{
    action_group_rects_measured, ActionGroupRects, MenuItem, OpenMenu, ACTIONS_SHORTCUT,
};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_TEXT_METRICS,
};

use crate::design_system::Fonts;

pub fn action_group_rects_for_fonts(
    fonts: &Fonts,
    panel_w: f32,
    panel_h: f32,
    primary_label: &str,
    actions_label: &str,
) -> Option<ActionGroupRects> {
    let action_caps: Vec<&str> = ACTIONS_SHORTCUT.split('+').collect();
    let primary_w = bar_button_width(fonts, primary_label, &["↵"]);
    let actions_w = bar_button_width(fonts, actions_label, &action_caps);
    action_group_rects_measured(panel_w, panel_h, primary_w, actions_w)
}

/// Same DirectWrite metrics as the footer paint, for hit testing.
pub fn action_group_rects_for_label(
    panel_w: f32,
    panel_h: f32,
    primary_label: &str,
    actions_label: &str,
) -> Option<ActionGroupRects> {
    let dwrite: IDWriteFactory =
        unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()? };
    let fonts = Fonts::new(&dwrite).ok()?;
    action_group_rects_for_fonts(&fonts, panel_w, panel_h, primary_label, actions_label)
}

pub struct FooterPaint<'a> {
    pub show_action_group: bool,
    pub primary_label: &'a str,
    pub actions_label: &'a str,
    pub primary_destructive: bool,
    pub appearance: u8,
}

pub struct MenuPaint<'a> {
    pub header: &'a str,
    pub items: &'a [MenuItem],
    pub selection: usize,
    pub kind: OpenMenu,
    pub appearance: u8,
}

/// The clipboard filter / AI model pill in the header's trailing slot.
pub struct FilterButtonPaint {
    pub title: String,
    pub open: bool,
    pub rect: DipRect,
}

fn bar_button_width(fonts: &Fonts, label: &str, caps: &[&str]) -> f32 {
    let pad = theme::spacing::MD;
    let mut w = pad
        + text_width(
            &fonts.dwrite,
            &fonts.bar,
            label,
            240.0,
            theme::size::BAR_BUTTON_HEIGHT,
        );
    if !caps.is_empty() {
        w += theme::spacing::SM;
    }
    for (i, cap) in caps.iter().enumerate() {
        if i > 0 {
            w += theme::spacing::XXS;
        }
        w += keycap_width(fonts, cap);
    }
    w + pad
}

fn keycap_width(fonts: &Fonts, text: &str) -> f32 {
    let pad = theme::spacing::XS;
    let text_w = text_width(
        &fonts.dwrite,
        &fonts.keycap,
        text,
        80.0,
        theme::size::KEY_CAP,
    );
    (text_w + pad * 2.0).max(theme::size::KEY_CAP)
}

fn text_width(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &str,
    max_w: f32,
    h: f32,
) -> f32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let Ok(layout) = dwrite.CreateTextLayout(&wide, format, max_w, h) else {
            return 0.0;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        if layout.GetMetrics(&mut metrics).is_err() {
            return 0.0;
        }
        metrics.width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_group_hits_match_measured_paint_rects() {
        let dwrite: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).expect("dwrite") };
        let fonts = Fonts::new(&dwrite).expect("fonts");
        let painted =
            action_group_rects_for_fonts(&fonts, 750.0, 475.0, "Open Application", "Actions")
                .unwrap();
        let hit =
            action_group_rects_for_label(750.0, 475.0, "Open Application", "Actions").unwrap();
        assert_eq!(painted, hit);
        let fallback = tinycast_pure::palette_menu::action_group_rects(750.0, 475.0).unwrap();
        assert!(
            (painted.primary.w - fallback.primary.w).abs() > 0.5
                || (painted.actions.w - fallback.actions.w).abs() > 0.5,
            "measured footer must not use the abolished 176/126 fallback"
        );
    }
}
