//! Insert expanded snippet text into the previously focused window.

use std::time::Duration;

use unicode_segmentation::UnicodeSegmentation;
use windows::core::{Interface, BSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, IUIAutomationTextRange,
    IUIAutomationValuePattern, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_IsPasswordPropertyId, UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_CONTROL, VK_LEFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, GetForegroundWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, SetForegroundWindow, GWL_STYLE, WS_CAPTION,
};

/// `dwExtraInfo` tag so the keyword hook can ignore Tinycast-generated key events.
pub const SYNTHETIC_EXTRA: usize = 0x5443_5354;

pub fn is_synthetic(extra: usize) -> bool {
    extra == SYNTHETIC_EXTRA
}

pub fn text_ends_with_keyword(text: &str, keyword: &str) -> bool {
    let needle = keyword.trim();
    if needle.is_empty() {
        return false;
    }
    text.to_lowercase().ends_with(&needle.to_lowercase())
}

pub fn strip_keyword_suffix<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    if !text_ends_with_keyword(text, keyword) {
        return None;
    }
    let kw_len = keyword.trim().encode_utf16().count();
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() < kw_len {
        return None;
    }
    let keep = units.len() - kw_len;
    let mut end = 0usize;
    for (i, ch) in text.char_indices() {
        let at = text[..i].encode_utf16().count();
        if at >= keep {
            end = i;
            break;
        }
        end = i + ch.len_utf8();
    }
    if text[..end].encode_utf16().count() > keep {
        None
    } else {
        Some(&text[..end])
    }
}

pub fn insertion_refused(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return true;
    }
    crate::features::window_management::ui::coordinator::hwnd_is_ours(hwnd)
        || is_password_field(hwnd)
        || is_cross_integrity(hwnd)
        || is_exclusive_fullscreen(hwnd)
}

pub fn password_blocks_capture(is_password: bool) -> bool {
    is_password
}

pub fn accept_synthetic_copy(sequence_moved: bool, text: Option<&str>) -> Option<String> {
    if !sequence_moved {
        return None;
    }
    text.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[allow(dead_code)]
pub fn delete_chars(count: usize) {
    for _ in 0..count {
        send_vk(windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK);
    }
}

pub fn inject_into(previous: HWND, text: &str, cursor: Option<usize>) {
    if insertion_refused(previous) {
        return;
    }
    let payload = text.to_string();
    let bits = previous.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-inject".into())
        .spawn(move || {
            let _ = unsafe {
                windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                )
            };
            std::thread::sleep(Duration::from_millis(80));
            let hwnd = HWND(bits as *mut core::ffi::c_void);
            if !hwnd.is_invalid() {
                restore_foreground(hwnd);
                std::thread::sleep(Duration::from_millis(40));
            }
            if insertion_refused(hwnd) {
                return;
            }
            send_unicode(&payload);
            if let Some(at) = cursor {
                let total = payload.graphemes(true).count();
                if at < total {
                    for _ in 0..(total - at) {
                        send_vk(VK_LEFT);
                    }
                }
            }
        });
}

/// Replace `keyword` with `replacement` in the target. Refuses elevated/fullscreen/mismatch.
pub fn deliver_keyword(
    hwnd: HWND,
    keyword: &str,
    replacement: &str,
    cursor: Option<usize>,
) -> bool {
    if insertion_refused(hwnd) {
        return false;
    }
    restore_foreground(hwnd);
    if let Some(ok) = uia_replace_keyword(hwnd, keyword, replacement) {
        if ok {
            move_cursor(replacement, cursor);
            return true;
        }
        return false;
    }
    false
}

pub fn capture_selection(hwnd: HWND, allow_synthetic_copy: bool) -> Option<String> {
    if hwnd.is_invalid() || insertion_refused(hwnd) {
        return None;
    }
    if let Some(text) = uia_selected_text(hwnd) {
        if !text.is_empty() {
            return Some(text);
        }
    }
    if !allow_synthetic_copy {
        return None;
    }
    synthetic_copy_restore(hwnd)
}

pub fn capture_for_quick_action(hwnd: HWND) -> Result<String, String> {
    if hwnd.is_invalid()
        || crate::features::window_management::ui::coordinator::hwnd_is_ours(hwnd)
    {
        return Err("Tinycast is never the target.".into());
    }
    if is_password_field(hwnd) {
        return Err("Password fields are skipped.".into());
    }
    if insertion_refused(hwnd) {
        return Err("This window cannot be edited.".into());
    }
    capture_selection(hwnd, true)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Nothing is selected.".into())
}

/// Replace the live selection (UIA, then paste). Does not type into an empty caret.
pub fn replace_selection(hwnd: HWND, text: &str) {
    if text.is_empty() || insertion_refused(hwnd) {
        return;
    }
    let payload = text.to_string();
    let bits = hwnd.0 as isize;
    let _ = std::thread::Builder::new()
        .name("tinycast-replace".into())
        .spawn(move || {
            let _ = unsafe {
                windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                )
            };
            std::thread::sleep(Duration::from_millis(80));
            let hwnd = HWND(bits as *mut core::ffi::c_void);
            if hwnd.is_invalid() || insertion_refused(hwnd) {
                return;
            }
            restore_foreground(hwnd);
            std::thread::sleep(Duration::from_millis(40));
            if insertion_refused(hwnd) {
                return;
            }
            if uia_replace_selected(hwnd, &payload) {
                return;
            }
            paste_over(hwnd, &payload);
        });
}

fn restore_foreground(target: HWND) -> bool {
    unsafe {
        let _ = AllowSetForegroundWindow(u32::MAX);
        SetForegroundWindow(target).as_bool()
    }
}

fn is_cross_integrity(hwnd: HWND) -> bool {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return true;
        }
        let proc = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return true,
        };
        let theirs = token_elevated(HANDLE(proc.0));
        let _ = CloseHandle(proc);
        let ours = token_elevated(GetCurrentProcess());
        match (ours, theirs) {
            (Some(self_el), Some(other_el)) => other_el && !self_el,
            _ => true,
        }
    }
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        ok.ok()?;
        Some(elevation.TokenIsElevated != 0)
    }
}

fn is_exclusive_fullscreen(hwnd: HWND) -> bool {
    unsafe {
        let mut wr = RECT::default();
        if GetWindowRect(hwnd, &mut wr).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return false;
        }
        let covers = wr.left <= mi.rcMonitor.left + 2
            && wr.top <= mi.rcMonitor.top + 2
            && wr.right >= mi.rcMonitor.right - 2
            && wr.bottom >= mi.rcMonitor.bottom - 2;
        if !covers {
            return false;
        }
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        style & WS_CAPTION.0 == 0
    }
}

fn automation() -> Option<IUIAutomation> {
    unsafe {
        windows::Win32::System::Com::CoCreateInstance(
            &CUIAutomation,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )
        .ok()
    }
}

fn is_password_field(hwnd: HWND) -> bool {
    let Some(automation) = automation() else {
        return false;
    };
    let Ok(element) = (unsafe { automation.ElementFromHandle(hwnd) }) else {
        return false;
    };
    let Ok(value) = (unsafe { element.GetCurrentPropertyValue(UIA_IsPasswordPropertyId) }) else {
        return false;
    };
    bool::try_from(&value).unwrap_or(false)
}

fn uia_selected_text(hwnd: HWND) -> Option<String> {
    let automation = automation()?;
    let element = unsafe { automation.ElementFromHandle(hwnd) }.ok()?;
    let unk = unsafe { element.GetCurrentPattern(UIA_TextPatternId) }.ok()?;
    let pattern: IUIAutomationTextPattern = unk.cast().ok()?;
    let ranges = unsafe { pattern.GetSelection() }.ok()?;
    let n = unsafe { ranges.Length() }.ok().unwrap_or(0);
    if n <= 0 {
        return None;
    }
    let range = unsafe { ranges.GetElement(0) }.ok()?;
    let bstr = unsafe { range.GetText(-1) }.ok()?;
    let text: String = bstr.to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn uia_document_text(hwnd: HWND) -> Option<String> {
    let automation = automation()?;
    let element = unsafe { automation.ElementFromHandle(hwnd) }.ok()?;
    if let Ok(unk) = unsafe { element.GetCurrentPattern(UIA_TextPatternId) } {
        if let Ok(pattern) = unk.cast::<IUIAutomationTextPattern>() {
            if let Ok(range) = unsafe { pattern.DocumentRange() } {
                if let Ok(bstr) = unsafe { range.GetText(-1) } {
                    return Some(bstr.to_string());
                }
            }
        }
    }
    if let Ok(unk) = unsafe { element.GetCurrentPattern(UIA_ValuePatternId) } {
        if let Ok(pattern) = unk.cast::<IUIAutomationValuePattern>() {
            if let Ok(value) = unsafe { pattern.CurrentValue() } {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// `Some(true)` replaced, `Some(false)` confirmed mismatch/failure, `None` no UIA text.
fn uia_replace_keyword(hwnd: HWND, keyword: &str, replacement: &str) -> Option<bool> {
    let text = uia_document_text(hwnd)?;
    if !text_ends_with_keyword(&text, keyword) {
        return Some(false);
    }
    let prefix = strip_keyword_suffix(&text, keyword).unwrap_or("");
    let new_value = format!("{prefix}{replacement}");
    let automation = automation()?;
    let element = unsafe { automation.ElementFromHandle(hwnd) }.ok()?;
    if let Ok(unk) = unsafe { element.GetCurrentPattern(UIA_TextPatternId) } {
        if let Ok(pattern) = unk.cast::<IUIAutomationTextPattern>() {
            if let Ok(range) = unsafe { pattern.DocumentRange() } {
                if select_suffix(&range, keyword) {
                    send_unicode(replacement);
                    return Some(true);
                }
            }
        }
    }
    if let Ok(unk) = unsafe { element.GetCurrentPattern(UIA_ValuePatternId) } {
        if let Ok(pattern) = unk.cast::<IUIAutomationValuePattern>() {
            if unsafe { pattern.SetValue(&BSTR::from(new_value.as_str())) }.is_ok() {
                return Some(true);
            }
        }
    }
    Some(false)
}

fn select_suffix(range: &IUIAutomationTextRange, keyword: &str) -> bool {
    let units = keyword.trim().encode_utf16().count() as i32;
    if units <= 0 {
        return false;
    }
    unsafe {
        if range
            .MoveEndpointByRange(TextPatternRangeEndpoint_Start, range, TextPatternRangeEndpoint_End)
            .is_err()
        {
            return false;
        }
        if range
            .MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -units)
            .is_err()
        {
            return false;
        }
        let got = range.GetText(-1).ok().map(|b| b.to_string()).unwrap_or_default();
        if !text_ends_with_keyword(&got, keyword) && got.to_lowercase() != keyword.trim().to_lowercase()
        {
            return false;
        }
        range.Select().is_ok()
    }
}

fn synthetic_copy_restore(hwnd: HWND) -> Option<String> {
    let previous = crate::platform::clipboard::read_unicode_text();
    let before = unsafe { GetClipboardSequenceNumber() };
    restore_foreground(hwnd);
    std::thread::sleep(Duration::from_millis(40));
    send_ctrl(0x43); // C
    let moved = wait_for_sequence_change(before, Duration::from_millis(300));
    let got = if moved {
        crate::platform::clipboard::read_unicode_text()
    } else {
        None
    };
    if let Some(prev) = previous {
        let _ = crate::platform::clipboard::write_text(&prev, false);
    } else if moved {
        let _ = crate::platform::clipboard::write_text("", false);
    }
    accept_synthetic_copy(moved, got.as_deref())
}

fn wait_for_sequence_change(before: u32, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if unsafe { GetClipboardSequenceNumber() } != before {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

fn uia_replace_selected(hwnd: HWND, text: &str) -> bool {
    let Some(automation) = automation() else {
        return false;
    };
    let Ok(element) = (unsafe { automation.ElementFromHandle(hwnd) }) else {
        return false;
    };
    let Ok(unk) = (unsafe { element.GetCurrentPattern(UIA_TextPatternId) }) else {
        return false;
    };
    let Ok(pattern) = unk.cast::<IUIAutomationTextPattern>() else {
        return false;
    };
    let Ok(ranges) = (unsafe { pattern.GetSelection() }) else {
        return false;
    };
    let n = unsafe { ranges.Length() }.ok().unwrap_or(0);
    if n <= 0 {
        return false;
    }
    let Ok(range) = (unsafe { ranges.GetElement(0) }) else {
        return false;
    };
    let selected = unsafe { range.GetText(-1) }
        .ok()
        .map(|b| b.to_string())
        .unwrap_or_default();
    if selected.is_empty() {
        return false;
    }
    if unsafe { range.Select() }.is_err() {
        return false;
    }
    send_unicode(text);
    true
}

fn paste_over(hwnd: HWND, text: &str) {
    let _ = hwnd;
    let previous = crate::platform::clipboard::read_unicode_text();
    if crate::platform::clipboard::write_text(text, true).is_err() {
        return;
    }
    send_ctrl(0x56); // V
    std::thread::sleep(Duration::from_millis(40));
    if let Some(prev) = previous {
        let _ = crate::platform::clipboard::write_text(&prev, false);
    }
}

fn send_ctrl(vk: u16) {
    unsafe {
        let mut inputs = [
            key(VK_CONTROL, false),
            key(VIRTUAL_KEY(vk), false),
            key(VIRTUAL_KEY(vk), true),
            key(VK_CONTROL, true),
        ];
        let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn move_cursor(text: &str, cursor: Option<usize>) {
    if let Some(at) = cursor {
        let total = text.graphemes(true).count();
        if at < total {
            for _ in 0..(total - at) {
                send_vk(VK_LEFT);
            }
        }
    }
}

fn send_unicode(text: &str) {
    for unit in text.encode_utf16() {
        unicode_key(unit, false);
        unicode_key(unit, true);
    }
}

fn send_vk(vk: VIRTUAL_KEY) {
    key_send(vk, false);
    key_send(vk, true);
}

fn unicode_key(scan: u16, up: bool) {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let mut inputs = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SYNTHETIC_EXTRA,
            },
        },
    }];
    unsafe {
        let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key_send(vk: VIRTUAL_KEY, up: bool) {
    let mut inputs = [key(vk, up)];
    unsafe {
        let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: SYNTHETIC_EXTRA,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_tag_is_stable() {
        assert!(is_synthetic(SYNTHETIC_EXTRA));
        assert!(!is_synthetic(0));
    }

    #[test]
    fn copy_fallback_requires_sequence_move() {
        assert_eq!(accept_synthetic_copy(false, Some("clip")), None);
        assert_eq!(
            accept_synthetic_copy(true, Some("sel")).as_deref(),
            Some("sel")
        );
        assert_eq!(accept_synthetic_copy(true, Some("  ")), None);
        assert!(password_blocks_capture(true));
        assert!(!password_blocks_capture(false));
        assert!(insertion_refused(HWND::default()));
    }

    #[test]
    fn keyword_suffix_must_match_before_delete() {
        assert!(text_ends_with_keyword("hello!notes", "!notes"));
        assert!(text_ends_with_keyword("HELLO!NOTES", "!notes"));
        assert!(!text_ends_with_keyword("hello", "!notes"));
        assert!(!text_ends_with_keyword("!note", "!notes"));
        assert_eq!(strip_keyword_suffix("xx!notes", "!notes"), Some("xx"));
    }

    #[test]
    fn invalid_hwnd_is_refused() {
        assert!(insertion_refused(HWND::default()));
    }

    #[allow(dead_code)]
    fn _foreground_unused() {
        let _ = GetForegroundWindow;
    }
}
