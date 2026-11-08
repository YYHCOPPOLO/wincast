#Requires -Version 7.0
<#
.SYNOPSIS
    Builds the ECDICT SQLite database the dictionary Tab reads.

.DESCRIPTION
    Downloads the pinned ECDICT CSV once into %TEMP%, builds
    %LOCALAPPDATA%\com.wincast.win\Dictionary\ecdict.sqlite through the Go
    `build-dict` command, and drops the upstream MIT notice next to it.

    Nothing is written inside the repository: the CSV cache, the database and
    the license all live outside it.

.PARAMETER CsvPath
    Use this CSV instead of downloading or reusing the cached one.

.PARAMETER Force
    Rebuild even when a database already exists.
#>
[CmdletBinding()]
param(
    [string]$CsvPath,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Commit = '9c46923fb81a2eb6c264a3a1689562ea7bc56b6a'
$ShortSha = $Commit.Substring(0, 8)
$CsvUrl = "https://raw.githubusercontent.com/skywind3000/ECDICT/$Commit/ecdict.csv"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$CacheCsv = Join-Path $env:TEMP "ecdict-$ShortSha.csv"
$OutDir = Join-Path $env:LOCALAPPDATA 'com.wincast.win\Dictionary'
$OutDb = Join-Path $OutDir 'ecdict.sqlite'
$LicensePath = Join-Path $OutDir 'LICENSE-ECDICT.txt'
$MinCachedCsv = 50MB

if (-not $CsvPath) {
    if ((Test-Path -LiteralPath $CacheCsv) -and (Get-Item -LiteralPath $CacheCsv).Length -gt $MinCachedCsv) {
        Write-Host "Reusing cached CSV $CacheCsv"
    }
    else {
        Write-Host "Downloading $CsvUrl"
        Invoke-WebRequest -Uri $CsvUrl -OutFile $CacheCsv -UseBasicParsing
    }
    $CsvPath = $CacheCsv
}
if (-not (Test-Path -LiteralPath $CsvPath)) {
    throw "CSV not found: $CsvPath"
}

if ((Test-Path -LiteralPath $OutDb) -and -not $Force) {
    throw "The dictionary database already exists at $OutDb. Re-run with -Force to rebuild it."
}
if (Get-Process -Name wincast -ErrorAction SilentlyContinue) {
    throw 'wincast.exe is running. Quit it before rebuilding the dictionary, then restart it to load the new data.'
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Push-Location -LiteralPath (Join-Path $RepoRoot 'go')
try {
    & go run ./cmd/build-dict --input $CsvPath --output $OutDb
    if ($LASTEXITCODE -ne 0) {
        throw "build-dict exited with code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

$License = @"
MIT License

Copyright (c) 2025 Linwei

Source: https://github.com/skywind3000/ECDICT
Commit: $Commit
Data:   ecdict.csv, built into ecdict.sqlite in this directory.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"@
Set-Content -LiteralPath $LicensePath -Value $License -Encoding utf8NoBOM

$Database = Get-Item -LiteralPath $OutDb
$SizeMb = [math]::Round($Database.Length / 1MB, 1)
Write-Host ''
Write-Host "Dictionary ready: $($Database.FullName) ($SizeMb MB)"
Write-Host "License: $LicensePath"
