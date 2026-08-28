use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetStockObject, SetBkMode, SetTextColor, HBRUSH, HDC, TRANSPARENT, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, IsWindow, LoadCursorW, RegisterClassW,
    ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, SW_HIDE, SW_SHOW,
    WINDOW_EX_STYLE, WM_CLOSE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_NCCREATE, WM_NCDESTROY, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
};

const CLASS: windows::core::PCWSTR = w!("TinycastStub");

pub struct StubWindow {
    pub hwnd: HWND,
}

impl StubWindow {
    pub fn about(host: HWND) -> windows::core::Result<Self> {
        create(
            host,
            w!("About Tinycast"),
            w!("About Tinycast — stub until plan 05."),
        )
    }

    pub fn support(host: HWND) -> windows::core::Result<Self> {
        create(
            host,
            w!("Support Tinycast"),
            w!("Support Tinycast — stub until plan 05."),
        )
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }
}

impl Drop for StubWindow {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

fn create(host: HWND, title: PCWSTR, body: PCWSTR) -> windows::core::Result<StubWindow> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
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
            CLASS,
            title,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            200,
            200,
            420,
            180,
            host,
            None,
            hinstance,
            None,
        )?;
        let _label = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            body,
            WS_CHILD | WS_VISIBLE,
            16,
            16,
            380,
            80,
            hwnd,
            None,
            hinstance,
            None,
        )?;
        Ok(StubWindow { hwnd })
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() {
                windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00111111));
            LRESULT(unsafe { GetStockObject(WHITE_BRUSH) }.0 as isize)
        }
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY | WM_NCDESTROY => DefWindowProcW(hwnd, msg, wparam, lparam),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
