//! Clipboard reads/writes. Own pastes carry a private format marker.

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

const CF_UNICODETEXT: u32 = 13;

pub fn internal_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("com.tinycast.win.internal")) }
}

pub fn has_internal_marker() -> bool {
    let fmt = internal_format();
    if fmt == 0 {
        return false;
    }
    unsafe { IsClipboardFormatAvailable(fmt).is_ok() }
}

pub fn write_text_marked(text: &str) -> windows::core::Result<()> {
    write_text(text, true)
}

pub fn write_text(text: &str, marked: bool) -> windows::core::Result<()> {
    let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * 2;
    unsafe {
        OpenClipboard(HWND::default())?;
        let _ = EmptyClipboard();
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes)?;
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            let _ = global_free_safe(handle);
            let _ = CloseClipboard();
            return Err(windows::core::Error::from_win32());
        }
        std::ptr::copy_nonoverlapping(wide.as_mut_ptr(), ptr as *mut u16, wide.len());
        let _ = GlobalUnlock(handle);
        if SetClipboardData(CF_UNICODETEXT, HANDLE(handle.0)).is_err() {
            let _ = global_free_safe(handle);
            let _ = CloseClipboard();
            return Err(windows::core::Error::from_win32());
        }
        if marked {
            stamp_marker();
        }
        CloseClipboard()?;
    }
    Ok(())
}

pub fn read_unicode_text() -> Option<String> {
    unsafe {
        if OpenClipboard(HWND::default()).is_err() {
            return None;
        }
        let result = (|| {
            if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
                return None;
            }
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let hg = HGLOBAL(handle.0);
            let ptr = GlobalLock(hg);
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            let p = ptr as *const u16;
            while *p.add(len) != 0 {
                len += 1;
                if len > 64 * 1024 {
                    break;
                }
            }
            let slice = std::slice::from_raw_parts(p, len);
            let text = String::from_utf16_lossy(slice);
            let _ = GlobalUnlock(hg);
            Some(text)
        })();
        let _ = CloseClipboard();
        result
    }
}

unsafe fn stamp_marker() {
    let fmt = internal_format();
    if fmt == 0 {
        return;
    }
    if let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, 4) {
        let ptr = GlobalLock(handle);
        if !ptr.is_null() {
            *(ptr as *mut u32) = 1;
            let _ = GlobalUnlock(handle);
            if SetClipboardData(fmt, HANDLE(handle.0)).is_err() {
                let _ = global_free_safe(handle);
            }
        } else {
            let _ = global_free_safe(handle);
        }
    }
}

unsafe fn global_free_safe(handle: HGLOBAL) {
    let _ = GlobalFree(handle);
}
