//! One WH_KEYBOARD_LL hook. Hyper first, then double-tap, then snippets.

use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
};

pub const SNIPPETS: u32 = 1;
pub const DOUBLE_TAP: u32 = 2;
pub const HYPER: u32 = 4;

static NEEDED: AtomicU32 = AtomicU32::new(0);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static HOST: AtomicIsize = AtomicIsize::new(0);

pub fn set_host(host: HWND) {
    HOST.store(host.0 as isize, Ordering::SeqCst);
}

pub fn host() -> HWND {
    HWND(HOST.load(Ordering::SeqCst) as *mut core::ffi::c_void)
}

pub fn retain(bit: u32) {
    let prev = NEEDED.fetch_or(bit, Ordering::SeqCst);
    if prev == 0 {
        install();
    }
}

pub fn release(bit: u32) {
    let prev = NEEDED.fetch_and(!bit, Ordering::SeqCst);
    if prev != 0 && NEEDED.load(Ordering::SeqCst) == 0 {
        uninstall();
    }
}

pub fn needed() -> u32 {
    NEEDED.load(Ordering::SeqCst)
}

fn install() {
    if HOOK.load(Ordering::SeqCst) != 0 {
        return;
    }
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        if let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hinstance, 0) {
            HOOK.store(hook.0 as isize, Ordering::SeqCst);
        }
    }
}

fn uninstall() {
    let hook = HOOK.swap(0, Ordering::SeqCst);
    if hook != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(hook as *mut core::ffi::c_void));
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let needed = NEEDED.load(Ordering::SeqCst);
        if needed & HYPER != 0
            && crate::features::hotkeys::service::hyper::on_ll(wparam, info)
        {
            return LRESULT(1);
        }
        if needed & DOUBLE_TAP != 0 {
            crate::features::hotkeys::service::double_tap_monitor::on_ll(wparam, info);
        }
        if needed & SNIPPETS != 0 {
            crate::features::snippets::service::listener::on_ll(wparam, info);
        }
    }
    CallNextHookEx(HHOOK(HOOK.load(Ordering::SeqCst) as *mut core::ffi::c_void), code, wparam, lparam)
}
