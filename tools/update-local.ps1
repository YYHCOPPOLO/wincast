# Build release Tinycast and refresh the daily copies:
#   %LOCALAPPDATA%\Tinycast\tinycast.exe  (login / tray instance)
#   Desktop\tinycast.exe
param(
    [switch]$NoStart
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Stop-Tinycast {
    $procs = @(Get-Process -Name tinycast -ErrorAction SilentlyContinue)
    if ($procs.Count -eq 0) {
        return
    }
    $procs | Stop-Process
    $deadline = (Get-Date).AddSeconds(8)
    do {
        Start-Sleep -Milliseconds 200
        $procs = @(Get-Process -Name tinycast -ErrorAction SilentlyContinue)
    } while ($procs.Count -gt 0 -and (Get-Date) -lt $deadline)
    if ($procs.Count -gt 0) {
        $procs | Stop-Process -Force
        Start-Sleep -Milliseconds 400
    }
}

Write-Host "Building release..."
cargo build -p tinycast --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed while a tinycast.exe may be locking target\release. Stopping and retrying..."
    Stop-Tinycast
    cargo build -p tinycast --release
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

$src = Join-Path $Root "target\release\tinycast.exe"
if (-not (Test-Path $src)) {
    throw "missing $src"
}

$installDir = Join-Path $env:LOCALAPPDATA "Tinycast"
$install = Join-Path $installDir "tinycast.exe"
$desktop = Join-Path ([Environment]::GetFolderPath("Desktop")) "tinycast.exe"

New-Item -ItemType Directory -Force $installDir | Out-Null
Stop-Tinycast
Copy-Item $src $install -Force
Copy-Item $src $desktop -Force

Write-Host "Installed: $install"
Write-Host "Desktop:   $desktop"

if (-not $NoStart) {
    Start-Process $install
    $ok = $false
    for ($i = 0; $i -lt 25; $i++) {
        Start-Sleep -Milliseconds 200
        if (Get-Process -Name tinycast -ErrorAction SilentlyContinue) {
            $ok = $true
            break
        }
    }
    if (-not $ok) {
        Start-Process $install
        Start-Sleep -Seconds 1
        $ok = [bool](Get-Process -Name tinycast -ErrorAction SilentlyContinue)
    }
    if (-not $ok) {
        throw "tinycast.exe did not stay running after Start-Process"
    }
    Write-Host "Started daily copy."
}
