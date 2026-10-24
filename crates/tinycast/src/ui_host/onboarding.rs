//! The Slint onboarding window: the step content is Slint, and the shortcut
//! recorder captures through an HWND subclass, exactly like the settings
//! wells. Steps and copy come from `tinycast_pure::i18n`; the step change
//! fades in the surface over `duration-onboarding`.

use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use tinycast_pure::hotkey::{CaptureOutcome, Modifiers};
use tinycast_pure::i18n::{
    onboarding_continue, onboarding_get_started, onboarding_subtitle, onboarding_title,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, SUBCLASSPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, WM_KEYDOWN, WM_KEYUP, WM_NCDESTROY,
};

use crate::features::hotkeys::ui::recorder::Recorder;
use crate::ui_host::app_core as core_from_host;

const SUBCLASS_ID: usize = 0x66;
/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window is born off-screen so winit can make its HWND.
const PARKED: i32 = -32_000;

/// The client size in DIP (`surfaces::onboarding::WINDOW` kept for the shell
/// test); the hero/well/button geometry lives in the surface.
pub const WINDOW: (f32, f32) = (520.0, 400.0);

const STEP_COUNT: usize = 4;
const RECORDER_ACTION: &str = "hotkey.togglePalette";

pub struct OnboardingWindow {
    ui: tinycast_ui::OnboardingWindow,
    host: HWND,
    inner: *mut Inner,
}

struct Inner {
    host: HWND,
    weak: slint::Weak<tinycast_ui::OnboardingWindow>,
    recorder: Recorder,
    step: usize,
    ready: bool,
    pending_show: bool,
    lang: tinycast_pure::i18n::UiLang,
}

impl OnboardingWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        let ui = tinycast_ui::OnboardingWindow::new().map_err(crate::ui_host::platform_error)?;
        let appearance = core_from_host(host)
            .map(|core| unsafe { (*core).appearance_key() })
            .unwrap_or(1);
        tinycast_ui::apply_appearance(&ui, appearance);
        let lang = core_from_host(host)
            .map(|core| unsafe { (*core).ui_lang() })
            .unwrap_or(tinycast_pure::i18n::UiLang::ZhHans);
        let inner = Box::into_raw(Box::new(Inner {
            host,
            weak: ui.as_weak(),
            recorder: Recorder::new(),
            step: 0,
            ready: false,
            pending_show: false,
            lang,
        }));
        let window = Self { ui, host, inner };
        window.ui.window().set_position(slint::LogicalPosition {
            x: PARKED as f32,
            y: PARKED as f32,
        });
        window.ui.show().map_err(crate::ui_host::platform_error)?;
        let ptr = window.inner as usize;
        window.ui.on_record_clicked(move || {
            start_recording(ptr);
        });
        window.ui.on_continue_clicked(move || {
            continue_clicked(ptr);
        });
        window.ui.window().on_close_requested(move || {
            // Closing the onboarding finishes it, like WM_CLOSE did.
            if let Some(core) = core_from_host(host_of(ptr)) {
                unsafe {
                    (*core).finish_onboarding();
                }
            }
            slint::CloseRequestResponse::HideWindow
        });
        apply_chrome(ptr);
        arm_init(inner);
        Ok(window)
    }

    pub fn show(&self) {
        let ready = unsafe { (*self.inner).ready };
        if !ready {
            unsafe {
                (*self.inner).pending_show = true;
            }
            return;
        }
        let _ = self.ui.show();
        unsafe {
            if let Some(hwnd) = window_hwnd(&self.ui) {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn hide(&self) {
        unsafe {
            (*self.inner).pending_show = false;
            (*self.inner).recorder.cancel();
        }
        let _ = self.ui.hide();
    }

    pub fn is_visible(&self) -> bool {
        unsafe {
            window_hwnd(&self.ui)
                .map(|hwnd| {
                    windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool()
                })
                .unwrap_or(false)
        }
    }

    pub fn set_locale(&self, _locale: &str) {
        let lang = core_from_host(self.host)
            .map(|core| unsafe { (*core).ui_lang() })
            .unwrap_or(tinycast_pure::i18n::UiLang::ZhHans);
        unsafe {
            (*self.inner).lang = lang;
        }
        apply_chrome(self.inner as usize);
    }
}

impl Drop for OnboardingWindow {
    fn drop(&mut self) {
        unsafe {
            if let Some(hwnd) = window_hwnd(&self.ui) {
                if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd).as_bool() {
                    let _ = RemoveWindowSubclass(
                        hwnd,
                        SUBCLASSPROC::Some(onboarding_subclass),
                        SUBCLASS_ID,
                    );
                }
            }
            drop(Box::from_raw(self.inner));
            self.inner = std::ptr::null_mut();
        }
    }
}

fn host_of(ptr: usize) -> HWND {
    unsafe { (*(ptr as *mut Inner)).host }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn current_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            win: GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0,
        }
    }
}

fn start_recording(ptr: usize) {
    unsafe {
        let inner = ptr as *mut Inner;
        (*inner).recorder.begin(RECORDER_ACTION.into());
        if let Some(core) = core_from_host((*inner).host) {
            (*core).pause_global_hotkeys();
        }
        if let Some(hwnd) = (*inner).weak.upgrade().and_then(|ui| window_hwnd(&ui)) {
            let _ = SetForegroundWindow(hwnd);
        }
        refresh_labels(ptr);
    }
}

/// Continues to the next step, or finishes onboarding on the last one.
fn continue_clicked(ptr: usize) {
    unsafe {
        let inner = ptr as *mut Inner;
        if (*inner).step + 1 < STEP_COUNT {
            (*inner).step += 1;
            if let Some(ui) = (*inner).weak.upgrade() {
                ui.set_step((*inner).step as i32);
            }
            apply_chrome(ptr);
            return;
        }
        if let Some(core) = core_from_host((*inner).host) {
            (*core).finish_onboarding();
        }
    }
}

unsafe fn apply_capture(ptr: usize, outcome: CaptureOutcome) {
    let inner = ptr as *mut Inner;
    match outcome {
        CaptureOutcome::Ignore => {}
        CaptureOutcome::Cancel | CaptureOutcome::Clear => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).resume_global_hotkeys();
            }
            refresh_labels(ptr);
        }
        CaptureOutcome::Commit(binding) => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).set_hotkey(RECORDER_ACTION, Some(binding));
                (*core).resume_global_hotkeys();
            }
            refresh_labels(ptr);
        }
    }
}

/// The step copy, the hero glyph and the well caption all follow the step and
/// the locale.
fn apply_chrome(ptr: usize) {
    unsafe {
        let inner = ptr as *mut Inner;
        let Some(ui) = (*inner).weak.upgrade() else {
            return;
        };
        let lang = (*inner).lang;
        let step = (*inner).step.min(STEP_COUNT - 1);
        ui.set_step(step as i32);
        ui.set_title_label(onboarding_title(step, lang).into());
        ui.set_subtitle_label(onboarding_subtitle(step, lang).into());
        ui.set_hero_symbol(
            (match step {
                1 => "lock.shield",
                2 => "wand.and.sparkles",
                3 => "checkmark",
                _ => "magnifyingglass",
            })
            .into(),
        );
        ui.set_continue_label(
            if step + 1 == STEP_COUNT {
                onboarding_get_started(lang)
            } else {
                onboarding_continue(lang)
            }
            .into(),
        );
        refresh_labels(ptr);
    }
}

/// The well caption follows the recorder state (`well_caption_lang`).
fn refresh_labels(ptr: usize) {
    unsafe {
        let inner = ptr as *mut Inner;
        let Some(ui) = (*inner).weak.upgrade() else {
            return;
        };
        let caption = crate::features::hotkeys::ui::recorder::well_caption_lang(
            None,
            (*inner).recorder.is_recording(),
            (*inner).lang,
        );
        ui.set_well_label(caption.into());
    }
}

fn arm_init(inner: *mut Inner) {
    let ptr = inner as usize;
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let inner = ptr as *mut Inner;
        if unsafe { (*inner).ready } {
            return;
        }
        let Some(ui) = (unsafe { (*inner).weak.upgrade() }) else {
            return;
        };
        let Some(hwnd) = window_hwnd(&ui) else {
            let _ = ui.hide();
            arm_init(inner);
            return;
        };
        let ok = unsafe {
            SetWindowSubclass(
                hwnd,
                SUBCLASSPROC::Some(onboarding_subclass),
                SUBCLASS_ID,
                ptr,
            )
            .as_bool()
        };
        if !ok {
            arm_init(inner);
            return;
        }
        unsafe {
            (*inner).ready = true;
        }
        let pending = unsafe { (*inner).pending_show };
        if !pending {
            let _ = ui.hide();
        } else {
            unsafe {
                (*inner).pending_show = false;
            }
            let _ = ui.show();
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    });
}

/// Keyboard capture for the recorder: while a recording is armed, the subclass
/// feeds key down/up to the `Recorder` like the Direct2D wndproc did. Slint
/// still sees the keys (nothing consumes them), but no surface control needs
/// them while recording.
unsafe extern "system" fn onboarding_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    dwrefdata: usize,
) -> LRESULT {
    let inner = dwrefdata as *mut Inner;
    if !inner.is_null() && ((*inner).recorder.is_recording()) {
        match msg {
            WM_KEYDOWN => {
                let outcome =
                    (*inner)
                        .recorder
                        .on_keydown(wparam.0 as u16, current_modifiers(), now_ms());
                apply_capture(dwrefdata, outcome);
                return LRESULT(0);
            }
            WM_KEYUP => {
                let outcome = (*inner).recorder.on_keyup(wparam.0 as u16, now_ms());
                apply_capture(dwrefdata, outcome);
                return LRESULT(0);
            }
            _ => {}
        }
    }
    match msg {
        WM_NCDESTROY => {
            let _ =
                RemoveWindowSubclass(hwnd, SUBCLASSPROC::Some(onboarding_subclass), SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

fn window_hwnd<T: ComponentHandle>(ui: &T) -> Option<HWND> {
    ui.window()
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::Win32(win32) = handle.as_raw() else {
                return None;
            };
            Some(HWND(win32.hwnd.get() as *mut core::ffi::c_void))
        })
        .flatten()
}
