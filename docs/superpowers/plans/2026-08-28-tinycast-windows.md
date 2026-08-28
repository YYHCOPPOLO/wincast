# Tinycast for Windows — Plan Index

> **For agentic workers:** Execute the numbered plans **in order**. Each plan is independently testable. REQUIRED SUB-SKILL for a given plan file: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Implement **every task in the current plan**, then review that plan **once**. Do not review after each task. Do not start plan N+1 until plan N's tests pass and that plan-level review is done. See `AGENTS.md`.

**Goal:** Ship a Windows Tinycast whose user-visible behavior matches v0.10.2, minus Apple Intelligence and the Raycast extension runtime.

**Architecture:** `tinycast-pure` (no `windows` crate) holds decisions; `tinycast` exe holds Win32 Effect, `AppCore` on the UI thread, and Direct2D views.

**Tech Stack:** Rust 2021, `windows` 0.58, `rusqlite` bundled, `serde`/`serde_json`, `unicode-segmentation`. Win32 + Direct2D + system Acrylic. No WebView, no tokio.

**Spec:** `docs/superpowers/specs/2026-08-28-tinycast-windows-design.md`

## Global Constraints

Copied from the spec. Every task in every plan inherits these:

- **Review:** one review per plan file, after all of that plan's tasks are committed. Never per-task. `AGENTS.md` overrides the Superpowers per-task review loop.
- Independent rewrite. Do not copy Tinycast Swift sources. Behavior oracle is Tinycast **v0.10.2** `docs/` plus shipped catalogs.
- Bundle `com.tinycast.win`. Display name `Tinycast`.
- Windows 11 24H2+ (build 26100+), `x86_64-pc-windows-msvc` only.
- One exe. No runtime, WebView, self-contained .NET, winit, wgpu, egui, iced, tao, wry, V8, tokio.
- UI copy stays original English. `Show in menu bar` string kept (tray is the Effect). Session-ending confirmations say `PC` not `Mac`.
- Default summon: Alt+Space (⌥Space). In-palette ⌘ → Ctrl (`Ctrl+K`, `Ctrl+1…0`).
- `tinycast-pure` must not depend on the `windows` crate (checked by test).
- `AppCore` is the only long-lived owner; `start()` is the only boot wiring. Coordinators own confirmation gates.
- Theme numbers from v0.10.2 `Theme.swift`. Dark literals frozen. DIP units. Per-Monitor V2.
- `AppSettingsKey` raw values identical to v0.10.2. Backup coverage tables identical.
- Capability flags (`snippetsEnabled`, `aiEnabled`, `calendarEnabled`, `extensionsEnabled`, …) are never granted by backup import.
- Low-level keyboard hooks install only while a feature that needs them is on.
- Uninstall moves to Recycle Bin only. Never permanent delete.
- No Apple Intelligence route. No JS engine in v1. Extensions settings pane exists but cannot run extensions.
- 5 MB / 100 MB RSS are squeeze targets; do not cut in-scope features to hit them. Record a ledger if exceeded.
- Release profile: `lto = true`, `codegen-units = 1`, `opt-level = "s"`, `panic = "abort"`, `strip = true`.
- When this index and a slice plan disagree on a type name, the **lowest-numbered plan that Produces it** wins.

## File Map (locked)

```
.gitignore
Cargo.toml                          workspace + release profile
crates/tinycast-pure/Cargo.toml     NO windows dependency
crates/tinycast-pure/src/lib.rs
crates/tinycast-pure/src/theme.rs
crates/tinycast-pure/src/palette_mode.rs
crates/tinycast-pure/src/palette_state.rs
crates/tinycast-pure/src/palette_placement.rs
crates/tinycast-pure/src/palette_row_index.rs
crates/tinycast-pure/src/settings_tab.rs
crates/tinycast-pure/src/app_settings_key.rs
crates/tinycast-pure/src/hotkey.rs
crates/tinycast-pure/src/app_entry.rs
crates/tinycast-pure/src/command_id.rs
crates/tinycast-pure/src/search_relevance.rs
crates/tinycast-pure/src/search_scopes.rs
crates/tinycast-pure/src/launcher_ranking.rs
crates/tinycast-pure/src/visibility.rs
crates/tinycast-pure/src/calc/
crates/tinycast-pure/src/window_command.rs
crates/tinycast-pure/src/window_layout.rs
crates/tinycast-pure/src/window_action_memory.rs
crates/tinycast-pure/src/system_action.rs
crates/tinycast-pure/src/snippet/
crates/tinycast-pure/src/quicklink/
crates/tinycast-pure/src/uninstall/
crates/tinycast-pure/src/meeting/
crates/tinycast-pure/src/file_search/
crates/tinycast-pure/src/ai/
crates/tinycast-pure/src/settings_backup.rs
crates/tinycast/Cargo.toml
crates/tinycast/src/main.rs
crates/tinycast/src/app_core.rs
crates/tinycast/src/app_settings.rs
crates/tinycast/src/platform/          paths, dpi, single-instance, messages, dpapi, winhttp
crates/tinycast/src/design_system/     D2D brushes from theme tokens
crates/tinycast/src/palette/           HWND, D2D, EDIT, controller
crates/tinycast/src/surfaces/          settings, dialog, hud, notes, onboarding, support, camera, updates
crates/tinycast/src/features/<name>/   {service,ui,settings} — model lives in tinycast-pure
```

## Execution Order

| Plan | File | Working software when done |
|---|---|---|
| 00 | `2026-08-28-tinycast-windows-00-panel-shell.md` | Tray app, Alt+Space panel (Acrylic+scrim+EDIT), empty settings window |
| 01 | `2026-08-28-tinycast-windows-01-launcher.md` | Fuzzy app search, launch, favorites, aliases, visibility, Commands catalog rows |
| 02 | `2026-08-28-tinycast-windows-02-clipboard-calculator.md` | Clipboard history + inline calculator + history screen |
| 03 | `2026-08-28-tinycast-windows-03-commands-snippets.md` | Custom commands, Quicklinks, Emoji, Snippets (panel then keyword) |
| 04 | `2026-08-28-tinycast-windows-04-system-windows.md` | System actions + 34 window commands + hotkey/Hyper engines |
| 05 | `2026-08-28-tinycast-windows-05-files-notes-calendar-uninstall-backup.md` | File search, Notes, Calendar, Uninstall, Backup, Onboarding, Support |
| 06 | `2026-08-28-tinycast-windows-06-ai-updates.md` | HTTP AI Chat, Quick Actions, Check for Updates |
| 07 | `2026-08-28-tinycast-windows-07-extensions-placeholder.md` | Extensions settings pane that cannot run JS |

Do not implement later-plan files early. If a type is needed, it is Produces'd in the earlier plan.
