//! One confirmation at a time, resumed instead of pumped.
//!
//! A caller registers a Pending continuation here and shows a dialog; the
//! dialog's surface runs that continuation when it closes. Nothing nests a
//! message loop, so the palette, AppCore and the dialogs keep sharing one Slint
//! event loop.
//!
//! The slot, the live surface and the closing state are per thread on purpose.
//! Tinycast has one UI thread that owns every dialog, its callers and AppCore,
//! so in the app this is the same as a global. Under cargo test each test runs
//! on its own thread, so tests that hold the dialog slot no longer leak into
//! parallel tests (previously a process-wide AtomicBool made them flaky).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use tinycast_pure::theme;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WA_INACTIVE, WM_ACTIVATE, WM_CLOSE, WM_NCDESTROY, WS_EX_TOOLWINDOW,
};

type Dialog = tinycast_ui::DialogWindow;

const SUBCLASS_ID: usize = 0x44;
/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be measured, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's skip_taskbar leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;

/// Everything the card draws. Copy arrives from tinycast_pure::i18n; symbol is
/// already a Segoe Fluent glyph.
pub struct DialogRequest {
    pub heading: String,
    pub message: String,
    pub accept: String,
    pub cancel: String,
    pub symbol: String,
    pub destructive: bool,
    pub show_cancel: bool,
    pub form: bool,
    /// The volume accessory's shared level: the slider writes it, the accepted
    /// continuation reads it.
    pub volume: Option<Rc<Cell<f32>>>,
}

pub struct Pending {
    pub on_accept: Box<dyn FnOnce()>,
    pub on_cancel: Box<dyn FnOnce()>,
}

impl Pending {
    fn noop() -> Self {
        Self {
            on_accept: Box::new(|| {}),
            on_cancel: Box::new(|| {}),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyOpen;

pub struct ModalGate {
    pending: Option<Pending>,
}

impl ModalGate {
    pub fn new() -> Self {
        Self { pending: None }
    }

    pub fn open(&mut self, pending: Pending) -> Result<(), AlreadyOpen> {
        if self.pending.is_some() {
            return Err(AlreadyOpen);
        }
        self.pending = Some(pending);
        Ok(())
    }

    pub fn accept(&mut self) {
        if let Some(pending) = self.pending.take() {
            (pending.on_accept)();
        }
    }

    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending.take() {
            (pending.on_cancel)();
        }
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// Releases the slot without running either continuation.
    pub fn take_pending(&mut self) -> Option<Pending> {
        self.pending.take()
    }
}

struct DialogHost {
    ui: Dialog,
}

struct Modal {
    gate: ModalGate,
    host: Option<DialogHost>,
    /// An accept/cancel was chosen; the surface is fading out and the pending
    /// must not be touched until it is gone.
    closing: bool,
    /// The surface has been focused once; before that, losing activation is not
    /// a user clicking outside.
    armed: bool,
}

thread_local! {
    static MODAL: RefCell<Modal> = RefCell::new(Modal {
        gate: ModalGate::new(),
        host: None,
        closing: false,
        armed: false,
    });
}

/// Claims the single dialog slot. The legacy half of the API: the custom
/// command coordinator's logic tests hold the slot to prove a stacked confirm
/// does not run. Production opens dialogs through open().
pub fn begin() -> bool {
    MODAL.with(|modal| modal.borrow_mut().gate.open(Pending::noop()).is_ok())
}

/// Releases the slot and drops any surface, without running a continuation.
pub fn end() {
    let host = MODAL.with(|modal| {
        let mut modal = modal.borrow_mut();
        modal.closing = false;
        modal.armed = false;
        modal.host.take()
    });
    drop(host);
    MODAL.with(|modal| {
        let _ = modal.borrow_mut().gate.take_pending();
    });
}

pub fn is_up() -> bool {
    MODAL.with(|modal| modal.borrow().gate.is_open())
}

/// Claims the slot, then shows a fresh card. Returns false when another dialog
/// is already up or the surface could not be created; neither continuation runs
/// in that case.
pub fn open(request: DialogRequest, on_accept: Box<dyn FnOnce()>) -> bool {
    let claimed = MODAL.with(|modal| {
        modal
            .borrow_mut()
            .gate
            .open(Pending {
                on_accept,
                on_cancel: Box::new(|| {}),
            })
            .is_ok()
    });
    if !claimed {
        return false;
    }
    let Some(host) = DialogHost::create(request) else {
        // Nothing was shown, so nothing may run.
        MODAL.with(|modal| {
            let _ = modal.borrow_mut().gate.take_pending();
        });
        return false;
    };
    MODAL.with(|modal| modal.borrow_mut().host = Some(host));
    true
}

/// Claims the slot for a caller-owned surface (the camera preview). The caller
/// shows its own window and completes through [`complete_external`]; when the
/// surface could not be shown it releases the claim with
/// [`release_external`] so nothing runs. A dialog (or a second claim) already
/// holding the slot refuses with `AlreadyOpen`.
pub fn claim_external(pending: Pending) -> Result<(), AlreadyOpen> {
    MODAL.with(|modal| modal.borrow_mut().gate.open(pending))
}

/// True while the slot is held by a caller-owned surface rather than a card.
pub fn claimed_externally() -> bool {
    MODAL.with(|modal| {
        let modal = modal.borrow();
        modal.gate.is_open() && modal.host.is_none()
    })
}

/// Drops an unshown external claim: nothing was on screen, so nothing runs.
pub fn release_external() {
    MODAL.with(|modal| {
        let mut modal = modal.borrow_mut();
        modal.armed = false;
        let _ = modal.gate.take_pending();
    });
}

/// Runs the external continuation (accept or cancel) and frees the slot. A
/// card dialog keeps its own path; this only answers external claims.
pub fn complete_external(accepted: bool) {
    let pending = MODAL.with(|modal| {
        let mut modal = modal.borrow_mut();
        if modal.host.is_some() {
            return None;
        }
        modal.armed = false;
        modal.gate.take_pending()
    });
    if let Some(pending) = pending {
        if accepted {
            (pending.on_accept)();
        } else {
            (pending.on_cancel)();
        }
    }
}

/// Rewrites the copy of the live card. The update check opens one card as
/// its progress display and rewrites it with the outcome; a no-op when no
/// card is up (the user closed it before the check finished).
pub fn update_copy(heading: &str, message: &str) -> bool {
    MODAL.with(|modal| {
        let modal = modal.borrow_mut();
        match modal.host.as_ref() {
            Some(host) => {
                host.ui.set_heading(heading.into());
                host.ui.set_message(message.into());
                true
            }
            None => false,
        }
    })
}

impl DialogHost {
    fn create(request: DialogRequest) -> Option<Self> {
        let ui = Dialog::new().ok()?;
        tinycast_ui::apply_appearance(&ui, crate::design_system::appearance::resolved());
        ui.set_heading(request.heading.into());
        ui.set_message(request.message.into());
        ui.set_accept_label(request.accept.into());
        ui.set_cancel_label(request.cancel.into());
        ui.set_symbol(request.symbol.into());
        ui.set_show_cancel(request.show_cancel);
        ui.set_form(request.form);
        ui.set_destructive(request.destructive);
        if let Some(level) = request.volume.as_ref() {
            ui.set_show_volume(true);
            ui.set_volume(level.get());
            ui.set_volume_readout(readout(level.get()).into());
        }

        ui.on_accepted(|| request_close(true));
        ui.on_cancelled(|| request_close(false));

        // The slider edits the shared level; the accepted continuation reads it.
        if let Some(level) = request.volume.clone() {
            let nudged = Rc::clone(&level);
            let weak = ui.as_weak();
            ui.on_volume_changed(move |value| {
                let value = tinycast_pure::volume::clamped(value);
                level.set(value);
                if let Some(ui) = weak.upgrade() {
                    ui.set_volume_readout(readout(value).into());
                }
            });
            let weak = ui.as_weak();
            ui.on_volume_nudged(move |up| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let next = tinycast_pure::volume::volume_step(ui.get_volume(), up);
                nudged.set(next);
                ui.set_volume(next);
                ui.set_volume_readout(readout(next).into());
            });
        }

        // No HWND exists until the event loop runs; park the card out of sight.
        ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        ui.show().ok()?;
        let weak = ui.as_weak();
        arm_init(weak);
        Some(Self { ui })
    }
}

fn readout(level: f32) -> String {
    tinycast_pure::volume::percentage(level)
}

/// Polls for the HWND winit creates on the first event-loop turn, then places
/// the card and starts the entrance.
fn arm_init(weak: slint::Weak<Dialog>) {
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if init_window(&ui) {
            return;
        }
        arm_init(weak);
    });
}

/// One-time setup, run as soon as winit has an HWND. Returns false while the
/// window is still pending.
fn init_window(ui: &Dialog) -> bool {
    let Some(hwnd) = window_hwnd(ui) else {
        return false;
    };
    // Showing re-applies winit's own extended style, so the tool-window bit is
    // set afterwards; the subclass also has to outlive every show.
    apply_tool_window(hwnd);
    if !subclass(hwnd) {
        return false;
    }
    place(hwnd);
    // Only now, where the card will rest, does it rise into view.
    ui.set_reveal(1.0);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
    }
    // The window has focus, so the default action can take it.
    ui.invoke_focus_default();
    MODAL.with(|modal| modal.borrow_mut().armed = true);
    true
}

fn window_hwnd(ui: &Dialog) -> Option<HWND> {
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

/// Centres the card on the primary work area, a third of the way down.
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
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

fn subclass(hwnd: HWND) -> bool {
    unsafe {
        SetWindowSubclass(hwnd, SUBCLASSPROC::Some(dialog_subclass), SUBCLASS_ID, 0).as_bool()
    }
}

unsafe extern "system" fn dialog_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        // A click outside the card takes activation away; the old card cancelled
        // on the same event.
        WM_ACTIVATE => {
            if (wparam.0 as u32) & 0xffff == WA_INACTIVE {
                deactivated();
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            deactivated();
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(dialog_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

/// Focus left an armed card: cancel, like the Direct2D dialog did.
fn deactivated() {
    let cancel = MODAL.with(|modal| {
        let modal = modal.borrow();
        modal.armed && !modal.closing && modal.host.is_some()
    });
    if cancel {
        request_close(false);
    }
}

/// Plays the exit fade, then drops the card and runs the chosen continuation.
fn request_close(accepted: bool) {
    let closing = MODAL.with(|modal| {
        let mut modal = modal.borrow_mut();
        if modal.closing || modal.host.is_none() {
            return false;
        }
        modal.closing = true;
        if let Some(host) = modal.host.as_ref() {
            host.ui.set_exit_fade(0.0);
        }
        true
    });
    if !closing {
        return;
    }
    // The pending stays registered while the card fades, so a second request
    // still sees the slot as taken.
    let millis = (theme::duration::DIALOG_EXIT_SECS * 1000.0)
        .round()
        .max(0.0) as u64;
    slint::Timer::single_shot(Duration::from_millis(millis), move || finish(accepted));
}

fn finish(accepted: bool) {
    let (host, pending) = MODAL.with(|modal| {
        let mut modal = modal.borrow_mut();
        modal.closing = false;
        modal.armed = false;
        let host = modal.host.take();
        (host, modal.gate.take_pending())
    });
    if let Some(host) = host {
        let _ = host.ui.hide();
        drop(host);
    }
    // Outside the borrow: a continuation may open the next dialog.
    if let Some(pending) = pending {
        if accepted {
            (pending.on_accept)();
        } else {
            (pending.on_cancel)();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::{AlreadyOpen, ModalGate, Pending};

    fn counters() -> (Rc<Cell<u32>>, Rc<Cell<u32>>, Pending) {
        let accept = Rc::new(Cell::new(0));
        let cancel = Rc::new(Cell::new(0));
        let accept_hit = Rc::clone(&accept);
        let cancel_hit = Rc::clone(&cancel);
        let pending = Pending {
            on_accept: Box::new(move || accept_hit.set(accept_hit.get() + 1)),
            on_cancel: Box::new(move || cancel_hit.set(cancel_hit.get() + 1)),
        };
        (accept, cancel, pending)
    }

    #[test]
    fn accept_calls_only_on_accept() {
        let (accept, cancel, pending) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        gate.accept();
        assert_eq!(accept.get(), 1);
        assert_eq!(cancel.get(), 0);
    }

    #[test]
    fn cancel_calls_only_on_cancel() {
        let (accept, cancel, pending) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        gate.cancel();
        assert_eq!(accept.get(), 0);
        assert_eq!(cancel.get(), 1);
    }

    #[test]
    fn second_open_returns_already_open_without_calling() {
        let (accept, cancel, pending) = counters();
        let (second_accept, second_cancel, second) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        assert_eq!(gate.open(second), Err(AlreadyOpen));
        assert_eq!(accept.get(), 0);
        assert_eq!(cancel.get(), 0);
        assert_eq!(second_accept.get(), 0);
        assert_eq!(second_cancel.get(), 0);
    }

    #[test]
    fn accept_allows_another_open() {
        let (accept, cancel, pending) = counters();
        let (again_accept, again_cancel, again) = counters();
        let mut gate = ModalGate::new();
        gate.open(pending).unwrap();
        gate.accept();
        gate.open(again).unwrap();
        gate.cancel();
        assert_eq!(accept.get(), 1);
        assert_eq!(cancel.get(), 0);
        assert_eq!(again_accept.get(), 0);
        assert_eq!(again_cancel.get(), 1);
    }
}
