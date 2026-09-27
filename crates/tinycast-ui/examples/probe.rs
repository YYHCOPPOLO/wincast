//! Stage-0 visual probe. The process always leaves the event loop itself.
//!
//! winit 0.30's attribute hook can skip the taskbar, but it does not set
//! `WS_EX_TOOLWINDOW`. That bit is applied with `WinitWindowAccessor` once the
//! HWND exists, which is the gap the design leaves for styles the hook cannot
//! express.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;
use windows::core::w;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    SRCCOPY,
};
use windows::Win32::UI::Input::Ime::{
    ImmGetCandidateWindow, ImmGetContext, ImmReleaseContext, CANDIDATEFORM,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ActivateKeyboardLayout, GetKeyboardLayout, LoadKeyboardLayoutW, SendInput, HKL, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KLF_ACTIVATE, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WS_EX_TOOLWINDOW,
};

const SQUIRCLE_W: f32 = 220.0;
const SQUIRCLE_H: f32 = 120.0;
const SQUIRCLE_R: f32 = 26.0;
const WS_EX_TOOLWINDOW_BIT: isize = 0x0000_0080;
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;
const WS_EX_TOPMOST_BIT: isize = 0x0000_0008;

struct Args {
    scale: f32,
    out: PathBuf,
    ime: bool,
}

struct LayoutGuard(HKL);

impl Drop for LayoutGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ActivateKeyboardLayout(self.0, Default::default());
        }
    }
}

struct Run {
    args: Args,
    started: Instant,
    hwnd: isize,
    exstyle: isize,
    styled: bool,
    animated: bool,
    layout_tried: bool,
    keys_sent: u8,
    keys_sent_at: Option<Instant>,
    finished: bool,
    layout: Option<LayoutGuard>,
    notes: String,
    failed: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    // Slint reads this once, when the winit window is created. Set it before
    // any Slint thread exists; Rust 1.87 made `set_var` unsafe.
    unsafe {
        std::env::set_var("SLINT_SCALE_FACTOR", format!("{}", args.scale));
    }

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(|attributes| attributes.with_skip_taskbar(true))
        .select()?;

    let ui = tinycast_ui::ProbeWindow::new()?;
    let (red, green, blue, alpha) = tinycast_pure::theme::colors::scrim_rgba(0);
    ui.set_scrim(slint::Color::from_argb_f32(alpha, red, green, blue));
    ui.set_squircle_commands(tinycast_ui::squircle_commands(
        0.0, 0.0, SQUIRCLE_W, SQUIRCLE_H, SQUIRCLE_R,
    ));
    ui.window()
        .set_position(slint::LogicalPosition { x: 48.0, y: 48.0 });

    let run = Rc::new(RefCell::new(Run {
        args,
        started: Instant::now(),
        hwnd: 0,
        exstyle: 0,
        styled: false,
        animated: false,
        layout_tried: false,
        keys_sent: 0,
        keys_sent_at: None,
        finished: false,
        layout: None,
        notes: String::new(),
        failed: None,
    }));
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    let tick_run = Rc::clone(&run);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(ui) = weak.upgrade() else {
                let _ = slint::quit_event_loop();
                return;
            };
            tick(&ui, &tick_run);
        },
    );

    ui.run()?;
    let run = run.borrow();
    if let Some(error) = &run.failed {
        return Err(error.clone().into());
    }
    Ok(())
}

fn tick(ui: &tinycast_ui::ProbeWindow, run: &Rc<RefCell<Run>>) {
    let mut run = run.borrow_mut();
    if run.failed.is_some() || run.finished {
        return;
    }
    if !run.styled {
        match apply_tool_window(ui) {
            Ok((hwnd, exstyle)) => {
                run.hwnd = hwnd;
                run.exstyle = exstyle;
                run.styled = true;
                ui.set_entered(true);
                ui.set_sprung(true);
            }
            Err(error) => {
                if run.started.elapsed() > Duration::from_millis(1500) {
                    run.failed = Some(error);
                    let _ = slint::quit_event_loop();
                }
                return;
            }
        }
    }

    let settled = ui.get_intro_scale() > 0.999
        && ui.get_intro_opacity() > 0.999
        && !ui.window().has_active_animations();
    if !run.animated && (settled || run.started.elapsed() > Duration::from_millis(1200)) {
        run.animated = true;
    }
    if !run.animated {
        return;
    }

    if run.args.ime && run.keys_sent < 2 {
        if !run.layout_tried {
            run.layout_tried = true;
            run.layout = activate_pinyin();
            if run.layout.is_none() {
                run.notes
                    .push_str("ime: LoadKeyboardLayout 00000804 failed\n");
            }
            return;
        }
        let hwnd = HWND(run.hwnd as *mut core::ffi::c_void);
        let foreground = unsafe { GetForegroundWindow() };
        if foreground != hwnd {
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
        let foreground = unsafe { GetForegroundWindow() };
        if foreground != hwnd || !ui.get_query_focused() {
            run.notes.push_str(&format!(
                "ime: skipped keys foreground_match={} query_focused={}\n",
                foreground == hwnd,
                ui.get_query_focused()
            ));
            finish(ui, &mut run);
            return;
        }
        let vk = if run.keys_sent == 0 { 0x4E } else { 0x49 };
        if let Err(error) = send_vk(VIRTUAL_KEY(vk)) {
            run.notes
                .push_str(&format!("ime: SendInput failed {error}\n"));
            finish(ui, &mut run);
            return;
        }
        run.keys_sent += 1;
        if run.keys_sent == 2 {
            run.keys_sent_at = Some(Instant::now());
        }
        return;
    }

    if run.args.ime {
        let ready = run
            .keys_sent_at
            .is_some_and(|sent| sent.elapsed() >= Duration::from_millis(600));
        if !ready {
            return;
        }
    }
    finish(ui, &mut run);
}

fn finish(ui: &tinycast_ui::ProbeWindow, run: &mut Run) {
    run.finished = true;
    if let Err(error) = capture(ui, run) {
        run.failed = Some(error);
    }
    let _ = slint::quit_event_loop();
}

fn capture(ui: &tinycast_ui::ProbeWindow, run: &mut Run) -> Result<(), String> {
    let buffer = ui
        .window()
        .take_snapshot()
        .map_err(|error| error.to_string())?;
    let width = buffer.width();
    let height = buffer.height();
    let bytes = buffer.as_bytes();
    if bytes.len() != width as usize * height as usize * 4 {
        return Err(format!(
            "snapshot stride {} != {width}x{height}",
            bytes.len()
        ));
    }
    let corner = pixel_alpha(bytes, width, 2, 2);
    std::fs::create_dir_all(run.args.out.parent().unwrap_or(std::path::Path::new(".")))
        .map_err(|error| error.to_string())?;
    write_png(&raw_path(&run.args.out), width, height, bytes)?;
    write_png(&run.args.out, width, height, &composite_on_white(bytes))?;
    let desktop = desktop_path(&run.args.out);
    let desktop_note = match grab_window(HWND(run.hwnd as *mut core::ffi::c_void), &desktop) {
        Ok(()) => format!("desktop={}\n", desktop.display()),
        Err(error) => format!("desktop_grab_failed={error}\n"),
    };
    let candidate = candidate_note(HWND(run.hwnd as *mut core::ffi::c_void));
    let actual = ui.window().scale_factor();
    let tool = run.exstyle & WS_EX_TOOLWINDOW_BIT != 0;
    let top = run.exstyle & WS_EX_TOPMOST_BIT != 0;
    let metrics = metrics_path(&run.args.out);
    let body = format!(
        "renderer=femtovg\nrequested_scale={}\nactual_scale={actual}\nsnapshot_px={width}x{height}\nintro_scale={}\nintro_opacity={}\nknob_x={}\nexstyle={:#x}\nws_ex_toolwindow={tool}\nws_ex_topmost={top}\ncorner_alpha={corner}\nquery_focused={}\nquery_text={}\npreedit={}\nime_requested={}\n{desktop_note}{candidate}{}\n",
        run.args.scale,
        ui.get_intro_scale(),
        ui.get_intro_opacity(),
        ui.get_knob_x(),
        run.exstyle,
        ui.get_query_focused(),
        ui.get_query_text(),
        ui.get_preedit(),
        run.args.ime,
        run.notes,
    );
    std::fs::write(&metrics, body).map_err(|error| error.to_string())?;
    println!("{}", metrics.display());
    Ok(())
}

fn apply_tool_window(ui: &tinycast_ui::ProbeWindow) -> Result<(isize, isize), String> {
    let mut found = None;
    let saw = ui.window().with_winit_window(|window| {
        let handle = window.window_handle().map_err(|error| error.to_string())?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return Err("winit window is not a Win32 HWND".to_string());
        };
        let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
        unsafe {
            let before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let tool = WS_EX_TOOLWINDOW.0 as isize;
            // winit's skip-taskbar hook does not set this bit, and it also
            // leaves WS_EX_APPWINDOW, which puts a tool window back on the taskbar.
            let next = (before | tool) & !WS_EX_APPWINDOW_BIT;
            if next != before {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
            let after = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            found = Some((win32.hwnd.get(), after));
        }
        Ok(())
    });
    match saw {
        Some(Ok(())) => found.ok_or_else(|| "HWND missing after the style hook".to_string()),
        Some(Err(error)) => Err(error),
        None => Err("winit window is not created yet".to_string()),
    }
}

fn activate_pinyin() -> Option<LayoutGuard> {
    unsafe {
        let previous = GetKeyboardLayout(0);
        let loaded = LoadKeyboardLayoutW(w!("00000804"), KLF_ACTIVATE).ok()?;
        if loaded.is_invalid() {
            return None;
        }
        Some(LayoutGuard(previous))
    }
}

fn send_vk(vk: VIRTUAL_KEY) -> Result<(), String> {
    unsafe {
        let mut inputs = [key_input(vk, false), key_input(vk, true)];
        let sent = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
        if sent == 0 {
            Err("SendInput returned 0".to_string())
        } else {
            Ok(())
        }
    }
}

fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
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

fn candidate_note(hwnd: HWND) -> String {
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.0.is_null() {
            return "candidate=no ImmGetContext\n".to_string();
        }
        let mut form = CANDIDATEFORM::default();
        let ok = ImmGetCandidateWindow(himc, 0, &mut form).as_bool();
        let _ = ImmReleaseContext(hwnd, himc);
        if !ok {
            return "candidate=ImmGetCandidateWindow failed\n".to_string();
        }
        format!(
            "candidate=pt {},{} area {},{},{},{}\n",
            form.ptCurrentPos.x,
            form.ptCurrentPos.y,
            form.rcArea.left,
            form.rcArea.top,
            form.rcArea.right,
            form.rcArea.bottom
        )
    }
}

fn grab_window(hwnd: HWND, path: &std::path::Path) -> Result<(), String> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect).map_err(|error| error.to_string())?;
    }
    let x = rect.left.saturating_sub(24);
    let y = rect.top.saturating_sub(24);
    let width = (rect.right - rect.left + 48).max(1);
    let height = (rect.bottom - rect.top + 220).max(1);
    let pixels = unsafe { grab_screen(x, y, width, height)? };
    write_png(path, width as u32, height as u32, &pixels)
}

unsafe fn grab_screen(x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u8>, String> {
    let screen = GetDC(None);
    if screen.is_invalid() {
        return Err("GetDC failed".to_string());
    }
    let memory = CreateCompatibleDC(screen);
    if memory.is_invalid() {
        ReleaseDC(None, screen);
        return Err("CreateCompatibleDC failed".to_string());
    }
    let bitmap = CreateCompatibleBitmap(screen, width, height);
    if bitmap.is_invalid() {
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        return Err("CreateCompatibleBitmap failed".to_string());
    }
    let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
    let copied = BitBlt(memory, 0, 0, width, height, screen, x, y, SRCCOPY);
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    let rows = GetDIBits(
        memory,
        bitmap,
        0,
        height as u32,
        Some(bgra.as_mut_ptr().cast()),
        &mut info,
        DIB_RGB_COLORS,
    );
    SelectObject(memory, previous);
    let _ = DeleteObject(bitmap);
    let _ = DeleteDC(memory);
    ReleaseDC(None, screen);
    if copied.is_err() || rows == 0 {
        return Err(format!("screen copy copied={copied:?} rows={rows}"));
    }
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    Ok(bgra)
}

fn pixel_alpha(bytes: &[u8], width: u32, x: u32, y: u32) -> u8 {
    let index = ((y as usize * width as usize) + x as usize) * 4;
    bytes.get(index + 3).copied().unwrap_or(255)
}

fn composite_on_white(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for pixel in bytes.chunks_exact(4) {
        let alpha = pixel[3] as f32 / 255.0;
        let blend = |channel: u8| (channel as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8;
        out.push(blend(pixel[0]));
        out.push(blend(pixel[1]));
        out.push(blend(pixel[2]));
        out.push(255);
    }
    out
}

fn write_png(path: &std::path::Path, width: u32, height: u32, bytes: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(bytes)
        .map_err(|error| error.to_string())
}

fn raw_path(out: &std::path::Path) -> PathBuf {
    suffix(out, "-raw")
}

fn desktop_path(out: &std::path::Path) -> PathBuf {
    suffix(out, "-desktop")
}

fn metrics_path(out: &std::path::Path) -> PathBuf {
    let mut path = out.to_path_buf();
    path.set_extension("txt");
    path
}

fn suffix(out: &std::path::Path, extra: &str) -> PathBuf {
    let stem = out
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("probe");
    let mut path = out.to_path_buf();
    path.set_file_name(format!("{stem}{extra}.png"));
    path
}

fn parse_args() -> Args {
    let mut scale = 1.0;
    let mut out = PathBuf::from("probe.png");
    let mut ime = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ime" => ime = true,
            "--scale" => {
                scale = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(scale);
            }
            "--out" => {
                if let Some(value) = args.next() {
                    out = PathBuf::from(value);
                }
            }
            _ => {}
        }
    }
    Args { scale, out, ime }
}
