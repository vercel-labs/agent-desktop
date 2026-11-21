#Requires -Version 5.1
<#
.SYNOPSIS
    Builds the scroll foreground probe outside the workspace.

.DESCRIPTION
    Copies probe*.rs into a scratch crate that depends on the adapter crate of
    -RepoRoot (the checkout whose client stack the probe must use) and builds
    it in release mode. Prints the path of the binary.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$RepoRoot,
    [Parameter(Mandatory = $true)][string]$WorkDir,
    [string]$TargetDir = ''
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $PSCommandPath
$repo = (Resolve-Path -LiteralPath $RepoRoot).ProviderPath.Replace('\', '/')
New-Item -ItemType Directory -Force (Join-Path $WorkDir 'src') | Out-Null
$manifest = @(
    '[package]'
    'name = "agent-desktop-scroll-foreground-probe"'
    'version = "0.0.0"'
    'edition = "2021"'
    ''
    '[dependencies]'
    'serde_json = "1"'
    'uiautomation = { version = "=0.25.0", default-features = false, features = ["control", "input"] }'
    ('agent-desktop-windows = { path = "' + $repo + '/crates/windows" }')
    'windows = { version = "0.62.2", features = ["Win32_UI_Accessibility", "Win32_System_Com"] }'
    'windows-sys = { version = "0.61", features = ['
    '  "Win32_Foundation",'
    '  "Win32_Graphics_Dwm",'
    '  "Win32_System_SystemInformation",'
    '  "Win32_System_Threading",'
    '  "Win32_UI_Accessibility",'
    '  "Win32_UI_WindowsAndMessaging",'
    '] }'
    ''
    '[workspace]'
) -join "`n"
$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[IO.File]::WriteAllText((Join-Path $WorkDir 'Cargo.toml'), $manifest, $utf8NoBom)
Copy-Item -LiteralPath (Join-Path $RepoRoot 'Cargo.lock') -Destination (Join-Path $WorkDir 'Cargo.lock') -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot 'rust-toolchain.toml') -Destination (Join-Path $WorkDir 'rust-toolchain.toml') -Force
Get-ChildItem -LiteralPath $here -Filter 'probe*.rs' | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $WorkDir ('src\' + $_.Name)) -Force
}
Copy-Item -LiteralPath (Join-Path $WorkDir 'src\probe.rs') -Destination (Join-Path $WorkDir 'src\main.rs') -Force
if ($TargetDir) { $env:CARGO_TARGET_DIR = $TargetDir }
Push-Location $WorkDir
try {
    $ErrorActionPreference = 'Continue'
    & cargo build --release 2>&1 | ForEach-Object { "$_" } | Select-Object -Last 6
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
} finally {
    Pop-Location
}
$root = if ($TargetDir) { $TargetDir } else { Join-Path $WorkDir 'target' }
'probe=' + (Join-Path $root 'release\agent-desktop-scroll-foreground-probe.exe')
