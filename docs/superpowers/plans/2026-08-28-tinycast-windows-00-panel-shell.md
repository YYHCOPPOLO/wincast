# Panel Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Implement **every task in this plan file** first. **Do not review after each task.** When the last task is committed, review this plan's full commit range once. See `AGENTS.md`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A resident Windows tray process that summons a Tinycast-sized Acrylic panel with a real `EDIT` search field via Alt+Space, and opens an empty settings window.

**Architecture:** `tinycast-pure` owns tokens, placement, palette state, settings tabs, and hotkey models. `tinycast` owns the UI-thread `AppCore`, Win32 host, Direct2D panel, and tray.

**Tech Stack:** Rust 2021 workspace, `windows` 0.58, `serde`/`serde_json`. Direct2D/DirectWrite + DWM Acrylic. No extra GUI crates.

**Spec:** `docs/superpowers/specs/2026-08-28-tinycast-windows-design.md` (plus `docs/superpowers/plans/2026-08-28-tinycast-windows.md` Global Constraints).

## Global Constraints

All constraints in the plan index apply, including **one review after this entire plan**, not after each task (`AGENTS.md`). This plan additionally:

- Default hotkey is Alt+Space (`VK_SPACE` + `MOD_ALT`).
- Compact height is `headerHeight + 2*headerPadding` = 64 DIP. Expanded panel is 750×475 DIP. Corner 26. Top margin fraction 0.18.
- Panel is WS_EX_TOOLWINDOW | WS_EX_TOPMOST, activatable (IME). No taskbar button.
- Search field is a child `EDIT`, not a custom IME.
- Verify Acrylic/corners over a **light** wallpaper.

---

### Task 1: Workspace and pure-crate gate

**Files:**
- Create: `.gitignore`
- Create: `Cargo.toml`
- Create: `crates/tinycast-pure/Cargo.toml`
- Create: `crates/tinycast-pure/src/lib.rs`
- Create: `crates/tinycast-pure/tests/no_windows_dep.rs`

**Interfaces:**
- Consumes: nothing
- Produces: crate `tinycast-pure` v0.1.0; workspace members `crates/tinycast-pure`

- [ ] **Step 1: Write the failing test**

Create `crates/tinycast-pure/tests/no_windows_dep.rs`:

```rust
#[test]
fn pure_crate_toml_does_not_depend_on_windows() {
    let toml = include_str!("../Cargo.toml");
    assert!(
        !toml.lines().any(|l| l.contains("windows")),
        "tinycast-pure must not depend on the windows crate"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure --test no_windows_dep`

Expected: FAIL — package `tinycast-pure` not found.

- [ ] **Step 3: Write minimal implementation**

`.gitignore`:

```
/target
**/*.rs.bk
```

Root `Cargo.toml`:

```toml
[workspace]
members = ["crates/tinycast-pure"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT"
version = "0.1.0"

[profile.release]
lto = true
codegen-units = 1
opt-level = "s"
panic = "abort"
strip = true
```

`crates/tinycast-pure/Cargo.toml`:

```toml
[package]
name = "tinycast-pure"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`crates/tinycast-pure/src/lib.rs`:

```rust
//! Decision layer. Must not depend on `windows`.
```

If the repo has no `.git`, run `git init` in the workspace root before committing.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure --test no_windows_dep`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add .gitignore Cargo.toml crates/tinycast-pure
git commit -m "chore: add workspace and tinycast-pure crate gate"
```

---

### Task 2: Theme tokens

**Files:**
- Create: `crates/tinycast-pure/src/theme.rs`
- Modify: `crates/tinycast-pure/src/lib.rs`
- Create: `crates/tinycast-pure/src/theme.rs` tests as `#[cfg(test)]` module

**Interfaces:**
- Consumes: nothing
- Produces:

```rust
pub mod theme {
    pub mod spacing {
        pub const XXS: f32 = 2.0;
        pub const XS: f32 = 4.0;
        pub const SM: f32 = 6.0;
        pub const MD: f32 = 8.0;
        pub const LG: f32 = 10.0;
        pub const XL: f32 = 12.0;
        pub const XXL: f32 = 20.0;
        pub const XXXL: f32 = 28.0;
        pub const SECTION_HEADER_BOTTOM: f32 = 4.0;
        pub const SECTION_SPACING: f32 = 12.0;
    }
    pub mod radius {
        pub const PANEL: f32 = 26.0;
        pub const ROW: f32 = 10.0;
        pub const DIALOG: f32 = 20.0;
        pub const MENU_PANEL: f32 = 16.0;
        pub const KEY_CAP: f32 = 6.0;
    }
    pub mod size {
        pub const PANEL_WIDTH: f32 = 750.0;
        pub const PANEL_HEIGHT: f32 = 475.0;
        pub const PALETTE_TOP_MARGIN_FRACTION: f32 = 0.18;
        pub const HEADER_HEIGHT: f32 = 44.0;
        pub const HEADER_PADDING: f32 = 10.0;
        pub const COMPACT_HEIGHT: f32 = 64.0; // HEADER_HEIGHT + HEADER_PADDING * 2
        pub const BOTTOM_BAR_HEIGHT: f32 = 52.0;
        pub const ROW_ICON: f32 = 24.0;
        pub const SETTINGS_WINDOW: (f32, f32) = (860.0, 700.0);
        pub const SETTINGS_SIDEBAR: f32 = 215.0;
        pub const SETTINGS_DETAIL_MINIMUM: f32 = 420.0;
        pub const DIALOG_WIDTH: f32 = 420.0;
    }
    pub mod duration {
        pub const ENTER_SECS: f32 = 0.18;
        pub const EXIT_SECS: f32 = 0.12;
    }
    pub mod colors {
        pub const PANEL_SCRIM_DARK_ALPHA: f32 = 0.40;
        pub const PANEL_SCRIM_LIGHT_ALPHA: f32 = 0.55;
        pub const SELECTION_DARK_ALPHA: f32 = 0.10;
        pub const ROW_HOVER_DARK_ALPHA: f32 = 0.05;
    }
}
```

Copy remaining v0.10.2 `Theme.swift` size/radius tokens into the same modules as needed so later plans never invent numbers.

- [ ] **Step 1: Write the failing test**

In `theme.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_height_is_header_plus_symmetric_padding() {
        assert_eq!(
            size::COMPACT_HEIGHT,
            size::HEADER_HEIGHT + size::HEADER_PADDING * 2.0
        );
    }
    #[test]
    fn dark_scrim_is_frozen() {
        assert_eq!(colors::PANEL_SCRIM_DARK_ALPHA, 0.40);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure compact_height -- --nocapture`

Expected: FAIL compiling (`theme` not found) or assertion if stubs exist with wrong values.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod theme;` to `lib.rs`. Fill `theme.rs` with the constants above, matching v0.10.2 `Theme.swift` literals exactly.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure theme`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add frozen Theme tokens in tinycast-pure"
```

---

### Task 3: PalettePlacement

**Files:**
- Create: `crates/tinycast-pure/src/palette_placement.rs`
- Modify: `crates/tinycast-pure/src/lib.rs`

**Interfaces:**
- Consumes: `theme::size`
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DipRect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenDip {
    pub frame: DipRect,          // full display in DIP, global origin
    pub work: DipRect,           // work area (taskbar excluded)
    pub origin_is_primary: bool, // frame.origin == (0,0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaletteAnchor { pub left: f32, pub top: f32 }

pub fn compact_size() -> (f32, f32) {
    (theme::size::PANEL_WIDTH, theme::size::COMPACT_HEIGHT)
}
pub fn expanded_size() -> (f32, f32) {
    (theme::size::PANEL_WIDTH, theme::size::PANEL_HEIGHT)
}

/// Default top-left of the compact bar on `screen`.
/// Top = work.y + work.h * PALETTE_TOP_MARGIN_FRACTION.
/// Horizontal: centered in work.
pub fn default_anchor(screen: ScreenDip) -> PaletteAnchor;

/// Pick the screen: if `open_on_cursor` then the screen containing `cursor`,
/// else the primary (`origin_is_primary`). Never “the focused window’s screen”.
pub fn target_screen<'a>(
    screens: &'a [ScreenDip],
    cursor: (f32, f32),
    open_on_cursor: bool,
) -> Option<&'a ScreenDip>;

pub fn frame_for(anchor: PaletteAnchor, expanded: bool) -> DipRect;
```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn default_anchor_is_18_percent_down_work_area_centered() {
    let screen = ScreenDip {
        frame: DipRect { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 },
        work: DipRect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 },
        origin_is_primary: true,
    };
    let a = default_anchor(screen);
    assert!((a.top - 1040.0 * 0.18).abs() < 0.01);
    assert!((a.left - (1920.0 - 750.0) / 2.0).abs() < 0.01);
}

#[test]
fn follow_cursor_uses_containing_screen_not_primary() {
    let screens = [
        ScreenDip {
            frame: DipRect { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 },
            work: DipRect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 },
            origin_is_primary: true,
        },
        ScreenDip {
            frame: DipRect { x: 1920.0, y: 0.0, w: 1920.0, h: 1080.0 },
            work: DipRect { x: 1920.0, y: 0.0, w: 1920.0, h: 1080.0 },
            origin_is_primary: false,
        },
    ];
    let s = target_screen(&screens, (2000.0, 10.0), true).unwrap();
    assert!(!s.origin_is_primary);
    let primary = target_screen(&screens, (2000.0, 10.0), false).unwrap();
    assert!(primary.origin_is_primary);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure default_anchor`

Expected: FAIL compile (`default_anchor` not found).

- [ ] **Step 3: Write minimal implementation**

Implement the functions using work-area math only. Cursor hit test: `x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h`. If cursor hits no screen, fall back to primary.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure palette_placement`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add PalettePlacement in DIP space"
```

---

### Task 4: PaletteMode, PaletteState, PaletteRowIndex, SettingsTab

**Files:**
- Create: `crates/tinycast-pure/src/palette_mode.rs`
- Create: `crates/tinycast-pure/src/palette_state.rs`
- Create: `crates/tinycast-pure/src/palette_row_index.rs`
- Create: `crates/tinycast-pure/src/settings_tab.rs`
- Modify: `crates/tinycast-pure/src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteMode {
    Launcher, Clipboard, CalculatorHistory, Emoji, FileSearch, Schedule,
    Uninstall, Quicklinks, QuicklinkArguments, Ai, AiHistory, ExtensionCommand,
}

#[derive(Clone, Debug)]
pub struct PaletteState {
    pub mode: PaletteMode,
    pub query: String,
    pub selection: usize,
    pub focus_token: u128,
    pub is_composing: bool,
}
impl PaletteState {
    pub fn new() -> Self;
    /// Reset query/selection, set mode, bump focus_token.
    pub fn prepare(&mut self, mode: PaletteMode);
}

/// Flat selection: headers consume no index. `card` occupies index 0 when present.
pub fn selectable_count(card: bool, rows: usize) -> usize {
    rows + usize::from(card)
}
pub fn clamp_selection(selection: usize, count: usize) -> usize {
    if count == 0 { 0 } else { selection.min(count - 1) }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SettingsTab {
    General, Applications, SystemSettings, SystemActions, Commands, Quicklinks,
    Ai, QuickActions, FileSearch, Notes, Snippets, WindowManagement, Clipboard,
    Emoji, Calendar, Extensions, Permissions, Backup, About,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SettingsSection { General, Launcher, Features, Advanced }
impl SettingsSection {
    pub fn all() -> [SettingsSection; 4];
    pub fn title(self) -> &'static str;
    pub fn tabs(self) -> &'static [SettingsTab];
}
impl SettingsTab {
    pub fn title(self) -> &'static str; // original English: "Window Management", etc.
}
```

`SettingsSection::tabs` order must match spec §7.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn settings_sidebar_order() {
    use SettingsSection::*;
    assert_eq!(SettingsSection::all(), [General, Launcher, Features, Advanced]);
    assert_eq!(General.tabs(), &[SettingsTab::General, SettingsTab::Permissions]);
    assert_eq!(
        Launcher.tabs(),
        &[
            SettingsTab::Applications,
            SettingsTab::SystemSettings,
            SettingsTab::SystemActions,
            SettingsTab::Commands,
            SettingsTab::Quicklinks,
        ]
    );
}

#[test]
fn prepare_clears_query_and_bumps_focus() {
    let mut s = PaletteState::new();
    let t0 = s.focus_token;
    s.query = "abc".into();
    s.prepare(PaletteMode::Clipboard);
    assert!(s.query.is_empty());
    assert_eq!(s.mode, PaletteMode::Clipboard);
    assert_ne!(s.focus_token, t0);
}

#[test]
fn card_occupies_index_zero() {
    assert_eq!(selectable_count(true, 3), 4);
    assert_eq!(clamp_selection(10, 4), 3);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure settings_sidebar_order`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Fill the four modules. `SettingsTab::title` uses v0.10.2 strings (`"System Settings"`, `"Emoji & Symbols"`, `"File Search"`, …). Features tabs: Ai, QuickActions, FileSearch, Notes, Snippets, WindowManagement, Clipboard, Emoji, Calendar, Extensions. Advanced: Backup, About.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure`

Expected: PASS all pure tests so far.

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add palette state, row index, and settings tabs"
```

---

### Task 5: Hotkey model and AppSettingsKey

**Files:**
- Create: `crates/tinycast-pure/src/hotkey.rs`
- Create: `crates/tinycast-pure/src/app_settings_key.rs`
- Modify: `crates/tinycast-pure/src/lib.rs`

**Interfaces:**
- Consumes: `serde`
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct KeyShortcut {
    pub vk: u16,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DoubleTapModifier { Control, Option, Shift, Command }

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HotKeyBinding {
    Combo(KeyShortcut),
    DoubleTap(DoubleTapModifier),
}

pub fn default_toggle_palette() -> HotKeyBinding {
    HotKeyBinding::Combo(KeyShortcut {
        vk: 0x20, // VK_SPACE
        modifiers: Modifiers { ctrl: false, alt: true, shift: false, win: false },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppSettingsKey { /* every v0.10.2 case */ }
impl AppSettingsKey {
    pub fn as_str(self) -> &'static str; // raw values identical to v0.10.2
    pub fn ALL: &'static [AppSettingsKey];
}
```

`AppSettingsKey::as_str` must equal the Swift raw values (`clipboardRetentionDays`, `hyperKeyPhysicalKey`, `launcherSearchScopes`, …). Copy the full enum from spec-era `AppSettingsKey.swift`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn default_summon_is_alt_space() {
    match default_toggle_palette() {
        HotKeyBinding::Combo(k) => {
            assert_eq!(k.vk, 0x20);
            assert!(k.modifiers.alt && !k.modifiers.ctrl && !k.modifiers.win);
        }
        _ => panic!("expected combo"),
    }
}

#[test]
fn settings_key_raw_values_match_v0102() {
    assert_eq!(AppSettingsKey::ClipboardRetention.as_str(), "clipboardRetentionDays");
    assert_eq!(AppSettingsKey::SnippetsEnabled.as_str(), "snippetsEnabled");
    assert_eq!(AppSettingsKey::AiEnabled.as_str(), "aiEnabled");
    assert_eq!(AppSettingsKey::SearchScopes.as_str(), "launcherSearchScopes");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure default_summon_is_alt_space`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Implement both modules. For `AppSettingsKey`, include every case from v0.10.2 (see spec §7 / upstream file).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add hotkey model and AppSettingsKey raw values"
```

---

### Task 6: `tinycast` exe — DPI, single instance, message loop, tray

**Files:**
- Modify: `Cargo.toml` (add member `crates/tinycast`)
- Create: `crates/tinycast/Cargo.toml`
- Create: `crates/tinycast/src/main.rs`
- Create: `crates/tinycast/src/app_core.rs`
- Create: `crates/tinycast/src/platform/mod.rs`
- Create: `crates/tinycast/src/platform/dpi.rs`
- Create: `crates/tinycast/src/platform/single_instance.rs`
- Create: `crates/tinycast/src/platform/tray.rs`
- Create: `crates/tinycast/src/platform/messages.rs`

**Interfaces:**
- Consumes: `tinycast-pure::{PaletteState, PaletteMode, default_toggle_palette}`
- Produces:

```rust
// app_core.rs
pub struct AppCore {
    pub palette: tinycast_pure::palette_state::PaletteState,
    pub palette_visible: bool,
}
impl AppCore {
    pub fn new() -> Self;
    pub fn start(&mut self);
    pub fn toggle_palette(&mut self);
    pub fn hide_palette(&mut self);
}

// platform::messages — WM_APP + n
pub const WM_TRAY: u32 = 0x8000 + 1;
pub const WM_TOGGLE_PALETTE: u32 = 0x8000 + 2;
pub const WM_OPEN_SETTINGS: u32 = 0x8000 + 3;
pub const WM_QUIT_APP: u32 = 0x8000 + 4;

pub fn run() -> windows::core::Result<()>; // message loop
```

`crates/tinycast/Cargo.toml`:

```toml
[package]
name = "tinycast"
version.workspace = true
edition.workspace = true

[dependencies]
tinycast-pure = { path = "../tinycast-pure" }
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_Graphics_Dwm",
  "Win32_Graphics_Direct2D",
  "Win32_Graphics_DirectWrite",
  "Win32_Graphics_Dxgi",
  "Win32_System_Com",
  "Win32_System_LibraryLoader",
  "Win32_UI_HiDpi",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_Shell",
  "Win32_UI_WindowsAndMessaging",
] }
```

- [ ] **Step 1: Write the failing test**

Create `crates/tinycast-pure/tests/app_core_toggle.rs` is the wrong crate. Put a unit test next to AppCore once the file exists. First, a pure-side test already exists for PaletteState. For the exe, add `crates/tinycast/src/app_core.rs` test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggle_palette_flips_visible_and_prepares_launcher() {
        let mut c = AppCore::new();
        assert!(!c.palette_visible);
        c.toggle_palette();
        assert!(c.palette_visible);
        assert_eq!(c.palette.mode, tinycast_pure::palette_mode::PaletteMode::Launcher);
        c.toggle_palette();
        assert!(!c.palette_visible);
    }
}
```

This file will fail to compile until AppCore exists.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast toggle_palette_flips`

Expected: FAIL — package `tinycast` not found.

- [ ] **Step 3: Write minimal implementation**

`main.rs`:

```rust
mod app_core;
mod platform;

fn main() {
    if let Err(e) = platform::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
```

`single_instance.rs`: create a named mutex `Local\\com.tinycast.win`. If `ERROR_ALREADY_EXISTS`, exit 0.

`dpi.rs`: call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at startup.

`tray.rs`: hidden message-only window; `Shell_NotifyIconW(NIM_ADD)` with `WM_TRAY`. Left click posts `WM_TOGGLE_PALETTE`. Right click popup: `Settings` → `WM_OPEN_SETTINGS`, `Quit Tinycast` → `WM_QUIT_APP`. Tooltip `Tinycast`.

`run()`: dpi, mutex, `AppCore::new(); core.start();` register class, tray, `GetMessageW` loop. `start()` currently only sets initial `PaletteState`.

Do not create the palette HWND yet (next task). Toggle from tray may no-op on HWND but must flip `AppCore` flags when wired; for this task, posting `WM_TOGGLE_PALETTE` calls `core.toggle_palette()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast toggle_palette_flips`

Expected: PASS. Also `cargo build -p tinycast` succeeds.

Manual: run `cargo run -p tinycast`, confirm tray icon, right-click menu, Quit exits.

- [ ] **Step 5: Commit**

```
git add Cargo.toml crates/tinycast
git commit -m "feat: add tray host, single instance, and AppCore toggle"
```

---

### Task 7: Palette HWND, Acrylic, Direct2D scrim, compact/expand

**Files:**
- Create: `crates/tinycast/src/palette/mod.rs`
- Create: `crates/tinycast/src/palette/hwnd.rs`
- Create: `crates/tinycast/src/palette/d2d.rs`
- Create: `crates/tinycast/src/platform/screens.rs`
- Modify: `crates/tinycast/src/app_core.rs`
- Modify: `crates/tinycast/src/platform/mod.rs`

**Interfaces:**
- Consumes: `default_anchor`, `target_screen`, `frame_for`, `theme`, `AppCore`
- Produces:

```rust
pub struct PaletteWindow {
    pub hwnd: windows::Win32::Foundation::HWND,
}
impl PaletteWindow {
    pub fn create(host: HWND) -> windows::core::Result<Self>;
    pub fn show_at(&self, frame_px: physical::Rect);
    pub fn hide(&self);
    pub fn set_expanded(&self, expanded: bool, anchor_px: physical::Point);
}

pub fn screens_dip() -> Vec<tinycast_pure::palette_placement::ScreenDip>;
pub fn dip_to_px(hwnd: HWND, r: tinycast_pure::palette_placement::DipRect) -> /* RECT */;
```

Use `GetDpiForWindow`. DIP → px: `px = dip * dpi / 96`.

DWM:
- `DWMWA_WINDOW_CORNER_PREFERENCE` = 33, `DWMWCP_ROUND` = 2
- `DWMWA_SYSTEMBACKDROP_TYPE` = 38, `DWMSBT_TRANSIENTWINDOW` = 3

If the clip is square, switch to `UpdateLayeredWindow` path **without changing tokens**.

Direct2D: fill client with black at alpha 0.40 (dark) over the blur. No gray chrome.

- [ ] **Step 1: Write the failing test**

Placement is already tested in pure. Add:

```rust
#[test]
fn expanded_frame_keeps_top_left_anchor() {
    let a = PaletteAnchor { left: 100.0, top: 80.0 };
    let c = frame_for(a, false);
    let e = frame_for(a, true);
    assert_eq!(c.x, e.x);
    assert_eq!(c.y, e.y);
    assert_eq!(e.h, 475.0);
    assert_eq!(c.h, 64.0);
}
```

in `palette_placement.rs` tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure expanded_frame_keeps_top_left_anchor`

Expected: FAIL until `frame_for` uses a shared anchor (if Task 3 used independent math, fix it here).

- [ ] **Step 3: Write minimal implementation**

Create a popup HWND: `WS_POPUP`, `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`. `SetWindowPos` from `frame_for`. On `WM_TOGGLE_PALETTE`, resolve screen from cursor via `screens_dip` + `target_screen(..., open_on_cursor=true)` (hardcode true until AppSettings exists), store anchor, show compact. Empty query stays compact; any `query` or Down-arrow expands using the same anchor.

Paint via Direct2D in `WM_PAINT` / `WM_SIZE`. `DwmExtendFrameIntoClientArea` margins `-1` if needed for blur.

`AppCore` holds `Option<PaletteWindow>`. `toggle_palette` shows/hides HWND.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure expanded_frame_keeps_top_left_anchor`

Expected: PASS.

Manual: `cargo run -p tinycast`, tray click, panel 750×64 then type/Down to 750×475, top edge must not drift. Light wallpaper: rounded 26, desktop visible through scrim, not a solid gray slab.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add Acrylic palette HWND with compact and expanded frames"
```

---

### Task 8: Child EDIT, placeholder, composing flag

**Files:**
- Create: `crates/tinycast/src/palette/edit.rs`
- Modify: `crates/tinycast/src/palette/hwnd.rs`
- Modify: `crates/tinycast/src/app_core.rs`

**Interfaces:**
- Consumes: `PaletteState.query`, `is_composing`
- Produces: child `HWND` of class `EDIT`, transparent background; `WM_IME_COMPOSITION` / `EM_GETSEL` observers set `palette.is_composing`. Placeholder drawn by D2D when `query.is_empty() && !is_composing`, string `Search` for launcher (original mode placeholders later).

Subclass the EDIT with `SetWindowSubclass`. On `EN_CHANGE`, copy text to `AppCore.palette.query` and expand if non-empty. Down arrow in compact expands and selects row 0 without leaving the field.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn placeholder_hidden_while_composing_even_if_query_empty() {
    let mut s = PaletteState::new();
    s.is_composing = true;
    assert!(s.query.is_empty());
    assert!(!should_draw_placeholder(&s));
}

pub fn should_draw_placeholder(s: &PaletteState) -> bool {
    s.query.is_empty() && !s.is_composing
}
```

Put `should_draw_placeholder` on `PaletteState` in `tinycast-pure`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure placeholder_hidden`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Add the helper; create the EDIT as a child, no cue banner (`EM_SETCUEBANNER` forbidden — IME jump). D2D draws placeholder behind/around the field. Font 20pt regular (theme search field size).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure placeholder_hidden`

Expected: PASS.

Manual: Chinese IME composition must not sit under the placeholder; committing text expands the panel.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: host system EDIT search field with IME-safe placeholder"
```

---

### Task 9: RegisterHotKey, Escape, resign-key hide, footer chrome

**Files:**
- Create: `crates/tinycast/src/platform/hotkey.rs`
- Modify: `crates/tinycast/src/palette/hwnd.rs`

**Interfaces:**
- Consumes: `default_toggle_palette()`, `WM_TOGGLE_PALETTE`
- Produces: `RegisterHotKey(hwnd, 1, MOD_ALT, VK_SPACE)`. `WM_HOTKEY` → `AppCore.toggle_palette()`. Escape: if query non-empty, clear query; else hide. `WM_ACTIVATE` inactive → `hide_palette`. Footer: menu circle left, empty action capsule right (no rows yet). Enter animation 180ms scale 0.94, exit 120ms (timer or DComp; a simple `SetWindowPos`+alpha is acceptable if scale is applied).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn escape_clears_query_before_hiding() {
    // pure policy
    assert_eq!(escape_outcome("abc", true), EscapeOutcome::ClearQuery);
    assert_eq!(escape_outcome("", true), EscapeOutcome::Hide);
}

#[derive(Debug, PartialEq)]
pub enum EscapeOutcome { ClearQuery, Hide }
pub fn escape_outcome(query: &str, launcher: bool) -> EscapeOutcome {
    if !query.is_empty() { EscapeOutcome::ClearQuery } else { EscapeOutcome::Hide }
}
```

in `palette_state.rs`. `launcher` unused until Tab-ring modes exist; keep the signature so later plans pass mode.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure escape_clears_query_before_hiding`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Implement policy + `RegisterHotKey` + Escape handling in the panel WndProc. Unregister hotkey on quit.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure escape_clears_query_before_hiding`

Expected: PASS.

Manual: Alt+Space toggles. Alt+Space while another app is focused still works. Esc twice from a typed query hides.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: register Alt+Space hotkey and Escape hide policy"
```

---

### Task 10: Settings window skeleton

**Files:**
- Create: `crates/tinycast/src/surfaces/mod.rs`
- Create: `crates/tinycast/src/surfaces/settings.rs`
- Modify: `crates/tinycast/src/platform/tray.rs` (already posts `WM_OPEN_SETTINGS`)

**Interfaces:**
- Consumes: `SettingsSection`, `SettingsTab`, `theme::size::SETTINGS_*`
- Produces: titled HWND 860×700 DIP, left column 215 DIP listing sections/tabs in spec order, right pane empty grouped-form background. Selecting a tab stores `selected: SettingsTab` (default `General`). Independent of palette: opening settings does not hide/show rules of the panel except the tray command. Close box hides (does not quit the app).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn default_settings_tab_is_general() {
    assert_eq!(SettingsTab::General.title(), "General");
    assert_eq!(SettingsSection::Features.tabs()[0].title(), "AI");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure default_settings_tab_is_general`

Expected: FAIL if titles wrong.

- [ ] **Step 3: Write minimal implementation**

Draw sidebar with Direct2D or child listbox. Click maps to `SettingsTab`. Do not implement pane bodies.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure default_settings_tab_is_general`

Expected: PASS.

Manual: tray → Settings, sidebar groups General / Launcher / Features / Advanced, width 215, window does not create a taskbar-less tool window (settings is a normal titled window).

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add settings window skeleton with original sidebar order"
```

---

### Task 11: AppSettings persistence stub + launch at login hook site

**Files:**
- Create: `crates/tinycast/src/app_settings.rs`
- Create: `crates/tinycast/src/platform/paths.rs`
- Create: `crates/tinycast/src/platform/launch_at_login.rs`

**Interfaces:**
- Consumes: `AppSettingsKey`
- Produces:

```rust
pub fn roaming_dir() -> PathBuf; // %APPDATA%\com.tinycast.win
pub fn local_dir() -> PathBuf;   // %LOCALAPPDATA%\com.tinycast.win

pub struct AppSettings {
    pub compact_mode: bool,           // default true
    pub open_on_cursor_screen: bool,  // default true
    pub appearance: Appearance,       // System | Light | Dark
    pub launch_at_login: bool,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Appearance { System, Light, Dark }

impl AppSettings {
    pub fn load() -> Self; // missing file → defaults
    pub fn save(&self) -> std::io::Result<()>; // settings.json, keys = AppSettingsKey::as_str()
}
```

JSON object keys must be the raw v0.10.2 strings. `launch_at_login` writes/deletes `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `Tinycast` pointing at the exe; it is still stored in JSON for UI.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn settings_json_uses_upstream_keys() {
    let v = serde_json::json!({ "compactMode": true, "openOnCursorScreen": true });
    assert!(v.get("compactMode").is_some());
    assert!(v.get("openOnCursorScreen").is_some());
}
```

Better: round-trip `AppSettings` through serde with `#[serde(rename = "compactMode")]`.

```rust
#[test]
fn app_settings_serde_names() {
    let s = AppSettings { compact_mode: true, open_on_cursor_screen: false, appearance: Appearance::Dark, launch_at_login: false };
    let j = serde_json::to_value(&s).unwrap();
    assert_eq!(j["compactMode"], true);
    assert_eq!(j["openOnCursorScreen"], false);
    assert_eq!(j["appearance"], "dark");
}
```

Put serde on `AppSettings` in the **exe** crate; tests in `crates/tinycast/src/app_settings.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast app_settings_serde_names`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Implement load/save. Create roaming/local dirs on first save. Wire `open_on_cursor_screen` into Task 7 screen pick.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast app_settings_serde_names`

Expected: PASS. `cargo test` workspace PASS.

- [ ] **Step 5: Commit**

```
git add crates/tinycast
git commit -m "feat: persist AppSettings JSON under com.tinycast.win"
```

---

## Plan 00 done when

- `cargo test` (workspace) passes.
- `cargo run -p tinycast`: tray, Alt+Space, compact/expanded panel, IME-safe EDIT, Esc, Settings sidebar, Quit.
- Light wallpaper: rounded acrylic, not a gray box.
- `tinycast-pure` Cargo.toml still has no `windows`.
