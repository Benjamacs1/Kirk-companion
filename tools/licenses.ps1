# Copyright (c) 2026 Benja (Benjamacs1)
# SPDX-License-Identifier: MIT
param([string]$OutputFile = (Join-Path $PSScriptRoot '../dist/THIRD-PARTY-LICENSES.txt'))
$ErrorActionPreference = 'Stop'
$rootPath = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $rootPath
try {
    $metadata = (& cargo metadata --locked --format-version 1 | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0) { throw 'Cargo dependency metadata failed.' }
    $notice = [System.Text.StringBuilder]::new()
    [void]$notice.AppendLine('Kirk Companion dependency notices. Copyrights and licenses remain with their respective owners.')
    [void]$notice.AppendLine((Get-Content -LiteralPath assets/fonts/OFL-Squares.txt -Raw))
    foreach ($package in ($metadata.packages | Sort-Object name,version)) {
        if ($package.name -eq 'kirk-companion') { continue }
        [void]$notice.AppendLine("`n===== $($package.name) $($package.version) =====")
        [void]$notice.AppendLine("SPDX: $($package.license) | $($package.repository)")
        $packageRoot = Split-Path $package.manifest_path
        $licenseFiles = @(Get-ChildItem -LiteralPath $packageRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|OFL|UNLICENSE)' })
        if ($package.license_file) {
            $declaredLicense = Join-Path $packageRoot $package.license_file
            if (Test-Path -LiteralPath $declaredLicense -PathType Leaf) { $licenseFiles += Get-Item -LiteralPath $declaredLicense }
        }
        if ($package.name -eq 'epaint_default_fonts') {
            $licenseFiles += Get-ChildItem -LiteralPath (Join-Path $packageRoot 'fonts') -File | Where-Object { $_.Name -match '(?i)license|licence|ofl|\.txt$' }
        }
        foreach ($file in ($licenseFiles | Sort-Object FullName -Unique)) {
            [void]$notice.AppendLine("--- $($file.Name) ---")
            [void]$notice.AppendLine((Get-Content -LiteralPath $file.FullName -Raw))
        }
    }
    [System.IO.File]::WriteAllText([System.IO.Path]::GetFullPath($OutputFile),$notice.ToString())
} finally { Pop-Location }
