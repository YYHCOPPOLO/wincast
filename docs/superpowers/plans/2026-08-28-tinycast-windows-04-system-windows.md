# System Actions, Window Management, and Hotkeys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Implement **every task in this plan file** first. **Do not review after each task.** When the last task is committed, review this plan's full commit range once. See `AGENTS.md`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Full `SystemAction` and 34 `WindowCommand` catalogs plus combo / double-tap / Hyper key engines, matching v0.10.2 names and rules.

**Architecture:** Catalogs, volume grid, window geometry, double-tap detector, Hyper chord spelling are pure. Runners and hooks are Effect. Coordinators own confirmation. Hooks install only while needed.

**Tech Stack:** `RegisterHotKey`, WH_KEYBOARD_LL (double-tap and Hyper), HKCU Scan Code Map for Caps Lock, `SetWindowPos`, DWM, `IVirtualDesktopManager`, WASAPI/mixer for volume.

**Spec:** spec §8.6, §8.7, §8.12. Oracle: `launcher.md` system actions, `window-management.md`, `hotkeys.md`.

## Global Constraints

Index constraints apply, including **one review after this entire plan**, not after each task (`AGENTS.md`). Window layout math is in DIP top-left origin; convert to pixels only in the mover. Virtual desktops use public APIs — no HID forgery. `toggle-stage-manager` stays in the catalog and HUDs that it is unavailable. Confirmations: Restart/Shut Down/Log Out say `PC`. Empty Recycle Bin confirms. Fail quiet on window commands when the target cannot move.

Default: `windowManagementEnabled = false`. System actions publish on.

---

### Task 1: SystemAction catalog + VolumeLevel

**Files:**
- Create: `crates/tinycast-pure/src/system_action.rs`
- Create: `crates/tinycast-pure/src/volume.rs`

**Interfaces:**

```rust
pub enum SystemActionId { LockScreen, Sleep, /* every v0.10.2 ID */ ToggleBluetooth }
impl SystemActionId {
    pub fn all() -> &'static [Self];
    pub fn raw(self) -> &'static str; // "lock-screen"
    pub fn name(self) -> &'static str; // "Lock Screen"
    pub fn confirmation(self) -> Confirmation;
}
pub enum Confirmation { None, Required { title: &'static str, message: &'static str }, Computed }
pub fn volume_step(current: f32, up: bool) -> f32; // 5% grid, snap toward
```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn volume_grid_snaps() {
    assert!((volume_step(0.37, true) - 0.40).abs() < 1e-6);
    assert!((volume_step(0.37, false) - 0.35).abs() < 1e-6);
}

#[test]
fn restart_confirms_pc_not_mac() {
    match SystemActionId::Restart.confirmation() {
        Confirmation::Required { title, .. } => assert!(title.contains("PC")),
        _ => panic!(),
    }
}

#[test]
fn stage_manager_still_exists() {
    assert!(SystemActionId::all().iter().any(|a| a.raw() == "toggle-stage-manager"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure volume_grid_snaps`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Copy IDs and names from v0.10.2 `SystemAction.swift`. Change only Mac→PC in those three titles.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure system_action volume`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add SystemAction catalog and volume grid"
```

---

### Task 2: Dialog + HUD surfaces, then SystemActionRunner + coordinator

**Files:**
- Create: `crates/tinycast-pure/src/dialog.rs`
- Create: `crates/tinycast/src/surfaces/dialog.rs`
- Create: `crates/tinycast/src/surfaces/hud.rs`
- Create: `crates/tinycast/src/features/system_actions/service/runner.rs`
- Create: `crates/tinycast/src/features/system_actions/ui/coordinator.rs`

**Interfaces:**
- `SystemActionCoordinator::run(id)` hides palette, confirms if needed, calls runner. Runner returns `Option<Feedback { message, noop: bool }>`. HUD success/neutral. Volume actions also show Volume HUD. `toggle-stage-manager` → HUD `"Stage Manager is not available on Windows."` without error dialog.

Map:
- lock → `LockWorkStation`
- sleep → `SetSuspendState`
- sleep displays → `WM_SYSCOMMAND SC_MONITORPOWER 2`
- restart/shutdown/logoff → `ExitWindowsEx` after confirm
- volume → WASAPI/mixer
- empty trash → empty Recycle Bin (`SHEmptyRecycleBin` after confirm)
- open trash → `shell:RecycleBinFolder`
- toggle appearance → AppsUseLightTheme / SystemUsesLightTheme
- hide others / quit all → previousApp
- bluetooth → WinRT radios
- show desktop → `COM IShellDispatch ToggleDesktop`
- dismiss notifications → WinRT Action Center if public; else HUD unavailable
- eject disks → `CM_Request_Device_Eject` removable only
- hidden files → Explorer `Hidden` value
- screensaver → `SC_SCREENSAVE`

Nothing-to-do (empty bin, no disks) is `.neutral` HUD.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn coordinator_cannot_skip_confirmation() {
    assert!(SystemActionId::EmptyTrash.confirmation() != Confirmation::None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure coordinator_cannot_skip` — assertion should PASS once catalog exists; add a unit test that a fake runner is not called when confirm is cancelled:

```rust
#[test]
fn cancel_does_not_run() {
    let mut ran = false;
    let confirm = false;
    if confirm { ran = true; }
    assert!(!ran);
}
```

Prefer testing a small `fn gated_run(confirm_ok: bool) -> bool { confirm_ok }` in the coordinator module with `#[cfg(test)]`.

- [ ] **Step 3: Write minimal implementation**

Implement runner + dialog. Publish `AppKind::SystemAction` entries from catalog.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS. Manual: Lock Screen, Empty Trash confirm, Stage Manager HUD.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: run system actions through a confirming coordinator"
```

---

### Task 3: WindowCommand catalog + WindowLayout

**Files:**
- Create: `crates/tinycast-pure/src/window_command.rs`
- Create: `crates/tinycast-pure/src/window_layout.rs`
- Create: `crates/tinycast-pure/src/window_action_memory.rs`

**Interfaces:**
Copy all 34 IDs and English names from v0.10.2 `WindowCommand.swift`.

```rust
pub fn placement(id: WindowCommandId, input: LayoutInput) -> Option<DipRect>;
pub struct LayoutInput {
    pub visible: DipRect, // work area
    pub window: DipRect,
    pub gap: f32,
    pub step: u8, // cycle 0..2
}
```

Gap rule: screen-edge full gap, interior half. Make Larger/Smaller: 5% of **screen**, invertible. Reasonable Size: 60% canvas, cap 1025×900, centered. Restore: `WindowActionMemory` rules 1–4 from window-management.md (2pt drift, cycleTimeout, halves only cycle).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn there_are_34_window_commands() {
    assert_eq!(WindowCommandId::all().len(), 34);
}

#[test]
fn left_half_uses_work_area_and_gap() {
    let vis = DipRect { x: 0.0, y: 0.0, w: 1000.0, h: 800.0 };
    let r = placement(WindowCommandId::LeftHalf, LayoutInput { visible: vis, window: vis, gap: 10.0, step: 0 }).unwrap();
    assert!((r.x - 10.0).abs() < 0.01);
    assert!(r.w <= 500.0);
}

#[test]
fn make_larger_then_smaller_round_trips_on_even_screen() {
    // 5% screen step, even dimensions
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure there_are_34`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Port geometry. Fuzz finite rects. Memory keyed by `u64` in tests, HWND in Effect.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure window_`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add WindowCommand catalog and pure layout math"
```

---

### Task 4: WindowMover + virtual desktops

**Files:**
- Create: `crates/tinycast/src/features/window_management/service/mover.rs`
- Create: `crates/tinycast/src/features/window_management/service/desktops.rs`
- Create: `crates/tinycast/src/features/window_management/ui/coordinator.rs`

**Interfaces:**
- Target is `previousApp` HWND, not Tinycast. Write sequence size → position → size. Position not settable → zero writes. Next/Previous Space → `IVirtualDesktopManager` / `IVirtualDesktopManagerInternal` **only if those COM interfaces are documented public**; otherwise Windows 11 Virtual Desktop WinRT. If unavailable, HUD. Do not synthesize trackpad swipes.

Coordinator re-checks `windowManagementEnabled`. Settings pane: enable, show in launcher, gap, cycle on repeat.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn space_commands_are_not_geometry() {
    assert_eq!(WindowCommandId::NextSpace.kind(), WindowKind::Space);
    assert_eq!(WindowCommandId::Restore.kind(), WindowKind::Restore);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure space_commands_are_not_geometry`

Expected: FAIL compile if kind missing.

- [ ] **Step 3: Write minimal implementation**

Mover + coordinator. Publish entries only when enabled.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS. Manual: Left Half on Notepad, mixed DPI if possible.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: apply window placements via SetWindowPos"
```

---

### Task 5: HotKeyManager, double-tap, Hyper

**Files:**
- Create: `crates/tinycast-pure/src/double_tap.rs`
- Create: `crates/tinycast/src/features/hotkeys/service/center.rs`
- Create: `crates/tinycast/src/features/hotkeys/service/double_tap_monitor.rs`
- Create: `crates/tinycast/src/features/hotkeys/service/hyper.rs`
- Create: `crates/tinycast/src/features/hotkeys/ui/recorder.rs`

**Interfaces:**

```rust
pub struct DoubleTapDetector { /* max_hold 250ms, max_gap 300ms */ }
impl DoubleTapDetector {
    pub fn on_flags(&mut self, modifier: DoubleTapModifier, down: bool, now_ms: u64) -> bool;
}
```

Fires on second **release**. Combo bindings: `RegisterHotKey` (MOD_NOREPEAT). Persistence JSON under `hotkey.<action>` using `HotKeyAction` string ids (`togglePalette`, `systemAction.lock-screen`, `windowCommand.left-half`, …). Conflict detection shared.

Hyper: Caps Lock → Scan Code Map to unused scancode (e.g. 0x6A), intercept as Ctrl+Alt+Win (+Shift if include). Clear map on disable and `WM_ENDSESSION` / process exit. Right-side modifiers as Hyper do not use Scan Code Map. ✦ keycap collapse when a Hyper key is configured.

Double-tap monitor: LL hook **only if some binding is DoubleTap**. Tail-append relative to Hyper rewrite: process Hyper first.

Recorder: not focusable; local key hook while `recordingAction` set; global engines paused.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn double_tap_fires_on_second_release() {
    let mut d = DoubleTapDetector::new();
    let m = DoubleTapModifier::Control;
    assert!(!d.on_flags(m, true, 0));
    assert!(!d.on_flags(m, false, 100));
    assert!(!d.on_flags(m, true, 200));
    assert!(d.on_flags(m, false, 300));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure double_tap_fires`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Detector + manager + Hyper remap with documented cleanup. Include Shift retargets stored combos (`retarget_hyper_bindings`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS. Manual: bind double-tap Ctrl to toggle palette; Caps Lock Hyper then Hyper+key fires a combo; quit restores Caps Lock.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add combo, double-tap, and Hyper key engines"
```

---

### Task 6: General + Permissions panes

**Files:**
- Modify: `crates/tinycast/src/surfaces/settings.rs`
- Create: `crates/tinycast/src/features/settings/panes/general.rs`
- Create: `crates/tinycast/src/features/settings/panes/permissions.rs`

**Interfaces:**
- General: Global Shortcuts (toggle palette recorder), Search (reset ranking), Hyper Key pickers, Appearance (Theme, Compact mode, Show favorites in compact, Follow the cursor, Drag to reposition), General (Launch at login, Show in menu bar, Pop to Root, Auto-switch input source).
- Permissions: rows for UI Automation, low-level hook, Calendar, Camera, each with Open Settings (Windows Settings URI). No prompt at launch.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn permissions_tab_is_under_general_section() {
    assert_eq!(SettingsSection::General.tabs()[1], SettingsTab::Permissions);
    assert_eq!(SettingsTab::Permissions.title(), "Permissions");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure permissions_tab_is_under_general`

Expected: PASS if titles exist; implement pane regardless.

- [ ] **Step 3: Write minimal implementation**

Fill both panes. `Show in menu bar` hides/shows the tray icon without quitting; hotkeys keep working.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast
git commit -m "feat: fill General and Permissions settings panes"
```
