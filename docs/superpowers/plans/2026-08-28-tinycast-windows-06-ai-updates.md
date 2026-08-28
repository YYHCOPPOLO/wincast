# HTTP AI, Quick Actions, and Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** v0.10.2 AI Chat and Quick Actions over HTTP (no Apple Intelligence), plus Check for Updates against this Windows repo's GitHub Releases.

**Architecture:** Endpoint policy, message bounding, open policy are pure. WinHTTP streaming and DPAPI keys are Effect. Chat is a palette mode; Tab ring gains AI when enabled.

**Tech Stack:** WinHTTP (SSE / chunked). DPAPI. SQLite `ai-chats.sqlite3`. Optional `codex` binary on PATH for ChatGPT subscription — never bundled.

**Spec:** spec §2, §8.13, §8.16. Oracle: `ai.md`, `quick-actions.md`, `updates.md`.

## Global Constraints

Index constraints apply. **No Apple Intelligence case, setting, or credential field.** `aiEnabled` and all `ai*` keys are backup-excluded. Off means fully off: no command, no DB open, no Tab stop, cancel stream. HTTPS required; plain HTTP only for localhost/127.0.0.1/::1. Keys never logged. Unavailable local model must not fail-over onto a billed endpoint (there is no local model — do not invent one). ChatGPT subscription: Connect only if `codex` exists; otherwise Settings links to install docs.

---

### Task 1: AIEndpointPolicy + models

**Files:**
- Create: `crates/tinycast-pure/src/ai/endpoint.rs`
- Create: `crates/tinycast-pure/src/ai/instructions.rs`
- Create: `crates/tinycast-pure/src/ai/open_policy.rs`

**Interfaces:**

```rust
pub enum ProviderKind { OpenAi, Anthropic, Gemini, OpenRouter, OpenAiCompatible }
pub fn validate_url(url: &str) -> Result<Url, EndpointError>;
pub enum ModelSelection { Api { connection: Uuid, model: String }, ChatGpt { model: String, effort: Option<String> } }
// NOTE: no AppleIntelligence variant
pub fn compose_instructions(user: Option<&str>, enabled: bool) -> Option<String>;
pub enum OpenPolicy { Recent { after_minutes: u32 }, New }
pub fn should_resume(policy: OpenPolicy, last_activity: i64, now: i64) -> bool;
```

Preamble lives in `instructions.rs` only (capabilities, honest comparisons, not “prefer Tinycast”). `compose` returns None when system prompt is off.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn https_required_except_loopback() {
    assert!(validate_url("http://example.com").is_err());
    assert!(validate_url("http://127.0.0.1:11434/v1").is_ok());
    assert!(validate_url("https://api.openai.com/v1").is_ok());
    assert!(validate_url("ftp://x").is_err());
}

#[test]
fn model_selection_has_no_apple_case() {
    let names = ["Api", "ChatGpt"];
    assert!(!names.contains(&"AppleIntelligence"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure https_required_except_loopback`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Policy + open policy + preamble.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure ai`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast-pure
git commit -m "feat: add AI endpoint policy without Apple Intelligence"
```

---

### Task 2: Connections, DPAPI keys, HTTP stream

**Files:**
- Create: `crates/tinycast/src/platform/dpapi.rs`
- Create: `crates/tinycast/src/platform/winhttp.rs` (if missing)
- Create: `crates/tinycast/src/features/ai/service/provider.rs`
- Create: `crates/tinycast/src/features/ai/service/factory.rs`

**Interfaces:**

```rust
pub trait AiProvider {
    fn stream(&mut self, req: AiRequest, sink: mpsc::Sender<AiEvent>);
}
pub enum AiEvent { Text(String), Thinking(bool), Usage, Done, Error(String) }
```

Factory reads key at the last moment from DPAPI keyed by connection UUID. Changing provider/base URL forgets the old key. OpenAI-compatible completions + SSE. Anthropic Messages. Gemini OpenAI-compat URL. Ephemeral, no URL cache. Stream on thread pool; `PostMessage` chunks to UI.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn same_destination_policy() {
    assert!(same_destination("https://api.openai.com/v1", "https://api.openai.com/v1/chat/completions"));
    assert!(!same_destination("https://api.openai.com/v1", "https://api.anthropic.com"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure same_destination_policy`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

`same_destination` in pure. DPAPI wrap. One provider implementation for OpenAI-compat first; Anthropic mapper second.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add DPAPI API keys and HTTP AI streaming"
```

---

### Task 3: Chat palette + history store

**Files:**
- Create: `crates/tinycast/src/features/ai/service/history.rs`
- Create: `crates/tinycast/src/features/ai/ui/screen.rs`
- Create: `crates/tinycast/src/features/ai/ui/coordinator.rs`
- Modify: `crates/tinycast-pure/src/palette_tab.rs`

**Interfaces:**
- `command:ai-chat` → `PaletteMode::Ai` only if `aiEnabled`. Search field is composer. Footer Send/Stop ↵. Ctrl+K: New Chat, Chat History, AI Settings, Stop, Copy Last. Model switcher header menu (no Apple row). `tab_from(..., ai_enabled: true)`: Launcher → Ai → Clipboard → Launcher. Query does **not** seed chat; chat draft does not seed search. Streaming continues while hidden. Bound context: newest message whole, older text newest-first into ~100KB. History `ai-chats.sqlite3`. Empty chats not saved. Retention prune only while AI on.

Update `tab_from` tests:

```rust
#[test]
fn tab_ring_with_ai() {
    assert_eq!(tab_from(PaletteMode::Launcher, true, false), TabHop::Ai);
    assert_eq!(tab_from(PaletteMode::Ai, true, false), TabHop::Clipboard);
    assert_eq!(tab_from(PaletteMode::Clipboard, true, false), TabHop::Launcher);
}
```

- [ ] **Step 1: Write the failing test** (the tab_ring_with_ai test)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure tab_ring_with_ai`

Expected: FAIL (still hops to Clipboard).

- [ ] **Step 3: Write minimal implementation**

Update `tab_from`. Chat UI D2D. Markdown render for assistant; user literal.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tinycast-pure palette_tab`

Expected: PASS. Manual: enable AI, add OpenAI-compat connection, send a message.

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add AI Chat palette mode and conversation history"
```

---

### Task 4: AI settings pane + ChatGPT subscription optional

**Files:**
- Create: `crates/tinycast/src/features/ai/settings/pane.rs`
- Create: `crates/tinycast/src/features/ai/service/chatgpt.rs`

**Interfaces:**
- Master switch, connections editor, default model, Opens to, Keep conversations, system prompt, web search flag. No Apple Intelligence row. ChatGPT: if `which_codex()` finds a binary, show Connect; else show install-docs link. Private `CODEX_HOME` under roaming. Tools disabled.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn ai_settings_do_not_serialize_into_backup() {
    assert_eq!(coverage_bucket(AppSettingsKey::AiEnabled), Bucket::Excluded);
    assert_eq!(coverage_bucket(AppSettingsKey::AiConnections), Bucket::Excluded);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure ai_settings_do_not_serialize`

Expected: PASS if plan 05 coverage exists; else FAIL.

- [ ] **Step 3: Write minimal implementation**

Pane + optional Codex manager. Switching AI off stops Codex and cancels streams.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast
git commit -m "feat: add AI settings pane and optional ChatGPT subscription"
```

---

### Task 5: Quick Actions

**Files:**
- Create: `crates/tinycast-pure/src/ai/quick_action.rs`
- Create: `crates/tinycast/src/features/quick_actions/ui/coordinator.rs`
- Create: `crates/tinycast/src/features/quick_actions/ui/result.rs`

**Interfaces:**
- Four actions: Fix Grammar, Rewrite, Translate, Summarize. `CommandID` already has them. Read selection from previousApp (UIA). Do **not** hide palette before capturing. Result panel 520 DIP wide. Optional apply via snippet injector. `quickActionsEnabled` default false, backup-excluded. Own model selection (`quickActionModel`) via factory overload.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn four_quick_actions_map_to_command_ids() {
    assert_eq!(CommandID::from_quick(QuickAction::FixGrammar), CommandID::FixGrammar);
    assert_eq!(QuickAction::all().len(), 4);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure four_quick_actions`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Prompts in `QuickActionPrompt`. Coordinator capture → stream → result view → apply.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add Quick Actions over the HTTP provider"
```

---

### Task 6: Check for Updates

**Files:**
- Create: `crates/tinycast-pure/src/update.rs`
- Create: `crates/tinycast/src/features/updates/service/feed.rs`
- Create: `crates/tinycast/src/surfaces/updates.rs`

**Interfaces:**

```rust
pub enum Channel { Stable, Development }
pub fn channel_for_bundle(id: &str) -> Channel {
    if id == "com.tinycast.win" { Channel::Stable } else { Channel::Development }
}
pub fn parse_version(s: &str) -> Option<Version>;
```

`com.tinycast.win.dev` (if used) never updates. Feed: GitHub Releases of **this** repo. Constant `RELEASE_REPO: &str` — if empty or 404, HUD `"No updates configured."`. Zip only. Cache `%LOCALAPPDATA%\com.tinycast.win\update-check.json`, 24h. Defer prompt while palette/dialog/snippet/uninstall/recording. Dev channel: hide command.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn stable_bundle_is_the_only_updater() {
    assert_eq!(channel_for_bundle("com.tinycast.win"), Channel::Stable);
    assert_eq!(channel_for_bundle("com.tinycast.win.dev"), Channel::Development);
}

#[test]
fn semver_beta_below_release() {
    assert!(parse_version("0.10.2").unwrap() > parse_version("0.10.2-beta.1").unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tinycast-pure stable_bundle_is_the_only_updater`

Expected: FAIL compile.

- [ ] **Step 3: Write minimal implementation**

Version parse + feed + window. Until a repo exists, `RELEASE_REPO` empty string is valid and produces the HUD.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```
git add crates/tinycast crates/tinycast-pure
git commit -m "feat: add GitHub Releases updater with empty-feed HUD"
```
