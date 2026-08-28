use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

pub struct InstanceGuard {
    handle: HANDLE,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub fn acquire() -> windows::core::Result<InstanceGuard> {
    unsafe {
        let handle = CreateMutexW(None, true, w!("Local\\com.tinycast.win"))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            std::process::exit(0);
        }
        Ok(InstanceGuard { handle })
    }
}
