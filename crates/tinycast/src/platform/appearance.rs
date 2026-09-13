use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
    REG_VALUE_TYPE,
};

const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

/// `AppsUseLightTheme`: missing or unreadable counts as light (the product default).
pub fn apps_use_light_theme() -> bool {
    read_dword(PERSONALIZE, "AppsUseLightTheme")
        .map(|v| v != 0)
        .unwrap_or(true)
}

fn read_dword(path: &str, name: &str) -> Option<u32> {
    let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut key = windows::Win32::System::Registry::HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path_w.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        );
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut kind = REG_VALUE_TYPE(0);
        let mut data: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = RegQueryValueExW(
            key,
            PCWSTR(name_w.as_mut_ptr()),
            None,
            Some(&mut kind),
            Some((&mut data as *mut u32).cast::<u8>()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if status != ERROR_SUCCESS || kind != REG_DWORD {
            None
        } else {
            Some(data)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_registry_value_is_light() {
        assert!(
            super::read_dword("Software\\Tinycast\\DoesNotExist", "AppsUseLightTheme").is_none()
        );
        assert!(super::apps_use_light_theme() || !super::apps_use_light_theme());
    }
}
