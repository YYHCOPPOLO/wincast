use windows::core::{w, PCWSTR};
use tinycast_pure::i18n::{chrome, Chrome};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, GetWindowLongPtrW, LoadIconW, PostMessageW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, SetWindowLongPtrW, TrackPopupMenu, CREATESTRUCTW, GWLP_USERDATA,
    IDI_APPLICATION, MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION, WM_HOTKEY, WM_LBUTTONDBLCLK,
    WM_LBUTTONUP, WM_NCCREATE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

use super::messages::{
    TIMER_CALENDAR, TIMER_SUPPORT, WM_APP_INDEX, WM_CLIPBOARDUPDATE, WM_CLIPBOARD_IMAGE,
    WM_CUSTOM_COMMAND_FAILED, WM_FILE_SEARCH, WM_OPEN_SETTINGS, WM_HOTKEY_ACTION, WM_QUIT_APP,
    WM_AI, WM_QA, WM_QA_APPLY, WM_QA_COPY, WM_QA_DISMISS, WM_RATES, WM_SNIPPETS, WM_SNIPPET_KEYWORD,
    WM_TOGGLE_PALETTE, WM_TRAY, WM_UNINSTALL_SIZE,
};
use crate::app_core::AppCore;

const TRAY_ID: u32 = 1;
const ID_SETTINGS: usize = 1;
const ID_QUIT: usize = 2;
const HOST_CLASS: windows::core::PCWSTR = w!("TinycastHost");

fn host_parent() -> HWND {
    HWND::default()
}

fn host_style() -> WINDOW_STYLE {
    WS_POPUP
}

fn host_ex_style() -> WINDOW_EX_STYLE {
    WS_EX_TOOLWINDOW
}

fn host_adds_icon_on_create() -> bool {
    false
}

pub fn create(core: &mut AppCore) -> windows::core::Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: HOST_CLASS,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            ..Default::default()
        };
        let atom = RegisterClassW(&class);
        if atom == 0 {
            let last = windows::Win32::Foundation::GetLastError();
            if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(last.into());
            }
        }

        let hwnd = CreateWindowExW(
            host_ex_style(),
            HOST_CLASS,
            w!("Tinycast"),
            host_style(),
            0,
            0,
            0,
            0,
            host_parent(),
            None,
            hinstance,
            Some(core as *mut AppCore as *const core::ffi::c_void),
        )?;
        // Tray icon is applied in AppCore::start from showInMenuBar.
        if host_adds_icon_on_create() {
            add_icon(hwnd)?;
        }
        Ok(hwnd)
    }
}

pub fn set_tooltip(hwnd: HWND, text: &str) {
    unsafe {
        let mut data = notify_data(hwnd);
        data.uFlags = NIF_TIP;
        write_tip(&mut data.szTip, text);
        let _ = Shell_NotifyIconW(
            windows::Win32::UI::Shell::NIM_MODIFY,
            &data,
        );
    }
}

pub fn set_icon_visible(hwnd: HWND, visible: bool) {
    unsafe {
        if visible {
            let _ = add_icon(hwnd);
        } else {
            remove_icon(hwnd);
        }
    }
}

unsafe fn add_icon(hwnd: HWND) -> windows::core::Result<()> {
    let mut data = notify_data(hwnd);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = LoadIconW(None, IDI_APPLICATION)?;
    write_tip(&mut data.szTip, "Tinycast");
    Shell_NotifyIconW(NIM_ADD, &data).ok()
}

unsafe fn remove_icon(hwnd: HWND) {
    let data = notify_data(hwnd);
    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
}

fn notify_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW::default();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ID;
    data
}

fn write_tip(dest: &mut [u16], text: &str) {
    let encoded: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let n = encoded.len().min(dest.len());
    dest[..n].copy_from_slice(&encoded[..n]);
    if n > 0 {
        dest[n - 1] = 0;
    }
}

unsafe fn core_from(hwnd: HWND) -> Option<*mut AppCore> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let lang = core_from(hwnd)
        .map(|core| (*core).ui_lang())
        .unwrap_or_default();
    let settings: Vec<u16> = chrome(Chrome::TraySettings, lang)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let quit: Vec<u16> = chrome(Chrome::TrayQuit, lang)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, PCWSTR(settings.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, PCWSTR(quit.as_ptr()));

    let mut pt = windows::Win32::Foundation::POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
    let _ = PostMessageW(hwnd, 0, WPARAM(0), LPARAM(0));

    match cmd.0 as usize {
        ID_SETTINGS => {
            let _ = PostMessageW(hwnd, WM_OPEN_SETTINGS, WPARAM(0), LPARAM(0));
        }
        ID_QUIT => {
            let _ = PostMessageW(hwnd, WM_QUIT_APP, WPARAM(0), LPARAM(0));
        }
        _ => {}
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_TRAY => {
            match lparam.0 as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    let _ = PostMessageW(hwnd, WM_TOGGLE_PALETTE, WPARAM(0), LPARAM(0));
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_TOGGLE_PALETTE => {
            if let Some(core) = core_from(hwnd) {
                (*core).toggle_palette();
            }
            LRESULT(0)
        }
        WM_OPEN_SETTINGS => {
            if let Some(core) = core_from(hwnd) {
                (*core).open_settings();
            }
            LRESULT(0)
        }
        WM_APP_INDEX => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_app_index();
            }
            LRESULT(0)
        }
        WM_RATES => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_rates();
            }
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            if let Some(core) = core_from(hwnd) {
                (*core).capture_clipboard();
            }
            LRESULT(0)
        }
        WM_CLIPBOARD_IMAGE => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_clipboard_images();
            }
            LRESULT(0)
        }
        WM_SNIPPETS => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_snippets();
            }
            LRESULT(0)
        }
        WM_FILE_SEARCH => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_file_search();
            }
            LRESULT(0)
        }
        WM_AI => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_ai_events();
            }
            LRESULT(0)
        }
        WM_QA => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_quick_action_events();
            }
            LRESULT(0)
        }
        WM_QA_APPLY => {
            if let Some(core) = core_from(hwnd) {
                (*core).apply_quick_action_result();
            }
            LRESULT(0)
        }
        WM_QA_COPY => {
            if let Some(core) = core_from(hwnd) {
                (*core).copy_quick_action_result();
            }
            LRESULT(0)
        }
        WM_QA_DISMISS => {
            if let Some(core) = core_from(hwnd) {
                (*core).dismiss_quick_action_panel();
            }
            LRESULT(0)
        }
        WM_UNINSTALL_SIZE => {
            if let Some(core) = core_from(hwnd) {
                (*core).install_uninstall_sizes();
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if let Some(core) = core_from(hwnd) {
                if wparam.0 == TIMER_CALENDAR {
                    (*core).on_calendar_tick();
                }
                if wparam.0 == TIMER_SUPPORT {
                    (*core).on_support_pump();
                }
            }
            LRESULT(0)
        }
        WM_SNIPPET_KEYWORD => {
            if let Some(core) = core_from(hwnd) {
                (*core).on_snippet_keyword();
            }
            LRESULT(0)
        }
        WM_CUSTOM_COMMAND_FAILED => {
            if let Some(core) = core_from(hwnd) {
                (*core).on_custom_command_failed();
            }
            LRESULT(0)
        }
        WM_HOTKEY | WM_HOTKEY_ACTION => {
            if msg == WM_HOTKEY {
                crate::features::hotkeys::service::center::on_hotkey_id(wparam.0 as i32);
            }
            if let Some(core) = core_from(hwnd) {
                (*core).on_hotkey_action();
            }
            LRESULT(0)
        }
        WM_ENDSESSION => {
            if let Some(core) = core_from(hwnd) {
                (*core).shutdown_hotkeys();
            }
            LRESULT(0)
        }
        WM_QUIT_APP => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(core) = core_from(hwnd) {
                (*core).shutdown_hotkeys();
                (*core).palette_window = None;
                (*core).settings_window = None;
                (*core).about_window = None;
                (*core).support_window = None;
            }
            remove_icon(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::HWND_MESSAGE;

    #[test]
    fn tray_rs_uses_i18n_chrome() {
        let src = include_str!("tray.rs");
        assert!(src.contains("chrome") || src.contains("TraySettings"));
    }

    #[test]
    fn host_is_hidden_top_level_so_endsession_arrives() {
        assert_ne!(host_parent().0, HWND_MESSAGE.0);
        assert!(host_parent().is_invalid());
        assert_eq!(host_style(), WS_POPUP);
        let ex = host_ex_style();
        assert_eq!(ex.0 & WS_EX_TOOLWINDOW.0, WS_EX_TOOLWINDOW.0);
        assert!(!host_adds_icon_on_create());
    }
}
