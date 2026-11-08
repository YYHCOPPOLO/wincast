# Build Wincast and refresh this machine's installed and Desktop copies.
# InstallDir is a deployment preference, never an application data path.
param(
    [switch]$Deploy,
    [switch]$NoStart,
    [string]$InstallDir = "D:\DailySoftware\win-tinycast"
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$GoRoot = Join-Path $Root "go"
$Source = Join-Path $GoRoot "build\windows-amd64\wincast.exe"
$env:GOTOOLCHAIN = "local"
$env:GOWORK = "off"
$env:CGO_ENABLED = "0"
Push-Location $GoRoot
try {
    & go tool mygo build -platform windows/amd64
    if ($LASTEXITCODE -ne 0) { throw "mygo build failed: $LASTEXITCODE" }
}
finally { Pop-Location }
if (-not (Test-Path -LiteralPath $Source)) { throw "missing $Source" }

$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$DesktopDir = [Environment]::GetFolderPath("Desktop")
$install = Join-Path $InstallDir "wincast.exe"
$desktop = Join-Path $DesktopDir "wincast.exe"
$previousInstall = Join-Path $InstallDir "tinycast.exe"
$previousDesktop = Join-Path $DesktopDir "tinycast.exe"
$destinations = @($install, $desktop, $previousInstall, $previousDesktop)
function Get-InstallProcesses([string]$Path) {
    @(Get-Process -Name wincast,tinycast -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $Path) } catch { $false }
    })
}
function Stop-InstallCopy([string]$Path) {
    if (@(Get-InstallProcesses $Path).Count -eq 0) { return }
    Write-Host "Requesting saved shutdown: $Path"
    $quit = Start-Process -FilePath $Path -ArgumentList '--quit' -WindowStyle Hidden -PassThru
    if (-not $quit.WaitForExit(10000)) { throw "quit request timed out: $Path" }
    if ($quit.ExitCode -ne 0) { throw "quit request failed: $Path" }
    $deadline = (Get-Date).AddSeconds(10)
    while (@(Get-InstallProcesses $Path).Count -gt 0) {
        if ((Get-Date) -ge $deadline) { throw "Save or resolve pending app changes before updating: $Path" }
        Start-Sleep -Milliseconds 200
    }
}
function Copy-WithRetry([string]$From, [string]$To) {
    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        try { Copy-Item -LiteralPath $From -Destination $To -Force; return }
        catch { if ($attempt -eq 49) { throw }; Start-Sleep -Milliseconds 100 }
    }
}
function Start-InstallCopy([string]$Path) {
    $started = Start-Process -FilePath $Path -ArgumentList '--hidden' -WindowStyle Hidden -PassThru
    if ($started.WaitForExit(2000)) { throw "Installed copy did not stay running: $Path" }
    if (@(Get-InstallProcesses $Path | Where-Object Id -eq $started.Id).Count -ne 1) {
        throw "Installed copy is not running at the expected path: $Path"
    }
    Write-Host "Started daily copy: $Path"
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$backupDir = Join-Path $InstallDir ("backups\" + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-wincast-' + [Guid]::NewGuid().ToString('N').Substring(0,8))
$oldFiles = @()
$wasRunning = @($destinations | Where-Object { @(Get-InstallProcesses $_).Count -gt 0 })
$replacing = $false
try {
    foreach ($destination in $destinations) { Stop-InstallCopy $destination }
    New-Item -ItemType Directory -Path $backupDir | Out-Null
    for ($i = 0; $i -lt $destinations.Count; $i++) {
        if (Test-Path -LiteralPath $destinations[$i]) {
            $backup = Join-Path $backupDir ("$i-" + [IO.Path]::GetFileName($destinations[$i]))
            Copy-WithRetry $destinations[$i] $backup
            $oldFiles += [pscustomobject]@{ Path=$destinations[$i]; Backup=$backup }
        }
    }
    $data = Join-Path $InstallDir 'data'
    if (Test-Path -LiteralPath $data) { Copy-Item -LiteralPath $data -Destination (Join-Path $backupDir 'data') -Recurse }
    foreach ($id in @('com.wincast.win', 'com.tinycast.win.go-preview')) {
        $settings = Join-Path ([Environment]::GetFolderPath('ApplicationData')) $id
        if (Test-Path -LiteralPath $settings) { Copy-Item -LiteralPath $settings -Destination (Join-Path $backupDir $id) -Recurse }
    }
    $replacing = $true
    Copy-WithRetry $Source $install
    Copy-WithRetry $Source $desktop
    $hash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
    foreach ($destination in @($install, $desktop)) {
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $hash) { throw "Hash mismatch: $destination" }
    }
    if (-not $NoStart) { Start-InstallCopy $install }
    foreach ($previous in @($previousInstall, $previousDesktop)) {
        if (Test-Path -LiteralPath $previous) { Remove-Item -LiteralPath $previous -Force }
    }
    Write-Host "Installed: $install"
    Write-Host "Desktop:   $desktop"
    Write-Host "Backup:    $backupDir"
    Write-Host "SHA256:    $hash"
}
catch {
    $deployError = $_
    if ($replacing) {
        Stop-InstallCopy $install
        Stop-InstallCopy $desktop
        foreach ($saved in $oldFiles) { Copy-WithRetry $saved.Backup $saved.Path }
        foreach ($destination in @($install, $desktop)) {
            if ($destination -notin $oldFiles.Path -and (Test-Path -LiteralPath $destination)) { Remove-Item -LiteralPath $destination -Force }
        }
    }
    if ($wasRunning.Count -gt 0 -and @(Get-InstallProcesses $wasRunning[0]).Count -eq 0) { Start-InstallCopy $wasRunning[0] }
    throw $deployError
}
