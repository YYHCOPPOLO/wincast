//! Clipboard reads/writes. Own pastes carry a private format marker.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;
const OPEN_ATTEMPTS: u32 = 8;
const OPEN_SLEEP_MS: u64 = 8;
const MAX_CLIPBOARD_TEXT_UNITS: usize = 64 * 1024;
const MAX_CLIPBOARD_IMAGE_BYTES: usize = 64 * 1024 * 1024;

static OWNER: AtomicIsize = AtomicIsize::new(0);

pub fn set_owner(hwnd: HWND) {
    OWNER.store(hwnd.0 as isize, Ordering::SeqCst);
}

fn owner() -> HWND {
    HWND(OWNER.load(Ordering::SeqCst) as *mut core::ffi::c_void)
}

pub fn internal_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("com.tinycast.win.internal")) }
}

fn png_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("PNG")) }
}

fn png_mime_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("image/png")) }
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
        open_clipboard()?;
        let _ = EmptyClipboard();
        let handle = match GlobalAlloc(GMEM_MOVEABLE, bytes) {
            Ok(h) => h,
            Err(err) => {
                let _ = CloseClipboard();
                return Err(err);
            }
        };
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

pub fn write_png_marked(png: &[u8]) -> windows::core::Result<()> {
    if png.is_empty() {
        return Err(windows::core::Error::from_win32());
    }
    let fmt = png_format();
    if fmt == 0 {
        return Err(windows::core::Error::from_win32());
    }
    unsafe {
        open_clipboard()?;
        let _ = EmptyClipboard();
        if set_bytes(fmt, png).is_err() {
            let _ = CloseClipboard();
            return Err(windows::core::Error::from_win32());
        }
        stamp_marker();
        CloseClipboard()?;
    }
    Ok(())
}

pub fn read_unicode_text() -> Option<String> {
    unsafe {
        if open_clipboard().is_err() {
            return None;
        }
        let result = (|| {
            if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
                return None;
            }
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            read_unicode_handle_text(HGLOBAL(handle.0))
        })();
        let _ = CloseClipboard();
        result
    }
}

unsafe fn read_unicode_handle_text(hg: HGLOBAL) -> Option<String> {
    let size = GlobalSize(hg);
    if size < 2 || size % 2 != 0 {
        return None;
    }
    let ptr = GlobalLock(hg);
    if ptr.is_null() {
        return None;
    }
    let _lock = GlobalReadLock(hg);
    // Include room for the terminator, never more than the actual allocation.
    // Decode bytes rather than forming a potentially unaligned u16 slice.
    let len = size.min((MAX_CLIPBOARD_TEXT_UNITS + 1) * 2);
    decode_unicode_text(std::slice::from_raw_parts(ptr as *const u8, len))
}

fn decode_unicode_text(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.len() % 2 != 0 {
        return None;
    }
    let units = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let len = units.clone().take(MAX_CLIPBOARD_TEXT_UNITS + 1).position(|u| u == 0)?;
    let wide: Vec<_> = units.take(len).collect();
    Some(String::from_utf16_lossy(&wide))
}

struct GlobalReadLock(HGLOBAL);

impl Drop for GlobalReadLock {
    fn drop(&mut self) {
        unsafe { let _ = GlobalUnlock(self.0); }
    }
}

/// PNG bytes from the clipboard: native PNG, else CF_DIBV5 / CF_DIB converted off-thread later.
pub fn read_image_bytes() -> Option<ImageBytes> {
    unsafe {
        if open_clipboard().is_err() {
            return None;
        }
        let result = read_image_bytes_open();
        let _ = CloseClipboard();
        result
    }
}

unsafe fn read_image_bytes_open() -> Option<ImageBytes> {
    for fmt in [png_format(), png_mime_format()] {
        if fmt != 0 && IsClipboardFormatAvailable(fmt).is_ok() {
            if let Some(bytes) = copy_handle_bytes(fmt) {
                if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                    return Some(ImageBytes::Png(bytes));
                }
            }
        }
    }
    if IsClipboardFormatAvailable(CF_DIBV5).is_ok() {
        if let Some(bytes) = copy_handle_bytes(CF_DIBV5) {
            return Some(ImageBytes::Dib(bytes));
        }
    }
    if IsClipboardFormatAvailable(CF_DIB).is_ok() {
        if let Some(bytes) = copy_handle_bytes(CF_DIB) {
            return Some(ImageBytes::Dib(bytes));
        }
    }
    None
}

unsafe fn copy_handle_bytes(fmt: u32) -> Option<Vec<u8>> {
    let handle = GetClipboardData(fmt).ok()?;
    copy_global_image_bytes(HGLOBAL(handle.0))
}

unsafe fn copy_global_image_bytes(hg: HGLOBAL) -> Option<Vec<u8>> {
    let size = GlobalSize(hg);
    if !image_allocation_size_allowed(size) {
        return None;
    }
    let ptr = GlobalLock(hg);
    if ptr.is_null() {
        return None;
    }
    let _lock = GlobalReadLock(hg);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).ok()?;
    bytes.extend_from_slice(std::slice::from_raw_parts(ptr as *const u8, size));
    Some(bytes)
}

fn image_allocation_size_allowed(size: usize) -> bool {
    (1..=MAX_CLIPBOARD_IMAGE_BYTES).contains(&size)
}

pub fn encode_image_png(bytes: ImageBytes) -> Option<Vec<u8>> {
    match bytes {
        ImageBytes::Png(png) => Some(png),
        ImageBytes::Dib(dib) => dib_to_png(&dib),
    }
}

#[derive(Clone)]
pub enum ImageBytes {
    Png(Vec<u8>),
    Dib(Vec<u8>),
}

fn dib_to_png(data: &[u8]) -> Option<Vec<u8>> {
    let (width, height, rgba) = dib_to_rgba(data)?;
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&rgba).ok()?;
    drop(writer);
    Some(out)
}

fn dib_to_rgba(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if data.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(data[0..4].try_into().ok()?);
    if header_size < 40 || data.len() < header_size as usize {
        return None;
    }
    let width = i32::from_le_bytes(data[4..8].try_into().ok()?);
    let height_s = i32::from_le_bytes(data[8..12].try_into().ok()?);
    let planes = u16::from_le_bytes(data[12..14].try_into().ok()?);
    let bpp = u16::from_le_bytes(data[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(data[16..20].try_into().ok()?);
    if planes != 1 || width == 0 || height_s == 0 {
        return None;
    }
    if compression != 0 && compression != 3 {
        return None;
    }
    if !matches!(bpp, 24 | 32) {
        return None;
    }
    let top_down = height_s < 0;
    let height = height_s.unsigned_abs();
    let width = width.unsigned_abs();
    if width > 8192 || height > 8192 {
        return None;
    }
    let mut offset = header_size as usize;
    if compression == 3 {
        offset = offset.saturating_add(12);
    }
    let row_bytes = ((width as usize * bpp as usize + 31) / 32) * 4;
    let needed = offset.checked_add(row_bytes.checked_mul(height as usize)?)?;
    if data.len() < needed {
        return None;
    }
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    let mut any_alpha = false;
    for y in 0..height as usize {
        let src_y = if top_down { y } else { height as usize - 1 - y };
        let src = &data[offset + src_y * row_bytes..];
        for x in 0..width as usize {
            let dst = (y * width as usize + x) * 4;
            match bpp {
                32 => {
                    let b = src[x * 4];
                    let g = src[x * 4 + 1];
                    let r = src[x * 4 + 2];
                    let a = src[x * 4 + 3];
                    rgba[dst] = r;
                    rgba[dst + 1] = g;
                    rgba[dst + 2] = b;
                    rgba[dst + 3] = a;
                    if a != 0 {
                        any_alpha = true;
                    }
                }
                24 => {
                    let b = src[x * 3];
                    let g = src[x * 3 + 1];
                    let r = src[x * 3 + 2];
                    rgba[dst] = r;
                    rgba[dst + 1] = g;
                    rgba[dst + 2] = b;
                    rgba[dst + 3] = 255;
                    any_alpha = true;
                }
                _ => return None,
            }
        }
    }
    if bpp == 32 && !any_alpha {
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    Some((width, height, rgba))
}

fn open_clipboard() -> windows::core::Result<()> {
    let hwnd = owner();
    for attempt in 0..OPEN_ATTEMPTS {
        if unsafe { OpenClipboard(hwnd) }.is_ok() {
            return Ok(());
        }
        if attempt + 1 < OPEN_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(OPEN_SLEEP_MS));
        }
    }
    Err(windows::core::Error::from_win32())
}

unsafe fn set_bytes(fmt: u32, bytes: &[u8]) -> windows::core::Result<()> {
    let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len())?;
    let ptr = GlobalLock(handle);
    if ptr.is_null() {
        let _ = global_free_safe(handle);
        return Err(windows::core::Error::from_win32());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
    let _ = GlobalUnlock(handle);
    if SetClipboardData(fmt, HANDLE(handle.0)).is_err() {
        let _ = global_free_safe(handle);
        return Err(windows::core::Error::from_win32());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    struct OwnedGlobal(HGLOBAL);

    impl OwnedGlobal {
        fn filled(size: usize, byte: u8) -> Self {
            unsafe {
                let handle = GlobalAlloc(GMEM_MOVEABLE, size).unwrap();
                let owned = Self(handle);
                let ptr = GlobalLock(handle);
                assert!(!ptr.is_null());
                std::ptr::write_bytes(
                    ptr,
                    byte,
                    windows::Win32::System::Memory::GlobalSize(handle),
                );
                let _ = GlobalUnlock(handle);
                owned
            }
        }
    }

    impl Drop for OwnedGlobal {
        fn drop(&mut self) {
            unsafe { global_free_safe(self.0) }
        }
    }

    fn utf16_bytes(text: &str) -> Vec<u8> {
        text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn bounded_text_decodes_chinese_and_surrogate_pairs() {
        let bytes = utf16_bytes("剪贴板 🦀");
        assert_eq!(decode_unicode_text(&bytes).as_deref(), Some("剪贴板 🦀"));
    }

    #[test]
    fn bounded_text_requires_a_complete_terminated_allocation() {
        assert_eq!(decode_unicode_text(&[0, 0]), Some(String::new()));
        assert_eq!(decode_unicode_text(&[]), None);
        assert_eq!(decode_unicode_text(&[b'A', 0]), None);
        assert_eq!(decode_unicode_text(&[b'A', 0, 0]), None);
        assert_eq!(decode_unicode_text(&[0, 0, 1]), None);
        // A zero byte alone is not a UTF-16 terminator.
        assert_eq!(decode_unicode_text(&[0, 1]), None);
    }

    #[test]
    fn bounded_text_obeys_the_unit_limit() {
        let text = "x".repeat(MAX_CLIPBOARD_TEXT_UNITS);
        assert_eq!(decode_unicode_text(&utf16_bytes(&text)).as_deref(), Some(text.as_str()));
        assert_eq!(decode_unicode_text(&utf16_bytes(&(text + "x"))), None);
    }

    #[test]
    fn bounded_text_stops_at_nul_and_accepts_unaligned_bytes() {
        let bytes = [255, b'A', 0, 0, 0, b'B', 0];
        assert_eq!(decode_unicode_text(&bytes[1..]).as_deref(), Some("A"));
    }

    #[test]
    fn global_text_reads_use_allocation_bounds_and_release_the_lock() {
        let memory = OwnedGlobal::filled(16, 0);
        assert_eq!(unsafe { read_unicode_handle_text(memory.0) }, Some(String::new()));
        let memory = OwnedGlobal::filled(16, 1);
        assert_eq!(unsafe { read_unicode_handle_text(memory.0) }, None);
        assert_eq!(unsafe { windows::Win32::System::Memory::GlobalFlags(memory.0) } & 0xff, 0);
    }

    #[test]
    fn global_image_copy_is_bounded_and_releases_the_lock() {
        let memory = OwnedGlobal::filled(16, 0x89);
        let size = unsafe { GlobalSize(memory.0) };
        assert_eq!(unsafe { copy_global_image_bytes(memory.0) }, Some(vec![0x89; size]));
        assert_eq!(unsafe { windows::Win32::System::Memory::GlobalFlags(memory.0) } & 0xff, 0);
    }

    #[test]
    fn unterminated_global_text_is_rejected_without_using_clipboard() {
        // Leave ample allocated memory beyond the old scan limit so the failing
        // regression itself never reads out of bounds.
        let memory = OwnedGlobal::filled((MAX_CLIPBOARD_TEXT_UNITS + 8) * 2, 1);
        assert!(unsafe { read_unicode_handle_text(memory.0) }.is_none());
    }

    #[test]
    fn clipboard_image_allocation_is_bounded() {
        assert!(!image_allocation_size_allowed(0));
        assert!(image_allocation_size_allowed(1));
        assert!(image_allocation_size_allowed(MAX_CLIPBOARD_IMAGE_BYTES));
        assert!(!image_allocation_size_allowed(MAX_CLIPBOARD_IMAGE_BYTES + 1));
        assert!(!image_allocation_size_allowed(usize::MAX));
    }

    #[test]
    fn dib_24bpp_bottom_up_roundtrips() {
        let width = 2u32;
        let height = 2u32;
        let mut dib = vec![0u8; 40];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib[4..8].copy_from_slice(&(width as i32).to_le_bytes());
        dib[8..12].copy_from_slice(&(height as i32).to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&24u16.to_le_bytes());
        // Two 8-byte rows (BGR + pad), bottom-up: first stored row is bottom.
        let mut bits = vec![0u8; 16];
        bits[0..3].copy_from_slice(&[0, 0, 255]); // bottom-left red in BGR
        bits[3..6].copy_from_slice(&[0, 255, 0]);
        bits[8..11].copy_from_slice(&[255, 0, 0]); // top-left blue
        bits[11..14].copy_from_slice(&[255, 255, 255]);
        dib.extend_from_slice(&bits);
        let (w, h, rgba) = dib_to_rgba(&dib).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]); // top-left blue → RGB
        assert_eq!(&rgba[8..12], &[255, 0, 0, 255]); // bottom-left red
        let png = dib_to_png(&dib).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
