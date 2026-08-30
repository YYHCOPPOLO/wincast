mod dpi;
pub mod hotkey;
pub(crate) mod launch_at_login;
pub mod messages;
pub(crate) mod paths;
pub mod screens;
mod single_instance;
mod tray;
pub(crate) mod winhttp;
pub(crate) mod clipboard;
pub(crate) mod paster;

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
    core.about_window = Some(crate::surfaces::StubWindow::about(host)?);
    core.support_window = Some(crate::surfaces::StubWindow::support(host)?);
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
