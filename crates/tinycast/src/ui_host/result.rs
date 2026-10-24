//! The Slint Quick Action result panel: a 520 DIP card that replaces the old
//! layered Direct2D panel.
//!
//! The panel never takes focus (`WS_EX_NOACTIVATE`), so the low-level keyboard
//! hook — kept from the Direct2D panel — routes Esc / Enter / Ctrl+C to the
//! tray host as `WM_QA_*` messages. The Replace and Copy buttons are Slint and
//! post the same messages, so `AppCore`'s handlers are unchanged. The window
//! is created on first use, born off-screen so winit makes its HWND, then
//! hidden and reused.

use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Timer};
use tinycast_pure::theme;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, PostMessageW,
    SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, UnhookWindowsHookEx, GWL_EXSTYLE, HHOOK,
    HWND_TOPMOST, KBDLLHOOKSTRUCT, SWP_NOACTIVATE, SWP_NOCOPYBITS, WH_KEYBOARD_LL, WM_KEYDOWN,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::design_system::appearance;
use crate::platform::screens::{dip_scalar_to_px, screens_px};

type Surface = tinycast_ui::ResultWindow;

/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window has to exist before it can be styled, so it is born off-screen.
const PARKED: i32 = -32_000;
/// winit's skip_taskbar leaves this bit set, which would put a tool window
/// back on the taskbar.
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;

static HOOK_PANEL: AtomicIsize = AtomicIsize::new(0);
static HOOK_HOST: AtomicIsize = AtomicIsize::new(0);
static KEY_HOOK: AtomicIsize = AtomicIsize::new(0);

pub struct ResultPanel {
    ui: Surface,
    host: HWND,
    visible: Cell<bool>,
    appearance: Cell<u8>,
    /// The exit fade; a show during the fade cancels it instead.
    exit_timer: Timer,
}

impl ResultPanel {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        let ui = Surface::new().map_err(crate::ui_host::platform_error)?;
        tinycast_ui::apply_appearance(&ui, appearance::resolved());
        // No HWND exists until the event loop runs; park the window off-screen.
        ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        ui.show().map_err(crate::ui_host::platform_error)?;
        let panel = Self {
            ui,
            host,
            visible: Cell::new(false),
            appearance: Cell::new(u8::MAX),
            exit_timer: Timer::default(),
        };
        // The header glyph does not follow the locale.
        panel.ui.set_symbol("sparkles".into());
        panel.apply_locale();
        let weak = panel.ui.as_weak();
        arm_init(weak);
        Ok(panel)
    }

    pub fn show(&self, title: &str, body: &str) {
        self.apply_appearance();
        self.apply_locale();
        self.ui.set_heading(title.into());
        self.ui.set_body(body.into());
        self.exit_timer.stop();
        self.ui.set_exit_fade(1.0);
        if !self.visible.get() {
            self.ui.set_presented(true);
        }
        if self.ui.show().is_err() {
            return;
        }
        if let Some(hwnd) = window_hwnd(&self.ui) {
            apply_panel_style(hwnd);
            self.place(hwnd, true);
        }
        install_key_hook(self.host, window_hwnd(&self.ui));
        self.visible.set(true);
    }

    pub fn set_body(&self, body: &str) {
        self.apply_locale();
        self.ui.set_body(body.into());
        // The window height follows the measured body; keep the position.
        if let Some(hwnd) = window_hwnd(&self.ui) {
            self.place(hwnd, false);
        }
    }

    pub fn hide(&self) {
        if !self.visible.get() {
            remove_key_hook();
            return;
        }
        self.ui.set_exit_fade(0.0);
        let weak = self.ui.as_weak();
        let millis = (theme::duration::EXIT_SECS * 1000.0).round().max(0.0) as u64;
        self.exit_timer.stop();
        self.exit_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(millis),
            move || {
                let Some(ui) = weak.upgrade() else {
                    remove_key_hook();
                    return;
                };
                let _ = ui.hide();
                ui.set_presented(false);
                remove_key_hook();
            },
        );
    }

    pub fn set_locale(&self, _locale: &str) {
        self.apply_appearance();
        self.apply_locale();
    }

    /// Button labels follow the locale; the copy arrives from the callers.
    fn apply_locale(&self) {
        let lang = crate::ui_host::app_core(self.host)
            .map(|core| unsafe { (*core).ui_lang() })
            .unwrap_or(tinycast_pure::i18n::UiLang::ZhHans);
        self.ui
            .set_replace_label(tinycast_pure::i18n::qa_replace(lang).into());
        self.ui
            .set_copy_label(tinycast_pure::i18n::qa_copy(lang).into());
        self.ui
            .set_working_label(tinycast_pure::i18n::qa_working(lang).into());
    }

    fn apply_appearance(&self) {
        let appearance = crate::ui_host::app_core(self.host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or_else(appearance::resolved);
        if self.appearance.get() != appearance {
            self.appearance.set(appearance);
            tinycast_ui::apply_appearance(&self.ui, appearance);
        }
    }

    /// Primary screen, centred, a third of the way down — the old `place`.
    /// `recenter` keeps the x on body updates, like the old call.
    fn place(&self, hwnd: HWND, recenter: bool) {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let dpi = if dpi == 0 { 96 } else { dpi };
        let w = dip_scalar_to_px(theme::size::QUICK_ACTION_PANEL, dpi);
        let h = dip_scalar_to_px(
            self.ui.get_header_height() + self.ui.get_body_height() + self.ui.get_foot_height(),
            dpi,
        );
        let (x, y) = if recenter {
            let screens = screens_px();
            let screen = screens
                .iter()
                .find(|s| s.origin_is_primary)
                .or_else(|| screens.first());
            match screen {
                Some(s) => {
                    let work_w = s.work.right - s.work.left;
                    let work_h = s.work.bottom - s.work.top;
                    (
                        s.work.left + (work_w - w) / 2,
                        s.work.top + (work_h - h) / 3,
                    )
                }
                None => (80, 80),
            }
        } else {
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
                return;
            }
            (rect.left, rect.top)
        };
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_NOCOPYBITS,
            );
        }
    }
}

impl Drop for ResultPanel {
    fn drop(&mut self) {
        self.exit_timer.stop();
        remove_key_hook();
        let _ = self.ui.hide();
    }
}

/// Polls for the HWND winit creates on the first event-loop turn, styles it,
/// then hides the parked window until the first result.
fn arm_init(weak: slint::Weak<Surface>) {
    Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(hwnd) = window_hwnd(&ui) else {
            let _ = ui.hide();
            arm_init(ui.as_weak());
            return;
        };
        apply_panel_style(hwnd);
        let _ = ui.hide();
    });
}

/// The style winit cannot express: tool window on top that never activates.
/// (No `WS_EX_TRANSPARENT`: the buttons must receive clicks.)
fn apply_panel_style(hwnd: HWND) {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (before | WS_EX_TOOLWINDOW.0 as isize | WS_EX_NOACTIVATE.0 as isize)
            & !WS_EX_APPWINDOW_BIT;
        if next != before {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
    }
}

fn window_hwnd(ui: &Surface) -> Option<HWND> {
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

/// The hook posts to the tray host directly: the Slint panel has no owner, so
/// the old `GA_ROOTOWNER` walk would only find the panel itself.
fn install_key_hook(host: HWND, panel: Option<HWND>) {
    let Some(panel) = panel else {
        return;
    };
    HOOK_PANEL.store(panel.0 as isize, Ordering::SeqCst);
    HOOK_HOST.store(host.0 as isize, Ordering::SeqCst);
    if KEY_HOOK.load(Ordering::SeqCst) != 0 {
        return;
    }
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), hinstance, 0);
        if let Ok(hook) = hook {
            KEY_HOOK.store(hook.0 as isize, Ordering::SeqCst);
        }
    }
}

fn remove_key_hook() {
    HOOK_PANEL.store(0, Ordering::SeqCst);
    HOOK_HOST.store(0, Ordering::SeqCst);
    let bits = KEY_HOOK.swap(0, Ordering::SeqCst);
    if bits != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(bits as *mut core::ffi::c_void));
        }
    }
}

unsafe extern "system" fn key_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && (wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == 0x0104) {
        let panel = HWND(HOOK_PANEL.load(Ordering::SeqCst) as *mut core::ffi::c_void);
        let host = HWND(HOOK_HOST.load(Ordering::SeqCst) as *mut core::ffi::c_void);
        if !panel.is_invalid() && !host.is_invalid() && IsWindowVisible(panel).as_bool() {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;
            if vk == 0x1B {
                let _ = PostMessageW(
                    host,
                    crate::platform::messages::WM_QA_DISMISS,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
            if vk == 0x0D {
                let _ = PostMessageW(
                    host,
                    crate::platform::messages::WM_QA_APPLY,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
            if vk == 0x43 && GetAsyncKeyState(0x11) < 0 {
                let _ = PostMessageW(
                    host,
                    crate::platform::messages::WM_QA_COPY,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}
