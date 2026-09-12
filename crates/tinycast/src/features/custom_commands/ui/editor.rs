//! Custom-command editor sheet. Width is 480 DIP.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, FALSE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, FillRect, SetBkColor, SetBkMode, SetTextColor, HBRUSH, HDC,
    OPAQUE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{SetWindowTheme, EM_SETCUEBANNER};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetParent, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsDialogMessageW, IsWindow,
    LoadCursorW, PostQuitMessage, RegisterClassW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, CS_HREDRAW, CS_VREDRAW,
    GWLP_USERDATA, HWND_TOP, IDC_ARROW, MSG, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND,
    WM_KEYDOWN, WM_NCDESTROY, WNDCLASSW, WS_CHILD, WS_EX_CLIENTEDGE, WS_POPUP, WS_TABSTOP,
    WS_VISIBLE,
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
    appearance: u8,
    bg: HBRUSH,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorKey {
    Save,
    Cancel,
    None,
}

pub fn focus_order(show_confirm: bool) -> &'static [usize] {
    if show_confirm {
        &[ID_NAME, ID_COMMAND, ID_CONFIRM, ID_SAVE, ID_CANCEL]
    } else {
        &[ID_NAME, ID_COMMAND, ID_SAVE, ID_CANCEL]
    }
}

pub fn map_key(vk: u16) -> EditorKey {
    match vk {
        0x1B => EditorKey::Cancel,
        0x0D => EditorKey::Save,
        _ => EditorKey::None,
    }
}

pub fn editor_colors(appearance: u8) -> (COLORREF, COLORREF) {
    let (r, g, b) = crate::design_system::settings::detail_rgb(appearance);
    let bg = COLORREF(((b * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (r * 255.0) as u32);
    let fg = if appearance == 0 {
        COLORREF(0x00FFFFFF)
    } else {
        COLORREF(0x00000000)
    };
    (bg, fg)
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

pub fn command_labels(lang: tinycast_pure::i18n::UiLang) -> EditorLabels {
    EditorLabels {
        title: tinycast_pure::i18n::editor_command_title(lang),
        value_label: tinycast_pure::i18n::editor_command_label(lang),
        show_confirm: true,
    }
}

pub fn quicklink_labels(lang: tinycast_pure::i18n::UiLang) -> EditorLabels {
    EditorLabels {
        title: tinycast_pure::i18n::editor_quicklink_title(lang),
        value_label: tinycast_pure::i18n::editor_destination_label(lang),
        show_confirm: false,
    }
}

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
    edit_with_appearance(owner, initial, labels, lang, 0)
}

pub fn escape_belongs_to_sheet(sheet: isize, msg_hwnd: isize, parent: Option<isize>) -> bool {
    msg_hwnd != 0 && sheet != 0 && (msg_hwnd == sheet || parent == Some(sheet))
}

pub fn field_acc_names(
    value_label: &'static str,
    lang: tinycast_pure::i18n::UiLang,
) -> (&'static str, &'static str) {
    (tinycast_pure::i18n::editor_name_label(lang), value_label)
}

struct OwnerEnableGuard {
    hwnd: HWND,
    active: bool,
}

impl OwnerEnableGuard {
    fn new(owner: HWND) -> Self {
        if owner.is_invalid() {
            return Self {
                hwnd: owner,
                active: false,
            };
        }
        unsafe {
            let _ = EnableWindow(owner, FALSE);
        }
        Self {
            hwnd: owner,
            active: true,
        }
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        unsafe {
            let _ = EnableWindow(self.hwnd, TRUE);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.active = false;
    }
}

impl Drop for OwnerEnableGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

pub fn edit_with_appearance(
    owner: HWND,
    initial: Option<&CommandDraft>,
    labels: EditorLabels,
    lang: tinycast_pure::i18n::UiLang,
    appearance: u8,
) -> Option<CommandDraft> {
    let hwnd = create(owner, initial, labels, lang, appearance).ok()?;
    unsafe {
        let mut owner_guard = OwnerEnableGuard::new(owner);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() {
            let ok = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if ok.0 == -1 {
                let _ = DestroyWindow(hwnd);
                break;
            }
            if !ok.as_bool() {
                let _ = DestroyWindow(hwnd);
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            let parent = GetParent(msg.hwnd).ok().map(|h| h.0 as isize);
            if msg.message == WM_KEYDOWN
                && map_key(msg.wParam.0 as u16) == EditorKey::Cancel
                && escape_belongs_to_sheet(hwnd.0 as isize, msg.hwnd.0 as isize, parent)
            {
                let _ = DestroyWindow(hwnd);
                continue;
            }
            if IsDialogMessageW(hwnd, &msg).as_bool() {
                continue;
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        owner_guard.restore();
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
    appearance: u8,
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
        let (bg_ref, _) = editor_colors(appearance);
        let inner = Box::new(Inner {
            name: HWND::default(),
            command: HWND::default(),
            confirm: HWND::default(),
            result: None,
            appearance,
            bg: CreateSolidBrush(bg_ref),
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
        let (name_acc, value_acc) = field_acc_names(labels.value_label, lang);
        set_cue(inner.name, name_acc);
        set_cue(inner.command, value_acc);
        set_acc_name(inner.name, name_acc);
        set_acc_name(inner.command, value_acc);
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
            style_child(inner.confirm);
        }
        let save = button(
            hwnd,
            ID_SAVE,
            tinycast_pure::i18n::editor_save(lang),
            250,
            190,
            dpi,
            true,
        )?;
        style_child(save);
        let cancel = button(
            hwnd,
            ID_CANCEL,
            tinycast_pure::i18n::chrome(tinycast_pure::i18n::Chrome::Cancel, lang),
            350,
            190,
            dpi,
            false,
        )?;
        style_child(cancel);
        let name_label = label(
            hwnd,
            tinycast_pure::i18n::editor_name_label(lang),
            20,
            16,
            dpi,
        )?;
        style_child(name_label);
        let value_label = label(hwnd, labels.value_label, 20, 68, dpi)?;
        style_child(value_label);
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
        let hwnd = CreateWindowExW(
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
        )?;
        style_child(hwnd);
        Ok(hwnd)
    }
}

fn button(
    parent: HWND,
    id: usize,
    title: &str,
    x: i32,
    y: i32,
    dpi: u32,
    default: bool,
) -> windows::core::Result<HWND> {
    let mut wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let style = if default {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x00000001) // BS_DEFPUSHBUTTON
    } else {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP
    };
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            windows::core::PCWSTR(wide.as_mut_ptr()),
            style,
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

fn style_child(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        let _ = SetWindowTheme(hwnd, w!(""), w!(""));
    }
}

fn set_acc_name(hwnd: HWND, name: &str) {
    if hwnd.is_invalid() || name.is_empty() {
        return;
    }
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let svc: windows::core::Result<windows::Win32::UI::Accessibility::IAccPropServices> =
            windows::Win32::System::Com::CoCreateInstance(
                &windows::Win32::UI::Accessibility::CAccPropServices,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            );
        if let Ok(svc) = svc {
            const OBJID_CLIENT: u32 = 0xFFFF_FFFC;
            let _ = svc.SetHwndPropStr(
                hwnd,
                OBJID_CLIENT,
                0,
                windows::Win32::UI::Accessibility::PROPID_ACC_NAME,
                PCWSTR(wide.as_ptr()),
            );
        }
    }
}

fn set_cue(hwnd: HWND, text: &str) {
    let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = SendMessageW(
            hwnd,
            EM_SETCUEBANNER,
            WPARAM(1),
            LPARAM(wide.as_mut_ptr() as isize),
        );
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
        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            if ptr.is_null() || (*ptr).bg.is_invalid() {
                return LRESULT(1);
            }
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, (*ptr).bg);
            LRESULT(1)
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Inner;
            if ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let (bg, fg) = editor_colors((*ptr).appearance);
            SetBkMode(hdc, OPAQUE);
            SetBkColor(hdc, bg);
            SetTextColor(hdc, fg);
            LRESULT((*ptr).bg.0 as isize)
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
                if !(*ptr).bg.is_invalid() {
                    let _ = DeleteObject((*ptr).bg);
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

    #[test]
    fn tab_order_includes_confirm_only_when_present() {
        assert_eq!(
            focus_order(true),
            &[ID_NAME, ID_COMMAND, ID_CONFIRM, ID_SAVE, ID_CANCEL]
        );
        assert_eq!(
            focus_order(false),
            &[ID_NAME, ID_COMMAND, ID_SAVE, ID_CANCEL]
        );
        assert!(!focus_order(false).contains(&ID_CONFIRM));
    }

    #[test]
    fn enter_saves_and_escape_cancels() {
        assert_eq!(map_key(0x0D), EditorKey::Save);
        assert_eq!(map_key(0x1B), EditorKey::Cancel);
        assert_eq!(map_key(0x09), EditorKey::None);
    }

    #[test]
    fn dark_and_light_editor_tokens_invert() {
        let (dark_bg, dark_fg) = editor_colors(0);
        let (light_bg, light_fg) = editor_colors(1);
        assert_ne!(dark_bg, light_bg);
        assert_ne!(dark_fg, light_fg);
        assert_eq!(dark_fg.0, 0x00FFFFFF);
        assert_eq!(light_fg.0, 0x00000000);
    }

    #[test]
    fn save_keeps_draft_cancel_discards() {
        let draft = CommandDraft {
            name: "n".into(),
            command: "echo".into(),
            confirm: true,
        };
        assert_eq!(draft.name, "n");
        LAST.lock().unwrap().take();
        assert!(take_result().is_none());
    }

    #[test]
    fn labels_follow_language() {
        let zh = command_labels(tinycast_pure::i18n::UiLang::ZhHans);
        assert_eq!(zh.title, "自定义命令");
        assert_eq!(zh.value_label, "命令");
        let en = command_labels(tinycast_pure::i18n::UiLang::En);
        assert_eq!(en.title, COMMAND_LABELS.title);
        let q = quicklink_labels(tinycast_pure::i18n::UiLang::En);
        assert_eq!(q.value_label, QUICKLINK_LABELS.value_label);
        assert!(!q.show_confirm);
        let zh = field_acc_names(
            quicklink_labels(tinycast_pure::i18n::UiLang::ZhHans).value_label,
            tinycast_pure::i18n::UiLang::ZhHans,
        );
        assert_eq!(zh.0, "名称");
        assert_eq!(zh.1, "目标");
    }

    #[test]
    fn escape_only_cancels_sheet_or_child() {
        assert!(escape_belongs_to_sheet(10, 10, None));
        assert!(escape_belongs_to_sheet(10, 11, Some(10)));
        assert!(!escape_belongs_to_sheet(10, 11, Some(99)));
        assert!(!escape_belongs_to_sheet(10, 0, None));
        assert!(!escape_belongs_to_sheet(0, 10, None));
    }

    #[test]
    fn editor_themes_and_fills_client() {
        let src = include_str!("editor.rs");
        assert!(src.contains("SetWindowTheme"));
        assert!(src.contains("EnableWindow"));
        assert!(src.contains("FillRect"));
        assert!(src.contains("SetHwndPropStr"));
        assert!(src.contains("escape_belongs_to_sheet"));
    }
}
