//! EXPERIMENTAL live blur under the palette, off by default.
//!
//! Upstream draws the panel as a scrim over a blurred background. The palette here is a
//! layered window (`UpdateLayeredWindow`, per-pixel alpha for the 26-DIP squircle), so
//! the DWM system backdrop set in `apply_dwm` never shows through it. This spike puts a
//! second, click-through window *directly below* the palette in z-order and fills it
//! with a `Windows.UI.Composition` host-backdrop brush (the blur Acrylic is built on),
//! clipped to a rounded rect a little larger than the squircle
//! (`tinycast_pure::backdrop::SQUIRCLE_TO_CIRCLE`). The palette's own 0.40 / 0.55 scrim
//! then lands on top of the blur exactly as upstream layers it.
//!
//! Enable: set `TINYCAST_EXPERIMENTAL_BLUR=1` before launching. Any failure (Windows 10,
//! transparency effects off, composition unavailable) leaves the plain scrim.
//!
//! Needs verification on real Windows 11 (Wine has no DWM composition):
//! - blur is visible behind the palette and follows it on move / resize / expand;
//! - nothing blurred leaks past the squircle corners at 100 / 125 / 150 %;
//! - the backdrop never takes clicks or focus, and hides with the palette;
//! - whether the host backdrop stays live while the backdrop window is inactive.

use std::cell::RefCell;

use tinycast_pure::backdrop;
use tinycast_pure::theme;
use windows::core::{w, Interface, PCWSTR};
use windows::Foundation::Numerics::Vector2;
use windows::System::DispatcherQueueController;
use windows::Win32::Foundation::{
    BOOL, COLORREF, ERROR_SUCCESS, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_HOSTBACKDROPBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DispatcherQueueOptions, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowRect, IsWindowVisible, RegisterClassW,
    SetLayeredWindowAttributes, SetWindowPos, HTTRANSPARENT, LWA_ALPHA, SWP_HIDEWINDOW,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, WM_NCHITTEST, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{
    CompositionRoundedRectangleGeometry, Compositor, ContainerVisual, SpriteVisual,
};

const CLASS: PCWSTR = w!("TinycastBackdrop");

thread_local! {
    /// One dispatcher queue per UI thread; composition needs it alive for the thread's life.
    static QUEUE: RefCell<Option<DispatcherQueueController>> = const { RefCell::new(None) };
}

/// `TINYCAST_EXPERIMENTAL_BLUR=1` and Windows transparency effects on.
pub fn requested() -> bool {
    backdrop::flag_enabled(std::env::var(backdrop::ENV_FLAG).ok().as_deref())
        && backdrop::transparency_enabled(transparency_registry_value())
}

fn transparency_registry_value() -> Option<u32> {
    let mut value = 0u32;
    let mut bytes = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("EnableTransparency"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut bytes),
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

fn ensure_dispatcher_queue() -> windows::core::Result<()> {
    QUEUE.with(|queue| {
        if queue.borrow().is_some() {
            return Ok(());
        }
        let controller = unsafe {
            CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_STA,
            })?
        };
        *queue.borrow_mut() = Some(controller);
        Ok(())
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        return LRESULT(HTTRANSPARENT as isize);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// The blur window that tracks one surface window.
pub struct Backdrop {
    hwnd: HWND,
    _compositor: Compositor,
    _target: DesktopWindowTarget,
    _root: ContainerVisual,
    blur: SpriteVisual,
    clip: CompositionRoundedRectangleGeometry,
    radius_dip: f32,
    shown: bool,
}

impl Backdrop {
    /// `None` when the flag is off or any piece of the composition stack is missing.
    pub fn create_if_requested(owner: HWND) -> Option<Self> {
        if !requested() {
            return None;
        }
        Self::create(owner, theme::radius::PANEL).ok()
    }

    pub fn create(owner: HWND, radius_dip: f32) -> windows::core::Result<Self> {
        ensure_dispatcher_queue()?;
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            let _ = RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TRANSPARENT
                    | WS_EX_LAYERED
                    | WS_EX_NOREDIRECTIONBITMAP,
                CLASS,
                w!("Tinycast backdrop"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                owner,
                None,
                hinstance,
                None,
            )?;
            let built = (|| {
                // Layered so WS_EX_TRANSPARENT makes it click-through; alpha stays 255
                // and the composition content decides what is drawn.
                SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
                let on = BOOL(1);
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_HOSTBACKDROPBRUSH,
                    &on as *const BOOL as *const _,
                    std::mem::size_of::<BOOL>() as u32,
                )?;
                let compositor = Compositor::new()?;
                let interop: ICompositorDesktopInterop = compositor.cast()?;
                let target = interop.CreateDesktopWindowTarget(hwnd, BOOL(0))?;
                let root = compositor.CreateContainerVisual()?;
                let blur = compositor.CreateSpriteVisual()?;
                blur.SetBrush(&compositor.CreateHostBackdropBrush()?)?;
                let clip = compositor.CreateRoundedRectangleGeometry()?;
                blur.SetClip(&compositor.CreateGeometricClipWithGeometry(&clip)?)?;
                root.Children()?.InsertAtTop(&blur)?;
                target.SetRoot(&root)?;
                Ok::<_, windows::core::Error>((compositor, target, root, blur, clip))
            })();
            match built {
                Ok((compositor, target, root, blur, clip)) => Ok(Self {
                    hwnd,
                    _compositor: compositor,
                    _target: target,
                    _root: root,
                    blur,
                    clip,
                    radius_dip,
                    shown: false,
                }),
                Err(err) => {
                    let _ = DestroyWindow(hwnd);
                    Err(err)
                }
            }
        }
    }

    /// Match `surface`'s rect and visibility and sit directly beneath it.
    pub fn follow(&mut self, surface: HWND) {
        unsafe {
            let visible = IsWindowVisible(surface).as_bool();
            let mut rc = RECT::default();
            if !visible || GetWindowRect(surface, &mut rc).is_err() {
                if self.shown {
                    let _ =
                        SetWindowPos(self.hwnd, None, 0, 0, 0, 0, SWP_HIDEWINDOW | SWP_NOACTIVATE);
                    self.shown = false;
                }
                return;
            }
            let w = (rc.right - rc.left).max(0);
            let h = (rc.bottom - rc.top).max(0);
            let size = Vector2 {
                X: w as f32,
                Y: h as f32,
            };
            let radius = backdrop::clip_radius_px(self.radius_dip, GetDpiForWindow(surface))
                .min(size.X * 0.5)
                .min(size.Y * 0.5);
            let _ = self.blur.SetSize(size);
            let _ = self.clip.SetSize(size);
            let _ = self.clip.SetCornerRadius(Vector2 {
                X: radius,
                Y: radius,
            });
            // Insert-after = surface puts the backdrop immediately below it.
            let _ = SetWindowPos(
                self.hwnd,
                surface,
                rc.left,
                rc.top,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            self.shown = true;
        }
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
