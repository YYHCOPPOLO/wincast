//! Clipboard settings: retention, disabled apps, clear history.

use std::path::Path;

use tinycast_pure::theme;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

use crate::design_system::settings as ds;

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
