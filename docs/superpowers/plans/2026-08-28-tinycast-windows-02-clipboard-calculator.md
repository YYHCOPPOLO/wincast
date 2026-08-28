# Clipboard and Calculator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Implement **every task in this plan file** first. **Do not review after each task.** When the last task is committed, review this plan's full commit range once. See `AGENTS.md`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clipboard history (SQLite FTS5, pins, type filter) and the inline calculator (math/units/FX/crypto) plus Calculator History screen, matching v0.10.2 user behavior.

**Architecture:** Calc engine is pure. Clipboard capture is Effect (`AddClipboardFormatListener`). Both publish into palette modes already enumerated in plan 00.

**Tech Stack:** `rusqlite` with `bundled` + `fts5`. WinHTTP for FX. `AddClipboardFormatListener`.

**Spec:** spec §8.2–8.3. Oracle: `docs/features/clipboard.md`, `docs/features/calculator.md`.

## Global Constraints

Plan index constraints apply, including **one review after this entire plan**, not after each task (`AGENTS.md`). Clipboard writes stamp a private format so the listener ignores Tinycast's own pastes. FTS trigram needs ≥ 3 characters. Memory window 1000 unpinned + all pins. Images on disk, thumbnails in RAM. Calculator card is selection index 0; never alongside the meeting card.

---

### Task 1: CalcEngine evaluate pipeline

**Files:**
- Create: `crates/tinycast-pure/src/calc/mod.rs`
- Create: `crates/tinycast-pure/src/calc/engine.rs`
- Create: `crates/tinycast-pure/src/calc/units.rs`
- Create: `crates/tinycast-pure/src/calc/currency.rs`
- Create: `crates/tinycast-pure/src/calc/datetime.rs`
- Create: `crates/tinycast-pure/src/calc/format.rs`

**Interfaces:**
- Consumes: injected `now: i64`, `calendar_tz: &str`, `rates: Option<&CurrencyRates>`, `region: Option<&str>`
- Produces:

```rust
pub struct CalcResult {
    pub expression: String,
    pub display: String,
    pub copy_text: String,
    pub source_badge: Option<String>,
    pub target_badge: Option<String>,
}
pub struct CurrencyRates { pub fetched_at: i64, pub units_per_base: Vec<(String, f64)> }
pub fn evaluate(
    query: &str,
    now: i64,
    rates: Option<&CurrencyRates>,
    region: Option<&str>,
) -> Option<CalcResult>;
```

Silent (None) unless the query is unambiguously calculator input. Errors only for impossible unit mixes.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn arithmetic_and_units() {
    let r = evaluate("2+2", 0, None, None).unwrap();
    assert_eq!(r.copy_text, "4");
    let r = evaluate("10km to mi", 0, None, None).unwrap();
    assert!(r.display.to_lowercase().contains("mi"));
}

#[test]
fn lone_integer_is_not_a_card() {
    assert!(evaluate("100000", 0, None, None).is_none());
}

#[test]
fn last_unit_typed_wins() {
    let r = evaluate("10kg + 500g", 0, None, None).unwrap();
    assert!(r.display.contains('g') || r.copy_text.contains("10500"));
}
```

Port more cases from v0.10.2 `calc-test` as they fail: `10k`, `1e5`, `4(2+3)`, trailing operator prefix `10+`, currency `$10 + €5` with injected rates.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure arithmetic_and_units`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Follow calculator.md pipeline order (datetime → numeric reject → tokenize → … → plain arithmetic). Do not fetch inside `evaluate`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure calc`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add CalcEngine with units and arithmetic"
```

---

### Task 2: CurrencyRateStore (Effect)

**Files:**
- Create: `crates/tinycast/src/features/calculator/service/rates.rs`
- Modify: `crates/tinycast/Cargo.toml` (no fat HTTP crate; WinHTTP helper in `platform/winhttp.rs`)

**Interfaces:**
- Consumes: `platform::winhttp::get_text(url) -> Result<String>`
- Produces: `CurrencyRateStore` loads `%LOCALAPPDATA%\com.tinycast.win\currency-rates.json`, refreshes 24h from `fetched_at`, cacheless. Fiat required; crypto best-effort. Partial snapshot not persisted. Hands `Option<CurrencyRates>` to `evaluate`.

Fiat URL constant: `https://api.frankfurter.app/latest`. Coin URL constant: `https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana&vs_currencies=usd` mapped onto BTC/ETH/SOL. Record both in `rates.rs`. Region currency from Windows `GetUserDefaultLocaleName` + ISO 4217 map.

- [ ] **Step 1: Write the failing test**

Pure merge:

```rust
#[test]
fn crypto_overwrites_fiat_on_same_code() {
    let rates = merge_feeds(&[("USD", 1.0), ("BTC", 999.0)], &[("BTC", 65000.0)]);
    assert_eq!(lookup(&rates, "BTC"), Some(65000.0));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure crypto_overwrites`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

`merge_feeds` in pure. Effect fetch in the exe. `AppCore.start` kicks a refresh if stale.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure crypto_overwrites`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add currency rate snapshot store"
```

---

### Task 3: Calculator card + Calculator History mode

**Files:**
- Create: `crates/tinycast-pure/src/calc/history.rs`
- Create: `crates/tinycast/src/features/calculator/ui/card.rs`
- Create: `crates/tinycast/src/features/calculator/ui/coordinator.rs`
- Modify: palette list

**Interfaces:**
- Consumes: `evaluate`, `selectable_count(card, rows)`
- Produces: when launcher (or history) query yields `Some(CalcResult)`, insert card at index 0. Enter copies `copy_text` and `CalculatorHistoryStore::push`. Command `command:calculator-history` → `PaletteMode::CalculatorHistory`. Tab ring unchanged.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn card_shifts_selection_count() {
    assert_eq!(selectable_count(true, 5), 6);
}
```

Already in plan 00; add:

```rust
#[test]
fn history_caps_and_newest_first() {
    let mut h = CalculatorHistoryStore::default();
    for i in 0..20 { h.push(format!("{i}")); }
    assert_eq!(h.items[0], "19");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure history_caps`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

JSON history in roaming. Card two-column D2D using `theme` calc typography.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure calc history`

Expected: PASS. Manual: type `2+2`, Enter copies 4.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: pin calculator card at selection 0 and record history"
```

---

### Task 4: ClipboardStore schema + FTS

**Files:**
- Create: `crates/tinycast/src/features/clipboard/service/store.rs`
- Modify: `crates/tinycast/Cargo.toml` add `rusqlite = { version = "0.32", features = ["bundled", "fts5"] }`

**Interfaces:**
- Produces: `%LOCALAPPDATA%\com.tinycast.win\clipboard.sqlite3` + `images\`. Schema: `items(id UNIQUE, created_at, pinned_at, kind TEXT CHECK IN ('text','image'), text, image_path)` + FTS5 trigram on text. Corrupt DB: delete and recreate.

```rust
pub struct ClipboardItem {
    pub id: i64,
    pub kind: ClipKind, // Text | Image
    pub text: Option<String>,
    pub image_path: Option<PathBuf>,
    pub pinned_at: Option<i64>,
}
impl ClipboardStore {
    pub fn open(dir: PathBuf) -> Self;
    pub fn insert_text(&mut self, text: String) -> i64;
    pub fn insert_image(&mut self, png: PathBuf) -> i64;
    pub fn search(&self, query: &str, filter: ClipboardFilter) -> Vec<ClipboardItem>;
    pub fn toggle_pin(&mut self, id: i64);
    pub fn promote(&mut self, id: i64); // skip if pinned
    pub fn clear(&mut self);
}
#[derive(Clone, Copy, PartialEq)]
pub enum ClipboardFilter { All, Text, Images, Links, Emails }
```

Search: `< 3 chars` filter in-memory window; else FTS LIMIT 200 then filter. Pins first (oldest pin at top). Unpin re-recencies via delete+insert.

- [ ] **Step 1: Write the failing test**

Use a temp dir:

```rust
#[test]
fn pins_lead_and_short_query_does_not_use_fts_requirement() {
    let dir = tempfile();
    let mut s = ClipboardStore::open(dir);
    let a = s.insert_text("alpha".into());
    let _b = s.insert_text("beta".into());
    s.toggle_pin(a);
    let rows = s.search("", ClipboardFilter::All);
    assert_eq!(rows[0].id, a);
}
```

Keep this test in `crates/tinycast` because it uses rusqlite.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast pins_lead`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

SQL as specified. TextForm classifier (plain/link/email) in `tinycast-pure` so it can be unit-tested without SQLite.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast clipboard`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure Cargo.toml
git commit -m "feat: add SQLite clipboard store with pins and FTS5"
```

---

### Task 5: Capture listener + paste + palette screen

**Files:**
- Create: `crates/tinycast/src/features/clipboard/service/manager.rs`
- Create: `crates/tinycast/src/features/clipboard/ui/screen.rs`
- Create: `crates/tinycast/src/platform/paster.rs`

**Interfaces:**
- `AddClipboardFormatListener(hwnd)`. On `WM_CLIPBOARDUPDATE`, skip if private marker present. Capture unicode text; for bitmaps write PNG off-thread then insert. Exclude processes named in `clipboardDisabledApps` (default password managers: `KeePass`, `1Password`, `Bitwarden`, `LastPass`, `Dashlane` — match exe stem).

Paste: write clipboard with marker, `Paster` targets `previousApp` via UIA `ValuePattern` or `SendInput` Ctrl+V. Command `command:clipboard-history` and Tab ring enter `PaletteMode::Clipboard`. Ctrl+P filter menu. Ctrl+. pin. Ctrl+1..0 address visible pinned/filter list. Header trailing filter button.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn classifier_url_is_link_not_text() {
    assert_eq!(text_form("https://example.com"), TextForm::Link);
    assert_eq!(text_form("not a url"), TextForm::Plain);
}
```

in `tinycast-pure/src/clipboard_text.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure classifier_url`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Classifier cheapest-first as clipboard.md (utf8 len, whitespace, scheme, email, bare domain). Then listener + screen: list width 290 DIP, preview pane, hairline separator.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS. Manual: copy text, Alt+Space, Tab to clipboard, Enter pastes into previous app.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: capture clipboard history and paste into previous app"
```

---

### Task 6: Clipboard settings pane

**Files:**
- Create: `crates/tinycast/src/features/clipboard/settings/pane.rs`

**Interfaces:**
- Retention days, disabled apps list, clear history. Keys `clipboardRetentionDays`, `clipboardDisabledApps`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn retention_key_is_upstream() {
    assert_eq!(AppSettingsKey::ClipboardRetention.as_str(), "clipboardRetentionDays");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure retention_key`

Expected: PASS if Task 5 plan 00 keys exist; otherwise FAIL.

- [ ] **Step 3: Write minimal implementation**

Wire the pane. Prune unpinned rows older than retention; never prune pins except Clear History.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast
git commit -m "feat: add clipboard settings pane and retention prune"
```

---

### Task 7: Tab ring (launcher ↔ clipboard)

**Files:**
- Create: `crates/tinycast-pure/src/palette_tab.rs`
- Modify: `crates/tinycast/src/palette/hwnd.rs`

**Interfaces:**

```rust
#[derive(Debug, PartialEq)]
pub enum TabHop { Clipboard, Launcher, Ai, StayForArguments }
pub fn tab_from(mode: PaletteMode, ai_enabled: bool, row_has_arguments: bool) -> TabHop;
```

While AI is off: Launcher → Clipboard → Launcher. Query text travels between launcher and clipboard.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn tab_ring_without_ai() {
    assert_eq!(tab_from(PaletteMode::Launcher, false, false), TabHop::Clipboard);
    assert_eq!(tab_from(PaletteMode::Clipboard, false, false), TabHop::Launcher);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure tab_ring_without_ai`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Implement `tab_from`. Do not draw the header `AI Chat` Tab hint while `ai_enabled` is false.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure palette_tab`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add launcher-clipboard Tab ring"
```
