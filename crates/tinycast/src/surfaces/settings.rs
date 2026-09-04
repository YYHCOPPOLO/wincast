use tinycast_pure::hotkey::{CaptureOutcome, Modifiers};
use tinycast_pure::settings_tab::{SettingsSection, SettingsTab};
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Foundation::{
    COLORREF, D2DERR_RECREATE_TARGET, FALSE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
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
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetStockObject, InvalidateRect, SetBkMode, SetTextColor, BLACK_BRUSH,
    HBRUSH, HDC, NULL_BRUSH, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    GetWindowRect, IsWindow, LoadCursorW, LoadIconW, RegisterClassW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, EN_CHANGE,
    EN_KILLFOCUS, GWLP_USERDATA, HWND_TOP, IDC_ARROW, IDI_APPLICATION, MINMAXINFO, SWP_NOACTIVATE,
    SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLOREDIT, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY,
    WM_PAINT, WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_CAPTION, WS_CLIPCHILDREN, WS_EX_APPWINDOW,
    WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW, WS_SYSMENU,
};

use crate::app_core::AppCore;
use crate::features::hotkeys::ui::recorder::Recorder;
use crate::features::launcher::settings::items::{
    hit_confirm, hit_launcher, hotkey_action_key, layout_confirm,
    layout_launcher_items, layout_search_section, paint_confirm, paint_confirm_copy,
    paint_launcher_items, paint_search_section, ConfirmCopy, FieldEdit, Formats, Hit,
    LauncherItemsSection, ALIAS_EDIT_ID, FILTER_EDIT_ID, ITEM_H,
};
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
    scroll: f32,
    filter_query: String,
    filter: Option<FieldEdit>,
    alias: Option<FieldEdit>,
    alias_index: Option<usize>,
    recorder: Recorder,
    confirming_reset: bool,
    confirming_clear: bool,
    confirming_snippets: bool,
    shown_tab: SettingsTab,
}

struct Renderer {
    factory: ID2D1Factory,
    header_format: IDWriteTextFormat,
    tab_format: IDWriteTextFormat,
    body_format: IDWriteTextFormat,
    caption_format: IDWriteTextFormat,
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
                scroll: 0.0,
                filter_query: String::new(),
                filter: None,
                alias: None,
                alias_index: None,
                recorder: Recorder::new(),
                confirming_reset: false,
                confirming_clear: false,
                confirming_snippets: false,
                shown_tab: SettingsTab::General,
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
            if let Ok(filter) = FieldEdit::create(hwnd, FILTER_EDIT_ID) {
                (*ptr).filter = Some(filter);
            }
            if let Ok(alias) = FieldEdit::create(hwnd, ALIAS_EDIT_ID) {
                (*ptr).alias = Some(alias);
            }
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

    pub fn on_tab_changed(&self) {
        unsafe {
            if let Some(inner) = inner_from(self.hwnd) {
                reset_pane_state(inner, true);
            }
        }
        self.invalidate();
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
        let body_format = unsafe {
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
        let caption_format = unsafe {
            dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                HEADER_FONT_DIP,
                w!("en-US"),
            )?
        };
        for format in [&header_format, &tab_format, &body_format] {
            unsafe {
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            }
        }
        unsafe {
            caption_format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            caption_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            caption_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
        }
        Ok(Self {
            factory,
            header_format,
            tab_format,
            body_format,
            caption_format,
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

    fn paint(&mut self, hwnd: HWND, inner: *mut SettingsInner) {
        if self.hwnd_target.is_none() {
            let (w, h) = client_size(hwnd);
            self.resize(hwnd, w, h);
        }
        let Some(target) = &self.hwnd_target else {
            return;
        };
        let selected = unsafe { selected_tab(inner) };
        let err = paint_scene(
            hwnd,
            target,
            &self.header_format,
            &self.tab_format,
            &self.body_format,
            &self.caption_format,
            selected,
            inner,
        );
        if let Err(err) = err {
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
    hwnd: HWND,
    target: &ID2D1RenderTarget,
    header_format: &IDWriteTextFormat,
    tab_format: &IDWriteTextFormat,
    body_format: &IDWriteTextFormat,
    caption_format: &IDWriteTextFormat,
    selected: SettingsTab,
    inner: *mut SettingsInner,
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
        let formats = Formats {
            header: header_format,
            body: body_format,
            caption: caption_format,
        };
        paint_detail(
            hwnd,
            target,
            &formats,
            selected,
            inner,
            size.width,
            size.height,
        )?;
        target.EndDraw(None, None)
    }
}

unsafe fn paint_detail(
    hwnd: HWND,
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    selected: SettingsTab,
    inner: *mut SettingsInner,
    width: f32,
    height: f32,
) -> windows::core::Result<()> {
    let sidebar_w = theme::size::SETTINGS_SIDEBAR;
    let detail_w = (width - sidebar_w).max(0.0);
    let core = core_from_host((*inner).host).map(|c| &*c);
    if selected != (*inner).shown_tab {
        reset_pane_state(inner, false);
        (*inner).shown_tab = selected;
    }
    if (*inner).confirming_reset || (*inner).confirming_clear || (*inner).confirming_snippets {
        hide_edits(inner);
        if let Some(core) = core {
            let layout = layout_confirm(width, height);
            if (*inner).confirming_snippets {
                crate::features::snippets::settings::pane::paint(
                    target,
                    formats,
                    core.settings.snippets_enabled,
                    core.settings.snippets_show_in_launcher,
                    detail_w,
                    (*inner).scroll,
                )?;
                paint_confirm_copy(
                    target,
                    formats,
                    &layout,
                    (width, height),
                    crate::features::snippets::settings::pane::enable_copy(),
                )?;
            } else if (*inner).confirming_clear {
                crate::features::clipboard::settings::pane::paint(
                    target,
                    formats,
                    core.settings.clipboard_retention_days,
                    &core.settings.clipboard_disabled_apps,
                    detail_w,
                    (*inner).scroll,
                )?;
                paint_confirm_copy(
                    target,
                    formats,
                    &layout,
                    (width, height),
                    ConfirmCopy {
                        title: crate::features::clipboard::settings::pane::CLEAR_CONFIRM_TITLE,
                        message: crate::features::clipboard::settings::pane::CLEAR_CONFIRM_MESSAGE,
                        accept: crate::features::clipboard::settings::pane::CLEAR_CONFIRM_ACTION,
                        cancel: crate::features::launcher::settings::items::RESET_CONFIRM_CANCEL,
                    },
                )?;
            } else {
                paint_search_section(
                    target,
                    formats,
                    &layout_search_section(detail_w),
                    core.ranking_is_empty(),
                    (*inner).scroll,
                )?;
                paint_confirm(target, formats, &layout, (width, height))?;
            }
        }
        return Ok(());
    }
    if selected == SettingsTab::Backup {
        hide_edits(inner);
        crate::features::backup::settings::pane::paint(
            target,
            formats,
            detail_w,
            (*inner).scroll,
        )?;
        return Ok(());
    }
    if selected == SettingsTab::Calendar {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::calendar::settings::pane::paint(
                target,
                formats,
                core.settings.calendar_enabled,
                core.settings.auto_join_meetings,
                core.settings.camera_preview,
                core.settings.join_window_minutes,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::Notes {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::notes::settings::pane::paint(
                target,
                formats,
                core.settings.notes_enabled,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::FileSearch {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::file_search::settings::pane::paint(
                target,
                formats,
                core.settings.file_search_enabled,
                &core.settings.file_search_scopes,
                &core.settings.file_search_ignore_patterns,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::Snippets {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::snippets::settings::pane::paint(
                target,
                formats,
                core.settings.snippets_enabled,
                core.settings.snippets_show_in_launcher,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::Clipboard {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::clipboard::settings::pane::paint(
                target,
                formats,
                core.settings.clipboard_retention_days,
                &core.settings.clipboard_disabled_apps,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::Emoji {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::emoji::settings::paint(
                target,
                formats,
                &core.settings.emoji_skin_tone,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::WindowManagement {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::window_management::settings::pane::paint(
                target,
                formats,
                core.settings.window_management_enabled,
                core.settings.window_management_show_in_launcher,
                core.settings.window_cycle_on_repeat,
                core.settings.window_gap,
                &core.visibility,
                &core.hotkeys,
                (*inner).recorder.action.as_deref(),
                sidebar_w,
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::Quicklinks {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::quicklinks::settings::pane::paint(
                target,
                formats,
                core.settings.quicklinks_enabled,
                core.settings.quicklinks_show_in_launcher,
                core.quicklink_records(),
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    let commands_shift = if selected == SettingsTab::Commands {
        if let Some(core) = core {
            crate::features::custom_commands::settings::pane::paint(
                target,
                formats,
                core.settings.custom_commands_enabled,
                core.settings.custom_commands_show_in_launcher,
                core.custom_command_records(),
                detail_w,
                (*inner).scroll,
            )?;
            crate::features::custom_commands::settings::pane::content_height(
                core.custom_command_records().len(),
            )
        } else {
            0.0
        }
    } else {
        0.0
    };
    if selected == SettingsTab::General {
        hide_edits(inner);
        if let Some(core) = core {
            crate::features::settings::panes::general::paint(
                target,
                formats,
                &crate::features::settings::panes::general::GeneralState {
                    ranking_empty: core.ranking_is_empty(),
                    hyper: &core.settings.hyper_key,
                    hyper_shift: core.settings.hyper_includes_shift,
                    appearance: core.appearance_label(),
                    compact: core.settings.compact_mode,
                    favorites_in_compact: core.settings.show_favorites_in_compact,
                    follow_cursor: core.settings.open_on_cursor_screen,
                    draggable: core.settings.palette_draggable,
                    launch_at_login: core.settings.launch_at_login,
                    show_in_menu_bar: core.settings.show_in_menu_bar,
                    pop_to_root: core.settings.pop_to_root_timeout,
                    auto_switch: core.settings.auto_switch_input_source,
                },
                detail_w,
                (*inner).scroll,
            )?;
        }
        return Ok(());
    }
    if selected == SettingsTab::Permissions {
        hide_edits(inner);
        crate::features::settings::panes::permissions::paint(
            target,
            formats,
            detail_w,
            (*inner).scroll,
        )?;
        return Ok(());
    }
    let Some(section) = LauncherItemsSection::for_tab(selected) else {
        hide_edits(inner);
        return Ok(());
    };
    let Some(core) = core else {
        hide_edits(inner);
        return Ok(());
    };
    let entries = core.settings_entries(section.kind);
    let filtered = crate::features::launcher::settings::items::filter_entries(
        &entries,
        section.kind,
        &(*inner).filter_query,
    );
    let refs: Vec<&tinycast_pure::app_entry::AppEntry> = filtered;
    let layout = layout_launcher_items(
        &section,
        refs.len(),
        |i| refs.get(i).and_then(|e| hotkey_action_key(e)).is_some(),
        detail_w,
        refs.is_empty(),
    );
    let layout = if commands_shift > 0.0 {
        layout.shifted(commands_shift)
    } else {
        layout
    };
    let scroll = (*inner).scroll;
    paint_launcher_items(
        target,
        formats,
        &section,
        &layout,
        &refs,
        &core.visibility,
        &core.aliases,
        &core.hotkeys,
        (*inner).recorder.action.as_deref(),
        (*inner).alias_index,
        &(*inner).filter_query,
        scroll,
    )?;
    layout_edits(
        hwnd,
        inner,
        &layout,
        sidebar_w,
        scroll,
        core.visibility.is_kind_enabled(section.kind),
    );
    Ok(())
}

unsafe fn layout_edits(
    hwnd: HWND,
    inner: *mut SettingsInner,
    layout: &crate::features::launcher::settings::items::LauncherLayout,
    sidebar_w: f32,
    scroll: f32,
    kind_on: bool,
) {
    let mut filter_rect = layout.filter;
    filter_rect.x += sidebar_w;
    filter_rect.y -= scroll;
    if let Some(filter) = (*inner).filter.as_mut() {
        filter.layout(hwnd, filter_rect, kind_on);
    }
    if let (Some(idx), Some(alias)) = ((*inner).alias_index, (*inner).alias.as_mut()) {
        if kind_on {
            if let Some(item) = layout.items.get(idx) {
                let mut rect = item.alias;
                rect.x += sidebar_w;
                rect.y -= scroll;
                alias.layout(hwnd, rect, true);
                return;
            }
        }
        alias.hide();
        (*inner).alias_index = None;
    } else if let Some(alias) = (*inner).alias.as_ref() {
        alias.hide();
    }
}

unsafe fn hide_edits(inner: *mut SettingsInner) {
    if let Some(filter) = (*inner).filter.as_ref() {
        filter.hide();
    }
    if let Some(alias) = (*inner).alias.as_ref() {
        alias.hide();
    }
}

unsafe fn reset_pane_state(inner: *mut SettingsInner, resume_hotkeys: bool) {
    commit_alias(inner);
    (*inner).scroll = 0.0;
    (*inner).filter_query.clear();
    if let Some(filter) = (*inner).filter.as_ref() {
        filter.set_text("");
        filter.hide();
    }
    if let Some(alias) = (*inner).alias.as_ref() {
        alias.hide();
    }
    (*inner).alias_index = None;
    (*inner).confirming_reset = false;
    (*inner).confirming_clear = false;
    (*inner).confirming_snippets = false;
    if (*inner).recorder.is_recording() && resume_hotkeys {
        (*inner).recorder.cancel();
        if let Some(core) = core_from_host((*inner).host) {
            (*core).resume_global_hotkeys();
        }
    } else if (*inner).recorder.is_recording() {
        (*inner).recorder.cancel();
    }
}

unsafe fn commit_alias(inner: *mut SettingsInner) {
    let Some(idx) = (*inner).alias_index.take() else {
        if let Some(alias) = (*inner).alias.as_ref() {
            alias.hide();
        }
        return;
    };
    let text = (*inner)
        .alias
        .as_ref()
        .map(FieldEdit::text)
        .unwrap_or_default();
    if let Some(alias) = (*inner).alias.as_ref() {
        alias.hide();
    }
    let Some(core) = core_from_host((*inner).host) else {
        return;
    };
    let tab = (*core).settings_tab;
    let Some(section) = LauncherItemsSection::for_tab(tab) else {
        return;
    };
    let entries = (*core).settings_entries(section.kind);
    let filtered = crate::features::launcher::settings::items::filter_entries(
        &entries,
        section.kind,
        &(*inner).filter_query,
    );
    if let Some(entry) = filtered.get(idx) {
        let id = entry.id.clone();
        (*core).set_alias_draft(&id, &text);
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

fn current_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            win: GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0,
        }
    }
}

fn client_dip_size(hwnd: HWND) -> (f32, f32) {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let (w, h) = client_size(hwnd);
    (px_to_dip(w as i32, dpi), px_to_dip(h as i32, dpi))
}

unsafe fn handle_command(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let notify = ((wparam.0 as u32) >> 16) & 0xffff;
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let child = HWND(lparam.0 as *mut core::ffi::c_void);
    if notify == EN_CHANGE {
        if let Some(filter) = (*inner).filter.as_ref() {
            if filter.hwnd == child {
                commit_alias(inner);
                (*inner).filter_query = filter.text();
                let _ = InvalidateRect(hwnd, None, FALSE);
            }
        }
    }
    if notify == EN_KILLFOCUS {
        if let Some(alias) = (*inner).alias.as_ref() {
            if alias.hwnd == child {
                commit_alias(inner);
                let _ = InvalidateRect(hwnd, None, FALSE);
            }
        }
    }
}

unsafe fn handle_wheel(hwnd: HWND, wparam: WPARAM) {
    let delta = ((wparam.0 as u32) >> 16) as i16;
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let (width, height) = client_dip_size(hwnd);
    (*inner).scroll -= (delta as f32 / 120.0) * ITEM_H;
    let max = (pane_content_height(inner, width) - height).max(0.0);
    (*inner).scroll = (*inner).scroll.clamp(0.0, max);
    let _ = InvalidateRect(hwnd, None, FALSE);
}

unsafe fn pane_content_height(inner: *mut SettingsInner, window_w: f32) -> f32 {
    let detail_w = (window_w - theme::size::SETTINGS_SIDEBAR).max(0.0);
    let tab = selected_tab(inner);
    if tab == SettingsTab::General {
        return crate::features::settings::panes::general::content_height();
    }
    if tab == SettingsTab::Permissions {
        return crate::features::settings::panes::permissions::content_height();
    }
    if tab == SettingsTab::Backup {
        return crate::features::backup::settings::pane::content_height();
    }
    if tab == SettingsTab::Calendar {
        return crate::features::calendar::settings::pane::content_height();
    }
    if tab == SettingsTab::Notes {
        return crate::features::notes::settings::pane::content_height();
    }
    if tab == SettingsTab::FileSearch {
        let (scopes, ignores) = core_from_host((*inner).host)
            .map(|c| {
                (
                    (*c).settings.file_search_scopes.len(),
                    (*c).settings.file_search_ignore_patterns.len(),
                )
            })
            .unwrap_or((0, 0));
        return crate::features::file_search::settings::pane::content_height(scopes, ignores);
    }
    if tab == SettingsTab::Snippets {
        return crate::features::snippets::settings::pane::content_height();
    }
    if tab == SettingsTab::WindowManagement {
        return crate::features::window_management::settings::pane::content_height();
    }
    if tab == SettingsTab::Emoji {
        return crate::features::emoji::settings::content_height();
    }
    if tab == SettingsTab::Quicklinks {
        let n = core_from_host((*inner).host)
            .map(|c| (*c).quicklink_records().len())
            .unwrap_or(0);
        return crate::features::quicklinks::settings::pane::content_height(n);
    }
    let commands_shift = if tab == SettingsTab::Commands {
        let n = core_from_host((*inner).host)
            .map(|c| (*c).custom_command_records().len())
            .unwrap_or(0);
        crate::features::custom_commands::settings::pane::content_height(n)
    } else {
        0.0
    };
    let Some(section) = LauncherItemsSection::for_tab(tab) else {
        return 0.0;
    };
    let Some(core) = core_from_host((*inner).host) else {
        return commands_shift;
    };
    let entries = (*core).settings_entries(section.kind);
    let filtered = crate::features::launcher::settings::items::filter_entries(
        &entries,
        section.kind,
        &(*inner).filter_query,
    );
    layout_launcher_items(
        &section,
        filtered.len(),
        |i| filtered.get(i).and_then(|e| hotkey_action_key(e)).is_some(),
        detail_w,
        filtered.is_empty(),
    )
    .shifted(commands_shift)
    .content_height
}

unsafe fn handle_keydown(hwnd: HWND, wparam: WPARAM) -> bool {
    let Some(inner) = inner_from(hwnd) else {
        return false;
    };
    if (*inner).confirming_reset || (*inner).confirming_clear || (*inner).confirming_snippets {
        let vk = wparam.0 as u16;
        if vk == 0x1B {
            (*inner).confirming_reset = false;
            (*inner).confirming_clear = false;
            (*inner).confirming_snippets = false;
            let _ = InvalidateRect(hwnd, None, FALSE);
            return true;
        }
        if vk == 0x0D {
            if let Some(core) = core_from_host((*inner).host) {
                if (*inner).confirming_snippets {
                    (*core).set_snippets_enabled(true);
                } else if (*inner).confirming_clear {
                    (*core).clear_clipboard_history();
                } else {
                    (*core).reset_learned_ranking();
                }
            }
            (*inner).confirming_reset = false;
            (*inner).confirming_clear = false;
            (*inner).confirming_snippets = false;
            let _ = InvalidateRect(hwnd, None, FALSE);
            return true;
        }
        return true;
    }
    let Some(action) = (*inner).recorder.action.clone() else {
        return false;
    };
    let vk = wparam.0 as u16;
    let outcome = (*inner)
        .recorder
        .on_keydown(vk, current_modifiers(), now_ms());
    apply_capture(hwnd, inner, &action, outcome)
}

unsafe fn handle_keyup(hwnd: HWND, wparam: WPARAM) -> bool {
    let Some(inner) = inner_from(hwnd) else {
        return false;
    };
    let Some(action) = (*inner).recorder.action.clone() else {
        return false;
    };
    let vk = wparam.0 as u16;
    let outcome = (*inner).recorder.on_keyup(vk, now_ms());
    apply_capture(hwnd, inner, &action, outcome)
}

unsafe fn apply_capture(
    hwnd: HWND,
    inner: *mut SettingsInner,
    action: &str,
    outcome: CaptureOutcome,
) -> bool {
    match outcome {
        CaptureOutcome::Ignore => true,
        CaptureOutcome::Cancel => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).resume_global_hotkeys();
            }
            let _ = InvalidateRect(hwnd, None, FALSE);
            true
        }
        CaptureOutcome::Clear => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).set_hotkey(action, None);
                (*core).resume_global_hotkeys();
            }
            let _ = InvalidateRect(hwnd, None, FALSE);
            true
        }
        CaptureOutcome::Commit(binding) => {
            (*inner).recorder.cancel();
            if let Some(core) = core_from_host((*inner).host) {
                (*core).set_hotkey(action, Some(binding));
                (*core).resume_global_hotkeys();
            }
            let _ = InvalidateRect(hwnd, None, FALSE);
            true
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

unsafe fn handle_lbutton(hwnd: HWND, lparam: LPARAM) {
    let (px, py) = point_from_lparam(lparam);
    let dpi = GetDpiForWindow(hwnd);
    let x = px_to_dip(px, dpi);
    let y = px_to_dip(py, dpi);
    if let Some(tab) = tab_at(x, y) {
        if let Some(inner) = inner_from(hwnd) {
            if (*inner).recorder.is_recording() {
                (*inner).recorder.cancel();
                if let Some(core) = core_from_host((*inner).host) {
                    (*core).resume_global_hotkeys();
                }
            }
            if let Some(core) = core_from_host((*inner).host) {
                (*core).select_settings_tab(tab);
            }
        }
        return;
    }
    let Some(inner) = inner_from(hwnd) else {
        return;
    };
    let (width, height) = client_dip_size(hwnd);
    if (*inner).confirming_reset || (*inner).confirming_clear || (*inner).confirming_snippets {
        let layout = layout_confirm(width, height);
        match hit_confirm(&layout, x, y) {
            Some(Hit::ConfirmReset) => {
                if let Some(core) = core_from_host((*inner).host) {
                    if (*inner).confirming_snippets {
                        (*core).set_snippets_enabled(true);
                    } else if (*inner).confirming_clear {
                        (*core).clear_clipboard_history();
                    } else {
                        (*core).reset_learned_ranking();
                    }
                }
                (*inner).confirming_reset = false;
                (*inner).confirming_clear = false;
                (*inner).confirming_snippets = false;
            }
            Some(Hit::ConfirmCancel) => {
                (*inner).confirming_reset = false;
                (*inner).confirming_clear = false;
                (*inner).confirming_snippets = false;
            }
            _ => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    let sidebar_w = theme::size::SETTINGS_SIDEBAR;
    if x < sidebar_w {
        return;
    }
    let detail_x = x - sidebar_w;
    let detail_y = y + (*inner).scroll;
    let detail_w = (width - sidebar_w).max(0.0);
    let tab = selected_tab(inner);
    if tab == SettingsTab::WindowManagement {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        match crate::features::window_management::settings::pane::hit(
            detail_x,
            y,
            (*inner).scroll,
            detail_w,
        ) {
            Some(crate::features::window_management::settings::pane::WindowHit::Enable) => {
                (*core).set_window_management_enabled(!(*core).settings.window_management_enabled);
            }
            Some(
                crate::features::window_management::settings::pane::WindowHit::ShowInLauncher,
            ) => {
                (*core).set_window_management_show_in_launcher(
                    !(*core).settings.window_management_show_in_launcher,
                );
            }
            Some(crate::features::window_management::settings::pane::WindowHit::Cycle) => {
                (*core).set_window_cycle_on_repeat(!(*core).settings.window_cycle_on_repeat);
            }
            Some(crate::features::window_management::settings::pane::WindowHit::Gap) => {
                (*core).cycle_window_gap();
            }
            Some(crate::features::window_management::settings::pane::WindowHit::Visible(i)) => {
                if let Some(id) = tinycast_pure::window_command::WindowCommandId::all().get(i) {
                    let entry = id.entry_id();
                    let vis = !(*core).visibility.is_item_visible(&entry);
                    (*core).set_item_visible(&entry, vis);
                }
            }
            Some(crate::features::window_management::settings::pane::WindowHit::Recorder(i)) => {
                if let Some(id) = tinycast_pure::window_command::WindowCommandId::all().get(i) {
                    let key = format!("hotkey.windowCommand.{}", id.raw());
                    if (*inner).recorder.action.as_deref() == Some(key.as_str()) {
                        (*inner).recorder.cancel();
                        (*core).resume_global_hotkeys();
                    } else {
                        (*inner).recorder.begin(key);
                        (*core).pause_global_hotkeys();
                    }
                }
            }
            Some(crate::features::window_management::settings::pane::WindowHit::RecorderClear(i)) => {
                if let Some(id) = tinycast_pure::window_command::WindowCommandId::all().get(i) {
                    let key = format!("hotkey.windowCommand.{}", id.raw());
                    (*inner).recorder.cancel();
                    (*core).set_hotkey(&key, None);
                    (*core).resume_global_hotkeys();
                }
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Backup {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        match crate::features::backup::settings::pane::hit(detail_x, y, (*inner).scroll) {
            Some(crate::features::backup::settings::pane::BackupHit::Export) => {
                (*core).export_settings(hwnd);
            }
            Some(crate::features::backup::settings::pane::BackupHit::Import) => {
                (*core).import_settings(hwnd);
            }
            Some(crate::features::backup::settings::pane::BackupHit::Raycast) => {
                (*core).import_raycast(hwnd);
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Calendar {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        match crate::features::calendar::settings::pane::hit(detail_x, y, (*inner).scroll) {
            Some(crate::features::calendar::settings::pane::CalendarHit::Enable) => {
                (*core).set_calendar_enabled(!(*core).settings.calendar_enabled);
            }
            Some(crate::features::calendar::settings::pane::CalendarHit::AutoJoin) => {
                (*core).set_auto_join_meetings(!(*core).settings.auto_join_meetings);
            }
            Some(crate::features::calendar::settings::pane::CalendarHit::Camera) => {
                (*core).set_camera_preview(!(*core).settings.camera_preview);
            }
            Some(crate::features::calendar::settings::pane::CalendarHit::JoinWindow) => {
                (*core).cycle_join_window();
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Notes {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        if crate::features::notes::settings::pane::hit(detail_x, y, (*inner).scroll)
            == Some(crate::features::notes::settings::pane::NotesHit::Enable)
        {
            (*core).set_notes_enabled(!(*core).settings.notes_enabled);
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::FileSearch {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        let scopes = (*core).settings.file_search_scopes.len();
        let ignores = (*core).settings.file_search_ignore_patterns.len();
        match crate::features::file_search::settings::pane::hit(
            detail_x,
            y,
            (*inner).scroll,
            scopes,
            ignores,
        ) {
            Some(crate::features::file_search::settings::pane::FileSearchHit::Enable) => {
                (*core).set_file_search_enabled(!(*core).settings.file_search_enabled);
            }
            Some(crate::features::file_search::settings::pane::FileSearchHit::AddFolder) => {
                if let Some(path) =
                    crate::features::file_search::settings::pane::pick_folder(hwnd)
                {
                    (*core).add_file_search_scope(path);
                }
            }
            Some(crate::features::file_search::settings::pane::FileSearchHit::RemoveScope(i)) => {
                (*core).remove_file_search_scope(i);
            }
            Some(crate::features::file_search::settings::pane::FileSearchHit::AddIgnore) => {
                if let Some(pattern) =
                    crate::features::file_search::settings::pane::ask_pattern(hwnd)
                {
                    (*core).add_file_search_ignore(pattern);
                }
            }
            Some(crate::features::file_search::settings::pane::FileSearchHit::RemoveIgnore(i)) => {
                (*core).remove_file_search_ignore(i);
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Snippets {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        match crate::features::snippets::settings::pane::hit(detail_x, detail_y, (*inner).scroll) {
            Some(crate::features::snippets::settings::pane::SnippetsHit::Enable) => {
                if (*core).settings.snippets_enabled {
                    (*core).set_snippets_enabled(false);
                } else {
                    (*inner).confirming_snippets = true;
                }
            }
            Some(crate::features::snippets::settings::pane::SnippetsHit::ShowInLauncher) => {
                (*core).set_snippets_show_in_launcher(!(*core).settings.snippets_show_in_launcher);
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Clipboard {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        let disabled_len = (*core).settings.clipboard_disabled_apps.len();
        match crate::features::clipboard::settings::pane::hit(
            detail_x,
            detail_y,
            (*inner).scroll,
            disabled_len,
        ) {
            Some(crate::features::clipboard::settings::pane::ClipboardHit::Retention) => {
                (*core).cycle_clipboard_retention();
            }
            Some(crate::features::clipboard::settings::pane::ClipboardHit::Clear) => {
                (*inner).confirming_clear = true;
            }
            Some(crate::features::clipboard::settings::pane::ClipboardHit::AddApp) => {
                if let Some(stem) =
                    crate::features::clipboard::settings::pane::pick_application_stem(hwnd)
                {
                    (*core).add_clipboard_disabled_app(stem);
                }
            }
            Some(crate::features::clipboard::settings::pane::ClipboardHit::RemoveApp(i)) => {
                (*core).remove_clipboard_disabled_app(i);
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::General {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        use crate::features::settings::panes::general::{GeneralHit, GeneralToggle};
        match crate::features::settings::panes::general::hit(detail_x, detail_y, (*inner).scroll) {
            Some(GeneralHit::PaletteRecorder) => {
                (*core).pause_global_hotkeys();
                (*inner).recorder.begin("hotkey.togglePalette".into());
            }
            Some(GeneralHit::ResetRanking) => {
                if !(*core).ranking_is_empty() {
                    (*inner).confirming_reset = true;
                }
            }
            Some(GeneralHit::HyperKey) => (*core).cycle_hyper_key(),
            Some(GeneralHit::HyperShift) => {
                if crate::features::settings::panes::general::hyper_includes_shift_enabled(
                    &(*core).settings.hyper_key,
                ) {
                    (*core).set_hyper_includes_shift(!(*core).settings.hyper_includes_shift);
                }
            }
            Some(GeneralHit::Appearance) => (*core).cycle_appearance(),
            Some(GeneralHit::Compact) => (*core).toggle_setting_bool(GeneralToggle::Compact),
            Some(GeneralHit::FavoritesInCompact) => {
                (*core).toggle_setting_bool(GeneralToggle::FavoritesInCompact)
            }
            Some(GeneralHit::FollowCursor) => {
                (*core).toggle_setting_bool(GeneralToggle::FollowCursor)
            }
            Some(GeneralHit::Draggable) => (*core).toggle_setting_bool(GeneralToggle::Draggable),
            Some(GeneralHit::LaunchAtLogin) => {
                (*core).toggle_setting_bool(GeneralToggle::LaunchAtLogin)
            }
            Some(GeneralHit::ShowInMenuBar) => {
                (*core).toggle_setting_bool(GeneralToggle::ShowInMenuBar)
            }
            Some(GeneralHit::PopToRoot) => (*core).cycle_pop_to_root(),
            Some(GeneralHit::AutoSwitchInput) => {
                (*core).toggle_setting_bool(GeneralToggle::AutoSwitch)
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Permissions {
        match crate::features::settings::panes::permissions::hit(
            detail_x,
            detail_y,
            (*inner).scroll,
            detail_w,
        ) {
            Some(crate::features::settings::panes::permissions::PermissionHit::Open(i)) => {
                if let Some(row) = crate::features::settings::panes::permissions::ROWS.get(i) {
                    let _ = crate::features::launcher::ui::coordinator::execute(
                        &crate::features::launcher::ui::coordinator::LaunchSpec::Uri(
                            row.uri.to_string(),
                        ),
                    );
                }
            }
            None => {}
        }
        return;
    }
    if tab == SettingsTab::Emoji {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        if crate::features::emoji::settings::hit(detail_x, y, (*inner).scroll)
            == Some(crate::features::emoji::settings::EmojiHit::SkinTone)
        {
            (*core).cycle_emoji_skin_tone();
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    if tab == SettingsTab::Quicklinks {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        let count = (*core).quicklink_records().len();
        match crate::features::quicklinks::settings::pane::hit(
            detail_x,
            y,
            (*inner).scroll,
            count,
        ) {
            Some(crate::features::quicklinks::settings::pane::QuicklinksHit::Enable) => {
                (*core).set_quicklinks_enabled(!(*core).settings.quicklinks_enabled);
            }
            Some(crate::features::quicklinks::settings::pane::QuicklinksHit::ShowInLauncher) => {
                (*core).set_quicklinks_show_in_launcher(
                    !(*core).settings.quicklinks_show_in_launcher,
                );
            }
            Some(crate::features::quicklinks::settings::pane::QuicklinksHit::Create) => {
                (*core).edit_quicklink(hwnd, None);
            }
            Some(crate::features::quicklinks::settings::pane::QuicklinksHit::Import) => {
                (*core).import_quicklinks(hwnd);
            }
            Some(crate::features::quicklinks::settings::pane::QuicklinksHit::Export) => {
                (*core).export_quicklinks(hwnd);
            }
            Some(crate::features::quicklinks::settings::pane::QuicklinksHit::Item(i)) => {
                (*core).edit_quicklink(hwnd, Some(i));
            }
            None => {}
        }
        let _ = InvalidateRect(hwnd, None, FALSE);
        return;
    }
    let mut launcher_shift = 0.0;
    if tab == SettingsTab::Commands {
        let Some(core) = core_from_host((*inner).host) else {
            return;
        };
        let count = (*core).custom_command_records().len();
        match crate::features::custom_commands::settings::pane::hit(
            detail_x,
            y,
            (*inner).scroll,
            count,
        ) {
            Some(crate::features::custom_commands::settings::pane::CustomCommandsHit::Enable) => {
                (*core).set_custom_commands_enabled(!(*core).settings.custom_commands_enabled);
                let _ = InvalidateRect(hwnd, None, FALSE);
                return;
            }
            Some(
                crate::features::custom_commands::settings::pane::CustomCommandsHit::ShowInLauncher,
            ) => {
                (*core).set_custom_commands_show_in_launcher(
                    !(*core).settings.custom_commands_show_in_launcher,
                );
                let _ = InvalidateRect(hwnd, None, FALSE);
                return;
            }
            Some(crate::features::custom_commands::settings::pane::CustomCommandsHit::New) => {
                (*core).new_custom_command(hwnd);
                let _ = InvalidateRect(hwnd, None, FALSE);
                return;
            }
            Some(crate::features::custom_commands::settings::pane::CustomCommandsHit::Item(i)) => {
                (*core).edit_custom_command_at(hwnd, i);
                let _ = InvalidateRect(hwnd, None, FALSE);
                return;
            }
            None => {
                launcher_shift = crate::features::custom_commands::settings::pane::content_height(
                    count,
                );
            }
        }
    }
    let Some(section) = LauncherItemsSection::for_tab(tab) else {
        return;
    };
    let Some(core) = core_from_host((*inner).host) else {
        return;
    };
    let entries = (*core).settings_entries(section.kind);
    let filtered = crate::features::launcher::settings::items::filter_entries(
        &entries,
        section.kind,
        &(*inner).filter_query,
    );
    let layout = layout_launcher_items(
        &section,
        filtered.len(),
        |i| filtered.get(i).and_then(|e| hotkey_action_key(e)).is_some(),
        detail_w,
        filtered.is_empty(),
    );
    let layout = if launcher_shift > 0.0 {
        layout.shifted(launcher_shift)
    } else {
        layout
    };
    let kind_on = (*core).visibility.is_kind_enabled(section.kind);
    match hit_launcher(&layout, detail_x, detail_y, kind_on, |i| {
        filtered
            .get(i)
            .and_then(|e| hotkey_action_key(e))
            .is_some_and(|k| (*core).hotkeys.get(&k).is_some())
    }) {
        Some(Hit::KindToggle) => {
            if (*inner).recorder.is_recording() {
                (*inner).recorder.cancel();
                (*core).resume_global_hotkeys();
            }
            let on = !kind_on;
            (*core).set_kind_enabled(section.kind, on);
        }
        Some(Hit::ItemVisible(i)) => {
            if let Some(entry) = filtered.get(i) {
                let id = entry.id.clone();
                let visible = !(*core).visibility.is_item_visible(&id);
                (*core).set_item_visible(&id, visible);
            }
        }
        Some(Hit::Alias(i)) => {
            commit_alias(inner);
            (*inner).alias_index = Some(i);
            if let Some(alias) = (*inner).alias.as_ref() {
                let text = filtered
                    .get(i)
                    .and_then(|e| (*core).aliases.get(&e.id))
                    .unwrap_or("");
                alias.set_text(text);
                alias.focus();
            }
            let _ = InvalidateRect(hwnd, None, FALSE);
        }
        Some(Hit::Recorder(i)) => {
            commit_alias(inner);
            if let Some(key) = filtered.get(i).and_then(|e| hotkey_action_key(e)) {
                if (*inner).recorder.action.as_deref() == Some(key.as_str()) {
                    (*inner).recorder.cancel();
                    (*core).resume_global_hotkeys();
                } else {
                    (*inner).recorder.begin(key);
                    (*core).pause_global_hotkeys();
                    let _ = SetFocus(hwnd);
                }
            }
            let _ = InvalidateRect(hwnd, None, FALSE);
        }
        Some(Hit::RecorderClear(i)) => {
            if let Some(key) = filtered.get(i).and_then(|e| hotkey_action_key(e)) {
                (*inner).recorder.cancel();
                (*core).set_hotkey(&key, None);
                (*core).resume_global_hotkeys();
            }
        }
        _ => {
            commit_alias(inner);
            if (*inner).recorder.is_recording() {
                (*inner).recorder.cancel();
                (*core).resume_global_hotkeys();
            }
            let _ = InvalidateRect(hwnd, None, FALSE);
        }
    }
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
                (*inner).renderer.paint(hwnd, inner);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            unsafe {
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00FFFFFF));
            }
            LRESULT(unsafe { GetStockObject(NULL_BRUSH) }.0 as isize)
        }
        WM_COMMAND => {
            handle_command(hwnd, wparam, lparam);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            handle_wheel(hwnd, wparam);
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if handle_keydown(hwnd, wparam) {
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_KEYUP | WM_SYSKEYUP => {
            if handle_keyup(hwnd, wparam) {
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_LBUTTONDOWN => {
            handle_lbutton(hwnd, lparam);
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
            if let Some(inner) = inner_from(hwnd) {
                commit_alias(inner);
                (*inner).confirming_reset = false;
                (*inner).confirming_clear = false;
                (*inner).confirming_snippets = false;
                if (*inner).recorder.is_recording() {
                    (*inner).recorder.cancel();
                    if let Some(core) = core_from_host((*inner).host) {
                        (*core).resume_global_hotkeys();
                    }
                }
                hide_edits(inner);
            }
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
        assert!(tabs.iter().any(|t| *t == SettingsTab::Commands));
        assert!(tabs.iter().any(|t| *t == SettingsTab::Quicklinks));
        assert!(tabs.iter().any(|t| *t == SettingsTab::Emoji));
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
