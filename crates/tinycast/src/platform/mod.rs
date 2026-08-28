mod dpi;
pub mod messages;
mod single_instance;
mod tray;

pub fn run() -> windows::core::Result<()> {
    dpi::apply()?;
    let _instance = single_instance::acquire()?;
    let mut core = crate::app_core::AppCore::new();
    core.start();
    let _hwnd = tray::create(&mut core)?;
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
