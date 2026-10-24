//! The Slint Support and About windows: one surface, two kinds. Both are born
//! off-screen at startup (winit makes the HWND once the loop runs), hidden
//! until shown, and every route (`showSupport`, `showAbout`, the reminder
//! pump, the settings rows) lands here unchanged.
//!
//! The Support action keeps opening the Polar checkout (`CHECKOUT`); the
//! Support window carries the remind-later checkbox, the About window the
//! Slint disclosure.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use tinycast_pure::i18n::{
    about_made_with_slint, about_product, about_slint_license, about_slint_link,
    about_window_title, support_built_with_love, support_checkout, support_keeps_independent,
    support_remind_later, support_title, UiLang,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

use crate::ui_host::app_core as core_from_host;

type Surface = tinycast_ui::SupportWindow;

/// winit creates the HWND on the first event-loop turn; poll until it exists.
const INIT_MS: u64 = 16;
/// The window is born off-screen so winit can make its HWND.
const PARKED: i32 = -32_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Support,
    About,
}

struct KindHost {
    ui: Surface,
    host: HWND,
    kind: Kind,
    ready: bool,
    pending_show: bool,
    lang: UiLang,
}

pub struct SupportWindow {
    cell: Rc<Cell<Option<KindHost>>>,
}

impl SupportWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        create(host, Kind::Support)
    }

    pub fn show(&self) {
        let Some(mut host) = self.cell.take() else {
            return;
        };
        if !host.ready {
            host.pending_show = true;
            self.cell.set(Some(host));
            return;
        }
        show_host(&host);
        self.cell.set(Some(host));
    }

    pub fn set_locale(&self, _locale: &str) {
        let Some(mut host) = self.cell.take() else {
            return;
        };
        host.lang = core_from_host(host.host)
            .map(|core| unsafe { (*core).ui_lang() })
            .unwrap_or(UiLang::ZhHans);
        apply_chrome(&host);
        self.cell.set(Some(host));
    }
}

impl Drop for SupportWindow {
    fn drop(&mut self) {
        if let Some(host) = self.cell.take() {
            let _ = host.ui.hide();
        }
    }
}

pub struct AboutWindow {
    inner: SupportWindow,
}

impl AboutWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        Ok(Self {
            inner: create(host, Kind::About)?,
        })
    }

    pub fn show(&self) {
        self.inner.show();
    }

    pub fn set_locale(&self, locale: &str) {
        self.inner.set_locale(locale);
    }
}

impl Drop for AboutWindow {
    fn drop(&mut self) {
        // The inner Drop hides the surface.
    }
}

fn create(host: HWND, kind: Kind) -> windows::core::Result<SupportWindow> {
    let ui = Surface::new().map_err(crate::ui_host::platform_error)?;
    let appearance = core_from_host(host)
        .map(|core| unsafe { (*core).appearance_key() })
        .unwrap_or(1);
    tinycast_ui::apply_appearance(&ui, appearance);
    let lang = core_from_host(host)
        .map(|core| unsafe { (*core).ui_lang() })
        .unwrap_or(UiLang::ZhHans);
    let kind_host = KindHost {
        ui,
        host,
        kind,
        ready: false,
        pending_show: false,
        lang,
    };
    apply_chrome(&kind_host);
    // No HWND exists until the event loop runs; park the window off-screen.
    kind_host.ui.window().set_position(slint::LogicalPosition {
        x: PARKED as f32,
        y: PARKED as f32,
    });
    kind_host
        .ui
        .show()
        .map_err(crate::ui_host::platform_error)?;
    let cell = Rc::new(Cell::new(Some(kind_host)));
    register_callbacks(&cell);
    arm_init_shared(&cell);
    Ok(SupportWindow { cell })
}

fn show_host(host: &KindHost) {
    let _ = host.ui.show();
    unsafe {
        if let Some(hwnd) = window_hwnd(&host.ui) {
            let _ = SetForegroundWindow(hwnd);
        }
    }
    if host.kind == Kind::Support {
        let on = core_from_host(host.host)
            .map(|core| unsafe { (*core).settings.support_reminders })
            .unwrap_or(true);
        host.ui.set_remind_on(on);
    }
}

fn apply_chrome(host: &KindHost) {
    let lang = host.lang;
    let (title, subtitle, caption) = match host.kind {
        Kind::Support => (
            support_title(lang),
            support_built_with_love(lang),
            support_title(lang),
        ),
        Kind::About => (
            about_product(lang),
            support_keeps_independent(lang),
            about_window_title(lang),
        ),
    };
    host.ui.set_caption(caption.into());
    host.ui.set_title_label(title.into());
    host.ui.set_subtitle_label(subtitle.into());
    host.ui.set_support_label(support_title(lang).into());
    host.ui.set_checkout_label(support_checkout(lang).into());
    host.ui.set_remind_label(support_remind_later(lang).into());
    host.ui.set_about_kind(host.kind == Kind::About);
    host.ui.set_slint_label(
        format!(
            "{} {}",
            about_made_with_slint(lang),
            about_slint_license(lang)
        )
        .into(),
    );
    host.ui.set_slint_link_label(about_slint_link(lang).into());
}

/// Registers the surface callbacks: the checkout action, the remind toggle
/// and the Slint disclosure link. Each callback re-enters through the cell.
fn register_callbacks(cell: &Rc<Cell<Option<KindHost>>>) {
    let Some(host) = cell.take() else {
        return;
    };
    host.ui.on_support_clicked(|| {
        let _ = crate::features::launcher::ui::coordinator::execute(
            &crate::features::launcher::ui::coordinator::LaunchSpec::Uri(
                crate::surfaces::CHECKOUT.into(),
            ),
        );
    });
    let remind_cell = Rc::clone(cell);
    host.ui.on_remind_toggled(move |value| {
        let Some(host) = remind_cell.take() else {
            return;
        };
        if let Some(core) = core_from_host(host.host) {
            unsafe {
                (*core).set_support_reminders(value);
            }
        }
        remind_cell.set(Some(host));
    });
    let link_cell = Rc::clone(cell);
    host.ui.on_slint_link_clicked(move || {
        let Some(host) = link_cell.take() else {
            return;
        };
        let _ = crate::features::launcher::ui::coordinator::execute(
            &crate::features::launcher::ui::coordinator::LaunchSpec::Uri(
                about_slint_link(host.lang).into(),
            ),
        );
        link_cell.set(Some(host));
    });
    cell.set(Some(host));
}

/// Shared init: polls for the HWND, then hides the parked window until the
/// first show.
fn arm_init_shared(cell: &Rc<Cell<Option<KindHost>>>) {
    let cell = Rc::clone(cell);
    slint::Timer::single_shot(Duration::from_millis(INIT_MS), move || {
        let Some(mut host) = cell.take() else {
            return;
        };
        let Some(hwnd) = window_hwnd(&host.ui) else {
            let _ = host.ui.hide();
            cell.set(Some(host));
            arm_init_shared(&cell);
            return;
        };
        let _ = hwnd;
        host.ready = true;
        let pending = host.pending_show;
        if !pending {
            let _ = host.ui.hide();
        } else {
            host.pending_show = false;
            show_host(&host);
        }
        cell.set(Some(host));
    });
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
