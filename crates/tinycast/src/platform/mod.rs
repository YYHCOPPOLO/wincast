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
    core.start();
    pump()
}

fn pump() -> windows::core::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG,
    };

    unsafe {
        loop {
            let mut msg = MSG::default();
            let status = GetMessageW(&mut msg, None, 0, 0);
            match status.0 {
                0 => return Ok(()),
                -1 => return Err(windows::core::Error::from_win32()),
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}
