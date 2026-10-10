# Regenerate design-system golden PNGs (crates/tinycast/tests/golden) on Windows.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$env:TINYCAST_UPDATE_GOLDENS = '1'
try {
    cargo test -p tinycast --lib design_system::golden -- --nocapture
} finally {
    Remove-Item Env:TINYCAST_UPDATE_GOLDENS
}
Get-ChildItem crates/tinycast/tests/golden
