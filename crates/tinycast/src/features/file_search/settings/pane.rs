//! Settings → File Search: enable, folders, ignore patterns.

use tinycast_pure::theme;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
    SIGDN_FILESYSPATH,
};

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;
const ITEM_H: f32 = 36.0;

pub fn section_header() -> &'static str {
    "File Search"
}

pub const ENABLE_TITLE: &str = "Enable File Search";
pub const ENABLE_SUBTITLE: &str =
    "Find files and folders through Windows Search, only on demand. Off by default.";
pub const ADD_FOLDER: &str = "Add folder…";
pub const ADD_IGNORE: &str = "Add ignore pattern…";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSearchHit {
    Enable,
    AddFolder,
    RemoveScope(usize),
    AddIgnore,
    RemoveIgnore(usize),
}

struct Layout {
    enable: f32,
    add_folder: f32,
    scopes: Vec<f32>,
    add_ignore: f32,
    ignores: Vec<f32>,
    bottom: f32,
}

fn layout(scope_len: usize, ignore_len: usize) -> Layout {
    let mut y = ds::form_origin();
    let enable = y;
    y = ds::switch_section_next_y(false);
    y += 22.0;
    let mut scopes = Vec::new();
    for _ in 0..scope_len {
        scopes.push(y);
        y += ITEM_H;
    }
    let add_folder = y;
    y += ITEM_H + theme::spacing::XL;
    y += 22.0;
    let mut ignores = Vec::new();
    for _ in 0..ignore_len {
        ignores.push(y);
        y += ITEM_H;
    }
    let add_ignore = y;
    y += ITEM_H + 24.0;
    Layout {
        enable,
        add_folder,
        scopes,
        add_ignore,
        ignores,
        bottom: y,
    }
}

pub fn content_height(scope_len: usize, ignore_len: usize) -> f32 {
    layout(scope_len, ignore_len).bottom
}

pub fn hit(
    _x: f32,
    y: f32,
    scroll: f32,
    scope_len: usize,
    ignore_len: usize,
) -> Option<FileSearchHit> {
    let y = y + scroll;
    let l = layout(scope_len, ignore_len);
    if y >= l.enable && y < l.enable + ROW_H {
        return Some(FileSearchHit::Enable);
    }
    for (i, top) in l.scopes.iter().enumerate() {
        if y >= *top && y < *top + ITEM_H {
            return Some(FileSearchHit::RemoveScope(i));
        }
    }
    if y >= l.add_folder && y < l.add_folder + ITEM_H {
        return Some(FileSearchHit::AddFolder);
    }
    for (i, top) in l.ignores.iter().enumerate() {
        if y >= *top && y < *top + ITEM_H {
            return Some(FileSearchHit::RemoveIgnore(i));
        }
    }
    if y >= l.add_ignore && y < l.add_ignore + ITEM_H {
        return Some(FileSearchHit::AddIgnore);
    }
    None
}

/// The native folder picker the Slint settings page still calls for "Add folder".
pub fn pick_folder(owner: HWND) -> Option<String> {
    unsafe {
        let dlg: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        dlg.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        dlg.Show(owner).ok()?;
        let item: IShellItem = dlg.GetResult().ok()?;
        let path = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        Some(path.to_string().unwrap_or_default()).filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_is_first_row() {
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0, 0, 0),
            Some(FileSearchHit::Enable)
        );
        assert_eq!(
            hit(20.0, layout(0, 0).add_folder + 2.0, 0.0, 0, 0),
            Some(FileSearchHit::AddFolder)
        );
    }
}
