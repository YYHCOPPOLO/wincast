//! Settings recorder: not focusable; local capture while recordingAction is set.

use tinycast_pure::double_tap::DoubleTapDetector;
use tinycast_pure::hotkey::{
    capture_keydown, CaptureOutcome, DoubleTapModifier, HotKeyBinding, Modifiers,
};
use tinycast_pure::layout::settings::{recorder_callout_above, recorder_well};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::features::launcher::settings::items::Formats;

pub struct Recorder {
    pub action: Option<String>,
    detector: DoubleTapDetector,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            action: None,
            detector: DoubleTapDetector::new(),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.action.is_some()
    }

    pub fn begin(&mut self, action: String) {
        self.action = Some(action);
        self.detector.reset();
    }

    pub fn cancel(&mut self) {
        self.action = None;
        self.detector.reset();
    }

    pub fn on_keydown(&mut self, vk: u16, modifiers: Modifiers, now_ms: u64) -> CaptureOutcome {
        if self.action.is_none() {
            return CaptureOutcome::Ignore;
        }
        if let Some(m) = double_tap_mod(vk) {
            if self.detector.on_flags(m, true, now_ms) {
                return CaptureOutcome::Commit(HotKeyBinding::DoubleTap(m));
            }
            return CaptureOutcome::Ignore;
        }
        capture_keydown(vk, modifiers)
    }

    pub fn on_keyup(&mut self, vk: u16, now_ms: u64) -> CaptureOutcome {
        if self.action.is_none() {
            return CaptureOutcome::Ignore;
        }
        if let Some(m) = double_tap_mod(vk) {
            if self.detector.on_flags(m, false, now_ms) {
                return CaptureOutcome::Commit(HotKeyBinding::DoubleTap(m));
            }
        }
        CaptureOutcome::Ignore
    }
}

pub fn well_in_row(row: DipRect) -> DipRect {
    let well = recorder_well();
    DipRect {
        x: row.x + row.w - crate::design_system::settings::CARD_PAD - well.w,
        y: row.y + (row.h - well.h) / 2.0,
        w: well.w,
        h: well.h,
    }
}

pub fn well_caption(binding: Option<&str>, listening: bool) -> String {
    well_caption_lang(binding, listening, tinycast_pure::i18n::UiLang::En)
}

pub fn well_caption_lang(
    binding: Option<&str>,
    listening: bool,
    lang: tinycast_pure::i18n::UiLang,
) -> String {
    if listening {
        tinycast_pure::i18n::listening_label(lang).into()
    } else if let Some(text) = binding.filter(|s| !s.is_empty()) {
        text.to_string()
    } else {
        tinycast_pure::i18n::record_label(lang).into()
    }
}

pub fn paint_callout(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    well: DipRect,
    modifiers: Modifiers,
    conflict: Option<&str>,
) -> windows::core::Result<()> {
    let mut pop = recorder_callout_above(well);
    if pop.y < 0.0 {
        pop.y = well.y + well.h + theme::spacing::SM;
    }
    let fill = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 0.16,
                g: 0.16,
                b: 0.16,
                a: 0.96,
            },
            None,
        )?
    };
    let stroke = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: theme::colors::CARD_STROKE_ALPHA,
            },
            None,
        )?
    };
    let rounded = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: pop.x,
            top: pop.y,
            right: pop.x + pop.w,
            bottom: pop.y + pop.h - theme::size::CALLOUT_CARET_HEIGHT,
        },
        radiusX: theme::radius::MENU_PANEL,
        radiusY: theme::radius::MENU_PANEL,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &fill);
        target.DrawRoundedRectangle(&rounded, &stroke, theme::size::HAIRLINE, None);
    }
    let (label, tint) = if let Some(owner) = conflict {
        (owner, D2D1_COLOR_F { r: 1.0, g: 0.55, b: 0.2, a: 1.0 })
    } else if modifiers.ctrl || modifiers.alt || modifiers.shift || modifiers.win {
        ("Add a key", D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 0.6 })
    } else {
        ("Type a shortcut", D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 0.6 })
    };
    let brush = unsafe { target.CreateSolidColorBrush(&tint, None)? };
    let wide: Vec<u16> = label.encode_utf16().collect();
    let line_y = pop.y + theme::spacing::SM + theme::size::HERO_KEY_CAP + theme::spacing::SM;
    unsafe {
        target.DrawText(
            &wide,
            formats.caption,
            &D2D_RECT_F {
                left: pop.x + theme::spacing::MD,
                top: line_y,
                right: pop.x + pop.w - theme::spacing::MD,
                bottom: line_y + theme::size::SHORTCUT_POPOVER_LINE,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let mut cap_x = pop.x + theme::spacing::MD;
    let cap_y = pop.y + theme::spacing::SM;
    let shown = if conflict.is_some() {
        Vec::new()
    } else if modifiers.ctrl || modifiers.alt || modifiers.shift || modifiers.win {
        let mut v = Vec::new();
        if modifiers.ctrl {
            v.push("Ctrl");
        }
        if modifiers.alt {
            v.push("Alt");
        }
        if modifiers.shift {
            v.push("Shift");
        }
        if modifiers.win {
            v.push("Win");
        }
        v
    } else {
        vec!["⌥", "A"]
    };
    let cap_brush = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.12,
            },
            None,
        )?
    };
    let text_brush = unsafe {
        target.CreateSolidColorBrush(
            &D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.85,
            },
            None,
        )?
    };
    for cap in shown {
        let cap_w = theme::size::HERO_KEY_CAP + 8.0;
        let cap_h = theme::size::HERO_KEY_CAP;
        let rr = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cap_x,
                top: cap_y,
                right: cap_x + cap_w,
                bottom: cap_y + cap_h,
            },
            radiusX: theme::radius::KEY_CAP,
            radiusY: theme::radius::KEY_CAP,
        };
        unsafe {
            target.FillRoundedRectangle(&rr, &cap_brush);
        }
        let wide: Vec<u16> = cap.encode_utf16().collect();
        unsafe {
            target.DrawText(
                &wide,
                formats.body,
                &rr.rect,
                &text_brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        cap_x += cap_w + theme::spacing::SM;
    }
    let esc_w = 28.0;
    let esc_h = theme::size::COMPACT_KEY_CAP;
    let esc_x = pop.x + pop.w - theme::spacing::MD - esc_w;
    let esc_y = pop.y + pop.h - theme::size::CALLOUT_CARET_HEIGHT - theme::spacing::SM - esc_h;
    let esc_r = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: esc_x,
            top: esc_y,
            right: esc_x + esc_w,
            bottom: esc_y + esc_h,
        },
        radiusX: theme::radius::KEY_CAP,
        radiusY: theme::radius::KEY_CAP,
    };
    unsafe {
        target.FillRoundedRectangle(&esc_r, &cap_brush);
    }
    let esc: Vec<u16> = "esc".encode_utf16().collect();
    unsafe {
        target.DrawText(
            &esc,
            formats.caption,
            &esc_r.rect,
            &text_brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn double_tap_mod(vk: u16) -> Option<DoubleTapModifier> {
    match vk {
        0x11 | 0xA2 | 0xA3 => Some(DoubleTapModifier::Control),
        0x12 | 0xA4 | 0xA5 => Some(DoubleTapModifier::Option),
        0x10 | 0xA0 | 0xA1 => Some(DoubleTapModifier::Shift),
        0x5B | 0x5C => Some(DoubleTapModifier::Command),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_is_not_focusable_state() {
        let mut r = Recorder::new();
        assert!(!r.is_recording());
        r.begin("hotkey.togglePalette".into());
        assert!(r.is_recording());
        r.cancel();
        assert!(!r.is_recording());
    }

    #[test]
    fn well_caption_shows_record_listening_or_binding() {
        assert_eq!(well_caption(None, false), "Record");
        assert_eq!(well_caption(Some(""), false), "Record");
        assert_eq!(well_caption(None, true), "Listening…");
        assert_eq!(well_caption(Some("Ctrl+Space"), false), "Ctrl+Space");
        assert_eq!(well_caption(Some("Ctrl+Space"), true), "Listening…");
    }

    #[test]
    fn recorder_commits_double_tap_on_second_release() {
        let mut r = Recorder::new();
        r.begin("hotkey.togglePalette".into());
        let none = Modifiers::none();
        assert_eq!(r.on_keydown(0x11, none, 0), CaptureOutcome::Ignore);
        assert_eq!(r.on_keyup(0x11, 100), CaptureOutcome::Ignore);
        assert_eq!(r.on_keydown(0x11, none, 200), CaptureOutcome::Ignore);
        match r.on_keyup(0x11, 300) {
            CaptureOutcome::Commit(HotKeyBinding::DoubleTap(DoubleTapModifier::Control)) => {}
            other => panic!("{other:?}"),
        }
    }
}
