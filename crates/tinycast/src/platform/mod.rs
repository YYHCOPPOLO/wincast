pub(crate) mod appearance;
pub(crate) mod clipboard;
pub(crate) mod clock;
pub(crate) mod dpapi;
mod dpi;
pub(crate) mod keyboard_ll;
pub(crate) mod launch_at_login;
pub mod messages;
pub(crate) mod paster;
pub(crate) mod paths;
pub mod screens;
mod single_instance;
pub(crate) mod tray;
pub(crate) mod window;
pub(crate) mod winhttp;

pub fn run() -> windows::core::Result<()> {
    dpi::apply()?;
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        )
        .ok()?;
    }
    // Slint's winit backend has to be picked before any Slint window exists.
    // femtovg is the only renderer the probes validated; skip-taskbar is the one
    // window attribute the hook can express (the tool-window bit is applied per
    // window once its HWND exists).
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(|attributes| {
            use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
            attributes.with_skip_taskbar(true)
        })
        .select()
        .map_err(crate::ui_host::platform_error)?;
    let _instance = single_instance::acquire()?;
    let mut core = crate::app_core::AppCore::new();
    let host = tray::create(&mut core)?;
    core.palette_window = Some(crate::palette::PaletteWindow::create(host)?);
    core.settings_window = Some(crate::surfaces::SettingsWindow::create(host)?);
    core.about_window = None;
    core.support_surface = crate::surfaces::SupportWindow::create(host).ok();
    core.about_surface = crate::surfaces::AboutWindow::create(host).ok();
    core.onboarding_window = crate::surfaces::OnboardingWindow::create(host).ok();
    core.set_host(host);
    // Installs the low-level keyboard hook on this thread; its callbacks and the
    // posted tray messages keep arriving while the Slint loop runs.
    core.start();
    // The tray host is Win32, so no Slint window is visible most of the time:
    // the loop must not quit when the palette hides.
    let result = slint::run_event_loop_until_quit().map_err(crate::ui_host::platform_error);
    // A dialog still up when the loop ends drops its surface while the backend
    // is alive, and its continuation does not run on the way out.
    crate::ui_host::modal::end();
    result
}
