use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, GetWindowLongPtrW, LoadIconW, PostMessageW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, SetWindowLongPtrW, TrackPopupMenu, CREATESTRUCTW, GWLP_USERDATA,
    HWND_MESSAGE, IDI_APPLICATION, MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NCCREATE,
    WM_RBUTTONUP, WNDCLASSW,
};

use super::messages::{WM_OPEN_SETTINGS, WM_QUIT_APP, WM_TOGGLE_PALETTE, WM_TRAY};
use crate::app_core::AppCore;

const TRAY_ID: u32 = 1;
const ID_SETTINGS: usize = 1;
const ID_QUIT: usize = 2;
const HOST_CLASS: windows::core::PCWSTR = w!("TinycastHost");

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
            WINDOW_EX_STYLE::default(),
            HOST_CLASS,
            w!("Tinycast"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            hinstance,
            Some(core as *mut AppCore as *const core::ffi::c_void),
        )?;
        add_icon(hwnd)?;
        Ok(hwnd)
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
    let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, w!("Settings"));
    let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, w!("Quit Tinycast"));

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
        WM_OPEN_SETTINGS => LRESULT(0),
        WM_QUIT_APP => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(core) = core_from(hwnd) {
                (*core).palette_window = None;
            }
            remove_icon(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
