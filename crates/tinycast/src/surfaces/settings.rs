use tinycast_pure::settings_tab::{SettingsSection, SettingsTab};
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{
    D2DERR_RECREATE_TARGET, FALSE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F, D2D_RECT_F,
    D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    ID2D1SolidColorBrush, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetStockObject, InvalidateRect, BLACK_BRUSH, HBRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    GetWindowRect, IsWindow, LoadCursorW, LoadIconW, RegisterClassW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW,
    GWLP_USERDATA, HWND_TOP, IDC_ARROW, IDI_APPLICATION, MINMAXINFO, SWP_NOACTIVATE,
    SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_LBUTTONDOWN, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SIZE, WNDCLASSW, WS_CAPTION,
    WS_CLIPCHILDREN, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW, WS_SYSMENU,
};

use crate::app_core::AppCore;
use crate::platform::screens::{dip_scalar_to_px, screens_px, target_screen_from_cursor_px};

const CLASS: windows::core::PCWSTR = w!("TinycastSettings");
const SECTION_HEADER_HEIGHT: f32 = 22.0;
const TAB_ROW_HEIGHT: f32 = 28.0;
const HEADER_FONT_DIP: f32 = 11.0;
const TAB_FONT_DIP: f32 = 13.0;

fn settings_style() -> WINDOW_STYLE {
    let style = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
    debug_assert_ne!(style.0 & WS_CAPTION.0, 0);
    debug_assert_ne!(style.0 & WS_SYSMENU.0, 0);
    style
}

fn settings_ex_style() -> WINDOW_EX_STYLE {
    let ex = WS_EX_APPWINDOW;
    debug_assert_eq!(ex.0 & WS_EX_TOOLWINDOW.0, 0);
    ex
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowKind {
    Header(SettingsSection),
    Tab(SettingsTab),
}

struct SidebarRow {
    y: f32,
    height: f32,
    kind: RowKind,
}

fn sidebar_rows() -> Vec<SidebarRow> {
    let mut y = theme::spacing::XL;
    let mut rows = Vec::new();
    for (i, section) in SettingsSection::all().into_iter().enumerate() {
        if i > 0 {
            y += theme::spacing::SECTION_SPACING;
        }
        rows.push(SidebarRow {
            y,
            height: SECTION_HEADER_HEIGHT,
            kind: RowKind::Header(section),
        });
        y += SECTION_HEADER_HEIGHT;
        for &tab in section.tabs() {
            rows.push(SidebarRow {
                y,
                height: TAB_ROW_HEIGHT,
                kind: RowKind::Tab(tab),
            });
            y += TAB_ROW_HEIGHT;
        }
    }
    rows
}

fn tab_at(x: f32, y: f32) -> Option<SettingsTab> {
    if x < 0.0 || x >= theme::size::SETTINGS_SIDEBAR {
        return None;
    }
    for row in sidebar_rows() {
        if y >= row.y && y < row.y + row.height {
            return match row.kind {
                RowKind::Tab(tab) => Some(tab),
                RowKind::Header(_) => None,
            };
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CloseAction {
    Hide,
}

fn close_action() -> CloseAction {
    CloseAction::Hide
}

pub struct SettingsWindow {
    pub hwnd: HWND,
}

struct SettingsInner {
    host: HWND,
    renderer: Renderer,
    placing: bool,
}

struct Renderer {
    factory: ID2D1Factory,
    header_format: IDWriteTextFormat,
    tab_format: IDWriteTextFormat,
    hwnd_target: Option<ID2D1HwndRenderTarget>,
}

impl SettingsWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance.into(),
                lpszClassName: CLASS,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                ..Default::default()
            };
            let atom = RegisterClassW(&class);
            if atom == 0 {
                let last = windows::Win32::Foundation::GetLastError();
                if last != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                    return Err(last.into());
                }
            }

            let inner = Box::new(SettingsInner {
                host,
                renderer: Renderer::new()?,
                placing: false,
            });
            let ptr = Box::into_raw(inner);
            let hwnd = match CreateWindowExW(
                settings_ex_style(),
                CLASS,
                w!("Tinycast"),
                settings_style(),
                0,
                0,
                theme::size::SETTINGS_WINDOW.0 as i32,
                theme::size::SETTINGS_WINDOW.1 as i32,
                HWND::default(),
                None,
                hinstance,
                Some(ptr as *const core::ffi::c_void),
            ) {
                Ok(hwnd) => hwnd,
                Err(err) => {
                    drop(Box::from_raw(ptr));
                    return Err(err);
                }
            };
            place_hidden(hwnd);
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_RESTORE);
            apply_client_dip(self.hwnd);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = InvalidateRect(self.hwnd, None, FALSE);
        }
    }

    pub fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, FALSE);
        }
    }
}

impl Drop for SettingsWindow {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

impl Renderer {
    fn new() -> windows::core::Result<Self> {
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let header_format = unsafe {
            dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                HEADER_FONT_DIP,
                w!("en-US"),
            )?
        };
        let tab_format = unsafe {
            dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                TAB_FONT_DIP,
                w!("en-US"),
            )?
        };
        for format in [&header_format, &tab_format] {
            unsafe {
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            }
        }
        Ok(Self {
            factory,
            header_format,
            tab_format,
            hwnd_target: None,
        })
    }

    fn discard_target(&mut self) {
        self.hwnd_target = None;
    }

    fn resize(&mut self, hwnd: HWND, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(target) = &self.hwnd_target {
            let size = D2D_SIZE_U { width, height };
            if unsafe { target.Resize(&size) }.is_err() {
                self.hwnd_target = None;
            }
        }
        if self.hwnd_target.is_none() {
            if let Ok(target) = create_hwnd_target(&self.factory, hwnd, width, height) {
                self.hwnd_target = Some(target);
            }
        }
    }

    fn paint(&mut self, hwnd: HWND, selected: SettingsTab) {
        if self.hwnd_target.is_none() {
            let (w, h) = client_size(hwnd);
            self.resize(hwnd, w, h);
        }
        let Some(target) = &self.hwnd_target else {
            return;
        };
        if let Err(err) = paint_scene(target, &self.header_format, &self.tab_format, selected) {
            if err.code() == D2DERR_RECREATE_TARGET {
                self.hwnd_target = None;
            }
        }
    }
}

fn create_hwnd_target(
    factory: &ID2D1Factory,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> windows::core::Result<ID2D1HwndRenderTarget> {
    let dpi = dpi_of(hwnd);
    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: D2D_SIZE_U { width, height },
        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
    };
    unsafe { factory.CreateHwndRenderTarget(&target_properties(dpi), &hwnd_props) }
}

fn target_properties(dpi: f32) -> D2D1_RENDER_TARGET_PROPERTIES {
    D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: dpi,
        dpiY: dpi,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    }
}

fn dpi_of(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        96.0
    } else {
        dpi as f32
    }
}

fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    (
        (rc.right - rc.left).max(0) as u32,
        (rc.bottom - rc.top).max(0) as u32,
    )
}

fn paint_scene(
    target: &ID2D1RenderTarget,
    header_format: &IDWriteTextFormat,
    tab_format: &IDWriteTextFormat,
    selected: SettingsTab,
) -> windows::core::Result<()> {
    unsafe {
        target.BeginDraw();
        let size = target.GetSize();
        target.Clear(Some(&grouped_form_bg()));
        let sidebar_w = theme::size::SETTINGS_SIDEBAR;
        let sidebar_brush = target.CreateSolidColorBrush(&sidebar_bg(), None)?;
        target.FillRectangle(
            &D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: sidebar_w,
                bottom: size.height,
            },
            &sidebar_brush,
        );
        let line_brush = target.CreateSolidColorBrush(&hairline_color(), None)?;
        target.DrawLine(
            D2D_POINT_2F {
                x: sidebar_w,
                y: 0.0,
            },
            D2D_POINT_2F {
                x: sidebar_w,
                y: size.height,
            },
            &line_brush,
            theme::size::HAIRLINE,
            None,
        );
        let sel_brush = target.CreateSolidColorBrush(&selection_color(), None)?;
        let header_brush = target.CreateSolidColorBrush(&header_text_color(), None)?;
        let tab_brush = target.CreateSolidColorBrush(&tab_text_color(), None)?;
        for row in sidebar_rows() {
            match row.kind {
                RowKind::Header(section) => {
                    draw_label(
                        target,
                        header_format,
                        &header_brush,
                        header_rect(&row),
                        section.title(),
                    )?;
                }
                RowKind::Tab(tab) => {
                    if tab == selected {
                        let pill = D2D1_ROUNDED_RECT {
                            rect: pill_rect(&row),
                            radiusX: theme::radius::MENU_ROW,
                            radiusY: theme::radius::MENU_ROW,
                        };
                        target.FillRoundedRectangle(&pill, &sel_brush);
                    }
                    draw_label(target, tab_format, &tab_brush, tab_rect(&row), tab.title())?;
                }
            }
        }
        target.EndDraw(None, None)
    }
}

fn draw_label(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    brush: &ID2D1SolidColorBrush,
    rect: D2D_RECT_F,
    text: &str,
) -> windows::core::Result<()> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &utf16,
            format,
            &rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn header_rect(row: &SidebarRow) -> D2D_RECT_F {
    D2D_RECT_F {
        left: theme::spacing::XL,
        top: row.y,
        right: theme::size::SETTINGS_SIDEBAR - theme::spacing::MD,
        bottom: row.y + row.height,
    }
}

fn tab_rect(row: &SidebarRow) -> D2D_RECT_F {
    D2D_RECT_F {
        left: theme::spacing::XL + theme::size::SETTINGS_ROW_ICON,
        top: row.y,
        right: theme::size::SETTINGS_SIDEBAR - theme::spacing::MD,
        bottom: row.y + row.height,
    }
}

fn pill_rect(row: &SidebarRow) -> D2D_RECT_F {
    D2D_RECT_F {
        left: theme::spacing::SM,
        top: row.y,
        right: theme::size::SETTINGS_SIDEBAR - theme::spacing::SM,
        bottom: row.y + row.height,
    }
}

fn sidebar_bg() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.11,
        g: 0.11,
        b: 0.11,
        a: 1.0,
    }
}

fn grouped_form_bg() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.16,
        g: 0.16,
        b: 0.16,
        a: 1.0,
    }
}

fn hairline_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.08,
    }
}

fn selection_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: theme::colors::SELECTION_DARK_ALPHA,
    }
}

fn header_text_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.45,
    }
}

fn tab_text_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    }
}

unsafe fn place_hidden(hwnd: HWND) {
    if let Some(inner) = inner_from(hwnd) {
        (*inner).placing = true;
    }
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let screens = screens_px();
    let screen = target_screen_from_cursor_px((pt.x, pt.y), &screens);
    if let Some(s) = screen {
        // Move first so GetDpiForWindow matches the destination monitor.
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            s.work.left,
            s.work.top,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOSIZE,
        );
    }
    let dpi = screen
        .map(|s| s.dpi)
        .filter(|d| *d != 0)
        .unwrap_or_else(|| {
            let d = GetDpiForWindow(hwnd);
            if d == 0 {
                96
            } else {
                d
            }
        });
    let (outer_w, outer_h) = outer_px(
        dip_scalar_to_px(theme::size::SETTINGS_WINDOW.0, dpi),
        dip_scalar_to_px(theme::size::SETTINGS_WINDOW.1, dpi),
        dpi,
    );
    let (x, y) = match screen {
        Some(s) => center_in(s.work, outer_w, outer_h),
        None => (100, 100),
    };
    let _ = SetWindowPos(
        hwnd,
        HWND_TOP,
        x,
        y,
        outer_w,
        outer_h,
        SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
    );
    if let Some(inner) = inner_from(hwnd) {
        (*inner).placing = false;
        (*inner).renderer.discard_target();
    }
}

fn outer_px(client_w: i32, client_h: i32, dpi: u32) -> (i32, i32) {
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: client_w,
        bottom: client_h,
    };
    unsafe {
        let _ =
            AdjustWindowRectExForDpi(&mut rc, settings_style(), FALSE, settings_ex_style(), dpi);
    }
    (rc.right - rc.left, rc.bottom - rc.top)
}

unsafe fn apply_client_dip(hwnd: HWND) {
    let dpi = GetDpiForWindow(hwnd);
    let dpi = if dpi == 0 { 96 } else { dpi };
    let want_w = dip_scalar_to_px(theme::size::SETTINGS_WINDOW.0, dpi);
    let want_h = dip_scalar_to_px(theme::size::SETTINGS_WINDOW.1, dpi);
    let (cw, ch) = client_size(hwnd);
    if cw as i32 == want_w && ch as i32 == want_h {
        return;
    }
    let (ow, oh) = outer_px(want_w, want_h, dpi);
    let mut wr = RECT::default();
    let _ = GetWindowRect(hwnd, &mut wr);
    let x = wr.left + (wr.right - wr.left - ow) / 2;
    let y = wr.top + (wr.bottom - wr.top - oh) / 2;
    if let Some(inner) = inner_from(hwnd) {
        (*inner).placing = true;
    }
    let _ = SetWindowPos(
        hwnd,
        HWND_TOP,
        x,
        y,
        ow,
        oh,
        SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
    );
    if let Some(inner) = inner_from(hwnd) {
        (*inner).placing = false;
        (*inner).renderer.discard_target();
    }
}

fn center_in(work: RECT, w: i32, h: i32) -> (i32, i32) {
    (
        work.left + (work.right - work.left - w) / 2,
        work.top + (work.bottom - work.top - h) / 2,
    )
}

fn point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let v = lparam.0 as u32;
    (v as i16 as i32, (v >> 16) as i16 as i32)
}

fn px_to_dip(px: i32, dpi: u32) -> f32 {
    let dpi = if dpi == 0 { 96.0 } else { dpi as f32 };
    px as f32 * 96.0 / dpi
}

unsafe fn inner_from(hwnd: HWND) -> Option<*mut SettingsInner> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsInner;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn core_from_host(host: HWND) -> Option<*mut AppCore> {
    let ptr = GetWindowLongPtrW(host, GWLP_USERDATA) as *mut AppCore;
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn selected_tab(inner: *mut SettingsInner) -> SettingsTab {
    core_from_host((*inner).host)
        .map(|core| (*core).settings_tab)
        .unwrap_or(SettingsTab::General)
}

unsafe fn apply_min_track(hwnd: HWND, lparam: LPARAM) {
    let mmi = lparam.0 as *mut MINMAXINFO;
    if mmi.is_null() {
        return;
    }
    let dpi = GetDpiForWindow(hwnd);
    let min_w = dip_scalar_to_px(
        theme::size::SETTINGS_SIDEBAR + theme::size::SETTINGS_DETAIL_MINIMUM,
        dpi,
    );
    let min_h = dip_scalar_to_px(theme::size::SETTINGS_WINDOW.1, dpi);
    let (outer_w, outer_h) = outer_px(min_w, min_h, dpi);
    (*mmi).ptMinTrackSize.x = outer_w;
    (*mmi).ptMinTrackSize.y = outer_h;
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            if !cs.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_GETMINMAXINFO => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            apply_min_track(hwnd, lparam);
            result
        }
        WM_SIZE => {
            if let Some(inner) = inner_from(hwnd) {
                let width = (lparam.0 as u32) & 0xffff;
                let height = ((lparam.0 as u32) >> 16) & 0xffff;
                (*inner).renderer.resize(hwnd, width, height);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let _hdc = BeginPaint(hwnd, &mut ps);
            if let Some(inner) = inner_from(hwnd) {
                let selected = selected_tab(inner);
                (*inner).renderer.paint(hwnd, selected);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (px, py) = point_from_lparam(lparam);
            let dpi = GetDpiForWindow(hwnd);
            if let Some(tab) = tab_at(px_to_dip(px, dpi), px_to_dip(py, dpi)) {
                if let Some(inner) = inner_from(hwnd) {
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).select_settings_tab(tab);
                    }
                }
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let placing = inner_from(hwnd)
                .map(|inner| (*inner).placing)
                .unwrap_or(false);
            if !placing {
                let suggested = lparam.0 as *const RECT;
                if !suggested.is_null() {
                    let r = *suggested;
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOP,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            if let Some(inner) = inner_from(hwnd) {
                (*inner).renderer.discard_target();
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            match close_action() {
                CloseAction::Hide => {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        WM_NCDESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsInner;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_maps_to_settings_tab() {
        let rows = sidebar_rows();
        let general = rows
            .iter()
            .find(|r| r.kind == RowKind::Tab(SettingsTab::General))
            .unwrap();
        assert_eq!(
            tab_at(theme::spacing::MD, general.y + 1.0),
            Some(SettingsTab::General)
        );
        let ai = rows
            .iter()
            .find(|r| r.kind == RowKind::Tab(SettingsTab::Ai))
            .unwrap();
        assert_eq!(
            tab_at(theme::spacing::MD, ai.y + 1.0),
            Some(SettingsTab::Ai)
        );
        assert_eq!(
            tab_at(theme::size::SETTINGS_SIDEBAR + 8.0, general.y + 1.0),
            None
        );
        let header = rows
            .iter()
            .find(|r| r.kind == RowKind::Header(SettingsSection::Features))
            .unwrap();
        assert_eq!(tab_at(theme::spacing::MD, header.y + 1.0), None);
    }

    #[test]
    fn sidebar_lists_sections_in_spec_order() {
        let headers: Vec<_> = sidebar_rows()
            .into_iter()
            .filter_map(|r| match r.kind {
                RowKind::Header(section) => Some(section),
                RowKind::Tab(_) => None,
            })
            .collect();
        assert_eq!(headers, Vec::from(SettingsSection::all()));
        let tabs: Vec<_> = sidebar_rows()
            .into_iter()
            .filter_map(|r| match r.kind {
                RowKind::Tab(tab) => Some(tab),
                RowKind::Header(_) => None,
            })
            .collect();
        assert_eq!(tabs[0], SettingsTab::General);
        assert_eq!(tabs[2], SettingsTab::Applications);
        assert_eq!(
            tabs.iter().find(|t| **t == SettingsTab::Ai),
            Some(&SettingsTab::Ai)
        );
        assert_eq!(*tabs.last().unwrap(), SettingsTab::About);
        assert_eq!(theme::size::SETTINGS_SIDEBAR, 215.0);
        assert_eq!(theme::size::SETTINGS_WINDOW, (860.0, 700.0));
    }

    #[test]
    fn settings_is_not_a_tool_window() {
        assert_eq!(settings_ex_style().0 & WS_EX_TOOLWINDOW.0, 0);
        assert_ne!(settings_ex_style().0 & WS_EX_APPWINDOW.0, 0);
        assert_ne!(settings_style().0 & WS_CAPTION.0, 0);
        assert_ne!(settings_style().0 & WS_SYSMENU.0, 0);
    }

    #[test]
    fn settings_close_hides_does_not_quit() {
        assert_eq!(close_action(), CloseAction::Hide);
    }

    #[test]
    fn settings_client_scales_with_dpi() {
        assert_eq!(dip_scalar_to_px(theme::size::SETTINGS_WINDOW.0, 96), 860);
        assert_eq!(dip_scalar_to_px(theme::size::SETTINGS_WINDOW.1, 96), 700);
        assert_eq!(dip_scalar_to_px(theme::size::SETTINGS_SIDEBAR, 96), 215);
        assert_eq!(dip_scalar_to_px(theme::size::SETTINGS_WINDOW.0, 120), 1075);
        assert_eq!(dip_scalar_to_px(theme::size::SETTINGS_WINDOW.1, 120), 875);
    }
}
