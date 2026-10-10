#!/usr/bin/env bash
# Regenerate design-system golden PNGs (crates/tinycast/tests/golden) by running the
# Windows test build under Wine. On Windows use scripts/regen-goldens.ps1 instead.
set -euo pipefail
cd "$(dirname "$0")/.."
export TINYCAST_UPDATE_GOLDENS=1
export WINEDEBUG="${WINEDEBUG:--all}"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER="${CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER:-wine}"
cargo test --target x86_64-pc-windows-gnu -p tinycast --lib design_system::golden -- --nocapture
ls -l crates/tinycast/tests/golden
