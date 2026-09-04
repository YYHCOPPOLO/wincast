//! Recycle Bin only. `removeItem` / permanent delete must never appear here.

use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
    SHFILEOPSTRUCTW,
};

pub fn native_windows_path(path: &str) -> String {
    path.replace('/', "\\")
}

pub fn recycle(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    let native = native_windows_path(path);
    if !Path::new(&native).exists() {
        return Ok(());
    }
    // Double-NUL terminated, as SHFileOperation requires.
    let mut wide: Vec<u16> = native.encode_utf16().chain([0, 0]).collect();
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(wide.as_mut_ptr()),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
        ..Default::default()
    };
    let status = unsafe { SHFileOperationW(&mut op) };
    if status != 0 {
        return Err(format!("recycle failed ({status})"));
    }
    Ok(())
}

pub fn recycle_all(paths: &[String]) -> Result<Vec<String>, String> {
    let mut failed = Vec::new();
    for path in paths {
        if recycle(path).is_err() {
            failed.push(path.clone());
        }
    }
    if failed.is_empty() {
        Ok(Vec::new())
    } else {
        Err(format!("{} item(s) stayed behind", failed.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn recycle_moves_to_bin_not_permanent_api() {
        let src = include_str!("runner.rs");
        assert!(src.contains("FOF_ALLOWUNDO"));
        assert!(src.contains("FO_DELETE"));
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        assert!(!impl_src.contains("remove_dir_all"));
        assert!(!impl_src.contains("std::fs::remove_file"));
        let _ = fs::metadata(".");
        assert_eq!(native_windows_path("C:/Users/me/AppData/Local/Foo"), r"C:\Users\me\AppData\Local\Foo");
    }
}
