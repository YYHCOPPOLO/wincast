//! Settings → General: shortcuts, search, Hyper, appearance, launch.

use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralToggle {
    Compact,
    FavoritesInCompact,
    FollowCursor,
    Draggable,
    LaunchAtLogin,
    ShowInMenuBar,
    AutoSwitch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralHit {
    PaletteRecorder,
    ResetRanking,
    HyperKey,
    HyperShift,
    Appearance,
    Compact,
    FavoritesInCompact,
    FollowCursor,
    Draggable,
    LaunchAtLogin,
    ShowInMenuBar,
    PopToRoot,
    AutoSwitchInput,
}

pub struct GeneralState<'a> {
    pub ranking_empty: bool,
    pub hyper: &'a str,
    pub hyper_shift: bool,
    pub appearance: &'a str,
    pub compact: bool,
    pub favorites_in_compact: bool,
    pub follow_cursor: bool,
    pub draggable: bool,
    pub launch_at_login: bool,
    pub show_in_menu_bar: bool,
    pub pop_to_root: i64,
    pub auto_switch: bool,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H * 13.0 + theme::spacing::XL * 4.0
}

fn row_y(index: usize) -> f32 {
    24.0 + index as f32 * (ROW_H + theme::spacing::SM)
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<GeneralHit> {
    let y = y + scroll;
    let hits = [
        GeneralHit::PaletteRecorder,
        GeneralHit::ResetRanking,
        GeneralHit::HyperKey,
        GeneralHit::HyperShift,
        GeneralHit::Appearance,
        GeneralHit::Compact,
        GeneralHit::FavoritesInCompact,
        GeneralHit::FollowCursor,
        GeneralHit::Draggable,
        GeneralHit::LaunchAtLogin,
        GeneralHit::ShowInMenuBar,
        GeneralHit::PopToRoot,
        GeneralHit::AutoSwitchInput,
    ];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    None
}

pub fn cycle_hyper(current: &str) -> &'static str {
    match current {
        "none" => "capsLock",
        "capsLock" => "rightControl",
        "rightControl" => "rightShift",
        "rightShift" => "rightOption",
        "rightOption" => "rightCommand",
        _ => "none",
    }
}

pub fn cycle_pop_to_root(current: i64) -> i64 {
    match current {
        0 => 10,
        10 => 30,
        30 => 60,
        _ => 0,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    state: &GeneralState<'_>,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let origin = -scroll;
    let ranking_sub = if state.ranking_empty {
        "No learned ranking yet."
    } else {
        "Clear privately learned result order."
    };
    let pop_sub = format!("{} s idle timeout (0 = never).", state.pop_to_root);
    let rows = [
        (
            "Global Shortcuts",
            "Toggle palette recorder — click to rebind.",
            true,
        ),
        ("Reset learned ranking", ranking_sub, !state.ranking_empty),
        ("Hyper Key", state.hyper, state.hyper != "none"),
        (
            "Include Shift",
            "Hyper chord is Ctrl+Alt+Win+Shift.",
            state.hyper_shift,
        ),
        ("Appearance", state.appearance, true),
        (
            "Compact mode",
            "Start the palette as a compact bar.",
            state.compact,
        ),
        (
            "Show favorites in compact",
            "Keep Ctrl+1…0 slots in compact mode.",
            state.favorites_in_compact,
        ),
        (
            "Follow the cursor",
            "Open the palette on the cursor’s screen.",
            state.follow_cursor,
        ),
        (
            "Drag to reposition",
            "Drag the palette to a new anchor.",
            state.draggable,
        ),
        (
            "Launch at login",
            "Start Tinycast with Windows.",
            state.launch_at_login,
        ),
        (
            "Show in menu bar",
            "Hide the tray icon without quitting. Hotkeys keep working.",
            state.show_in_menu_bar,
        ),
        ("Pop to Root", pop_sub.as_str(), state.pop_to_root > 0),
        (
            "Auto-switch input source",
            "Switch IME when the palette opens.",
            state.auto_switch,
        ),
    ];
    for (i, (title, sub, on)) in rows.iter().enumerate() {
        paint_row(target, formats, title, sub, *on, row_y(i) + origin, width)?;
    }
    Ok(())
}

fn paint_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let text_w = width - pad * 3.0 - TOGGLE_W;
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    };
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.55,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&white, None)? };
    let title_wide: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title_wide,
            formats.body,
            &D2D_RECT_F {
                left: pad,
                top: y + 8.0,
                right: pad + text_w,
                bottom: y + 28.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let muted_brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let sub_wide: Vec<u16> = subtitle.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &sub_wide,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: y + 28.0,
                right: pad + text_w,
                bottom: y + ROW_H - 4.0,
            },
            &muted_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let fill = if on {
        D2D1_COLOR_F {
            r: 0.2,
            g: 0.55,
            b: 1.0,
            a: 1.0,
        }
    } else {
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.18,
        }
    };
    let toggle_brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: width - pad - TOGGLE_W,
                    top: y + (ROW_H - TOGGLE_H) / 2.0,
                    right: width - pad,
                    bottom: y + (ROW_H - TOGGLE_H) / 2.0 + TOGGLE_H,
                },
                radiusX: TOGGLE_H / 2.0,
                radiusY: TOGGLE_H / 2.0,
            },
            &toggle_brush,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_hits_hyper_and_menu_bar() {
        assert_eq!(hit(10.0, row_y(2) + 4.0, 0.0), Some(GeneralHit::HyperKey));
        assert_eq!(
            hit(10.0, row_y(10) + 4.0, 0.0),
            Some(GeneralHit::ShowInMenuBar)
        );
        assert_eq!(cycle_hyper("none"), "capsLock");
        assert_eq!(cycle_pop_to_root(0), 10);
    }
}
