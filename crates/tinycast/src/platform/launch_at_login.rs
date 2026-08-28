#![allow(dead_code)]

use windows::core::w;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

const RUN_SUBKEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("Tinycast");

/// Writes or deletes HKCU Run value `Tinycast` pointing at this exe.
pub fn apply(enabled: bool) -> std::io::Result<()> {
    unsafe {
        let mut key = HKEY::default();
        win32(RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_SUBKEY,
            0,
            windows::core::PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        ))?;
        let result = if enabled {
            set_run_value(key)
        } else {
            delete_run_value(key)
        };
        let _ = RegCloseKey(key);
        result
    }
}

unsafe fn set_run_value(key: HKEY) -> std::io::Result<()> {
    let path = std::env::current_exe()?;
    let cmd = format!("\"{}\"", path.to_string_lossy());
    let data = encode_reg_sz(&cmd);
    win32(RegSetValueExW(key, VALUE_NAME, 0, REG_SZ, Some(&data)))
}

unsafe fn delete_run_value(key: HKEY) -> std::io::Result<()> {
    let err = RegDeleteValueW(key, VALUE_NAME);
    if err == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        win32(err)
    }
}

fn encode_reg_sz(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn win32(err: WIN32_ERROR) -> std::io::Result<()> {
    err.ok().map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_reg_sz_is_utf16le_nul_terminated() {
        assert_eq!(encode_reg_sz("x"), [b'x', 0, 0, 0]);
    }
}
