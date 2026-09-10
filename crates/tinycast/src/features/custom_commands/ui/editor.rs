//! Custom-command editor sheet. Width is 480 DIP.

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsWindow, LoadCursorW, RegisterClassW,
    SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOP, IDC_ARROW, MSG, SW_SHOW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WNDCLASSW, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_POPUP, WS_TABSTOP, WS_VISIBLE,
};

use crate::platform::screens::dip_scalar_to_px;

pub const EDITOR_WIDTH_DIP: f32 = 480.0;
const HEIGHT_DIP: f32 = 260.0;
const CLASS: windows::core::PCWSTR = w!("TinycastCommandEditor");
const ID_NAME: usize = 101;
const ID_COMMAND: usize = 102;
const ID_CONFIRM: usize = 103;
const ID_SAVE: usize = 104;
const ID_CANCEL: usize = 105;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDraft {
    pub name: String,
    pub command: String,
    pub confirm: bool,
}

struct Inner {
    name: HWND,
    command: HWND,
    confirm: HWND,
    result: Option<CommandDraft>,
}

#[derive(Clone, Copy)]
pub struct EditorLabels {
    pub title: &'static str,
    pub value_label: &'static str,
    pub show_confirm: bool,
}

pub const COMMAND_LABELS: EditorLabels = EditorLabels {
    title: "Custom Command",
    value_label: "Command",
    show_confirm: true,
};

pub const QUICKLINK_LABELS: EditorLabels = EditorLabels {
    title: "Quicklink",
    value_label: "Destination",
    show_confirm: false,
};

pub fn edit(owner: HWND, initial: Option<&CommandDraft>) -> Option<CommandDraft> {
    edit_lang(owner, initial, tinycast_pure::i18n::UiLang::En)
}

pub fn edit_lang(
    owner: HWND,
    initial: Option<&CommandDraft>,
    lang: tinycast_pure::i18n::UiLang,
) -> Option<CommandDraft> {
    edit_with(owner, initial, COMMAND_LABELS, lang)
}

pub fn edit_with(
    owner: HWND,
    initial: Option<&CommandDraft>,
    labels: EditorLabels,
    lang: tinycast_pure::i18n::UiLang,
) -> Option<CommandDraft> {
    let hwnd = create(owner, initial, labels, lang).ok()?;
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
    }
    take_result()
}

static LAST: std::sync::Mutex<Option<CommandDraft>> = std::sync::Mutex::new(None);

fn take_result() -> Option<CommandDraft> {
    LAST.lock().ok()?.take()
}

fn create(
    owner: HWND,
    initial: Option<&CommandDraft>,
    labels: EditorLabels,
    lang: tinycast_pure::i18n::UiLang,
) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let atom = RegisterClassW(&class);
        if atom == 0 {
            let last = windows::Win32::Foundation::GetLastError();
            if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(last.into());
            }
        }
        let inner = Box::new(Inner {
            name: HWND::default(),
            command: HWND::default(),
            confirm: HWND::default(),
            result: None,
        });
        let ptr = Box::into_raw(inner);
        let mut title_wide: Vec<u16> = labels
            .title
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS,
            windows::core::PCWSTR(title_wide.as_mut_ptr()),
            WS_POPUP | WINDOW_STYLE(0x00C00000) | WINDOW_STYLE(0x00080000), // caption | sysmenu
            0,
            0,
            480,
            260,
            owner,
            None,
            hinstance,
            Some(ptr as *const core::ffi::c_void),
        ) {
            Ok(h) => h,
            Err(err) => {
                drop(Box::from_raw(ptr));
                return Err(err);
            }
        };
        let dpi = GetDpiForWindow(hwnd);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            200,
            200,
            dip_scalar_to_px(EDITOR_WIDTH_DIP, dpi),
            dip_scalar_to_px(HEIGHT_DIP, dpi),
            windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
        );
        let inner = &mut *ptr;
        inner.name = edit_field(hwnd, ID_NAME, 20, 36, 440, 24, dpi)?;
        inner.command = edit_field(hwnd, ID_COMMAND, 20, 88, 440, 24, dpi)?;
        if labels.show_confirm {
            inner.confirm = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                windows::core::PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x0003), // BS_AUTOCHECKBOX
                dip_scalar_to_px(20.0, dpi),
                dip_scalar_to_px(128.0, dpi),
                dip_scalar_to_px(240.0, dpi),
                dip_scalar_to_px(24.0, dpi),
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::HMENU(
                    ID_CONFIRM as *mut core::ffi::c_void,
                ),
                hinstance,
                None,
            )?;
            set_text(
                inner.confirm,
                tinycast_pure::i18n::editor_needs_confirmation(lang),
            );
        }
        let _ = button(
            hwnd,
            ID_SAVE,
            tinycast_pure::i18n::editor_save(lang),
            250,
            190,
            dpi,
        )?;
        let _ = button(
            hwnd,
            ID_CANCEL,
            tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang),
            350,
            190,
            dpi,
        )?;
        let _ = label(
            hwnd,
            tinycast_pure::i18n::editor_name_label(lang),
            20,
            16,
            dpi,
        );
        let _ = label(hwnd, labels.value_label, 20, 68, dpi);
        if let Some(init) = initial {
            set_text(inner.name, &init.name);
            set_text(inner.command, &init.command);
            if init.confirm {
                let _ = SendMessageW(inner.confirm, 0x00F1, WPARAM(1), LPARAM(0));
                // BM_SETCHECK
            }
        }
        let _ = SetFocus(inner.name);
        Ok(hwnd)
    }
}

fn edit_field(
    parent: HWND,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    dpi: u32,
) -> windows::core::Result<HWND> {
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x0080), // ES_AUTOHSCROLL
            dip_scalar_to_px(x as f32, dpi),
            dip_scalar_to_px(y as f32, dpi),
            dip_scalar_to_px(w as f32, dpi),
            dip_scalar_to_px(h as f32, dpi),
            parent,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as *mut core::ffi::c_void),
            GetModuleHandleW(None)?,
            None,
        )
    }
}

fn button(
    parent: HWND,
    id: usize,
    title: &str,
    x: i32,
    y: i32,
    dpi: u32,
) -> windows::core::Result<HWND> {
    let mut wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            windows::core::PCWSTR(wide.as_mut_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            dip_scalar_to_px(x as f32, dpi),
            dip_scalar_to_px(y as f32, dpi),
            dip_scalar_to_px(88.0, dpi),
            dip_scalar_to_px(28.0, dpi),
            parent,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as *mut core::ffi::c_void),
            GetModuleHandleW(None)?,
            None,
        )
    }
}

fn label(parent: HWND, title: &str, x: i32, y: i32, dpi: u32) -> windows::core::Result<HWND> {
    let mut wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            windows::core::PCWSTR(wide.as_mut_ptr()),
            WS_CHILD | WS_VISIBLE,
            dip_scalar_to_px(x as f32, dpi),
            dip_scalar_to_px(y as f32, dpi),
            dip_scalar_to_px(200.0, dpi),
            dip_scalar_to_px(16.0, dpi),
            parent,
            None,
            GetModuleHandleW(None)?,
            None,
        )
    }
}

fn set_text(hwnd: HWND, text: &str) {
    let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(wide.as_mut_ptr()));
    }
}

fn get_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

fn collect(inner: &Inner) -> CommandDraft {
    CommandDraft {
        name: get_text(inner.name),
        command: get_text(inner.command),
        confirm: checkbox_checked(inner.confirm),
    }
}

fn checkbox_checked(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    unsafe { SendMessageW(hwnd, 0x00F0, WPARAM(0), LPARAM(0)).0 == 1 }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        0x0081 => {
            let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            let id = (wparam.0 as u16) as usize;
            if id == ID_SAVE {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
                if !ptr.is_null() {
                    (*ptr).result = Some(collect(&*ptr));
                    if let Ok(mut slot) = LAST.lock() {
                        *slot = (*ptr).result.clone();
                    }
                }
                let _ = DestroyWindow(hwnd);
            } else if id == ID_CANCEL {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u16 == 0x1B {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ptr.is_null() {
                if let Some(result) = (*ptr).result.clone() {
                    if let Ok(mut slot) = LAST.lock() {
                        *slot = Some(result);
                    }
                }
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_sheet_is_480_dip() {
        assert_eq!(EDITOR_WIDTH_DIP, 480.0);
    }
}
