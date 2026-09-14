use std::path::{Path, PathBuf};

#[cfg(not(test))]
use windows::core::w;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_BINARY, REG_OPTION_NON_VOLATILE, REG_SZ,
};

#[cfg(not(test))]
const RUN_SUBKEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
#[cfg(not(test))]
const APPROVED_SUBKEY: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
#[cfg(not(test))]
const VALUE_NAME: windows::core::PCWSTR = w!("Tinycast");
const APPROVED_ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// Writes or deletes HKCU Run value `Tinycast` for the running exe.
///
/// The command is this process's image path, so a packaged install can live
/// wherever the user chose. Unit tests compile `apply` as a no-op so
/// `AppSettings::save` cannot clobber the real login item.
pub fn apply(enabled: bool) -> std::io::Result<()> {
    #[cfg(test)]
    {
        let _ = enabled;
        Ok(())
    }
    #[cfg(not(test))]
    {
        persist(enabled)
    }
}

#[cfg(not(test))]
fn persist(enabled: bool) -> std::io::Result<()> {
    if enabled {
        let path = normalize_exe_path(std::env::current_exe()?);
        persist_named(
            RUN_SUBKEY,
            APPROVED_SUBKEY,
            VALUE_NAME,
            Some(&quoted_command(&path)),
        )
    } else {
        persist_named(RUN_SUBKEY, APPROVED_SUBKEY, VALUE_NAME, None)
    }
}

fn persist_named(
    run_subkey: windows::core::PCWSTR,
    approved_subkey: windows::core::PCWSTR,
    value_name: windows::core::PCWSTR,
    command: Option<&str>,
) -> std::io::Result<()> {
    unsafe {
        let run = open_or_create(run_subkey)?;
        let result = match command {
            Some(cmd) => {
                let data = encode_reg_sz(cmd);
                win32(RegSetValueExW(run, value_name, 0, REG_SZ, Some(&data)))
            }
            None => delete_value(run, value_name),
        };
        let _ = RegCloseKey(run);
        result?;
        persist_approved(approved_subkey, value_name, command.is_some());
        Ok(())
    }
}

fn persist_approved(
    subkey: windows::core::PCWSTR,
    value_name: windows::core::PCWSTR,
    enabled: bool,
) {
    unsafe {
        let Ok(key) = open_or_create(subkey) else {
            return;
        };
        let _ = if enabled {
            win32(RegSetValueExW(
                key,
                value_name,
                0,
                REG_BINARY,
                Some(&APPROVED_ENABLED),
            ))
        } else {
            delete_value(key, value_name)
        };
        let _ = RegCloseKey(key);
    }
}

unsafe fn open_or_create(subkey: windows::core::PCWSTR) -> std::io::Result<HKEY> {
    let mut key = HKEY::default();
    win32(RegCreateKeyExW(
        HKEY_CURRENT_USER,
        subkey,
        0,
        windows::core::PCWSTR::null(),
        REG_OPTION_NON_VOLATILE,
        KEY_SET_VALUE | KEY_QUERY_VALUE,
        None,
        &mut key,
        None,
    ))?;
    Ok(key)
}

unsafe fn delete_value(key: HKEY, value_name: windows::core::PCWSTR) -> std::io::Result<()> {
    let err = RegDeleteValueW(key, value_name);
    if err == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        win32(err)
    }
}

fn quoted_command(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn normalize_exe_path(path: PathBuf) -> PathBuf {
    strip_verbatim(path.canonicalize().unwrap_or(path))
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    const VERBATIM: &str = r"\\?\";
    const UNC: &str = r"\\?\UNC\";
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix(UNC) {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = raw.strip_prefix(VERBATIM) {
        PathBuf::from(rest)
    } else {
        path
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
    use windows::core::w;
    use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, REG_VALUE_TYPE};

    const TEST_RUN: windows::core::PCWSTR =
        w!("Software\\com.tinycast.win\\LaunchAtLoginTest\\Run");
    const TEST_APPROVED: windows::core::PCWSTR =
        w!("Software\\com.tinycast.win\\LaunchAtLoginTest\\Approved");
    const TEST_NAME: windows::core::PCWSTR = w!("TinycastTest");

    #[test]
    fn encode_reg_sz_is_utf16le_nul_terminated() {
        assert_eq!(encode_reg_sz("x"), [b'x', 0, 0, 0]);
    }

    #[test]
    fn strip_verbatim_prefix_from_extended_path() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\Tinycast\tinycast.exe")),
            PathBuf::from(r"C:\Tinycast\tinycast.exe")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\server\share\tinycast.exe")),
            PathBuf::from(r"\\server\share\tinycast.exe")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"C:\Tinycast\tinycast.exe")),
            PathBuf::from(r"C:\Tinycast\tinycast.exe")
        );
    }

    #[test]
    fn normalize_exe_path_strips_verbatim_when_canonicalize_fails() {
        let missing = PathBuf::from(r"\\?\Z:\no-such-tinycast.exe");
        assert_eq!(
            normalize_exe_path(missing),
            PathBuf::from(r"Z:\no-such-tinycast.exe")
        );
    }

    #[test]
    fn quoted_command_wraps_paths_with_spaces() {
        assert_eq!(
            quoted_command(Path::new(r"C:\Program Files\Tinycast\tinycast.exe")),
            r#""C:\Program Files\Tinycast\tinycast.exe""#
        );
    }

    #[test]
    fn apply_is_a_noop_in_unit_tests() {
        apply(true).unwrap();
        apply(false).unwrap();
    }

    #[test]
    fn persist_named_writes_and_deletes_isolated_key() {
        let cmd = r#""C:\Tinycast\tinycast.exe""#;
        persist_named(TEST_RUN, TEST_APPROVED, TEST_NAME, Some(cmd)).unwrap();
        assert_eq!(read_sz(TEST_RUN, TEST_NAME).as_deref(), Some(cmd));
        let approved = read_bytes(TEST_APPROVED, TEST_NAME).expect("approved blob");
        assert_eq!(approved.first().copied(), Some(2));
        assert_eq!(approved.len(), APPROVED_ENABLED.len());
        persist_named(TEST_RUN, TEST_APPROVED, TEST_NAME, None).unwrap();
        assert_eq!(read_sz(TEST_RUN, TEST_NAME), None);
        assert_eq!(read_bytes(TEST_APPROVED, TEST_NAME), None);
    }

    fn read_sz(subkey: windows::core::PCWSTR, name: windows::core::PCWSTR) -> Option<String> {
        let buf = read_bytes(subkey, name)?;
        if buf.len() < 2 {
            return None;
        }
        let wide: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
        String::from_utf16(&wide[..end]).ok()
    }

    fn read_bytes(subkey: windows::core::PCWSTR, name: windows::core::PCWSTR) -> Option<Vec<u8>> {
        unsafe {
            let mut key = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_QUERY_VALUE, &mut key).is_err() {
                return None;
            }
            let mut kind = REG_VALUE_TYPE(0);
            let mut size = 0u32;
            let status = RegQueryValueExW(key, name, None, Some(&mut kind), None, Some(&mut size));
            if status.is_err() || size == 0 {
                let _ = RegCloseKey(key);
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let status = RegQueryValueExW(
                key,
                name,
                None,
                Some(&mut kind),
                Some(buf.as_mut_ptr()),
                Some(&mut size),
            );
            let _ = RegCloseKey(key);
            if status.is_err() {
                None
            } else {
                buf.truncate(size as usize);
                Some(buf)
            }
        }
    }
}
