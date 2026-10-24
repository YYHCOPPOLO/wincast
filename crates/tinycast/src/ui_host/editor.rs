//! One Slint editor sheet at a time, resumed instead of pumped: the Slint
//! replacement for the old nested-`GetMessageW` command/quicklink sheets and
//! the file-search text prompt.
//!
//! The mechanics mirror `ui_host::modal`: the surface is created on use, born
//! off-screen so winit makes its HWND, then placed and focused. The owner
//! window is disabled while the sheet is up, like the Direct2D editor's
//! `OwnerEnableGuard`. Cancel never writes storage; accept hands the draft to
//! the caller's continuation, which owns every side effect.

use std::cell::RefCell;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOMOVE,
    SWP_NOSIZE, WS_EX_TOOLWINDOW,
};

type EditorSurface = tinycast_ui::EditorWindow;

/// Everything the sheet draws and seeds. Copy comes from i18n; the drafts are
/// plain text.
pub struct EditorRequest {
    pub title: String,
    pub name_label: String,
    pub value_label: String,
    pub confirm_label: String,
    pub accept_label: String,
    pub cancel_label: String,
    pub show_confirm: bool,
    /// The text prompt hides the name row.
    pub name_hidden: bool,
    pub initial_name: String,
    pub initial_value: String,
    pub initial_confirm: bool,
}

/// The chosen continuation: accept carries the draft, cancel nothing.
type OnAccept = Box<dyn FnOnce(crate::features::custom_commands::ui::editor::CommandDraft)>;

struct EditorHost {
    ui: EditorSurface,
    owner: OwnerGuard,
}

struct Slot {
    host: Option<EditorHost>,
    accept: Option<OnAccept>,
    cancel: Option<Box<dyn FnOnce()>>,
}

thread_local! {
    static EDITOR: RefCell<Slot> = RefCell::new(Slot {
        host: None,
        accept: None,
        cancel: None,
    });
}

/// Disables the owner while the sheet is up and restores it on the way out,
/// exactly like the Direct2D editor's guard.
struct OwnerGuard {
    hwnd: HWND,
    active: bool,
}

impl OwnerGuard {
    fn new(hwnd: HWND) -> Self {
        if hwnd.is_invalid() {
            return Self {
                hwnd,
                active: false,
            };
        }
        unsafe {
            let _ = EnableWindow(hwnd, false);
        }
        Self { hwnd, active: true }
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        unsafe {
            let _ = EnableWindow(self.hwnd, true);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.active = false;
    }
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

/// True when no editor is up.
pub fn is_up() -> bool {
    EDITOR.with(|slot| slot.borrow().host.is_some())
}

/// Claims the single editor slot and shows the sheet. Returns false when an
/// editor is already open; neither continuation runs in that case.
pub fn open(
    owner: HWND,
    request: EditorRequest,
    on_accept: OnAccept,
    on_cancel: Box<dyn FnOnce()>,
) -> bool {
    let claimed = EDITOR.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.host.is_some() {
            return false;
        }
        slot.accept = Some(on_accept);
        slot.cancel = Some(on_cancel);
        true
    });
    if !claimed {
        return false;
    }
    let Some(host) = EditorHost::create(owner, request) else {
        // Nothing was shown, so nothing may run.
        EDITOR.with(|slot| {
            let mut slot = slot.borrow_mut();
            slot.accept = None;
            slot.cancel = None;
        });
        return false;
    };
    EDITOR.with(|slot| slot.borrow_mut().host = Some(host));
    true
}

impl EditorHost {
    fn create(owner: HWND, request: EditorRequest) -> Option<Self> {
        let ui = EditorSurface::new().ok()?;
        tinycast_ui::apply_appearance(&ui, crate::design_system::appearance::resolved());
        ui.set_window_title(request.title.into());
        ui.set_name_label(request.name_label.into());
        ui.set_value_label(request.value_label.into());
        ui.set_confirm_label(request.confirm_label.into());
        ui.set_accept_label(request.accept_label.into());
        ui.set_cancel_label(request.cancel_label.into());
        ui.set_show_confirm(request.show_confirm);
        ui.set_name_hidden(request.name_hidden);
        ui.set_name_text(request.initial_name.into());
        ui.set_value_text(request.initial_value.into());
        ui.set_confirm_checked(request.initial_confirm);

        ui.on_accepted(|| {
            let draft = EDITOR.with(|slot| {
                let slot = slot.borrow();
                slot.host.as_ref().map(|host| {
                    crate::features::custom_commands::ui::editor::CommandDraft {
                        name: host.ui.get_name_text().to_string(),
                        command: host.ui.get_value_text().to_string(),
                        confirm: host.ui.get_confirm_checked(),
                    }
                })
            });
            let Some(draft) = draft else {
                return;
            };
            finish(true, Some(draft));
        });
        ui.on_cancelled(|| finish(false, None));
        // The frame's X button cancels, like the old sheet's WM_CLOSE.
        ui.window().on_close_requested(|| {
            finish(false, None);
            slint::CloseRequestResponse::HideWindow
        });

        // No HWND exists until the event loop runs; park the sheet off-screen.
        ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        ui.show().ok()?;
        let weak = ui.as_weak();
        arm_init(weak, owner);
        Some(Self {
            ui,
            owner: OwnerGuard::new(owner),
        })
    }
}

/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's skip_taskbar leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;

fn arm_init(weak: slint::Weak<EditorSurface>, owner: HWND) {
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if init_window(&ui, owner) {
            return;
        }
        arm_init(weak, owner);
    });
}

/// One-time setup, run as soon as winit has an HWND. Returns false while the
/// window is still pending.
fn init_window(ui: &EditorSurface, _owner: HWND) -> bool {
    let Some(hwnd) = window_hwnd(ui) else {
        return false;
    };
    apply_tool_window(hwnd);
    place(hwnd);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
    }
    ui.invoke_focus_first_field();
    true
}

fn window_hwnd(ui: &EditorSurface) -> Option<HWND> {
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::Win32(win32) = handle.as_raw() else {
                return None;
            };
            let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
            crate::ui_host::clear_backdrop(hwnd);
            Some(hwnd)
        })
        .flatten()
}

/// Centres the sheet on the primary work area, a third of the way down.
fn place(hwnd: HWND) {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let screens = crate::platform::screens::screens_px();
    let screen = screens
        .iter()
        .find(|s| s.origin_is_primary)
        .or_else(|| screens.first());
    let (x, y) = match screen {
        Some(s) => {
            let sw = s.work.right - s.work.left;
            let sh = s.work.bottom - s.work.top;
            (s.work.left + (sw - w) / 2, s.work.top + (sh - h) / 3)
        }
        None => (100, 100),
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOCOPYBITS,
        );
    }
}

/// WS_EX_TOOLWINDOW and the taskbar: winit sets skip_taskbar, which does not
/// clear APPWINDOW, so the style is corrected once the HWND exists.
fn apply_tool_window(hwnd: HWND) {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (before | WS_EX_TOOLWINDOW.0 as isize) & !WS_EX_APPWINDOW_BIT;
        if next != before {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Hides the sheet, drops it (restoring the owner) and runs the chosen
/// continuation. Re-entry is safe because the slot frees before the
/// continuation runs, so an accept may open the next sheet.
fn finish(
    accepted: bool,
    draft: Option<crate::features::custom_commands::ui::editor::CommandDraft>,
) {
    let (host, accept, cancel) = EDITOR.with(|slot| {
        let mut slot = slot.borrow_mut();
        let host = slot.host.take();
        let accept = slot.accept.take();
        let cancel = slot.cancel.take();
        (host, accept, cancel)
    });
    let Some(host) = host else {
        return;
    };
    let _ = host.ui.hide();
    let mut host = host;
    // Restore the owner before the surface drops; the Drop guard then no-ops.
    host.owner.restore();
    drop(host);
    if accepted {
        if let (Some(accept), Some(draft)) = (accept, draft) {
            accept(draft);
        }
    } else if let Some(cancel) = cancel {
        cancel();
    }
}
