# Extensions Settings Placeholder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Implement **every task in this plan file** first. **Do not review after each task.** When the last task is committed, review this plan's full commit range once. See `AGENTS.md`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the v0.10.2 Extensions settings tab and palette mode bit without shipping a JS runtime.

**Architecture:** UI and settings keys exist. `extensionsEnabled` cannot become true in a way that runs code. `PaletteMode::ExtensionCommand` is never entered.

**Tech Stack:** Existing Win32 settings pane. No QuickJS, no V8, no `wry`.

**Spec:** spec §2, §8.15. Oracle: `extensions.md` (read for labels only).

## Global Constraints

Index constraints apply, including **one review after this entire plan**, not after each task (`AGENTS.md`). Backup already excludes `extensionsEnabled`, registries, package manager, custom search paths. Do not add a JS crate in this plan.

---

### Task 1: Extensions pane that cannot run

**Files:**
- Create: `crates/tinycast/src/features/extensions/settings/pane.rs`
- Modify: `crates/tinycast/src/surfaces/settings.rs`
- Modify: `crates/tinycast/src/app_settings.rs`

**Interfaces:**

```rust
pub fn try_set_extensions_enabled(_on: bool) -> Result<(), &'static str> {
    Err("The extension runtime is not in this version.")
}
```

Pane title `Extensions`. Copy: original switch **visible but disabled** (or toggling shows a dialog with that sentence). `extensionsShowInLauncher` checkbox may exist but publishes no rows while runtime is absent. Launcher must not grow extension entries.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn extensions_cannot_be_enabled() {
    assert!(try_set_extensions_enabled(true).is_err());
}

#[test]
fn extension_command_mode_is_never_the_tab_ring() {
    assert_ne!(tab_from(PaletteMode::Launcher, true, false), TabHop::StayForArguments);
    // ExtensionCommand is not a Tab hop
    match tab_from(PaletteMode::ExtensionCommand, false, false) {
        TabHop::Launcher => {}
        other => panic!("{other:?}"),
    }
}
```

Define `tab_from(ExtensionCommand, ..) = TabHop::Launcher` so a stray mode pops home.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure extensions_cannot_be_enabled`

Put `try_set_extensions_enabled` in `tinycast-pure/src/extensions.rs` so the exe cannot “just set a bool”.

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Pure reject function + settings pane. Do not add `windows` features for scripting. `cargo tree -p tinycast` must not contain `v8`, `quickjs`, `wry`, `deno`.

Add a test in the exe crate:

```rust
#[test]
fn no_js_engine_in_lockfile_or_manifest() {
    let toml = include_str!("../../../Cargo.toml");
    for forbid in ["v8", "quickjs", "wry", "deno"] {
        assert!(!toml.contains(forbid), "{forbid}");
    }
}
```

(Adjust relative path to the exe `Cargo.toml`.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure extensions` and `cargo test -p tinycast no_js_engine`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add Extensions settings placeholder without a JS runtime"
```

---

### Task 2: Size/RSS ledger (no feature cuts)

**Files:**
- Create: `docs/superpowers/plans/size-ledger.md`

**Interfaces:** none. After `cargo build -p tinycast --release`, record exe bytes and a note of working-set after hide. If over 5 MB / 100 MB, write **why** and what was squeezed; do not remove in-scope features.

- [ ] **Step 1: Write the failing test**

No unit test. Create the ledger file with a table:

```markdown
| date | git | exe bytes | idle RSS | notes |
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo build -p tinycast --release`

Expected: produces `target/release/tinycast.exe`.

- [ ] **Step 3: Write minimal implementation**

Fill one ledger row. `strip = true` already in workspace release profile.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test` (workspace). Confirm `tinycast-pure` still has no `windows` dependency.

Expected: PASS

- [ ] **Step 5: Commit**

```
git add docs/superpowers/plans/size-ledger.md
git commit -m "docs: record release size and RSS ledger"
```

---

## v1 plans complete when

Plans 00–07 tests pass, Alt+Space panel behaves as spec, Extensions cannot run JS, AI has no Apple route, uninstall only recycles, and the ledger exists.
