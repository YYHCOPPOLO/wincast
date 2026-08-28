# File Search, Notes, Calendar, Uninstall, Backup, Onboarding, Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Implement **every task in this plan file** first. **Do not review after each task.** When the last task is committed, review this plan's full commit range once. See `AGENTS.md`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remaining non-AI product surfaces from v0.10.2: Search Files, Notes window, Calendar, Uninstall, settings backup + Raycast import, Onboarding, Support.

**Architecture:** Query/ignore/uninstall/meeting policies are pure. Windows Search, WinRT appointments, Recycle Bin, file pickers are Effect.

**Tech Stack:** Windows Search OLE DB (`search.collatorDatalist` / `SystemIndex`) with a 1000-hit cap; fallback walk with the same cap. Notes: child RichEdit/EDIT. Calendar: WinRT `Appointments`. Uninstall: ARP + AppX + leftover dirs → Recycle Bin.

**Spec:** spec §8.8–8.11, §8.14, §8.16. Oracle: `file-search.md`, `notes.md`, `calendar.md`, `uninstall.md`, `backup.md`, `raycast-import.md`, `support.md`.

## Global Constraints

Index constraints apply, including **one review after this entire plan**, not after each task (`AGENTS.md`). File Search and Calendar default **off**. Uninstall never permanently deletes. `calendarEnabled`, `autoJoinMeetings`, `cameraPreview` excluded from backups. Home is not an uninstall root.

---

### Task 1: FileSearch query + ignore list

**Files:**
- Create: `crates/tinycast-pure/src/file_search/query.rs`
- Create: `crates/tinycast-pure/src/file_search/ignore.rs`
- Create: `crates/tinycast/src/features/file_search/service/session.rs`

**Interfaces:**

```rust
pub fn tokens(q: &str) -> Vec<String>;
pub fn cap_candidates(n: usize) -> usize { n.min(1000) }
pub fn cap_rows(n: usize) -> usize { n.min(200) }
pub struct IgnoreList { /* defaults compiled in + user patterns */ }
impl IgnoreList {
    pub fn drops(&self, path: &str) -> bool;
}
```

AND all tokens against filename. Empty query does no work. Hidden path components and install-package internals always dropped.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn caps_and_and_tokens() {
    assert_eq!(cap_candidates(5000), 1000);
    assert_eq!(tokens("annual report"), vec!["annual", "report"]);
}

#[test]
fn default_ignore_drops_node_modules() {
    let i = IgnoreList::defaults();
    assert!(i.drops("C:/src/foo/node_modules/x"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure caps_and_and_tokens`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

OLE DB query on `SystemIndex` with `SCOPE` of configured folders, `filename LIKE`. If the catalog is off, walk with the same caps. Debounce 120ms. Command `command:search-files` guarded by `fileSearchEnabled`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure file_search`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add filename file search with 1000/200 caps"
```

---

### Task 2: Notes window

**Files:**
- Create: `crates/tinycast/src/features/notes/service/store.rs`
- Create: `crates/tinycast/src/surfaces/notes.rs`

**Interfaces:**
- Markdown files under roaming `Notes\`. Commands show / create / search notes. Titled HWND, user-sized, autosave placement (`"Notes Window"`). Host RichEdit; displayed string **is** the file. No preview renderer. Switcher popover 300×240. `notesEnabled` default false.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn note_identity_is_path() {
    let n = Note { path: PathBuf::from("a.md"), body: "x".into() };
    assert_eq!(n.id(), "a.md");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure note_identity` — put `Note` in `tinycast-pure/src/note.rs`.

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Store + HWND. Search Notes filters filenames. Escape closes switcher then hides.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add Notes window with Markdown source editor"
```

---

### Task 3: MeetingLink + UpcomingWindow + Calendar store

**Files:**
- Create: `crates/tinycast-pure/src/meeting/link.rs`
- Create: `crates/tinycast-pure/src/meeting/window.rs`
- Create: `crates/tinycast/src/features/calendar/service/store.rs`
- Create: `crates/tinycast/src/features/calendar/ui/coordinator.rs`

**Interfaces:**

```rust
pub fn detect_link(fields: &[&str]) -> Option<MeetingLink>; // url, location, notes order; named provider beats earlier generic
pub struct UpcomingWindow;
impl UpcomingWindow {
    pub fn agenda(events: &[MeetingEvent], now: i64) -> Vec<MeetingEvent>;
    pub fn carded(events: &[MeetingEvent], now: i64, lead_secs: i64) -> Option<MeetingEvent>;
    pub fn joinable(...) -> Option<MeetingEvent>;
}
```

WinRT Appointments: fetch `[startOfToday, endOfTomorrow+1day)`. `calendarEnabled` consent dialog then OS prompt. Card only on **empty** launcher query. Five CommandIDs. Auto join once per meeting per launch. Camera preview optional via MediaCapture; deny is not fatal. Tray tooltip uses `MenuBarSummary`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn named_provider_beats_earlier_generic() {
    let l = detect_link(&["https://example.com/reset", "https://meet.google.com/abc-defg-hij"]).unwrap();
    assert_eq!(l.provider, Provider::GoogleMeet);
}

#[test]
fn card_window_does_not_outlive_short_meeting() {
    // lead 5min, meeting 2min long → window ends at end
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure named_provider_beats`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Port provider table (Zoom rewrite to `zoommtg:`, Teams to `msteams:`). Open https fallback.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure meeting`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add calendar agenda, join card, and meeting link detection"
```

---

### Task 4: Uninstall rules + scanner + runner

**Files:**
- Create: `crates/tinycast-pure/src/uninstall/rules.rs`
- Create: `crates/tinycast-pure/src/uninstall/plan.rs`
- Create: `crates/tinycast/src/features/uninstall/service/scanner.rs`
- Create: `crates/tinycast/src/features/uninstall/service/runner.rs`

**Interfaces:**
- `UninstallIdentity::make` returns None for running `com.tinycast.win`. Match styles: product code / AppX name / exact displayName (≥3, not shared) / install-dir leftovers. Roots: ARP uninstall strings, AppX, `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` immediate children. Home directory is not a root. Discover then measure. Locked rows cannot be checked. Runner: `SHFileOperation` / `IFileOperation` to Recycle Bin only. Bundle last. Confirm in coordinator. Only clear launcher prefs if the app itself went.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn refuses_self() {
    assert!(UninstallIdentity::make("com.tinycast.win", running_id="com.tinycast.win").is_none());
}

#[test]
fn prefix_bundle_does_not_eat_sibling() {
    // com.foo.bar must not claim com.foo.bar.beta
}

#[test]
fn locked_cannot_enter_selection() {
    let mut sel = UninstallSelection::new();
    sel.toggle("locked");
    sel.intersect_removable(&["a"]);
    assert!(!sel.contains("locked"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure refuses_self`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Port matcher ideas to Windows roots. Screen: `PaletteMode::Uninstall`, destructive primary pill.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure uninstall`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add uninstall plan that only recycles"
```

---

### Task 5: SettingsBackupCoverage + export/import + Raycast import

**Files:**
- Create: `crates/tinycast-pure/src/settings_backup.rs`
- Create: `crates/tinycast/src/features/backup/service/actions.rs`
- Create: `crates/tinycast/src/features/backup/service/raycast.rs`

**Interfaces:**
Every `AppSettingsKey` is in exactly one of `mirrored`, `externally_sourced`, `deliberately_excluded` — copy the v0.10.2 tables (including reasons). Export JSON. Import writes through `AppSettings` and **must not** set excluded keys. Report a summary. Raycast `.rayconfig` v1/v2: if decrypt fails, say wrong passphrase, not bad format. Snippet import never enables `snippetsEnabled`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn every_key_is_classified_once() {
    let mut seen = HashSet::new();
    for k in AppSettingsKey::ALL {
        let n = coverage_bucket(k);
        assert!(seen.insert(k), "duplicate {k:?}");
        assert!(n == Bucket::Mirrored || n == Bucket::External || n == Bucket::Excluded);
    }
    assert!(coverage_bucket(AppSettingsKey::SnippetsEnabled) == Bucket::Excluded);
    assert!(coverage_bucket(AppSettingsKey::AiEnabled) == Bucket::Excluded);
}

#[test]
fn import_cannot_enable_snippets() {
    let mut s = AppSettings::defaults();
    apply_backup(&mut s, json!({"snippetsEnabled": true}));
    assert!(!s.snippets_enabled);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure every_key_is_classified_once`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Coverage tables + apply_backup that drops excluded keys. File pickers in Effect.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure settings_backup`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add settings backup coverage and Raycast import entry"
```

---

### Task 6: Onboarding, Support, dialogs, About

**Files:**
- Create: `crates/tinycast/src/surfaces/onboarding.rs`
- Create: `crates/tinycast/src/surfaces/support.rs`
**Interfaces:**
- First launch: onboarding with `ShortcutRecorder` for `togglePalette`. Support window: every route (`showSupport`) from menu circle, Settings About, launcher Support command, 30-day reminder. Reuse dialog/HUD from plan 04. No `MessageBox`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn dialog_cancel_is_leading_policy() {
    let buttons = ["Cancel", "Restart"];
    assert_eq!(buttons[0], "Cancel");
}
```

Replace with a real helper:

```rust
pub fn ordered_actions(primary: &str, destructive: bool) -> Vec<DialogAction> {
    vec![DialogAction { label: "Cancel", role: Role::Cancel }, DialogAction { label: primary, role: if destructive { Role::Destructive } else { Role::Standard } }]
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure ordered_actions`

Put `DialogAction` in `tinycast-pure/src/dialog.rs`.

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Surfaces + reminder schedule (pure `SupportReminderSchedule`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add onboarding, support, dialogs, and HUDs"
```
