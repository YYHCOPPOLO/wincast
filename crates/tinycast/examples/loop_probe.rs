//! Same-thread `WH_KEYBOARD_LL` plus a custom `PostMessageW` while Slint's
//! winit loop is the only pump. Does not touch `platform::run`.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, PostMessageW, RegisterClassW,
    SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, WH_KEYBOARD_LL,
    WM_DESTROY, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

const WM_PROBE: u32 = 0x8000 + 40;
const VK_F24: VIRTUAL_KEY = VIRTUAL_KEY(0x87);

static HOOK: AtomicIsize = AtomicIsize::new(0);
static HOOK_HIT: AtomicBool = AtomicBool::new(false);
static MSG_HIT: AtomicBool = AtomicBool::new(false);
static INVOKE_HIT: AtomicBool = AtomicBool::new(false);
static NOTE: Mutex<String> = Mutex::new(String::new());

fn main() -> ExitCode {
    let evidence = evidence_path();
    if let Some(parent) = evidence.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()
    {
        return finish(&evidence, false, false, &error.to_string());
    }

    let hwnd = match create_host() {
        Ok(hwnd) => hwnd,
        Err(error) => return finish(&evidence, false, false, &error),
    };
    if let Err(error) = install_hook() {
        let _ = unsafe { DestroyWindow(hwnd) };
        return finish(&evidence, false, false, &error);
    }

    let host_bits = hwnd.0 as isize;
    let poster = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        let host = HWND(host_bits as *mut core::ffi::c_void);
        if let Err(error) = unsafe { PostMessageW(host, WM_PROBE, WPARAM(1), LPARAM(0)) } {
            push_note(&format!("PostMessageW: {error}"));
        }
        if let Err(error) = slint::invoke_from_event_loop(|| {
            INVOKE_HIT.store(true, Ordering::SeqCst);
        }) {
            push_note(&format!("invoke_from_event_loop: {error}"));
        }
    });

    let started = Instant::now();
    let key_sent = std::cell::Cell::new(false);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            if !key_sent.get() && started.elapsed() >= Duration::from_millis(80) {
                let _ = send_f24();
                key_sent.set(true);
            }
            let hook = HOOK_HIT.load(Ordering::SeqCst);
            let message = MSG_HIT.load(Ordering::SeqCst);
            let timed_out = started.elapsed() >= Duration::from_secs(2);
            if (hook && message) || timed_out {
                let _ = slint::quit_event_loop();
            }
        },
    );

    let loop_error = slint::run_event_loop().err().map(|error| error.to_string());
    let _ = poster.join();
    uninstall_hook();
    let _ = unsafe { DestroyWindow(hwnd) };

    let hook = HOOK_HIT.load(Ordering::SeqCst);
    let message = MSG_HIT.load(Ordering::SeqCst);
    let mut note = NOTE.lock().map(|note| note.clone()).unwrap_or_default();
    if let Some(error) = loop_error {
        if !note.is_empty() {
            note.push('\n');
        }
        note.push_str(&error);
    }
    finish(&evidence, hook, message, &note)
}

fn finish(path: &std::path::Path, hook: bool, message: bool, note: &str) -> ExitCode {
    let invoke = INVOKE_HIT.load(Ordering::SeqCst);
    let body = format!(
        "hook={}\nmessage={}\ninvoke_from_event_loop={invoke}\n{note}\n",
        hook as u8, message as u8
    );
    let _ = std::fs::write(path, &body);
    println!("{body}evidence={}", path.display());
    if hook && message {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn evidence_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.superpowers/sdd/2026-09-25-slint-ui/probe/loop-probe.txt")
}

fn push_note(text: &str) {
    if let Ok(mut note) = NOTE.lock() {
        if !note.is_empty() {
            note.push('\n');
        }
        note.push_str(text);
    }
}

fn create_host() -> Result<HWND, String> {
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: w!("TinycastLoopProbe"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            let last = windows::Win32::Foundation::GetLastError();
            if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                return Err(format!("RegisterClassW {last:?}"));
            }
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("TinycastLoopProbe"),
            w!("TinycastLoopProbe"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )
        .map_err(|error| error.to_string())?;
        Ok(hwnd)
    }
}

fn install_hook() -> Result<(), String> {
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), instance, 0)
            .map_err(|error| error.to_string())?;
        HOOK.store(hook.0 as isize, Ordering::SeqCst);
        Ok(())
    }
}

fn uninstall_hook() {
    let hook = HOOK.swap(0, Ordering::SeqCst);
    if hook != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(hook as *mut core::ffi::c_void));
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_PROBE {
        MSG_HIT.store(true, Ordering::SeqCst);
        return LRESULT(0);
    }
    if msg == WM_DESTROY {
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let injected = (info.flags.0 & LLKHF_INJECTED.0) != 0;
        if injected && info.vkCode == u32::from(VK_F24.0) {
            HOOK_HIT.store(true, Ordering::SeqCst);
            return LRESULT(1);
        }
    }
    CallNextHookEx(
        HHOOK(HOOK.load(Ordering::SeqCst) as *mut core::ffi::c_void),
        code,
        wparam,
        lparam,
    )
}

fn send_f24() -> Result<(), String> {
    unsafe {
        let mut inputs = [key(VK_F24, false), key(VK_F24, true)];
        let sent = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
        if sent == 0 {
            Err("SendInput returned 0".to_string())
        } else {
            Ok(())
        }
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
                dwExtraInfo: 0,
            },
        },
    }
}
