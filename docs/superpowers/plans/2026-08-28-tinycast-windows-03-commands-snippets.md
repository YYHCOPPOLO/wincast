# Custom Commands, Quicklinks, Emoji, Snippets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** User-authored custom commands and quicklinks, emoji grid, and snippets (launcher expansion then keyword expansion) matching v0.10.2.

**Architecture:** Template engine is shared and pure. Keyword listener is Effect and installs a WH_KEYBOARD_LL hook **only while snippets are enabled**. Confirmation gates live in coordinators.

**Tech Stack:** `unicode-segmentation` for `{cursor}` offsets. UIA + `SendInput` for insertion. Markdown snippet files under `%APPDATA%\com.tinycast.win\Snippets\`.

**Spec:** spec §8.4–8.5. Oracle: `custom-commands.md`, `quicklinks.md`, `emoji.md`, `snippets.md`.

## Global Constraints

Index constraints apply. `snippetsEnabled` is consent for keystroke listening and is excluded from backups. Enabling snippets: explain dialog first, then request UI Automation / accessibility-equivalent, then install the hook. Disable tears down hook, store, watchers; files remain.

---

### Task 1: Shared template engine

**Files:**
- Create: `crates/tinycast-pure/src/template.rs`
- Create: `crates/tinycast-pure/src/template_tokens.rs`

**Interfaces:**
- Consumes: injected context

```rust
pub struct ExpandContext {
    pub clipboard: Vec<String>, // 0 = current
    pub selection: Option<String>,
    pub now: i64,
    pub locale: String,
    pub tz: String,
    pub uuid: fn() -> String,
}
pub struct ExpandOutput {
    pub text: String,
    pub cursor: Option<usize>, // grapheme offset
    pub arguments: Vec<ArgumentSpec>,
}
pub fn expand(input: &str, ctx: &ExpandContext, args: &HashMap<String, String>) -> ExpandOutput;
```

Tokens: `{clipboard}`, `{clipboard offset=N}`, `{selection}`/`{selectedText}`, `{date}` `{time}` `{datetime}` `{day}` `{uuid}`, `{date format=...}`, `{date locale=...}` (not with format), `{time offset=...}`, `{argument}` / name / default / options, `{snippet:Name}`, `{cursor}`. Modifiers: uppercase, lowercase, trim, percent-encode, json-stringify, raw. Unknown tokens left verbatim. Nested snippets depth 5, cycle by path identity.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn clipboard_and_cursor_and_unknown_left_in_place() {
    let ctx = ExpandContext { clipboard: vec!["Hi".into()], selection: None, now: 0, locale: "en".into(), tz: "UTC".into(), uuid: || "u".into() };
    let o = expand("X{clipboard}{cursor}Y{nope}", &ctx, &Default::default());
    assert_eq!(o.text, "XHiY{nope}");
    assert_eq!(o.cursor, Some(3)); // graphemes: X H i
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure clipboard_and_cursor`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Parse tokens; apply modifiers LTR; compute cursor with `unicode_segmentation::UnicodeSegmentation`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure template`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure crates/tinycast-pure/Cargo.toml
git commit -m "feat: add shared snippet/quicklink template engine"
```

Add `unicode-segmentation = "1"` to `tinycast-pure` Cargo.toml in Step 3.

---

### Task 2: Snippet Markdown codec + repository

**Files:**
- Create: `crates/tinycast-pure/src/snippet/codec.rs`
- Create: `crates/tinycast-pure/src/snippet/model.rs`
- Create: `crates/tinycast/src/features/snippets/service/repository.rs`

**Interfaces:**

```rust
pub struct StoredSnippet {
    pub path: PathBuf, // identity
    pub name: String,
    pub keyword: Option<String>,
    pub enabled: bool,
    pub show_confirmation: bool,
    pub body: String,
}
pub fn parse_markdown(path: &Path, bytes: &str) -> Result<StoredSnippet, CodecError>;
pub fn serialize(s: &StoredSnippet) -> String; // canonical key order
```

Frontmatter optional. Keys case-insensitive, quoted strings, booleans `true`/`false` only.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn round_trip_canonical_frontmatter() {
    let raw = "---\nname: \"Meeting Notes\"\nkeyword: \"!notes\"\nenabled: true\nshow_confirmation: false\n---\n\nHello {clipboard}\n";
    let s = parse_markdown(Path::new("Meeting Notes.md"), raw).unwrap();
    assert_eq!(s.name, "Meeting Notes");
    assert_eq!(s.keyword.as_deref(), Some("!notes"));
    assert!(serialize(&s).starts_with("---\nname:"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure round_trip_canonical`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Codec + repository load/save/watch (exe crate). Empty library on first enable.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure snippet`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure crates/tinycast
git commit -m "feat: add snippet Markdown codec and repository"
```

---

### Task 3: Snippet launcher rows + panel expansion

**Files:**
- Create: `crates/tinycast/src/features/snippets/ui/coordinator.rs`
- Create: `crates/tinycast/src/features/snippets/service/injector.rs`

**Interfaces:**
- `AppIndex` publishes enabled snippets as `AppKind::Snippet` when `snippetsShowInLauncher`. Name and keyword both in `SearchFields`. Activate → expand (prompt arguments on `PaletteMode::QuicklinkArguments`-style argument screen reused) → inject into `previousApp` via UIA replace; fallback `SendInput`. Confirmation HUD if `show_confirmation`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn snippet_keyword_is_display_literal_band() {
    let mut f = SearchFields { display_name: "Meeting Notes".into(), snippet_keyword: Some("!notes".into()), ..Default::default() };
    assert!(score("!notes", &f).unwrap() >= Band::DisplayLiteral as i32 * BAND_STRIDE);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure snippet_keyword_is_display`

Expected: FAIL until SearchFields includes snippet_keyword in DisplayLiteral.

- [ ] **Step 3: Write minimal implementation**

Wire kind slice, injector, HUD presenter (shared `MessageHud`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS. Manual: enable snippets, create one, expand from launcher into Notepad.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: expand snippets from the launcher via UIA"
```

---

### Task 4: Keyword listener

**Files:**
- Create: `crates/tinycast-pure/src/snippet/keyword.rs`
- Create: `crates/tinycast/src/features/snippets/service/listener.rs`

**Interfaces:**

```rust
pub struct KeywordBuffer { /* 256 char cap */ }
impl KeywordBuffer {
    pub fn push(&mut self, ch: char, now_ms: u64) -> Option<String>; // longest suffix keyword
    pub fn reset(&mut self);
}
```

Reset on 15s idle, navigation/modifier shortcuts, app change. Match case-insensitive longest suffix. Listener: WH_KEYBOARD_LL **only if enabled**. Ignore events tagged as Tinycast synthetics. Before delete+insert, re-check consent and target. Failed gate leaves typed keyword.

`snippetsEnabled` off at startup: no hook. Settings pane: switch with explanation dialog (coordinator).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn longest_suffix_wins() {
    let mut b = KeywordBuffer::with_keywords(["!n", "!notes"]);
    for c in "!notes".chars() { b.push(c, 0); }
    assert_eq!(b.push('s', 0), None); // already consumed? design: evaluate after each char
}
```

Better:

```rust
#[test]
fn longest_suffix_wins() {
    let kw = ["!n", "!notes"];
    assert_eq!(match_suffix("xx!notes", &kw).as_deref(), Some("!notes"));
    assert_eq!(match_suffix("xx!n", &kw).as_deref(), Some("!n"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure longest_suffix_wins`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Pure matcher + Effect hook install/uninstall. Never prompt from the hook.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure keyword`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add snippet keyword listener behind enable switch"
```

---

### Task 5: Custom commands

**Files:**
- Create: `crates/tinycast-pure/src/custom_command.rs`
- Create: `crates/tinycast/src/features/custom_commands/service/store.rs`
- Create: `crates/tinycast/src/features/custom_commands/service/runner.rs`
- Create: `crates/tinycast/src/features/custom_commands/ui/coordinator.rs`

**Interfaces:**

```rust
pub struct CustomCommand {
    pub id: Uuid,
    pub name: String,
    pub command: String, // CreateProcess
    pub confirm: bool,
}
```

Slice alphabetized `AppKind::CustomCommand` before built-in Commands. Only name is indexed. Coordinator confirms then `ShellCommandRunner`. Failure → dialog. Success HUD if the command is silent. Settings pane editor sheet width 480 DIP.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn custom_commands_sort_by_name() {
    let mut v = vec![cmd("b"), cmd("a")];
    v.sort_by(|x, y| x.name.cmp(&y.name));
    assert_eq!(v[0].name, "a");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure custom_commands_sort`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

JSON store in roaming. `CreateProcessW` with no shell unless the command is explicitly `cmd.exe /c`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add custom commands store and runner"
```

---

### Task 6: Quicklinks

**Files:**
- Create: `crates/tinycast-pure/src/quicklink/mod.rs`
- Create: `crates/tinycast/src/features/quicklinks/` (store, coordinator, argument screen)

**Interfaces:**
- `Quicklink { id, name, destination, pinned, show_in_root }`. Name only indexed. Placeholders use `expand` with percent-encode by default (`raw` opts out). Commands: Create / Search / Import / Export Quicklink. `PaletteMode::Quicklinks` and `QuicklinkArguments`. Open via `ShellExecuteExW`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn quicklink_url_encodes_arguments_unless_raw() {
    let ctx = /* empty */;
    let o = expand("https://ex.com/q={argument name=\"q\"}", &ctx, &[("q".into(), "a b".into())].into());
    assert!(o.text.contains("a%20b"));
}
```

Template engine needs a `percent_encode_values: bool` parameter for quicklink mode — add it in this task if not present:

```rust
pub fn expand_with(input: &str, ctx: &ExpandContext, args: &HashMap<String,String>, auto_percent: bool) -> ExpandOutput;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure quicklink_url_encodes`

Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Store + UI + argument mode: search field is the current argument; Enter submits; Backspace steps back an argument.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add quicklinks with argument prompts"
```

---

### Task 7: Emoji grid

**Files:**
- Create: `crates/tinycast-pure/src/emoji.rs`
- Create: `crates/tinycast/src/features/emoji/`

**Interfaces:**
- Command `command:search-emoji` → `PaletteMode::Emoji`. Grid cell 56 DIP. Skin tone from `emojiSkinTone`. Search names. Enter inserts via Paster. Bundled catalog (CLDR short names); do not download.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn emoji_search_matches_short_name() {
    let hits = search_emoji("smile");
    assert!(hits.iter().any(|e| e.glyph.contains('☺') || e.glyph.contains('😊') || e.name.contains("smile")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure emoji_search`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Ship a compact catalog (common emoji + names). Grid D2D, Segoe UI Emoji.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add emoji palette grid"
```
