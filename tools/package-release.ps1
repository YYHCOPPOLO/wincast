# Stage an unsigned Windows x64 release folder and archive for Wincast.
#
# This builds the Go/MyGo production package, copies the runtime artifacts and
# third-party notices, then writes a manifest with SHA-256 hashes. It does not
# publish, upload, sign, or replace the daily installation; tools/update-local.ps1
# owns the machine-local delivery path.
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$GoRoot = Join-Path $Root "go"
$ConfigPath = Join-Path $GoRoot "mygo.json"
$Config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
$Version = $Config.version
if (-not $Version) { throw "mygo.json has no version" }
if (-not $Config.name) { throw "mygo.json has no name" }
if ($Config.name -notmatch '^[A-Za-z0-9._-]+$' -or $Version -notmatch '^\d+\.\d+\.\d+([+-][A-Za-z0-9.-]+)?$') {
    throw "mygo.json name or version is not safe for packaging"
}

$env:GOTOOLCHAIN = "local"
$env:GOWORK = "off"
$env:CGO_ENABLED = "0"

if (-not $SkipBuild) {
    Write-Host "Building $($Config.name) $Version windows/amd64..."
    Push-Location $GoRoot
    try {
        & go tool mygo build -platform windows/amd64
        if ($LASTEXITCODE -ne 0) { throw "mygo build failed with exit code $LASTEXITCODE" }
    }
    finally {
        Pop-Location
    }
}

$Built = Join-Path $GoRoot ("build/windows-amd64/" + $Config.name + ".exe")
if (-not (Test-Path -LiteralPath $Built)) { throw "missing $Built" }

$StageName = "$($Config.name)-$Version-win64"
$ReleaseRoot = [IO.Path]::GetFullPath((Join-Path $GoRoot "build/release"))
$Stage = [IO.Path]::GetFullPath((Join-Path $ReleaseRoot $StageName))
if (-not $Stage.StartsWith($ReleaseRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "release directory is outside the build workspace: $Stage"
}
if (Test-Path -LiteralPath $Stage) { Remove-Item -LiteralPath $Stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item -LiteralPath $Built -Destination (Join-Path $Stage ($Config.name + ".exe"))
$Setup = Join-Path $GoRoot ("build/windows-amd64/" + $Config.name + " Setup " + $Version + " amd64.exe")
if (Test-Path -LiteralPath $Setup) {
    Copy-Item -LiteralPath $Setup -Destination (Join-Path $Stage ($Config.name + " Setup " + $Version + " amd64.exe"))
}
else {
    throw "installer not found: $Setup"
}
$Icon = Join-Path $GoRoot "internal/brand/assets/icon.png"
if (Test-Path -LiteralPath $Icon) { Copy-Item -LiteralPath $Icon -Destination $Stage }
$Notices = Join-Path $GoRoot "THIRD_PARTY_NOTICES.md"
if (Test-Path -LiteralPath $Notices) { Copy-Item -LiteralPath $Notices -Destination $Stage }

$LicenseDir = Join-Path $Stage "licenses"
New-Item -ItemType Directory -Force -Path $LicenseDir | Out-Null
Push-Location $GoRoot
try {
    $Modules = & go list -deps -f "{{if .Module}}{{.Module.Path}}@{{.Module.Version}}{{end}}" ./cmd/wincast |
        Sort-Object -Unique |
        Where-Object { $_ -and $_ -ne "github.com/YYHCOPPOLO/wincast/go@" }
    foreach ($module in $Modules) {
        $modulePath = $module.Split("@")[0]
        $dir = & go list -m -f "{{.Dir}}" $module 2>$null
        if (-not $dir) { continue }
        $license = Get-ChildItem -LiteralPath $dir -File |
            Where-Object { $_.Name -match "^(LICENSE|LICENCE|COPYING|NOTICE)" } |
            Select-Object -First 1
        if (-not $license) { continue }
        $safe = $modulePath -replace "[/@]", "_"
        Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $LicenseDir ($safe + ".txt"))
    }
}
finally {
    Pop-Location
}

$GitSha = "unknown"
Push-Location $Root
try {
    $GitSha = (& git rev-parse HEAD 2>$null).Trim()
    if (-not $GitSha) { $GitSha = "unknown" }
}
finally {
    Pop-Location
}

$Files = Get-ChildItem -LiteralPath $Stage -Recurse -File | ForEach-Object {
    [pscustomobject]@{
        path   = $_.FullName.Substring($Stage.Length + 1).Replace("\", "/")
        bytes  = $_.Length
        sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
    }
}
$Manifest = [pscustomobject]@{
    name       = $Config.name
    identifier = $Config.identifier
    version    = $Version
    platform   = "windows-amd64"
    entrypoint = $Config.name + ".exe"
    builtAtUtc = [DateTime]::UtcNow.ToString("o")
    gitSha     = $GitSha
    signing    = "unsigned"
    files      = $Files
}
$ManifestPath = Join-Path $Stage "release.json"
$Manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ManifestPath -Encoding utf8

$Zip = Join-Path (Split-Path -Parent $Stage) ($StageName + ".zip")
if (Test-Path -LiteralPath $Zip) { Remove-Item -LiteralPath $Zip -Force }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip

$ExeHash = (Get-FileHash -LiteralPath (Join-Path $Stage ($Config.name + ".exe")) -Algorithm SHA256).Hash
$ZipHash = (Get-FileHash -LiteralPath $Zip -Algorithm SHA256).Hash
Write-Host ("Staged: {0}" -f $Stage)
Write-Host ("Archive: {0}" -f $Zip)
Write-Host ("Exe SHA256: {0}" -f $ExeHash)
Write-Host ("Zip SHA256: {0}" -f $ZipHash)
