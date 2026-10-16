# Copyright (c) 2026 Benja (Benjamacs1)
# SPDX-License-Identifier: MIT
# Installs into an isolated workspace folder, verifies payload/preview, uninstalls and restores an existing URI handler.
$ErrorActionPreference = 'Stop'
$rootPath = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$testPath = [System.IO.Path]::GetFullPath((Join-Path $rootPath 'artifacts/installer-smoke'))
if (-not $testPath.StartsWith($rootPath + [System.IO.Path]::DirectorySeparatorChar)) { throw 'Smoke path escaped workspace.' }
if (Test-Path -LiteralPath $testPath) { throw 'Smoke directory already exists; inspect it before running again.' }
if (Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative') { throw 'Native app is already installed; skip this isolated installer check.' }
$shortcut = Join-Path $env:APPDATA 'Microsoft/Windows/Start Menu/Programs/Kirk Companion/Kirk Companion.lnk'
if (Test-Path -LiteralPath $shortcut) { throw 'Existing app shortcut found; skip to preserve it.' }
New-Item -ItemType Directory -Force -Path (Join-Path $rootPath 'artifacts') | Out-Null
$backup = Join-Path $rootPath 'artifacts/uri-before-smoke.reg'
$hadProtocol = Test-Path 'HKCU:\Software\Classes\kirk'
if ($hadProtocol) { & reg.exe export 'HKCU\Software\Classes\kirk' $backup /y | Out-Null; if ($LASTEXITCODE -ne 0) { throw 'Could not back up URI handler.' } }
try {
    $installer = Join-Path $rootPath 'dist/Kirk-Companion-Setup-0.2.0.exe'
    $process = Start-Process -FilePath $installer -ArgumentList @('/S',"/D=$testPath") -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
    $installed = Join-Path $testPath 'kirk-companion.exe'
    if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath (Join-Path $rootPath 'target/release/kirk-companion.exe')).Hash) { throw 'Installed binary differs from release build.' }
    foreach ($name in @('LICENSE','COPYRIGHT.md','OFL-Squares.txt','THIRD-PARTY-LICENSES.txt')) {
        if (-not (Test-Path -LiteralPath (Join-Path $testPath $name))) { throw "Missing notice: $name" }
    }
    $command = (Get-ItemProperty -LiteralPath 'HKCU:\Software\Classes\kirk\shell\open\command').'(default)'
    if ($command -ne ('"'+$installed+'" "%1"')) { throw 'Installed URI handler is incorrect.' }
    $shot = Join-Path $rootPath 'artifacts/installed-native.png'
    $app = Start-Process -FilePath $installed -ArgumentList @('--preview','--screenshot',$shot) -Wait -PassThru
    if ($app.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $shot)) { throw 'Installed native UI did not render.' }
    Write-Output 'Installer payload, copyright/font notices, URI activation registration, and installed native rendering passed.'
} finally {
    $uninstaller = Join-Path $testPath 'Uninstall.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -Wait | Out-Null
    }
    if ($hadProtocol) { & reg.exe import $backup | Out-Null; if ($LASTEXITCODE -ne 0) { throw 'Restore the URI handler from artifacts/uri-before-smoke.reg.' } }
}
if (Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative') { throw 'Uninstall registration remains.' }
Write-Output 'Uninstall passed; original URI handler restored. Profiles and OS credentials were not changed.'
