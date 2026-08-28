# Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The palette lists and launches Windows apps (and built-in command rows) with v0.10.2 fuzzy bands, frecency, favorites, aliases, and visibility.

**Architecture:** Scoring and catalogs live in `tinycast-pure`. `AppIndex` scans Start Menu / AppX / App Paths off the UI thread and publishes `AppEntry` values to `AppCore`. The palette launcher screen renders rows; activation goes through `LauncherCoordinator`.

**Tech Stack:** Same as plan 00. Shell: `IShellItemImageFactory`, `ShellExecuteExW`, AppX `PackageManager` via `windows` WinRT features as needed.

**Spec:** `docs/superpowers/specs/2026-08-28-tinycast-windows-design.md` §6, §8.1. Oracle: Tinycast v0.10.2 `docs/features/launcher.md`.

## Global Constraints

Plan index Global Constraints apply. Band arithmetic is a contract, not tuning:

```
FuzzyMatch::MAXIMUM_SCORE = 100_000
BAND_STRIDE = 10 * MAXIMUM_SCORE     // 1_000_000
LauncherRankingStore::MAXIMUM_BOOST = 4_500
```

Empty-query section order: Favorites (optional) → Applications → System Settings → Quicklinks → Snippets → System Actions → Window Management → Custom Commands → Commands.

This plan implements Applications, System Settings, Commands, Favorites, aliases, visibility. Other sections publish empty slices until later plans.

---

### Task 1: FuzzyMatch

**Files:**
- Create: `crates/tinycast-pure/src/search_relevance.rs` (module `fuzzy`)
- Modify: `crates/tinycast-pure/src/lib.rs`

**Interfaces:**
- Consumes: nothing
- Produces:

```rust
pub struct FuzzyMatch;
impl FuzzyMatch {
    pub const MAXIMUM_SCORE: i32 = 100_000;
    /// Returns None if no match. Higher is better. Exact > prefix > word-start/substring > subsequence.
    pub fn score(query: &str, target: &str) -> Option<i32>;
}
```

Strip invisible Unicode format scalars from both strings before matching. Case-fold with Unicode lowercase.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn exact_beats_prefix_beats_subsequence() {
    let q = "code";
    let exact = FuzzyMatch::score(q, "code").unwrap();
    let prefix = FuzzyMatch::score(q, "codeberg").unwrap();
    let sub = FuzzyMatch::score(q, "xcxoxdxe").unwrap();
    assert!(exact > prefix && prefix > sub);
    assert!(exact <= FuzzyMatch::MAXIMUM_SCORE);
}

#[test]
fn no_match_is_none() {
    assert_eq!(FuzzyMatch::score("zzz", "abc"), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure exact_beats_prefix`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Implement a tiered scorer: exact (MAXIMUM_SCORE), prefix, substring/word-start, subsequence with consecutive/word-boundary bonuses. Keep all scores `< MAXIMUM_SCORE` except exact.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure fuzzy`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add FuzzyMatch scorer"
```

---

### Task 2: SearchRelevance bands

**Files:**
- Modify: `crates/tinycast-pure/src/search_relevance.rs`

**Interfaces:**
- Consumes: `FuzzyMatch`
- Produces:

```rust
#[derive(Clone, Debug, Default)]
pub struct SearchFields {
    pub user_alias: Option<String>,
    pub display_name: String,
    pub snippet_keyword: Option<String>,
    pub alternate_names: Vec<String>,
    pub bundle_id: Option<String>,
    pub executable_name: Option<String>,
}

#[repr(i32)]
pub enum Band { Alias = 6, DisplayLiteral = 5, AlternateLiteral = 4, DisplaySub = 3, AlternateSub = 2, Bundle = 1, Exe = 0 }

pub const BAND_STRIDE: i32 = 10 * FuzzyMatch::MAXIMUM_SCORE;

pub fn score(query: &str, fields: &SearchFields) -> Option<i32>;
```

Rules from v0.10.2: alias scores only exact/prefix (from start). Bundle id and exe never subsequence. Bundle id also matches with leading DNS component stripped (`apple.Photos` from `com.apple.Photos`); query `com` must not prefix-match everything. Identifier exact still works on the full id.

- [ ] **Step 1: Write the failing test**

```rust
fn fields(name: &str) -> SearchFields {
    SearchFields { display_name: name.into(), ..Default::default() }
}

#[test]
fn band_stride_dwarfs_fuzzy_and_boost() {
    assert_eq!(BAND_STRIDE, 1_000_000);
    assert!(FuzzyMatch::MAXIMUM_SCORE * 10 == BAND_STRIDE);
    assert!(4_500 < BAND_STRIDE / 100);
}

#[test]
fn alias_prefix_beats_display_exact() {
    let mut f = fields("Terminal");
    f.user_alias = Some("iterm".into());
    let alias = score("ite", &f).unwrap();
    let display = score("Terminal", &fields("Terminal")).unwrap();
    assert!(alias > display);
}

#[test]
fn bundle_id_does_not_subsequence() {
    let mut f = fields("Photos");
    f.bundle_id = Some("com.apple.Photos".into());
    assert_eq!(score("cop", &f), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure band_stride_dwarfs`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

`score` evaluates each field independently, adds `band as i32 * BAND_STRIDE`, returns the max.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure search_relevance`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add SearchRelevance field bands"
```

---

### Task 3: LauncherRankingStore

**Files:**
- Create: `crates/tinycast-pure/src/launcher_ranking.rs`

**Interfaces:**
- Consumes: injected `now: i64` unix seconds, `file_url: PathBuf`
- Produces:

```rust
pub struct LauncherRankingStore { /* private map */ }
impl LauncherRankingStore {
    pub const MAXIMUM_BOOST: i32 = 4_500;
    pub fn load(path: PathBuf) -> Self;
    pub fn record(&mut self, query: &str, entry_id: &str, now: i64);
    pub fn boost(&self, query: &str, entry_id: &str, now: i64) -> i32; // 0..=MAXIMUM_BOOST
    pub fn reset_all(&mut self);
    pub fn save(&self) -> std::io::Result<()>;
}
```

Recording a launch stores **every prefix** of the submitted query (`wha` → `w`, `wh`, `wha`). Empty-query favorites and hotkeys must not call `record`. Boost may reorder inside a band, never across (already guaranteed by magnitude).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn prefixes_are_recorded_and_boost_is_capped() {
    let dir = std::env::temp_dir().join("tc-rank-test.json");
    let _ = std::fs::remove_file(&dir);
    let mut s = LauncherRankingStore::load(dir);
    s.record("wha", "app:whatsapp", 1_000);
    assert!(s.boost("w", "app:whatsapp", 1_000) > 0);
    assert!(s.boost("wha", "app:whatsapp", 1_000) <= LauncherRankingStore::MAXIMUM_BOOST);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure prefixes_are_recorded`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

JSON file `launcher-ranking.json`. Frequency + exponential recency. Cap at MAXIMUM_BOOST.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure launcher_ranking`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add LauncherRankingStore with prefix learning"
```

---

### Task 4: AppEntry, Kind, CommandID

**Files:**
- Create: `crates/tinycast-pure/src/app_entry.rs`
- Create: `crates/tinycast-pure/src/command_id.rs`

**Interfaces:**
- Consumes: `SearchFields`
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AppKind {
    Application, SystemSettings, Quicklink, Snippet, SystemAction,
    WindowCommand, CustomCommand, Command, Favorite, // Favorite is a pin, not a Kind
}

#[derive(Clone, Debug)]
pub struct AppEntry {
    pub id: String,
    pub kind: AppKind,
    pub name: String,
    pub fields: SearchFields,
    pub hotkey: Option<crate::hotkey::HotKeyBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CommandID { /* all v0.10.2 cases including raw ids command:ai-chat ... */ }
impl CommandID {
    pub fn all() -> &'static [CommandID];
    pub fn raw(self) -> &'static str;   // "command:ai-chat"
    pub fn name(self) -> &'static str;  // "AI Chat"
    pub fn as_entry(self) -> AppEntry;
}
```

Do not use a numeric Kind. Category search: `AppKind::named_by(query)` exact-equals section title or singular (`Snippets`/`Snippet`).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn command_ids_match_upstream_names() {
    assert_eq!(CommandID::AiChat.raw(), "command:ai-chat");
    assert_eq!(CommandID::AiChat.name(), "AI Chat");
    assert_eq!(CommandID::SearchEmoji.name(), "Search Emoji & Symbols");
    assert_eq!(CommandID::Quit.name(), "Quit Tinycast");
}

#[test]
fn kind_named_by_is_exact() {
    assert_eq!(AppKind::named_by("Snippets"), Some(AppKind::Snippet));
    assert_eq!(AppKind::named_by("snip"), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure command_ids_match`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Copy every `CommandID` case and `name` from v0.10.2 `CommandID.swift`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure command_id app_entry`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add AppEntry, AppKind, and CommandID catalog"
```

---

### Task 5: VisibilityStore + FavoritesStore + AliasStore

**Files:**
- Create: `crates/tinycast-pure/src/visibility.rs`
- Create: `crates/tinycast-pure/src/favorites.rs`
- Create: `crates/tinycast-pure/src/alias.rs`

**Interfaces:**
- Consumes: `AppKind`, entry `preference_key: &str` (use `AppEntry.id`)
- Produces:

```rust
pub struct VisibilityStore { /* kind enabled + per-id hidden */ }
impl VisibilityStore {
    pub fn is_kind_enabled(&self, k: AppKind) -> bool;
    pub fn set_kind_enabled(&mut self, k: AppKind, on: bool);
    pub fn is_item_visible(&self, id: &str) -> bool;
    pub fn allows_hotkey(&self, kind: AppKind) -> bool; // false if kind disabled
}

pub struct FavoritesStore {
    pub ids: Vec<String>, // order = Ctrl+1..0
}
impl FavoritesStore {
    pub fn slot(&self, n: u8) -> Option<&str>; // 1..=9, 0 => 10th
    pub fn toggle(&mut self, id: String);
}

pub struct AliasStore;
impl AliasStore {
    pub fn get(&self, id: &str) -> Option<&str>;
    pub fn set(&mut self, id: String, alias: Option<String>);
}
```

Kind master switch off ⇒ rows and hotkeys off. Per-item hide ⇒ row off, hotkey still on.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn kind_switch_blocks_hotkeys_item_hide_does_not() {
    let mut v = VisibilityStore::default();
    v.set_kind_enabled(AppKind::Application, false);
    assert!(!v.allows_hotkey(AppKind::Application));
    v.set_kind_enabled(AppKind::Application, true);
    v.hide_item("app:x");
    assert!(v.allows_hotkey(AppKind::Application));
    assert!(!v.is_item_visible("app:x"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure kind_switch_blocks`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

In-memory + serde JSON later in AppSettings files `visibility.json` etc. under roaming. For this task, in-memory + `save/load` on PathBuf is enough.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure visibility favorites alias`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add visibility, favorites, and alias stores"
```

---

### Task 6: Ordered results (pure)

**Files:**
- Create: `crates/tinycast-pure/src/launcher_results.rs`

**Interfaces:**
- Consumes: entries, query, ranking, visibility, favorites
- Produces:

```rust
pub struct LauncherSection { pub kind: AppKind, pub rows: Vec<AppEntry> }

pub fn ordered_results(
    entries: &[AppEntry],
    query: &str,
    now: i64,
    ranking: &LauncherRankingStore,
    visibility: &VisibilityStore,
    favorites: &FavoritesStore,
    show_sections: bool,
) -> Vec<LauncherSection>;
```

Empty query: `show_sections=true`, favorites prefix (if any), then kind order. Non-empty: score + boost, drop None scores, do not show empty-query-only cards. Category exact name: list that kind unsorted (index order), no `record` on activate (flag `category_listing: bool` on a wrapper if needed).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn empty_query_section_order() {
    // two apps, one command
    let sections = ordered_results(&entries, "", 0, &rank, &vis, &fav, true);
    let kinds: Vec<_> = sections.iter().map(|s| s.kind).collect();
    assert_eq!(&kinds[..2], &[AppKind::Application, AppKind::Command]);
}

#[test]
fn weaker_band_cannot_outrank_with_boost() {
    let display_exact = score("Code", &SearchFields { display_name: "Code".into(), ..Default::default() }).unwrap();
    let mut alt = SearchFields { display_name: "Other".into(), alternate_names: vec!["codeberg helper".into()], ..Default::default() };
    let alt_score = score("code", &alt).unwrap() + LauncherRankingStore::MAXIMUM_BOOST;
    assert!(display_exact > alt_score);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure empty_query_section_order`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Filter visibility first. Apply score+boost. Sort by (score+boost desc, name).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure launcher_results`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add ordered launcher results"
```

---

### Task 7: AppIndex scan (Effect)

**Files:**
- Create: `crates/tinycast/src/features/launcher/service/app_index.rs`
- Create: `crates/tinycast/src/features/launcher/service/search_scopes.rs`
- Modify: `crates/tinycast/src/app_core.rs`

**Interfaces:**
- Consumes: `SearchScopes` defaults, thread pool, `WM` publish
- Produces: `AppIndex::start` scans:
  1. `%ProgramData%\Microsoft\Windows\Start Menu\Programs` and `%APPDATA%\Microsoft\Windows\Start Menu\Programs` `.lnk` (one folder + one nested folder, `.lnk` is a leaf)
  2. AppX via `Windows.Management.Deployment.PackageManager` (or `powershell`-free WinRT)
  3. `HKCU/HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths`
  Dedup by resolved target / AUMID. Alternate names: lnk arguments, AppX display name, VERSIONINFO `FileDescription` if different from display name.

Default scopes include those roots plus `%ProgramFiles%`, `%ProgramFiles(x86)%`. User-editable later in General; persist as `launcherSearchScopes`.

Off-main scan; publish Vec<AppEntry> to UI thread. Overlapping scans collapse to the latest.

- [ ] **Step 1: Write the failing test**

Pure scope walker:

```rust
#[test]
fn walk_is_one_level_and_treats_lnk_as_leaf() {
    // temp dir: a.lnk, Vendor/b.lnk, Vendor/Nested/c.lnk
    let found = collect_lnk_paths(&root);
    assert!(found.iter().any(|p| p.ends_with("a.lnk")));
    assert!(found.iter().any(|p| p.ends_with("b.lnk")));
    assert!(!found.iter().any(|p| p.ends_with("c.lnk")));
}
```

Put walker in `tinycast-pure/src/search_scopes.rs` taking directory listings as injected names to stay filesystem-free, **or** integration test with temp dirs in the exe crate. Prefer injected names:

```rust
pub fn visible_leaves(root_children: &[DirEnt]) -> Vec<String>;
pub struct DirEnt { pub name: String, pub is_dir: bool, pub nested: Vec<DirEnt> }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure walk_is_one_level`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Walker + Effect scan filling `AppEntry { kind: Application, id: "app:"+aumid_or_path, ... }`. Also publish System Settings entries as `ms-settings:` URIs (Display, Bluetooth, Apps, Power, Time, Update, Personalization, Privacy, Network, Gaming, Accessibility, Accounts, System) with kind `SystemSettings`.

On `AppCore.start()`, begin a scan. When complete, store `entries: Vec<AppEntry>`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure walk_is_one_level`

Expected: PASS. Manual: `cargo run`, type `not` and see Notepad or Settings.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: scan Start Menu, AppX, and App Paths into AppIndex"
```

---

### Task 8: Launcher screen + activate

**Files:**
- Create: `crates/tinycast/src/features/launcher/ui/list.rs`
- Create: `crates/tinycast/src/features/launcher/ui/coordinator.rs`
- Modify: `crates/tinycast/src/palette/hwnd.rs`

**Interfaces:**
- Consumes: `ordered_results`, D2D row metrics (`theme::size::ROW_ICON` 24, radius 10)
- Produces: `LauncherCoordinator::activate(entry, query)`:
  - Application → `ShellExecuteExW` / `IApplicationActivationManager` for AUMID, then hide palette (no focus restore needed)
  - SystemSettings → `ShellExecute` the `ms-settings:` URI
  - Command `command:quit` → `WM_QUIT_APP`
  - Command `command:settings` → `WM_OPEN_SETTINGS`
  - Command `command:about` / `support` → stub HWND until plan 05
  - Other commands: no-op until their plan (do not crash)
  - `ranking.record(query, id, now)` unless category listing or empty query favorite slot

Rows: icon 24, title, optional alias chip, kind trailing label, keycap if hotkey bound. Headers not selectable. Selection follows `PaletteState.selection`. Enter activates. Ctrl+1..0 activate favorite slots (physical number row).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn empty_query_does_not_record_ranking_on_activate_policy() {
    assert!(!should_record_ranking("", false));
    assert!(should_record_ranking("not", false));
    assert!(!should_record_ranking("Applications", true));
}
pub fn should_record_ranking(query: &str, category_listing: bool) -> bool {
    !query.is_empty() && !category_listing
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure should_record_ranking` — put fn in `launcher_ranking.rs`.

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

D2D list with edge dissolve using theme fade bands. Icons via `IShellItemImageFactory`; cache key path+mtime+dpi+appearance; **drop cache when panel hides**.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure`

Expected: PASS.

Manual: fuzzy find an app, Enter launches, panel hides. Ctrl+K not required yet except hide/show. Favorites via Actions later; for now Settings Applications pane can wait Task 9.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: render launcher rows and launch selected apps"
```

---

### Task 9: Settings panes — Applications, System Settings, Commands

**Files:**
- Create: `crates/tinycast/src/features/launcher/settings/items.rs` (`LauncherItemsSection`)
- Modify: `crates/tinycast/src/surfaces/settings.rs`

**Interfaces:**
- Consumes: `VisibilityStore`, `AliasStore`, `HotKeyBinding` recorder stub
- Produces: three panes sharing one row widget: enable-kind toggle, per-item checkbox, alias field, shortcut recorder (record combo via local WM_KEYDOWN while global hotkeys paused). Commands pane lists `CommandID::all()` with visibility.

General pane Search section: Reset learned ranking button (destructive confirm).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn commands_pane_lists_every_command_id() {
    assert!(CommandID::all().len() >= 30);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure commands_pane_lists`

Expected: FAIL if catalog incomplete.

- [ ] **Step 3: Write minimal implementation**

Fill the three launcher tabs. Feature-gated commands still listed in Commands settings (original: File Search row remains in the pane when the feature is off). For now show all CommandIDs; hiding from launcher follows enable flags defaulting to on for Commands, off for later features.

Default `fileSearchEnabled`, `notesEnabled`, `snippetsEnabled`, `windowManagementEnabled`, `calendarEnabled`, `aiEnabled`, `quickActionsEnabled`, `extensionsEnabled` = **false**. Built-in always-on: Settings, About, Support, Quit, Clipboard History, Calculator History, Search Emoji (emoji can wait plan 03 — still list the command, activating no-ops).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add launcher settings panes with visibility and aliases"
```

---

### Task 10: Ctrl+K Actions menu

**Files:**
- Create: `crates/tinycast-pure/src/palette_menu.rs`
- Create: `crates/tinycast/src/palette/menu.rs`

**Interfaces:**

```rust
pub struct MenuItem { pub id: &'static str, pub label: &'static str, pub shortcut: Option<&'static str> }
pub fn launcher_actions(kind: AppKind) -> Vec<MenuItem>;
```

One open menu at a time. Ctrl+K toggles Actions (`.bottomTrailing`). Escape closes menu first. Default items: Show in Folder (apps), Copy Path, Toggle Favorite, Uninstall Application (apps only — handler may no-op until plan 05). Footer Actions capsule labeled `Ctrl+K`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn app_actions_include_uninstall_and_favorite() {
    let labels: Vec<_> = launcher_actions(AppKind::Application).iter().map(|i| i.label).collect();
    assert!(labels.contains(&"Uninstall Application"));
    assert!(labels.contains(&"Toggle Favorite") || labels.iter().any(|l| l.contains("Favorite")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure app_actions_include`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Popover D2D, 276 DIP wide, glass. Search field does not resign first responder while menu open (input frozen).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure palette_menu`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add Ctrl+K Actions menu"
```

---

## Plan 01 done when

Typing in the panel fuzzy-finds Start Menu/AppX apps, Enter launches them, favorites/aliases/visibility persist, Settings Applications/Commands panes work, Ctrl+K Actions works, `cargo test` passes.
