use tinycast_pure::alias::AliasStore;
use tinycast_pure::app_entry::{AppEntry, AppKind};
use tinycast_pure::command_id::CommandID;
use tinycast_pure::hotkey_store::HotKeyStore;
use tinycast_pure::search_relevance::{score, SearchFields};
use tinycast_pure::settings_tab::SettingsTab;
use tinycast_pure::theme;
use tinycast_pure::visibility::VisibilityStore;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, ID2D1SolidColorBrush, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, DeleteObject, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET,
    DEFAULT_PITCH, FW_NORMAL, HFONT, OUT_DEFAULT_PRECIS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{SetWindowTheme, EM_SETMARGINS};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowTextLengthW, GetWindowTextW, SendMessageW, SetWindowPos,
    SetWindowTextW, ShowWindow, EC_LEFTMARGIN, EC_RIGHTMARGIN, ES_AUTOHSCROLL, ES_LEFT, HWND_TOP,
    SWP_NOACTIVATE, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETFONT, WS_CHILD,
};

use crate::platform::screens::dip_scalar_to_px;

pub const FILTER_EDIT_ID: usize = 201;
pub const ALIAS_EDIT_ID: usize = 202;
pub const ENABLE_SUBTITLE: &str =
    "Off hides them all and stops their shortcuts. Uncheck one below to hide just that one.";

pub const SEARCH_HEADER: &str = "Search";
pub const LEARNED_RANKING_TITLE: &str = "Learned ranking";
pub const RESET_RANKING_BUTTON: &str = "Reset…";
pub const RESET_RANKING_FOOTER: &str = "Tinycast privately learns which results you choose for each query. Reset all learned choices to restore the default order.";
pub const RESET_CONFIRM_TITLE: &str = "Reset learned launcher ranking?";
pub const RESET_CONFIRM_MESSAGE: &str =
    "Tinycast will relearn your preferred results as you use the launcher.";
pub const RESET_CONFIRM_ACTION: &str = "Reset Ranking";
pub const RESET_CONFIRM_CANCEL: &str = "Cancel";

#[derive(Clone, Copy, Debug)]
pub struct ConfirmCopy {
    pub title: &'static str,
    pub message: &'static str,
    pub accept: &'static str,
    pub cancel: &'static str,
}

impl ConfirmCopy {
    pub fn reset_ranking() -> Self {
        Self {
            title: RESET_CONFIRM_TITLE,
            message: RESET_CONFIRM_MESSAGE,
            accept: RESET_CONFIRM_ACTION,
            cancel: RESET_CONFIRM_CANCEL,
        }
    }
}

const SECTION_HEADER_H: f32 = 22.0;
const ENABLE_ROW_H: f32 = 52.0;
const FILTER_H: f32 = 28.0;
pub const ITEM_H: f32 = 36.0;
const CARD_PAD: f32 = theme::spacing::XL;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;
const WELL_H: f32 = 24.0;
const FIELD_FONT_DIP: f32 = 13.0;

/// One category's Settings sections; never filters by visibility, so hidden rows stay listed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LauncherItemsSection {
    pub kind: AppKind,
    pub header: &'static str,
    pub search_prompt: &'static str,
}

impl LauncherItemsSection {
    pub fn applications() -> Self {
        Self {
            kind: AppKind::Application,
            header: "Applications",
            search_prompt: "Search applications…",
        }
    }

    pub fn system_settings() -> Self {
        Self {
            kind: AppKind::SystemSettings,
            header: "System Settings",
            search_prompt: "Search System Settings…",
        }
    }

    pub fn commands() -> Self {
        Self {
            kind: AppKind::Command,
            header: "Commands",
            search_prompt: "Search commands…",
        }
    }

    pub fn system_actions() -> Self {
        Self {
            kind: AppKind::SystemAction,
            header: "System Actions",
            search_prompt: "Search system actions…",
        }
    }

    pub fn for_tab(tab: SettingsTab) -> Option<Self> {
        match tab {
            SettingsTab::Applications => Some(Self::applications()),
            SettingsTab::SystemSettings => Some(Self::system_settings()),
            SettingsTab::Commands => Some(Self::commands()),
            SettingsTab::SystemActions => Some(Self::system_actions()),
            _ => None,
        }
    }

    pub fn enable_title(self) -> String {
        format!("Enable {}", self.header)
    }
}

pub fn commands_catalog() -> Vec<AppEntry> {
    let mut entries: Vec<AppEntry> = CommandID::settings_pane_ids()
        .iter()
        .copied()
        .map(CommandID::as_entry)
        .collect();
    entries.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    entries
}

/// Membership only: keep the index's name order.
pub fn filter_entries<'a>(
    entries: &'a [AppEntry],
    kind: AppKind,
    query: &str,
) -> Vec<&'a AppEntry> {
    let scoped: Vec<&'a AppEntry> = entries.iter().filter(|e| e.kind == kind).collect();
    if query.is_empty() {
        return scoped;
    }
    scoped
        .into_iter()
        .filter(|entry| score(query, &entry.fields).is_some())
        .collect()
}

pub fn empty_list_label(query: &str) -> String {
    if query.is_empty() {
        "Nothing here yet.".into()
    } else {
        format!("No matches for “{query}”.")
    }
}

pub fn hotkey_action_key(entry: &AppEntry) -> Option<String> {
    match entry.kind {
        AppKind::Application => {
            let rest = entry
                .fields
                .bundle_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(entry.id.as_str());
            Some(format!("hotkey.app.{rest}"))
        }
        AppKind::SystemSettings => {
            let rest = entry
                .fields
                .bundle_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(entry.id.as_str());
            Some(format!("hotkey.pane.{rest}"))
        }
        AppKind::Command => CommandID::from_raw(&entry.id)
            .and_then(CommandID::hotkey_defaults_key)
            .map(str::to_string),
        AppKind::SystemAction => tinycast_pure::system_action::SystemActionId::from_entry_id(
            &entry.id,
        )
        .map(|id| format!("hotkey.systemAction.{}", id.raw())),
        _ => None,
    }
}

pub fn commit_alias_text(draft: &str) -> Option<String> {
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.w,
            bottom: self.y + self.h,
        }
    }

    pub fn inset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            w: (self.w - dx * 2.0).max(0.0),
            h: (self.h - dy * 2.0).max(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    KindToggle,
    ItemVisible(usize),
    Alias(usize),
    Recorder(usize),
    RecorderClear(usize),
    ResetRanking,
    ConfirmCancel,
    ConfirmReset,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemRowLayout {
    pub row: Rect,
    pub alias: Rect,
    pub recorder: Option<Rect>,
    pub recorder_clear: Option<Rect>,
    pub checkbox: Rect,
}

#[derive(Clone, Debug)]
pub struct LauncherLayout {
    pub header: Rect,
    pub enable_row: Rect,
    pub toggle: Rect,
    pub list_card: Rect,
    pub filter: Rect,
    pub items: Vec<ItemRowLayout>,
    pub empty: Option<Rect>,
    pub content_height: f32,
}

impl LauncherLayout {
    pub fn shifted(mut self, dy: f32) -> Self {
        fn shift(r: Rect, dy: f32) -> Rect {
            Rect { y: r.y + dy, ..r }
        }
        self.header = shift(self.header, dy);
        self.enable_row = shift(self.enable_row, dy);
        self.toggle = shift(self.toggle, dy);
        self.list_card = shift(self.list_card, dy);
        self.filter = shift(self.filter, dy);
        self.empty = self.empty.map(|r| shift(r, dy));
        for item in &mut self.items {
            item.row = shift(item.row, dy);
            item.alias = shift(item.alias, dy);
            item.checkbox = shift(item.checkbox, dy);
            item.recorder = item.recorder.map(|r| shift(r, dy));
            item.recorder_clear = item.recorder_clear.map(|r| shift(r, dy));
        }
        self.content_height += dy;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SearchLayout {
    pub header: Rect,
    pub row: Rect,
    pub reset: Rect,
    pub footer: Rect,
    pub content_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ConfirmLayout {
    pub card: Rect,
    pub cancel: Rect,
    pub accept: Rect,
}

pub fn layout_launcher_items(
    _section: &LauncherItemsSection,
    item_count: usize,
    has_recorder: impl Fn(usize) -> bool,
    detail_w: f32,
    empty: bool,
) -> LauncherLayout {
    let pad = theme::spacing::XXL;
    let x = pad;
    let w = (detail_w - pad * 2.0).max(0.0);
    let mut y = pad;
    let header = Rect {
        x,
        y,
        w,
        h: SECTION_HEADER_H,
    };
    y += SECTION_HEADER_H + theme::spacing::SM;
    let enable_row = Rect {
        x,
        y,
        w,
        h: ENABLE_ROW_H + CARD_PAD,
    };
    let toggle = Rect {
        x: enable_row.x + enable_row.w - CARD_PAD - TOGGLE_W,
        y: enable_row.y + (enable_row.h - TOGGLE_H) / 2.0,
        w: TOGGLE_W,
        h: TOGGLE_H,
    };
    y += enable_row.h + theme::spacing::SECTION_SPACING;
    let filter = Rect {
        x: x + CARD_PAD,
        y: y + CARD_PAD,
        w: (w - CARD_PAD * 2.0).max(0.0),
        h: FILTER_H,
    };
    let mut iy = filter.y + filter.h + theme::spacing::SM;
    let mut items = Vec::with_capacity(item_count);
    let mut empty_rect = None;
    if empty {
        empty_rect = Some(Rect {
            x: filter.x,
            y: iy,
            w: filter.w,
            h: ITEM_H,
        });
        iy += ITEM_H;
    } else {
        for i in 0..item_count {
            let row = Rect {
                x: filter.x,
                y: iy,
                w: filter.w,
                h: ITEM_H,
            };
            let checkbox = Rect {
                x: row.x + row.w - theme::size::CHECKBOX,
                y: row.y + (ITEM_H - theme::size::CHECKBOX) / 2.0,
                w: theme::size::CHECKBOX,
                h: theme::size::CHECKBOX,
            };
            let mut cursor = checkbox.x - theme::spacing::SM;
            let recorder = if has_recorder(i) {
                let rec = Rect {
                    x: cursor - theme::size::SHORTCUT_RECORDER,
                    y: row.y + (ITEM_H - WELL_H) / 2.0,
                    w: theme::size::SHORTCUT_RECORDER,
                    h: WELL_H,
                };
                cursor = rec.x - theme::spacing::SM;
                Some(rec)
            } else {
                None
            };
            let recorder_clear = recorder.map(|rec| Rect {
                x: rec.x + rec.w - 18.0,
                y: rec.y + (rec.h - 14.0) / 2.0,
                w: 14.0,
                h: 14.0,
            });
            let alias = Rect {
                x: cursor - theme::size::SHORTCUT_RECORDER,
                y: row.y + (ITEM_H - WELL_H) / 2.0,
                w: theme::size::SHORTCUT_RECORDER,
                h: WELL_H,
            };
            items.push(ItemRowLayout {
                row,
                alias,
                recorder,
                recorder_clear,
                checkbox,
            });
            iy += ITEM_H;
        }
    }
    let list_bottom = iy + CARD_PAD;
    let list_card = Rect {
        x,
        y,
        w,
        h: list_bottom - y,
    };
    LauncherLayout {
        header,
        enable_row,
        toggle,
        list_card,
        filter,
        items,
        empty: empty_rect,
        content_height: list_bottom + pad,
    }
}

pub fn layout_search_section(detail_w: f32) -> SearchLayout {
    let pad = theme::spacing::XXL;
    let x = pad;
    let w = (detail_w - pad * 2.0).max(0.0);
    let mut y = pad;
    let header = Rect {
        x,
        y,
        w,
        h: SECTION_HEADER_H,
    };
    y += SECTION_HEADER_H + theme::spacing::SM;
    let row = Rect {
        x,
        y,
        w,
        h: 36.0 + CARD_PAD,
    };
    let reset = Rect {
        x: row.x + row.w - CARD_PAD - 72.0,
        y: row.y + (row.h - 24.0) / 2.0,
        w: 72.0,
        h: 24.0,
    };
    y += row.h + theme::spacing::SM;
    let footer = Rect { x, y, w, h: 48.0 };
    SearchLayout {
        header,
        row,
        reset,
        footer,
        content_height: y + footer.h + pad,
    }
}

pub fn layout_confirm(window_w: f32, window_h: f32) -> ConfirmLayout {
    let w = theme::size::DIALOG_WIDTH.min(window_w - theme::spacing::XXL * 2.0);
    let h = 160.0;
    let x = ((window_w - w) / 2.0).max(0.0);
    let y = ((window_h - h) / 2.0).max(0.0);
    let card = Rect { x, y, w, h };
    let btn_h = 28.0;
    let btn_y = card.y + card.h - CARD_PAD - btn_h;
    let accept_w = 120.0;
    let cancel_w = 80.0;
    let accept = Rect {
        x: card.x + card.w - CARD_PAD - accept_w,
        y: btn_y,
        w: accept_w,
        h: btn_h,
    };
    let cancel = Rect {
        x: accept.x - theme::spacing::SM - cancel_w,
        y: btn_y,
        w: cancel_w,
        h: btn_h,
    };
    ConfirmLayout {
        card,
        cancel,
        accept,
    }
}

pub fn hit_launcher(
    layout: &LauncherLayout,
    x: f32,
    y: f32,
    kind_enabled: bool,
    has_binding: impl Fn(usize) -> bool,
) -> Option<Hit> {
    if layout.toggle.contains(x, y) || layout.enable_row.contains(x, y) {
        return Some(Hit::KindToggle);
    }
    if !kind_enabled {
        return None;
    }
    for (i, item) in layout.items.iter().enumerate() {
        if item.checkbox.contains(x, y) {
            return Some(Hit::ItemVisible(i));
        }
        if let Some(clear) = item.recorder_clear {
            if has_binding(i) && clear.contains(x, y) {
                return Some(Hit::RecorderClear(i));
            }
        }
        if let Some(rec) = item.recorder {
            if rec.contains(x, y) {
                return Some(Hit::Recorder(i));
            }
        }
        if item.alias.contains(x, y) {
            return Some(Hit::Alias(i));
        }
    }
    None
}

pub fn hit_search(layout: &SearchLayout, x: f32, y: f32, ranking_empty: bool) -> Option<Hit> {
    if !ranking_empty && layout.reset.contains(x, y) {
        Some(Hit::ResetRanking)
    } else {
        None
    }
}

pub fn hit_confirm(layout: &ConfirmLayout, x: f32, y: f32) -> Option<Hit> {
    if layout.accept.contains(x, y) {
        Some(Hit::ConfirmReset)
    } else if layout.cancel.contains(x, y) {
        Some(Hit::ConfirmCancel)
    } else if !layout.card.contains(x, y) {
        Some(Hit::ConfirmCancel)
    } else {
        None
    }
}

pub struct FieldEdit {
    pub hwnd: HWND,
    font: HFONT,
    dpi: u32,
}

impl FieldEdit {
    pub fn create(parent: HWND, id: usize) -> windows::core::Result<Self> {
        unsafe {
            let hinstance = GetModuleHandleW(None)?;
            let style =
                WS_CHILD | WINDOW_STYLE(ES_LEFT as u32) | WINDOW_STYLE(ES_AUTOHSCROLL as u32);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("EDIT"),
                w!(""),
                style,
                0,
                0,
                0,
                0,
                parent,
                windows::Win32::UI::WindowsAndMessaging::HMENU(id as *mut core::ffi::c_void),
                hinstance,
                None,
            )?;
            let _ = SetWindowTheme(hwnd, w!(""), w!(""));
            let _ = SendMessageW(
                hwnd,
                EM_SETMARGINS,
                WPARAM((EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize),
                LPARAM(0),
            );
            Ok(Self {
                hwnd,
                font: HFONT::default(),
                dpi: 0,
            })
        }
    }

    pub fn layout(&mut self, parent: HWND, rect: Rect, visible: bool) {
        unsafe {
            if !visible {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                return;
            }
            let dpi = GetDpiForWindow(parent);
            if dpi != self.dpi {
                self.dpi = dpi;
                self.apply_font(dpi);
            }
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOP,
                dip_scalar_to_px(rect.x, dpi),
                dip_scalar_to_px(rect.y, dpi),
                dip_scalar_to_px(rect.w, dpi),
                dip_scalar_to_px(rect.h, dpi),
                SWP_NOACTIVATE,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn focus(&self) {
        unsafe {
            let _ = SetFocus(self.hwnd);
        }
    }

    pub fn set_text(&self, text: &str) {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let _ = SetWindowTextW(self.hwnd, PCWSTR(wide.as_ptr()));
        }
    }

    pub fn text(&self) -> String {
        unsafe {
            let len = GetWindowTextLengthW(self.hwnd);
            if len <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = GetWindowTextW(self.hwnd, &mut buf);
            if n <= 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }

    fn apply_font(&mut self, dpi: u32) {
        let px = dip_scalar_to_px(FIELD_FONT_DIP, dpi);
        let font = unsafe {
            CreateFontW(
                -px,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            )
        };
        if font.is_invalid() {
            return;
        }
        unsafe {
            let _ = SendMessageW(self.hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font);
            }
        }
        self.font = font;
    }
}

impl Drop for FieldEdit {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font);
                self.font = HFONT::default();
            }
        }
        self.hwnd = HWND::default();
    }
}

pub struct Formats<'a> {
    pub header: &'a IDWriteTextFormat,
    pub body: &'a IDWriteTextFormat,
    pub caption: &'a IDWriteTextFormat,
}

pub fn paint_launcher_items(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    section: &LauncherItemsSection,
    layout: &LauncherLayout,
    entries: &[&AppEntry],
    visibility: &VisibilityStore,
    aliases: &AliasStore,
    hotkeys: &HotKeyStore,
    recording: Option<&str>,
    editing_alias: Option<usize>,
    query: &str,
    scroll: f32,
) -> windows::core::Result<()> {
    let kind_on = visibility.is_kind_enabled(section.kind);
    let header_brush = solid(target, header_text_color())?;
    let body_brush = solid(target, tab_text_color())?;
    let caption_brush = solid(target, header_text_color())?;
    let card_brush = solid(target, card_fill())?;
    let stroke_brush = solid(target, hairline_color())?;
    draw_label(
        target,
        formats.header,
        &header_brush,
        shifted(layout.header, scroll),
        section.header,
    )?;
    fill_card(target, &card_brush, shifted(layout.enable_row, scroll))?;
    draw_label(
        target,
        formats.body,
        &body_brush,
        shifted(
            Rect {
                x: layout.enable_row.x + CARD_PAD,
                y: layout.enable_row.y,
                w: layout.toggle.x - layout.enable_row.x - CARD_PAD - theme::spacing::SM,
                h: 22.0,
            },
            scroll,
        ),
        &section.enable_title(),
    )?;
    draw_label(
        target,
        formats.caption,
        &caption_brush,
        shifted(
            Rect {
                x: layout.enable_row.x + CARD_PAD,
                y: layout.enable_row.y + 22.0,
                w: layout.toggle.x - layout.enable_row.x - CARD_PAD - theme::spacing::SM,
                h: 28.0,
            },
            scroll,
        ),
        ENABLE_SUBTITLE,
    )?;
    paint_toggle(target, shifted(layout.toggle, scroll), kind_on)?;
    fill_card(target, &card_brush, shifted(layout.list_card, scroll))?;
    if query.is_empty() {
        draw_label(
            target,
            formats.caption,
            &caption_brush,
            shifted(layout.filter, scroll),
            section.search_prompt,
        )?;
    }
    if let Some(empty) = layout.empty {
        draw_label(
            target,
            formats.caption,
            &caption_brush,
            shifted(empty, scroll),
            &empty_list_label(query),
        )?;
    }
    for (i, item) in layout.items.iter().enumerate() {
        let Some(entry) = entries.get(i) else {
            continue;
        };
        if i > 0 {
            let line_y = item.row.y - scroll;
            unsafe {
                target.DrawLine(
                    D2D_POINT_2F {
                        x: item.row.x,
                        y: line_y,
                    },
                    D2D_POINT_2F {
                        x: item.row.x + item.row.w,
                        y: line_y,
                    },
                    &stroke_brush,
                    theme::size::HAIRLINE,
                    None,
                );
            }
        }
        let name_w = item.alias.x - item.row.x - theme::spacing::SM;
        draw_label(
            target,
            formats.body,
            &body_brush,
            shifted(
                Rect {
                    x: item.row.x,
                    y: item.row.y,
                    w: name_w.max(0.0),
                    h: item.row.h,
                },
                scroll,
            ),
            &entry.name,
        )?;
        if editing_alias != Some(i) {
            let alias = aliases.get(&entry.id).unwrap_or("");
            paint_well(
                target,
                formats.caption,
                shifted(item.alias, scroll),
                if alias.is_empty() { "Add Alias" } else { alias },
                alias.is_empty(),
                false,
            )?;
        }
        if let Some(rec) = item.recorder {
            let key = hotkey_action_key(entry);
            let listening = key
                .as_deref()
                .is_some_and(|k| recording.is_some_and(|r| r == k));
            let bound = key.as_deref().and_then(|k| hotkeys.get(k));
            let (label, placeholder) = if listening {
                ("Listening…".to_string(), true)
            } else if let Some(binding) = bound {
                (binding.label(), false)
            } else {
                ("Record".into(), true)
            };
            paint_well(
                target,
                formats.caption,
                shifted(rec, scroll),
                &label,
                placeholder,
                listening,
            )?;
            if bound.is_some() && !listening {
                if let Some(clear) = item.recorder_clear {
                    draw_label(
                        target,
                        formats.caption,
                        &caption_brush,
                        shifted(clear, scroll),
                        "×",
                    )?;
                }
            }
        }
        paint_checkbox(
            target,
            shifted(item.checkbox, scroll),
            visibility.is_item_visible(&entry.id),
        )?;
    }
    if !kind_on {
        let dim = solid(target, dim_fill())?;
        fill_card(target, &dim, shifted(layout.list_card, scroll))?;
    }
    Ok(())
}

pub fn paint_search_section(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    layout: &SearchLayout,
    ranking_empty: bool,
    scroll: f32,
) -> windows::core::Result<()> {
    let header_brush = solid(target, header_text_color())?;
    let body_brush = solid(target, tab_text_color())?;
    let caption_brush = solid(target, header_text_color())?;
    let card_brush = solid(target, card_fill())?;
    draw_label(
        target,
        formats.header,
        &header_brush,
        shifted(layout.header, scroll),
        SEARCH_HEADER,
    )?;
    fill_card(target, &card_brush, shifted(layout.row, scroll))?;
    draw_label(
        target,
        formats.body,
        &body_brush,
        shifted(
            Rect {
                x: layout.row.x + CARD_PAD,
                y: layout.row.y,
                w: layout.reset.x - layout.row.x - CARD_PAD,
                h: layout.row.h,
            },
            scroll,
        ),
        LEARNED_RANKING_TITLE,
    )?;
    paint_button(
        target,
        formats.caption,
        shifted(layout.reset, scroll),
        RESET_RANKING_BUTTON,
        true,
        !ranking_empty,
    )?;
    draw_label(
        target,
        formats.caption,
        &caption_brush,
        shifted(layout.footer, scroll),
        RESET_RANKING_FOOTER,
    )?;
    Ok(())
}

pub fn paint_confirm(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    layout: &ConfirmLayout,
    window: (f32, f32),
) -> windows::core::Result<()> {
    paint_confirm_copy(
        target,
        formats,
        layout,
        window,
        ConfirmCopy::reset_ranking(),
    )
}

pub fn paint_confirm_copy(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    layout: &ConfirmLayout,
    window: (f32, f32),
    copy: ConfirmCopy,
) -> windows::core::Result<()> {
    let scrim = solid(target, scrim_color())?;
    unsafe {
        target.FillRectangle(
            &D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: window.0,
                bottom: window.1,
            },
            &scrim,
        );
    }
    let card_brush = solid(target, card_fill())?;
    fill_card(target, &card_brush, layout.card)?;
    let body_brush = solid(target, tab_text_color())?;
    let caption_brush = solid(target, header_text_color())?;
    draw_label(
        target,
        formats.body,
        &body_brush,
        Rect {
            x: layout.card.x + CARD_PAD,
            y: layout.card.y + CARD_PAD,
            w: layout.card.w - CARD_PAD * 2.0,
            h: 24.0,
        },
        copy.title,
    )?;
    draw_label(
        target,
        formats.caption,
        &caption_brush,
        Rect {
            x: layout.card.x + CARD_PAD,
            y: layout.card.y + CARD_PAD + 28.0,
            w: layout.card.w - CARD_PAD * 2.0,
            h: 48.0,
        },
        copy.message,
    )?;
    paint_button(
        target,
        formats.caption,
        layout.cancel,
        copy.cancel,
        false,
        true,
    )?;
    paint_button(
        target,
        formats.caption,
        layout.accept,
        copy.accept,
        true,
        true,
    )?;
    Ok(())
}

fn shifted(rect: Rect, scroll: f32) -> Rect {
    Rect {
        y: rect.y - scroll,
        ..rect
    }
}

fn fill_card(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    rect: Rect,
) -> windows::core::Result<()> {
    let rounded = D2D1_ROUNDED_RECT {
        rect: rect.d2d(),
        radiusX: theme::radius::CARD,
        radiusY: theme::radius::CARD,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, brush);
    }
    Ok(())
}

fn paint_toggle(target: &ID2D1RenderTarget, rect: Rect, on: bool) -> windows::core::Result<()> {
    let fill = solid(target, if on { accent_color() } else { toggle_off() })?;
    let knob = solid(target, tab_text_color())?;
    let rounded = D2D1_ROUNDED_RECT {
        rect: rect.d2d(),
        radiusX: rect.h / 2.0,
        radiusY: rect.h / 2.0,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &fill);
    }
    let pad = 2.0;
    let kn = rect.h - pad * 2.0;
    let kx = if on {
        rect.x + rect.w - pad - kn
    } else {
        rect.x + pad
    };
    let knob_r = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: kx,
            top: rect.y + pad,
            right: kx + kn,
            bottom: rect.y + pad + kn,
        },
        radiusX: kn / 2.0,
        radiusY: kn / 2.0,
    };
    unsafe {
        target.FillRoundedRectangle(&knob_r, &knob);
    }
    Ok(())
}

fn paint_checkbox(target: &ID2D1RenderTarget, rect: Rect, on: bool) -> windows::core::Result<()> {
    let stroke = solid(target, hairline_color())?;
    let rounded = D2D1_ROUNDED_RECT {
        rect: rect.d2d(),
        radiusX: 3.0,
        radiusY: 3.0,
    };
    unsafe {
        target.DrawRoundedRectangle(&rounded, &stroke, theme::size::HAIRLINE, None);
    }
    if on {
        let fill = solid(target, accent_color())?;
        unsafe {
            target.FillRoundedRectangle(&rounded, &fill);
        }
        let mark = solid(target, tab_text_color())?;
        unsafe {
            target.DrawLine(
                D2D_POINT_2F {
                    x: rect.x + 3.0,
                    y: rect.y + rect.h * 0.55,
                },
                D2D_POINT_2F {
                    x: rect.x + rect.w * 0.42,
                    y: rect.y + rect.h - 4.0,
                },
                &mark,
                1.5,
                None,
            );
            target.DrawLine(
                D2D_POINT_2F {
                    x: rect.x + rect.w * 0.42,
                    y: rect.y + rect.h - 4.0,
                },
                D2D_POINT_2F {
                    x: rect.x + rect.w - 3.0,
                    y: rect.y + 4.0,
                },
                &mark,
                1.5,
                None,
            );
        }
    }
    Ok(())
}

fn paint_well(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    rect: Rect,
    text: &str,
    placeholder: bool,
    recording: bool,
) -> windows::core::Result<()> {
    let fill = solid(target, well_fill())?;
    let stroke = solid(
        target,
        if recording {
            accent_color()
        } else {
            hairline_color()
        },
    )?;
    let rounded = D2D1_ROUNDED_RECT {
        rect: rect.d2d(),
        radiusX: theme::radius::MENU,
        radiusY: theme::radius::MENU,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &fill);
        target.DrawRoundedRectangle(&rounded, &stroke, theme::size::HAIRLINE, None);
    }
    let brush = solid(
        target,
        if placeholder {
            header_text_color()
        } else {
            tab_text_color()
        },
    )?;
    draw_label(
        target,
        format,
        &brush,
        rect.inset(theme::spacing::SM, 0.0),
        text,
    )
}

fn paint_button(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    rect: Rect,
    text: &str,
    destructive: bool,
    enabled: bool,
) -> windows::core::Result<()> {
    let fill = solid(
        target,
        if destructive && enabled {
            destructive_fill()
        } else {
            well_fill()
        },
    )?;
    let rounded = D2D1_ROUNDED_RECT {
        rect: rect.d2d(),
        radiusX: theme::radius::MENU,
        radiusY: theme::radius::MENU,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, &fill);
    }
    let brush = solid(
        target,
        if enabled {
            tab_text_color()
        } else {
            header_text_color()
        },
    )?;
    draw_label(target, format, &brush, rect, text)
}

fn draw_label(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    brush: &ID2D1SolidColorBrush,
    rect: Rect,
    text: &str,
) -> windows::core::Result<()> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &utf16,
            format,
            &rect.d2d(),
            brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn solid(
    target: &ID2D1RenderTarget,
    color: D2D1_COLOR_F,
) -> windows::core::Result<ID2D1SolidColorBrush> {
    unsafe { target.CreateSolidColorBrush(&color, None) }
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

fn hairline_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.08,
    }
}

fn card_fill() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.20,
        g: 0.20,
        b: 0.20,
        a: 1.0,
    }
}

fn well_fill() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.06,
    }
}

fn accent_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.0,
        g: 0.47,
        b: 0.83,
        a: 1.0,
    }
}

fn toggle_off() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.18,
    }
}

fn dim_fill() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.16,
        g: 0.16,
        b: 0.16,
        a: 0.55,
    }
}

fn scrim_color() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.45,
    }
}

fn destructive_fill() -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: 0.72,
        g: 0.20,
        b: 0.18,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: AppKind, id: &str, name: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            kind,
            name: name.into(),
            fields: SearchFields {
                display_name: name.into(),
                ..Default::default()
            },
            hotkey: None,
        }
    }

    #[test]
    fn quicklinks_and_emoji_tabs_are_not_empty_launcher_sections() {
        assert!(LauncherItemsSection::for_tab(SettingsTab::Quicklinks).is_none());
        assert!(LauncherItemsSection::for_tab(SettingsTab::Emoji).is_none());
        assert_eq!(
            LauncherItemsSection::for_tab(SettingsTab::Commands),
            Some(LauncherItemsSection::commands())
        );
    }

    #[test]
    fn launcher_layout_shift_stacks_below_custom_commands() {
        let layout =
            layout_launcher_items(&LauncherItemsSection::commands(), 0, |_| false, 400.0, true);
        let shifted = layout.clone().shifted(80.0);
        assert_eq!(shifted.header.y, layout.header.y + 80.0);
        assert_eq!(shifted.enable_row.y, layout.enable_row.y + 80.0);
        assert_eq!(shifted.content_height, layout.content_height + 80.0);
    }

    #[test]
    fn commands_pane_lists_every_command_id() {
        let entries = commands_catalog();
        assert_eq!(entries.len(), CommandID::all().len());
        assert_eq!(CommandID::all().len(), 29);
        for id in CommandID::all() {
            assert!(
                entries.iter().any(|e| e.id == id.raw()),
                "missing {}",
                id.raw()
            );
        }
        assert!(entries.iter().any(|e| e.id == CommandID::SearchFiles.raw()));
        assert!(entries.iter().any(|e| e.id == CommandID::AiChat.raw()));
    }

    #[test]
    fn commands_catalog_is_sorted_by_name() {
        let names: Vec<String> = commands_catalog()
            .iter()
            .map(|e| e.name.to_ascii_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn filter_is_membership_and_keeps_index_order() {
        let entries = vec![
            entry(AppKind::Application, "app:b", "Browser"),
            entry(AppKind::Application, "app:c", "Code"),
            entry(AppKind::Command, "command:quit", "Quit Tinycast"),
            entry(AppKind::Application, "app:n", "Notepad"),
        ];
        let all = filter_entries(&entries, AppKind::Application, "");
        assert_eq!(
            all.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["Browser", "Code", "Notepad"]
        );
        let q = filter_entries(&entries, AppKind::Application, "code");
        assert_eq!(
            q.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["Code"]
        );
    }

    #[test]
    fn enable_toggle_and_item_checkbox_are_hittable() {
        let section = LauncherItemsSection::applications();
        assert_eq!(section.enable_title(), "Enable Applications");
        let layout = layout_launcher_items(&section, 2, |_| true, 600.0, false);
        assert_eq!(
            hit_launcher(
                &layout,
                layout.toggle.x + 1.0,
                layout.toggle.y + 1.0,
                true,
                |_| false
            ),
            Some(Hit::KindToggle)
        );
        let row = &layout.items[0];
        assert_eq!(
            hit_launcher(
                &layout,
                row.checkbox.x + 1.0,
                row.checkbox.y + 1.0,
                true,
                |_| false
            ),
            Some(Hit::ItemVisible(0))
        );
        assert_eq!(
            hit_launcher(&layout, row.alias.x + 1.0, row.alias.y + 1.0, true, |_| {
                false
            }),
            Some(Hit::Alias(0))
        );
        assert_eq!(
            hit_launcher(
                &layout,
                row.recorder.unwrap().x + 1.0,
                row.recorder.unwrap().y + 1.0,
                true,
                |_| false
            ),
            Some(Hit::Recorder(0))
        );
        assert_eq!(
            hit_launcher(
                &layout,
                row.checkbox.x + 1.0,
                row.checkbox.y + 1.0,
                false,
                |_| false
            ),
            None
        );
    }

    #[test]
    fn recorder_clear_only_hits_when_bound() {
        let section = LauncherItemsSection::applications();
        let layout = layout_launcher_items(&section, 1, |_| true, 600.0, false);
        let clear = layout.items[0].recorder_clear.unwrap();
        assert_eq!(
            hit_launcher(&layout, clear.x + 1.0, clear.y + 1.0, true, |_| false),
            Some(Hit::Recorder(0))
        );
        assert_eq!(
            hit_launcher(&layout, clear.x + 1.0, clear.y + 1.0, true, |_| true),
            Some(Hit::RecorderClear(0))
        );
    }

    #[test]
    fn commands_without_hotkey_have_no_recorder() {
        let quit = CommandID::Quit.as_entry();
        assert!(hotkey_action_key(&quit).is_none());
        let clip = CommandID::ClipboardHistory.as_entry();
        assert_eq!(
            hotkey_action_key(&clip).as_deref(),
            Some("hotkey.toggleClipboard")
        );
    }

    #[test]
    fn system_settings_hotkey_uses_ms_settings_pane_id() {
        let mut pane = entry(
            AppKind::SystemSettings,
            "app:ms-settings:display",
            "Display",
        );
        pane.fields.bundle_id = Some("ms-settings:display".into());
        assert_eq!(
            hotkey_action_key(&pane).as_deref(),
            Some("hotkey.pane.ms-settings:display")
        );
    }

    #[test]
    fn ranking_reset_copy_matches_v0102() {
        assert_eq!(SEARCH_HEADER, "Search");
        assert_eq!(RESET_RANKING_BUTTON, "Reset…");
        assert_eq!(RESET_CONFIRM_TITLE, "Reset learned launcher ranking?");
        assert_eq!(RESET_CONFIRM_ACTION, "Reset Ranking");
        let layout = layout_search_section(600.0);
        assert_eq!(
            hit_search(&layout, layout.reset.x + 1.0, layout.reset.y + 1.0, false),
            Some(Hit::ResetRanking)
        );
        assert_eq!(
            hit_search(&layout, layout.reset.x + 1.0, layout.reset.y + 1.0, true),
            None
        );
    }

    #[test]
    fn confirm_dialog_maps_buttons() {
        let layout = layout_confirm(860.0, 700.0);
        assert_eq!(
            hit_confirm(&layout, layout.accept.x + 1.0, layout.accept.y + 1.0),
            Some(Hit::ConfirmReset)
        );
        assert_eq!(
            hit_confirm(&layout, layout.cancel.x + 1.0, layout.cancel.y + 1.0),
            Some(Hit::ConfirmCancel)
        );
        assert_eq!(hit_confirm(&layout, 1.0, 1.0), Some(Hit::ConfirmCancel));
    }

    #[test]
    fn blank_alias_commits_as_none() {
        assert_eq!(commit_alias_text("   "), None);
        assert_eq!(commit_alias_text("iterm"), Some("iterm".into()));
        assert_eq!(commit_alias_text(" i term "), Some("i term".into()));
    }
}
