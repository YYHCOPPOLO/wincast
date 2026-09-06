# Release size and RSS ledger

Budget: release exe ≤ 5 MB; idle (hide-to-tray) RSS ≤ 100 MB. Over-budget rows must say why and what was squeezed; do not cut in-scope features.

`strip = true` is already set on the workspace release profile (`lto = true`, `codegen-units = 1`, `opt-level = "s"`, `panic = "abort"`).

| date | git | exe bytes | idle RSS | notes |
|---|---|---|---|---|
| 2026-09-05 | 58e03cb | 2714624 | not measured (no GUI session) | `target/release/tinycast.exe` after `cargo build -p tinycast --release`. 2.59 MB, under the 5 MB target. Idle RSS not measured: no hide-to-tray GUI session in this environment. No JS runtime (`v8` / `quickjs` / `wry` / `deno`) in the tree. |
| 2026-09-07 | 6fee3d1 | 2776064 | not measured | after UI overlay D2D; no JS runtime. `target/release/tinycast.exe` after `cargo build -p tinycast --release`. 2.65 MB, under the 5 MB target. Idle RSS not measured: no hide-to-tray GUI session in this environment. |
