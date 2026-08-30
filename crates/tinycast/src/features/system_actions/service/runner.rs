//! Platform effects for system actions. Confirmation lives in the coordinator.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use tinycast_pure::system_action::SystemActionId;
use tinycast_pure::volume::{clamped, volume_step};
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Request_Device_EjectW, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
    SetupDiGetClassDevsW, SetupDiGetDeviceRegistryPropertyW, CR_SUCCESS, DIGCF_PRESENT,
    GUID_DEVCLASS_DISKDRIVE, SPDRP_REMOVAL_POLICY, SP_DEVINFO_DATA,
};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::Foundation::{BOOL, ERROR_SUCCESS};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_READ,
    KEY_SET_VALUE, REG_DWORD, REG_VALUE_TYPE,
};
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, LockWorkStation, EWX_FORCEIFHUNG, EWX_LOGOFF, EWX_REBOOT, EWX_SHUTDOWN,
    SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER, SHTDN_REASON_MINOR_OTHER,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
};
use windows::Win32::UI::Shell::{
    IShellDispatch4, SHChangeNotify, SHEmptyRecycleBinW, SHQueryRecycleBinW, Shell,
    SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    SHQUERYRBINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsWindowVisible, PostMessageW, SendMessageTimeoutW, ShowWindow, GWL_EXSTYLE, GW_OWNER,
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SW_HIDE, SW_SHOWNA, WINDOW_EX_STYLE, WM_CLOSE,
    WM_SETTINGCHANGE, WM_SYSCOMMAND, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

const SC_MONITORPOWER: usize = 0xF170;
const SC_SCREENSAVE: usize = 0xF140;
const MONITOR_OFF: isize = 2;
const REMOVAL_EXPECT_NO_REMOVAL: u32 = 1;
const STAGE_MANAGER_HUD: &str = "Stage Manager is not available on Windows.";
const DISMISS_HUD: &str = "Dismissing notifications is not available on Windows.";

static LAST_NONZERO: AtomicU32 = AtomicU32::new(0x3f000000); // 0.5f32
static HIDDEN: Mutex<Vec<isize>> = Mutex::new(Vec::new());

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Feedback {
    pub message: String,
    pub noop: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Failure {
    pub message: String,
    pub settings_uri: Option<&'static str>,
}

impl Failure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            settings_uri: None,
        }
    }

    fn bluetooth(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            settings_uri: Some("ms-settings:bluetooth"),
        }
    }
}

pub fn stage_manager_message() -> &'static str {
    STAGE_MANAGER_HUD
}

pub fn dismiss_unavailable_message() -> &'static str {
    DISMISS_HUD
}

pub fn shows_volume_feedback(id: SystemActionId) -> bool {
    matches!(
        id,
        SystemActionId::SetVolume
            | SystemActionId::VolumeUp
            | SystemActionId::VolumeDown
            | SystemActionId::ToggleMute
            | SystemActionId::Volume0
            | SystemActionId::Volume25
            | SystemActionId::Volume50
            | SystemActionId::Volume75
            | SystemActionId::Volume100
    )
}

pub fn run(id: SystemActionId, previous: HWND) -> Result<Option<Feedback>, Failure> {
    match id {
        SystemActionId::LockScreen => lock_screen().map(|_| None),
        SystemActionId::Sleep => sleep().map(|_| None),
        SystemActionId::SleepDisplays => sleep_displays().map(|_| None),
        SystemActionId::Restart => session_end(EWX_REBOOT | EWX_FORCEIFHUNG).map(|_| None),
        SystemActionId::ShutDown => session_end(EWX_SHUTDOWN | EWX_FORCEIFHUNG).map(|_| None),
        SystemActionId::LogOut => session_end(EWX_LOGOFF).map(|_| None),
        SystemActionId::ShowScreenSaver => screensaver().map(|_| None),
        SystemActionId::PlayPause => media_key(VK_MEDIA_PLAY_PAUSE).map(|_| None),
        SystemActionId::NextTrack => media_key(VK_MEDIA_NEXT_TRACK).map(|_| None),
        SystemActionId::PreviousTrack => media_key(VK_MEDIA_PREV_TRACK).map(|_| None),
        SystemActionId::ToggleMute => toggle_mute().map(|_| None),
        SystemActionId::VolumeUp => step_volume(true).map(|_| None),
        SystemActionId::VolumeDown => step_volume(false).map(|_| None),
        SystemActionId::SetVolume => Ok(None),
        SystemActionId::Volume0 => set_volume(0.0).map(|_| None),
        SystemActionId::Volume25 => set_volume(0.25).map(|_| None),
        SystemActionId::Volume50 => set_volume(0.50).map(|_| None),
        SystemActionId::Volume75 => set_volume(0.75).map(|_| None),
        SystemActionId::Volume100 => set_volume(1.0).map(|_| None),
        SystemActionId::ShowDesktop => show_desktop().map(|_| None),
        SystemActionId::ToggleAppearance => toggle_appearance(),
        SystemActionId::ToggleStageManager => Ok(Some(Feedback {
            message: stage_manager_message().into(),
            noop: true,
        })),
        SystemActionId::OpenTrash => open_trash().map(|_| None),
        SystemActionId::EmptyTrash => empty_trash(),
        SystemActionId::EjectAllDisks => eject_all_disks(),
        SystemActionId::ToggleHiddenFiles => toggle_hidden_files(),
        SystemActionId::HideOtherApps => hide_other_apps(previous).map(|_| None),
        SystemActionId::UnhideAllApps => unhide_all_apps(),
        SystemActionId::QuitAllApps => {
            quit_all_apps();
            Ok(None)
        }
        SystemActionId::DismissNotifications => Ok(Some(Feedback {
            message: dismiss_unavailable_message().into(),
            noop: true,
        })),
        SystemActionId::ToggleBluetooth => toggle_bluetooth(),
    }
}

pub fn current_volume() -> Result<f32, Failure> {
    unsafe {
        endpoint()?
            .GetMasterVolumeLevelScalar()
            .map_err(|_| Failure::new("The current audio output does not support software volume."))
    }
}

pub fn output_state() -> Result<(f32, bool), Failure> {
    let ep = endpoint()?;
    let level = unsafe {
        ep.GetMasterVolumeLevelScalar().map_err(|_| {
            Failure::new("The current audio output does not support software volume.")
        })?
    };
    let muted = unsafe { ep.GetMute().map(|v| v.as_bool()).unwrap_or(level == 0.0) };
    Ok((level, muted || level == 0.0 && muted))
}

pub fn set_volume(level: f32) -> Result<(), Failure> {
    let ep = endpoint()?;
    let value = clamped(level);
    unsafe {
        ep.SetMasterVolumeLevelScalar(value, std::ptr::null())
            .map_err(|_| Failure::new("Windows could not change the output volume."))?;
        if value > 0.0 {
            let _ = ep.SetMute(false, std::ptr::null());
        }
    }
    Ok(())
}

pub fn quit_all_targets() -> Vec<u32> {
    let own = unsafe { GetCurrentProcessId() };
    let mut pids = Vec::new();
    let mut state = EnumState {
        own,
        previous: 0,
        pids: &mut pids,
        collect_quit: true,
        hide: false,
        unhide: false,
        shown: 0,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut state as *mut EnumState as isize),
        );
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

fn lock_screen() -> Result<(), Failure> {
    unsafe {
        LockWorkStation().map_err(|_| Failure::new("Windows could not lock the workstation."))
    }
}

fn sleep() -> Result<(), Failure> {
    let ok = unsafe { SetSuspendState(false, false, false) };
    if ok.as_bool() {
        Ok(())
    } else {
        Err(Failure::new("Windows could not put the PC to sleep."))
    }
}

fn sleep_displays() -> Result<(), Failure> {
    syscommand(SC_MONITORPOWER, MONITOR_OFF)
}

fn screensaver() -> Result<(), Failure> {
    syscommand(SC_SCREENSAVE, 0)
}

fn syscommand(cmd: usize, lparam: isize) -> Result<(), Failure> {
    unsafe {
        let mut result = 0usize;
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SYSCOMMAND,
            WPARAM(cmd),
            LPARAM(lparam),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        );
    }
    Ok(())
}

fn session_end(flags: windows::Win32::System::Shutdown::EXIT_WINDOWS_FLAGS) -> Result<(), Failure> {
    enable_shutdown_privilege();
    unsafe {
        ExitWindowsEx(
            flags,
            SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER | SHTDN_REASON_FLAG_PLANNED,
        )
        .map_err(|_| Failure::new("Windows could not end the session."))
    }
}

fn enable_shutdown_privilege() {
    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return;
        }
        let mut luid = windows::Win32::Foundation::LUID::default();
        if LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid).is_ok() {
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [windows::Win32::Security::LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            let _ = AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None);
            let _ = tp;
        }
        let _ = CloseHandle(token);
    }
}

fn media_key(vk: VIRTUAL_KEY) -> Result<(), Failure> {
    unsafe {
        let mut inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        let n = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
        if n == 0 {
            Err(Failure::new("Windows could not send the media key."))
        } else {
            Ok(())
        }
    }
}

fn endpoint() -> Result<IAudioEndpointVolume, Failure> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|_| Failure::new("No audio output device is available."))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .map_err(|_| Failure::new("No audio output device is available."))?;
        device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .map_err(|_| Failure::new("The current audio output does not support software volume."))
    }
}

fn step_volume(up: bool) -> Result<(), Failure> {
    let current = current_volume()?;
    set_volume(volume_step(current, up))
}

fn toggle_mute() -> Result<(), Failure> {
    let ep = endpoint()?;
    unsafe {
        match ep.GetMute() {
            Ok(muted) => {
                if muted.as_bool() {
                    let _ = ep.SetMute(false, std::ptr::null());
                } else {
                    let _ = ep.SetMute(true, std::ptr::null());
                }
                Ok(())
            }
            Err(_) => {
                let current = current_volume()?;
                if current > 0.0 {
                    LAST_NONZERO.store(current.to_bits(), Ordering::SeqCst);
                    set_volume(0.0)
                } else {
                    set_volume(f32::from_bits(LAST_NONZERO.load(Ordering::SeqCst)))
                }
            }
        }
    }
}

fn show_desktop() -> Result<(), Failure> {
    unsafe {
        let shell: IShellDispatch4 = CoCreateInstance(&Shell, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| Failure::new("Windows could not show the desktop."))?;
        shell
            .ToggleDesktop()
            .map_err(|_| Failure::new("Windows could not show the desktop."))
    }
}

fn toggle_appearance() -> Result<Option<Feedback>, Failure> {
    let path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let light = match read_dword(&path, "AppsUseLightTheme") {
        Some(v) => v != 0,
        None => {
            return Err(Failure::new(
                "Windows could not read the current appearance.",
            ))
        }
    };
    let next: u32 = if light { 0 } else { 1 };
    write_dword(&path, "AppsUseLightTheme", next)?;
    write_dword(&path, "SystemUsesLightTheme", next)?;
    broadcast_setting("ImmersiveColorSet");
    let message = if next == 0 {
        "Dark Appearance"
    } else {
        "Light Appearance"
    };
    Ok(Some(Feedback {
        message: message.into(),
        noop: false,
    }))
}

fn open_trash() -> Result<(), Failure> {
    crate::features::launcher::ui::coordinator::execute(
        &crate::features::launcher::ui::coordinator::LaunchSpec::Uri(
            "shell:RecycleBinFolder".into(),
        ),
    )
    .map_err(|_| Failure::new("Explorer could not open the Recycle Bin."))
}

fn empty_trash() -> Result<Option<Feedback>, Failure> {
    unsafe {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            i64Size: 0,
            i64NumItems: 0,
        };
        let _ = SHQueryRecycleBinW(PCWSTR::null(), &mut info);
        if info.i64NumItems <= 0 {
            return Ok(Some(Feedback {
                message: "Trash Is Already Empty".into(),
                noop: true,
            }));
        }
        SHEmptyRecycleBinW(
            HWND::default(),
            PCWSTR::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
        .map_err(|_| Failure::new("Windows could not empty the Recycle Bin."))?;
    }
    Ok(Some(Feedback {
        message: "Trash Emptied".into(),
        noop: false,
    }))
}

fn eject_all_disks() -> Result<Option<Feedback>, Failure> {
    let mut ejected = 0u32;
    unsafe {
        let set = SetupDiGetClassDevsW(
            Some(&GUID_DEVCLASS_DISKDRIVE),
            PCWSTR::null(),
            HWND::default(),
            DIGCF_PRESENT,
        )
        .map_err(|_| Failure::new("Windows could not list removable disks."))?;
        let mut info = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ClassGuid: GUID::default(),
            DevInst: 0,
            Reserved: 0,
        };
        let mut index = 0u32;
        while SetupDiEnumDeviceInfo(set, index, &mut info).is_ok() {
            index += 1;
            let mut policy: u32 = 0;
            let mut needed = 0u32;
            let mut reg_type = 0u32;
            let ok = SetupDiGetDeviceRegistryPropertyW(
                set,
                &info,
                SPDRP_REMOVAL_POLICY,
                Some(&mut reg_type),
                Some(std::slice::from_raw_parts_mut(
                    (&mut policy as *mut u32).cast::<u8>(),
                    std::mem::size_of::<u32>(),
                )),
                Some(&mut needed),
            );
            if ok.is_err() || policy == REMOVAL_EXPECT_NO_REMOVAL {
                continue;
            }
            if CM_Request_Device_EjectW(info.DevInst, None, None, 0) == CR_SUCCESS {
                ejected += 1;
            }
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
    }
    if ejected == 0 {
        Ok(Some(Feedback {
            message: "No Disks to Eject".into(),
            noop: true,
        }))
    } else if ejected == 1 {
        Ok(Some(Feedback {
            message: "1 Disk Ejected".into(),
            noop: false,
        }))
    } else {
        Ok(Some(Feedback {
            message: format!("{ejected} Disks Ejected"),
            noop: false,
        }))
    }
}

fn toggle_hidden_files() -> Result<Option<Feedback>, Failure> {
    let path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced\0"
        .encode_utf16()
        .collect();
    let current = read_dword(&path, "Hidden").unwrap_or(2);
    let show = current != 1;
    let next = if show { 1u32 } else { 2u32 };
    write_dword(&path, "Hidden", next)?;
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
    broadcast_setting("windows");
    Ok(Some(Feedback {
        message: if show {
            "Hidden Files Shown".into()
        } else {
            "Hidden Files Hidden".into()
        },
        noop: false,
    }))
}

fn hide_other_apps(previous: HWND) -> Result<(), Failure> {
    let own = unsafe { GetCurrentProcessId() };
    let mut previous_pid = 0u32;
    if !previous.is_invalid() {
        unsafe {
            GetWindowThreadProcessId(previous, Some(&mut previous_pid));
        }
    }
    let mut hidden = Vec::new();
    let mut state = EnumState {
        own,
        previous: previous_pid,
        pids: &mut Vec::new(),
        collect_quit: false,
        hide: true,
        unhide: false,
        shown: 0,
    };
    let mut captured = Vec::new();
    state.pids = &mut captured;
    // reuse pids vec as hwnd bits via a dedicated field
    let mut hwnds: Vec<isize> = Vec::new();
    let mut hide_state = HideState {
        own,
        previous: previous_pid,
        hwnds: &mut hwnds,
    };
    unsafe {
        let _ = EnumWindows(
            Some(hide_proc),
            LPARAM(&mut hide_state as *mut HideState as isize),
        );
        for bits in &hwnds {
            let hwnd = HWND(*bits as *mut core::ffi::c_void);
            let _ = ShowWindow(hwnd, SW_HIDE);
            hidden.push(*bits);
        }
    }
    if let Ok(mut slot) = HIDDEN.lock() {
        *slot = hidden;
    }
    let _ = state;
    Ok(())
}

fn unhide_all_apps() -> Result<Option<Feedback>, Failure> {
    let mut hwnds = HIDDEN.lock().ok().map(|mut g| std::mem::take(&mut *g));
    let list = hwnds.get_or_insert_with(Vec::new);
    if list.is_empty() {
        return Ok(Some(Feedback {
            message: "Nothing Was Hidden".into(),
            noop: true,
        }));
    }
    unsafe {
        for bits in list.iter() {
            let hwnd = HWND(*bits as *mut core::ffi::c_void);
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }
    }
    Ok(Some(Feedback {
        message: "All Apps Unhidden".into(),
        noop: false,
    }))
}

fn quit_all_apps() {
    for pid in quit_all_targets() {
        close_pid_windows(pid);
    }
}

fn close_pid_windows(pid: u32) {
    unsafe {
        let _ = EnumWindows(Some(close_proc), LPARAM(pid as isize));
    }
}

unsafe extern "system" fn close_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let target = lparam.0 as u32;
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == target && IsWindowVisible(hwnd).as_bool() && !has_owner(hwnd) {
        let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    true.into()
}

struct EnumState<'a> {
    own: u32,
    previous: u32,
    pids: &'a mut Vec<u32>,
    collect_quit: bool,
    hide: bool,
    unhide: bool,
    shown: u32,
}

struct HideState<'a> {
    own: u32,
    previous: u32,
    hwnds: &'a mut Vec<isize>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut EnumState);
    if !IsWindowVisible(hwnd).as_bool() {
        return true.into();
    }
    if has_owner(hwnd) {
        return true.into();
    }
    if is_shell_window(hwnd) {
        return true.into();
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == state.own {
        return true.into();
    }
    if is_explorer(pid) {
        return true.into();
    }
    if state.collect_quit {
        if GetWindowTextLengthW(hwnd) == 0 {
            return true.into();
        }
        state.pids.push(pid);
    }
    let _ = (state.hide, state.unhide, state.shown, state.previous);
    true.into()
}

unsafe extern "system" fn hide_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut HideState);
    if !IsWindowVisible(hwnd).as_bool() {
        return true.into();
    }
    if has_owner(hwnd) {
        return true.into();
    }
    if is_shell_window(hwnd) {
        return true.into();
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == state.own || (state.previous != 0 && pid == state.previous) {
        return true.into();
    }
    if is_explorer(pid) {
        return true.into();
    }
    let ex = WINDOW_EX_STYLE(windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
        hwnd,
        GWL_EXSTYLE,
    ) as u32);
    if ex.contains(WS_EX_TOOLWINDOW) && !ex.contains(WS_EX_APPWINDOW) {
        return true.into();
    }
    state.hwnds.push(hwnd.0 as isize);
    true.into()
}

fn has_owner(hwnd: HWND) -> bool {
    unsafe { GetWindow(hwnd, GW_OWNER).is_ok() }
}

fn is_shell_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    if n == 0 {
        return false;
    }
    let name = String::from_utf16_lossy(&buf[..n]);
    matches!(
        name.as_str(),
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "Progman"
            | "WorkerW"
            | "NotifyIconOverflowWindow"
    )
}

fn is_explorer(pid: u32) -> bool {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return false;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]).to_ascii_lowercase();
        path.ends_with("\\explorer.exe")
    }
}

fn toggle_bluetooth() -> Result<Option<Feedback>, Failure> {
    use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
    let access = Radio::RequestAccessAsync()
        .and_then(|op| op.get())
        .map_err(|_| Failure::bluetooth("Bluetooth permission is required."))?;
    if access != RadioAccessStatus::Allowed {
        return Err(Failure::bluetooth(
            "Allow Tinycast to control Bluetooth in Windows Settings, then try again.",
        ));
    }
    let radios = Radio::GetRadiosAsync()
        .and_then(|op| op.get())
        .map_err(|_| Failure::bluetooth("Windows could not list radios."))?;
    let mut bt = None;
    let n = radios.Size().unwrap_or(0);
    for i in 0..n {
        if let Ok(radio) = radios.GetAt(i) {
            if radio.Kind().ok() == Some(RadioKind::Bluetooth) {
                bt = Some(radio);
                break;
            }
        }
    }
    let radio = bt.ok_or_else(|| Failure::bluetooth("No Bluetooth radio is available."))?;
    let current = radio
        .State()
        .map_err(|_| Failure::bluetooth("Windows could not read Bluetooth state."))?;
    let next = if current == RadioState::On {
        RadioState::Off
    } else {
        RadioState::On
    };
    let status = radio
        .SetStateAsync(next)
        .and_then(|op| op.get())
        .map_err(|_| Failure::bluetooth("Windows could not change Bluetooth state."))?;
    if status != RadioAccessStatus::Allowed {
        return Err(Failure::bluetooth(
            "Windows did not allow changing Bluetooth state.",
        ));
    }
    Ok(Some(Feedback {
        message: if next == RadioState::On {
            "Bluetooth On".into()
        } else {
            "Bluetooth Off".into()
        },
        noop: false,
    }))
}

fn read_dword(path: &[u16], name: &str) -> Option<u32> {
    unsafe {
        let mut key = windows::Win32::System::Registry::HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        );
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
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

fn write_dword(path: &[u16], name: &str, value: u32) -> Result<(), Failure> {
    unsafe {
        let mut key = windows::Win32::System::Registry::HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            0,
            KEY_SET_VALUE | KEY_READ,
            &mut key,
        );
        if status != ERROR_SUCCESS {
            return Err(Failure::new("Windows could not write this setting."));
        }
        let mut name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = value.to_le_bytes();
        let status = RegSetValueExW(key, PCWSTR(name_w.as_mut_ptr()), 0, REG_DWORD, Some(&bytes));
        let _ = RegCloseKey(key);
        if status != ERROR_SUCCESS {
            Err(Failure::new("Windows could not write this setting."))
        } else {
            Ok(())
        }
    }
}

fn broadcast_setting(name: &str) {
    let mut wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut result = 0usize;
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(wide.as_mut_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        );
    }
}
