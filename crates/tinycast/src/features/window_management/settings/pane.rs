//! Settings → Window Management: enable, show in launcher, gap, cycle, per-command recorders.

use tinycast_pure::hotkey_store::HotKeyStore;
use tinycast_pure::theme;
use tinycast_pure::visibility::VisibilityStore;
use tinycast_pure::window_command::{WindowCommandId, WindowGroup};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;

const ROW_H: f32 = 52.0;
const CMD_H: f32 = 40.0;
const HEADER_H: f32 = 22.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;
const RECORDER_W: f32 = 140.0;
const CLEAR_W: f32 = 22.0;

pub fn section_header() -> &'static str {
    "Window Management"
}

pub const ENABLE_TITLE: &str = "Window Management";
pub const ENABLE_SUBTITLE: &str = "Move and resize the frontmost window. Off by default.";
pub const SHOW_IN_LAUNCHER: &str = "Show in launcher";
pub const CYCLE_TITLE: &str = "Cycle halves on repeat";
pub const GAP_TITLE: &str = "Gap";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowHit {
    Enable,
    ShowInLauncher,
    Cycle,
    Gap,
    Visible(usize),
    Recorder(usize),
    RecorderClear(usize),
}

pub fn catalog_commands() -> Vec<WindowCommandId> {
    WindowCommandId::all().to_vec()
}

pub fn toggles_bottom() -> f32 {
    ds::form_origin() + ROW_H * 4.0 + theme::spacing::XL * 3.0
}

fn group_blocks() -> Vec<(WindowGroup, Vec<WindowCommandId>)> {
    WindowGroup::all()
        .iter()
        .copied()
        .map(|group| {
            let cmds: Vec<WindowCommandId> = WindowCommandId::all()
                .iter()
                .copied()
                .filter(|id| id.group() == group)
                .collect();
            (group, cmds)
        })
        .filter(|(_, cmds)| !cmds.is_empty())
        .collect()
}

struct CatalogLayout {
    headers: Vec<(f32, &'static str)>,
    commands: Vec<(usize, f32)>,
    bottom: f32,
}

fn catalog_layout() -> CatalogLayout {
    let mut y = toggles_bottom() + theme::spacing::XL;
    let mut headers = Vec::new();
    let mut commands = Vec::new();
    let all = WindowCommandId::all();
    for (group, cmds) in group_blocks() {
        headers.push((y, group.title()));
        y += HEADER_H;
        for id in cmds {
            let index = all.iter().position(|c| *c == id).unwrap_or(0);
            commands.push((index, y));
            y += CMD_H;
        }
        y += theme::spacing::SM;
    }
    CatalogLayout {
        headers,
        commands,
        bottom: y + 24.0,
    }
}

pub fn content_height() -> f32 {
    catalog_layout().bottom
}

pub fn hit(_x: f32, y: f32, scroll: f32, width: f32) -> Option<WindowHit> {
    let y = y + scroll;
    let mut row = ds::form_origin();
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::Enable);
    }
    row += ROW_H + theme::spacing::XL;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::ShowInLauncher);
    }
    row += ROW_H + theme::spacing::XL;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::Cycle);
    }
    row += ROW_H + theme::spacing::XL;
    if y >= row && y < row + ROW_H {
        return Some(WindowHit::Gap);
    }
    let layout = catalog_layout();
    for (index, top) in layout.commands {
        if y < top || y >= top + CMD_H {
            continue;
        }
        let (rec_left, clear_left) = command_wells(width);
        if _x >= clear_left {
            return Some(WindowHit::RecorderClear(index));
        }
        if _x >= rec_left {
            return Some(WindowHit::Recorder(index));
        }
        return Some(WindowHit::Visible(index));
    }
    None
}

/// Detail-space x of the Record well and clear control. Paint adds `origin_x`.
fn command_wells(width: f32) -> (f32, f32) {
    let pad = theme::spacing::XL;
    let clear_left = width - pad - CLEAR_W;
    let rec_left = clear_left - 8.0 - RECORDER_W;
    (rec_left, clear_left)
}

pub fn cycle_gap(current: i32) -> i32 {
    match current {
        0 => 8,
        8 => 16,
        16 => 24,
        _ => 0,
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    show_in_launcher: bool,
    cycle: bool,
    gap: i32,
    visibility: &VisibilityStore,
    hotkeys: &HotKeyStore,
    recording: Option<&str>,
    origin_x: f32,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let (section, _, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        section_header(),
        true,
    );
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, 0)?;
    let origin = -scroll;
    let mut y = ds::form_origin() + origin;
    paint_row(target, formats, ENABLE_TITLE, ENABLE_SUBTITLE, enabled, y, origin_x, width)?;
    y += ROW_H + theme::spacing::XL;
    paint_row(
        target,
        formats,
        SHOW_IN_LAUNCHER,
        "Hide the Window Management section without disabling shortcuts.",
        show_in_launcher,
        y,
        origin_x,
        width,
    )?;
    y += ROW_H + theme::spacing::XL;
    paint_row(
        target,
        formats,
        CYCLE_TITLE,
        "Repeated Left/Right/Top/Bottom Half cycles ½ → ⅓ → ⅔.",
        cycle,
        y,
        origin_x,
        width,
    )?;
    y += ROW_H + theme::spacing::XL;
    paint_row(
        target,
        formats,
        GAP_TITLE,
        &format!("{gap} pt between tiles and screen edges."),
        gap > 0,
        y,
        origin_x,
        width,
    )?;
    let layout = catalog_layout();
    for (top, title) in layout.headers {
        paint_header(target, formats, title, top + origin, origin_x, width)?;
    }
    for (index, top) in layout.commands {
        let id = WindowCommandId::all()[index];
        let key = format!("hotkey.windowCommand.{}", id.raw());
        let label = hotkeys.get(&key).map(|b| b.label()).unwrap_or_else(|| "Record".into());
        let rec_label = if recording == Some(key.as_str()) {
            "Recording…".into()
        } else {
            label
        };
        paint_command(
            target,
            formats,
            id.name(),
            &rec_label,
            visibility.is_item_visible(&id.entry_id()),
            hotkeys.get(&key).is_some(),
            top + origin,
            origin_x,
            width,
        )?;
    }
    Ok(())
}

fn paint_header(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    y: f32,
    origin_x: f32,
    width: f32,
) -> windows::core::Result<()> {
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.55,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let wide: Vec<u16> = title.encode_utf16().collect();
    let pad = theme::spacing::XL;
    unsafe {
        target.DrawText(
            &wide,
            formats.caption,
            &D2D_RECT_F {
                left: origin_x + pad,
                top: y,
                right: origin_x + width - pad,
                bottom: y + HEADER_H,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_command(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    name: &str,
    rec: &str,
    visible: bool,
    bound: bool,
    y: f32,
    origin_x: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: if visible { 0.92 } else { 0.4 },
    };
    let brush = unsafe { target.CreateSolidColorBrush(&white, None)? };
    let name_w: Vec<u16> = name.encode_utf16().collect();
    let rec_w: Vec<u16> = rec.encode_utf16().collect();
    let (rec_left, clear_left) = command_wells(width);
    unsafe {
        target.DrawText(
            &name_w,
            formats.body,
            &D2D_RECT_F {
                left: origin_x + pad,
                top: y + 8.0,
                right: origin_x + rec_left - 8.0,
                bottom: y + CMD_H - 4.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.DrawText(
            &rec_w,
            formats.caption,
            &D2D_RECT_F {
                left: origin_x + rec_left,
                top: y + 10.0,
                right: origin_x + rec_left + RECORDER_W,
                bottom: y + CMD_H - 6.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        if bound {
            let x: Vec<u16> = "×".encode_utf16().collect();
            target.DrawText(
                &x,
                formats.body,
                &D2D_RECT_F {
                    left: origin_x + clear_left,
                    top: y + 8.0,
                    right: origin_x + width - pad,
                    bottom: y + CMD_H - 4.0,
                },
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
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
    origin_x: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let text_w = width - pad * 3.0 - TOGGLE_W;
    let title_rect = D2D_RECT_F {
        left: origin_x + pad,
        top: y + 8.0,
        right: origin_x + pad + text_w,
        bottom: y + 28.0,
    };
    let sub_rect = D2D_RECT_F {
        left: origin_x + pad,
        top: y + 28.0,
        right: origin_x + pad + text_w,
        bottom: y + ROW_H - 4.0,
    };
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
            &title_rect,
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
            &sub_rect,
            &muted_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let toggle = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: origin_x + width - pad - TOGGLE_W,
            top: y + (ROW_H - TOGGLE_H) / 2.0,
            right: origin_x + width - pad,
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
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.18,
        }
    };
    let toggle_brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe {
        target.FillRoundedRectangle(&toggle, &toggle_brush);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_management_hits_four_rows() {
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0, 400.0),
            Some(WindowHit::Enable)
        );
        assert_eq!(cycle_gap(0), 8);
        assert_eq!(cycle_gap(24), 0);
    }

    #[test]
    fn window_management_lists_all_commands_with_recorders() {
        assert_eq!(catalog_commands().len(), 34);
        assert!(content_height() > toggles_bottom() + 34.0 * 20.0);
        let layout = catalog_layout();
        assert_eq!(layout.commands.len(), 34);
        let (_, first_y) = layout.commands[0];
        let rec = hit(350.0, first_y + 8.0, 0.0, 400.0);
        assert_eq!(rec, Some(WindowHit::Recorder(0)));
        let vis = hit(20.0, first_y + 8.0, 0.0, 400.0);
        assert_eq!(vis, Some(WindowHit::Visible(0)));
        assert_eq!(
            WindowCommandId::all()[0].entry_id(),
            "window-command:left-half"
        );
    }

    #[test]
    fn painted_record_well_hits_recorder_after_sidebar_translate() {
        let origin_x = theme::size::SETTINGS_SIDEBAR;
        let detail_w = theme::size::SETTINGS_WINDOW.0 - origin_x;
        let (rec_left, clear_left) = command_wells(detail_w);
        let layout = catalog_layout();
        let (_, first_y) = layout.commands[0];
        // paint_command draws Record at origin_x + rec_left (HWND space).
        // settings hit() gets detail_x = window_x - sidebar.
        let painted_record_window_x = origin_x + rec_left + 1.0;
        let detail_x = painted_record_window_x - origin_x;
        assert_eq!(
            hit(detail_x, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::Recorder(0))
        );
        assert_eq!(
            hit(rec_left - 1.0, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::Visible(0))
        );
        assert_eq!(
            hit(clear_left + 1.0, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::RecorderClear(0))
        );
        // Pre-fix paint used HWND x=rec_left without origin; that click is Visible.
        let old_paint_as_detail = rec_left - origin_x;
        assert!(old_paint_as_detail > 0.0);
        assert_eq!(
            hit(old_paint_as_detail + 1.0, first_y + 8.0, 0.0, detail_w),
            Some(WindowHit::Visible(0))
        );
    }
}
