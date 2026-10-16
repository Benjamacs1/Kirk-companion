# Copyright (c) 2026 Benja (Benjamacs1)
# SPDX-License-Identifier: MIT
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$rootPath = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $rootPath
try {
    if (-not $SkipBuild) {
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
    }
    $versionLine = Get-Content -LiteralPath Cargo.toml | Where-Object { $_ -match '^version = "([0-9]+\.[0-9]+\.[0-9]+)"$' } | Select-Object -First 1
    if ($versionLine -notmatch '"([0-9]+\.[0-9]+\.[0-9]+)"') { throw 'Package version not found.' }
    $version = $Matches[1]
    $distPath = Join-Path $rootPath 'dist'
    New-Item -ItemType Directory -Force -Path $distPath | Out-Null
    $nsis = if ($env:NSIS_PATH) { Join-Path $env:NSIS_PATH 'makensis.exe' } else { (Get-Command makensis.exe -ErrorAction SilentlyContinue).Source }
    if (-not $nsis -or -not (Test-Path -LiteralPath $nsis)) { throw 'Install NSIS and add makensis.exe to PATH, or set NSIS_PATH to its directory.' }
    & (Join-Path $PSScriptRoot 'licenses.ps1') -OutputFile (Join-Path $distPath 'THIRD-PARTY-LICENSES.txt')
    & $nsis /V2 "/DROOT=$rootPath" "/DOUTDIR=$distPath" "/DVERSION=$version" (Join-Path $PSScriptRoot 'installer.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'NSIS packaging failed.' }
    $source = Join-Path $distPath "kirk-companion-$version-source.zip"
    $sourceFiles = @('.gitattributes','.gitignore','.github','Cargo.toml','Cargo.lock','build.rs','LICENSE','COPYRIGHT.md','README.md','SECURITY.md','THIRD_PARTY_NOTICES.md','src','assets','tests','examples','tools','docs') | ForEach-Object { Join-Path $rootPath $_ }
    Compress-Archive -LiteralPath $sourceFiles -DestinationPath $source -Force
    $files = @("Kirk-Companion-Setup-$version.exe","kirk-companion-$version-source.zip",'THIRD-PARTY-LICENSES.txt')
    $sums = $files | ForEach-Object { $hash = Get-FileHash -LiteralPath (Join-Path $distPath $_) -Algorithm SHA256; "$($hash.Hash.ToLowerInvariant())  $_" }
    [System.IO.File]::WriteAllLines((Join-Path $distPath 'SHA256SUMS.txt'),$sums)
    Get-Item -LiteralPath ($files | ForEach-Object { Join-Path $distPath $_ }) | Select-Object Name,Length
} finally { Pop-Location }
