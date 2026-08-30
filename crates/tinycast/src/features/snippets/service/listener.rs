//! WH_KEYBOARD_LL keyword listener. Installed only while snippets are enabled.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tinycast_pure::snippet::keyword::{
    classify_input, KeywordBuffer, KeywordInput,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ToUnicode, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME,
    VK_LEFT, VK_LWIN, VK_MENU, VK_RETURN, VK_RIGHT, VK_RWIN, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use super::injector;
use crate::platform::messages::WM_SNIPPET_KEYWORD;

static ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static HOST: AtomicIsize = AtomicIsize::new(0);
static LAST_FG: AtomicIsize = AtomicIsize::new(0);

struct Shared {
    buffer: KeywordBuffer,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
static PENDING: Mutex<Option<PendingMatch>> = Mutex::new(None);

#[derive(Clone, Debug)]
pub struct PendingMatch {
    pub keyword: String,
    pub target: isize,
}

pub struct KeywordListener {
    running: bool,
}

impl KeywordListener {
    pub fn new() -> Self {
        Self { running: false }
    }

    pub fn start(&mut self, host: HWND, keywords: impl IntoIterator<Item = String>) {
        self.stop();
        HOST.store(host.0 as isize, Ordering::SeqCst);
        LAST_FG.store(0, Ordering::SeqCst);
        if let Ok(mut slot) = SHARED.lock() {
            *slot = Some(Shared {
                buffer: KeywordBuffer::with_keywords(keywords),
            });
        }
        ENABLED.store(true, Ordering::SeqCst);
        if host.is_invalid() {
            return;
        }
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hinstance, 0);
            if let Ok(hook) = hook {
                HOOK.store(hook.0 as isize, Ordering::SeqCst);
                self.running = true;
            }
        }
    }

    pub fn update_keywords(&self, keywords: impl IntoIterator<Item = String>) {
        if let Ok(mut slot) = SHARED.lock() {
            if let Some(shared) = slot.as_mut() {
                shared.buffer.set_keywords(keywords);
            }
        }
    }

    pub fn stop(&mut self) {
        ENABLED.store(false, Ordering::SeqCst);
        let hook = HOOK.swap(0, Ordering::SeqCst);
        if hook != 0 {
            unsafe {
                let _ = UnhookWindowsHookEx(HHOOK(hook as *mut core::ffi::c_void));
            }
        }
        if let Ok(mut slot) = SHARED.lock() {
            if let Some(shared) = slot.as_mut() {
                shared.buffer.reset();
            }
            *slot = None;
        }
        if let Ok(mut pending) = PENDING.lock() {
            *pending = None;
        }
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Drop for KeywordListener {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn take_pending() -> Option<PendingMatch> {
    PENDING.lock().ok()?.take()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && ENABLED.load(Ordering::SeqCst) {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if injector::is_synthetic(info.dwExtraInfo) {
            return CallNextHookEx(current_hook(), code, wparam, lparam);
        }
        let key_down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
        let vk = info.vkCode as u16;
        let commanding = key_down && (ctrl_down() || win_down() || alt_down());
        let reset_key = is_reset_key(vk);
        let delete_back = vk == VK_BACK.0;
        let ch = if key_down && !commanding && !reset_key && !delete_back {
            to_char(vk, info.scanCode)
        } else {
            None
        };
        let class = classify_input(
            false,
            key_down,
            commanding,
            reset_key,
            delete_back && key_down,
            ch.is_some(),
        );
        let fg = GetForegroundWindow();
        let fg_bits = fg.0 as isize;
        let last = LAST_FG.swap(fg_bits, Ordering::SeqCst);
        if last != 0 && last != fg_bits {
            if let Ok(mut slot) = SHARED.lock() {
                if let Some(shared) = slot.as_mut() {
                    shared.buffer.reset();
                }
            }
        }
        match class {
            KeywordInput::Ignored => {}
            KeywordInput::Reset => {
                if let Ok(mut slot) = SHARED.lock() {
                    if let Some(shared) = slot.as_mut() {
                        shared.buffer.reset();
                    }
                }
            }
            KeywordInput::DeleteBackward => {
                if let Ok(mut slot) = SHARED.lock() {
                    if let Some(shared) = slot.as_mut() {
                        shared.buffer.delete_backward(now_ms());
                    }
                }
            }
            KeywordInput::Text => {
                if let Some(ch) = ch {
                    let hit = if let Ok(mut slot) = SHARED.lock() {
                        slot.as_mut().and_then(|s| s.buffer.push(ch, now_ms()))
                    } else {
                        None
                    };
                    if let Some(keyword) = hit {
                        if let Ok(mut pending) = PENDING.lock() {
                            *pending = Some(PendingMatch {
                                keyword,
                                target: fg_bits,
                            });
                        }
                        let host = HWND(HOST.load(Ordering::SeqCst) as *mut core::ffi::c_void);
                        if !host.is_invalid() {
                            let _ = PostMessageW(host, WM_SNIPPET_KEYWORD, WPARAM(0), LPARAM(0));
                        }
                    }
                }
            }
        }
    }
    CallNextHookEx(current_hook(), code, wparam, lparam)
}

fn current_hook() -> HHOOK {
    HHOOK(HOOK.load(Ordering::SeqCst) as *mut core::ffi::c_void)
}

fn ctrl_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 }
}

fn alt_down() -> bool {
    unsafe { GetKeyState(VK_MENU.0 as i32) < 0 }
}

fn win_down() -> bool {
    unsafe { GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0 }
}

fn is_reset_key(vk: u16) -> bool {
    matches!(
        vk,
        x if x == VK_TAB.0
            || x == VK_RETURN.0
            || x == VK_ESCAPE.0
            || x == VK_LEFT.0
            || x == VK_RIGHT.0
            || x == VK_UP.0
            || x == VK_DOWN.0
            || x == VK_HOME.0
            || x == VK_END.0
            || x == VK_DELETE.0
    )
}

fn to_char(vk: u16, scan: u32) -> Option<char> {
    let state = [0u8; 256];
    let mut buf = [0u16; 8];
    let n = unsafe { ToUnicode(vk as u32, scan, Some(&state), &mut buf, 0) };
    if n > 0 {
        char::decode_utf16(buf[..n as usize].iter().copied())
            .flatten()
            .next()
            .filter(|c| !c.is_control())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_starts_only_when_requested() {
        let mut l = KeywordListener::new();
        assert!(!l.is_running());
        l.start(HWND::default(), ["!notes".to_string()]);
        assert!(!l.is_running());
        l.stop();
        assert!(!l.is_running());
    }

    #[test]
    fn pending_slot_starts_empty() {
        assert!(take_pending().is_none());
    }
}
