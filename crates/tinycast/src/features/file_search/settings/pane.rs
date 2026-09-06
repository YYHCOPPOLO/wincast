//! Settings → File Search: enable, folders, ignore patterns.

use tinycast_pure::theme;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
    SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindow, LoadCursorW,
    RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WNDCLASSW, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_POPUP, WS_TABSTOP, WS_VISIBLE,
};

use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::Formats;
use crate::platform::screens::dip_scalar_to_px;

const ROW_H: f32 = 52.0;
const ITEM_H: f32 = 36.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;

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
    y += ROW_H + theme::spacing::XL;
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

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    scopes: &[String],
    ignores: &[String],
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let (section, _, _) =
        ds::feature_switch_section(width, ds::CARD_INSET - scroll, section_header(), false);
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, 0)?;
    let origin = -scroll;
    let l = layout(scopes.len(), ignores.len());
    paint_toggle(
        target,
        formats,
        ENABLE_TITLE,
        ENABLE_SUBTITLE,
        enabled,
        l.enable + origin,
        width,
    )?;
    header(target, formats, "Folders", 24.0, l.enable + ROW_H + theme::spacing::SM + origin, width)?;
    for (i, scope) in scopes.iter().enumerate() {
        paint_item(target, formats, scope, "Remove", l.scopes[i] + origin, width)?;
    }
    paint_item(target, formats, ADD_FOLDER, "", l.add_folder + origin, width)?;
    header(
        target,
        formats,
        "Ignore patterns",
        24.0,
        l.add_folder + ITEM_H + theme::spacing::SM + origin,
        width,
    )?;
    for (i, pattern) in ignores.iter().enumerate() {
        paint_item(target, formats, pattern, "Remove", l.ignores[i] + origin, width)?;
    }
    paint_item(target, formats, ADD_IGNORE, "", l.add_ignore + origin, width)?;
    Ok(())
}

fn header(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    text: &str,
    x: f32,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.55,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            formats.header,
            &D2D_RECT_F {
                left: x,
                top: y,
                right: width - 24.0,
                bottom: y + 20.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn paint_toggle(
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
    let toggle = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: width - pad - TOGGLE_W,
            top: y + (ROW_H - TOGGLE_H) / 2.0,
            right: width - pad,
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

fn paint_item(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    trailing: &str,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
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
                right: width - pad * 2.0 - 64.0,
                bottom: y + ITEM_H - 4.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    if !trailing.is_empty() {
        let muted = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.55,
        };
        let muted_brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
        let t: Vec<u16> = trailing.encode_utf16().collect();
        unsafe {
            target.DrawText(
                &t,
                formats.caption,
                &D2D_RECT_F {
                    left: width - pad - 64.0,
                    top: y + 8.0,
                    right: width - pad,
                    bottom: y + ITEM_H - 4.0,
                },
                &muted_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
    Ok(())
}

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

pub fn ask_pattern(owner: HWND) -> Option<String> {
    ask_string(owner, "Ignore pattern")
}

fn ask_string(owner: HWND, title: &str) -> Option<String> {
    let hwnd = create_prompt(owner, title).ok()?;
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() {
            let ok = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if !ok.as_bool() {
                break;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u16 == 0x1B {
                let _ = DestroyWindow(hwnd);
                continue;
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        take_prompt()
    }
}

const PROMPT_CLASS: windows::core::PCWSTR = w!("TinycastTextPrompt");
const ID_EDIT: usize = 101;
const ID_OK: usize = 102;
const ID_CANCEL: usize = 103;

struct PromptInner {
    edit: HWND,
    result: Option<String>,
}

static LAST_PROMPT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn take_prompt() -> Option<String> {
    LAST_PROMPT.lock().ok().and_then(|mut g| g.take())
}

fn create_prompt(owner: HWND, title: &str) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(prompt_wndproc),
            hInstance: hinstance.into(),
            lpszClassName: PROMPT_CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let _ = RegisterClassW(&class);
        let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(owner).max(96);
        let w = dip_scalar_to_px(360.0, dpi);
        let h = dip_scalar_to_px(140.0, dpi);
        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PROMPT_CLASS,
            PCWSTR(title_wide.as_ptr()),
            WINDOW_STYLE(WS_POPUP.0 | 0x00C0_0000 | 0x0008_0000),
            200,
            200,
            w,
            h,
            owner,
            None,
            hinstance,
            None,
        )?;
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | 0x0080),
            16,
            16,
            w - 32,
            dip_scalar_to_px(28.0, dpi),
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_EDIT as *mut core::ffi::c_void),
            hinstance,
            None,
        )?;
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Add"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | 1),
            w - 160,
            h - 56,
            64,
            28,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_OK as *mut core::ffi::c_void),
            hinstance,
            None,
        )?;
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Cancel"),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0),
            w - 88,
            h - 56,
            72,
            28,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(ID_CANCEL as *mut core::ffi::c_void),
            hinstance,
            None,
        )?;
        let inner = Box::new(PromptInner {
            edit,
            result: None,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(inner) as isize);
        Ok(hwnd)
    }
}

unsafe extern "system" fn prompt_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            if id == ID_OK || id == ID_CANCEL {
                finish_prompt(hwnd, id == ID_OK);
            }
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u16 == 0x0D => {
            finish_prompt(hwnd, true);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_CLOSE => {
            finish_prompt(hwnd, false);
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let inner = Box::from_raw(ptr as *mut PromptInner);
                if let Ok(mut g) = LAST_PROMPT.lock() {
                    *g = inner.result;
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn finish_prompt(hwnd: HWND, ok: bool) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr != 0 && ok {
        let inner = &mut *(ptr as *mut PromptInner);
        let len = GetWindowTextLengthW(inner.edit) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(inner.edit, &mut buf);
        if n > 0 {
            inner.result = Some(String::from_utf16_lossy(&buf[..n as usize]));
        }
    }
    let _ = DestroyWindow(hwnd);
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
