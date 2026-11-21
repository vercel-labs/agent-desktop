#Requires -Version 5.1
<#
.SYNOPSIS
    Windows counterpart of scripts/prune-ci-target-cache.sh: reduces a cargo
    target directory to the compiled third-party dependencies a later run can
    reuse.

.DESCRIPTION
    Incremental state, example and test binaries, workspace rlibs, top-level
    executables and symbol trees are rebuilt on every run, so they are removed
    before the directory is cached. Entries whose crate Cargo.lock resolves but
    the workspace does not own are kept: `windows_*` also spells the windows-*
    dependency family, and dropping those rlibs would discard the most
    expensive work the cache exists to carry.

.PARAMETER TargetDir
    The target directory to prune. Defaults to CARGO_TARGET_DIR, then target.
#>
[CmdletBinding()]
param(
    [string] $TargetDir = $(if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' })
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $TargetDir)) {
    exit 0
}

$workspacePatterns = @(
    'agent_desktop*', 'libagent_desktop*', 'agent-desktop*', 'c_abi_*', 'c_header_*',
    'cpp_header_*', 'error_lifetime*', 'cli_*', 'mcp_*', 'command_*', 'snapshot_*',
    'windows_*', 'retained_fixture*', 'locator_benchmark*', 'uia_tree_dump*', 'ax_probe*'
)

$externalCrates = @()
$lockFile = Join-Path (Split-Path -Parent $PSScriptRoot) 'Cargo.lock'
if (Test-Path -LiteralPath $lockFile) {
    $externalCrates = @(Get-Content -LiteralPath $lockFile |
        Select-String -Pattern '^name = "(.*)"$' |
        ForEach-Object { $_.Matches[0].Groups[1].Value.Replace('-', '_') } |
        Where-Object { -not $_.StartsWith('agent_desktop') } |
        Sort-Object -Unique)
}

function Test-ExternalCrate([string] $Stem) {
    return ($externalCrates -contains $Stem)
}

function Get-ArtifactStem([string] $Name) {
    $stem = [System.IO.Path]::GetFileNameWithoutExtension($Name)
    if ($stem.StartsWith('lib')) {
        $stem = $stem.Substring(3)
    }
    $cut = $stem.LastIndexOf('-')
    if ($cut -gt 0) {
        $stem = $stem.Substring(0, $cut)
    }
    return $stem
}

function Remove-Tree([string] $Path) {
    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
}

function Remove-WorkspaceArtifacts([string] $Directory) {
    if (-not (Test-Path -LiteralPath $Directory)) {
        return
    }
    foreach ($entry in Get-ChildItem -LiteralPath $Directory -Force) {
        $matched = $false
        foreach ($pattern in $workspacePatterns) {
            if ($entry.Name -like $pattern) {
                $matched = $true
                break
            }
        }
        if (-not $matched) {
            continue
        }
        if (Test-ExternalCrate (Get-ArtifactStem $entry.Name)) {
            continue
        }
        Remove-Item -LiteralPath $entry.FullName -Recurse -Force
    }
}

foreach ($profileDir in Get-ChildItem -LiteralPath $TargetDir -Directory -Force) {
    Remove-Tree (Join-Path $profileDir.FullName 'incremental')
    Remove-Tree (Join-Path $profileDir.FullName 'examples')
    Remove-Tree (Join-Path $profileDir.FullName 'tmp')
    Remove-WorkspaceArtifacts (Join-Path $profileDir.FullName 'deps')
    Remove-WorkspaceArtifacts (Join-Path $profileDir.FullName '.fingerprint')
}

Remove-Tree (Join-Path $TargetDir 'tmp')

foreach ($flavor in @('debug', 'release', 'release-ffi')) {
    $flavorDir = Join-Path $TargetDir $flavor
    if (-not (Test-Path -LiteralPath $flavorDir)) {
        continue
    }
    foreach ($entry in Get-ChildItem -LiteralPath $flavorDir -Force) {
        if ($entry.Name -like 'agent-desktop*' -or $entry.Name -like 'libagent_desktop*' -or $entry.Name -like '*.dSYM') {
            Remove-Item -LiteralPath $entry.FullName -Recurse -Force
        }
    }
}

$debugDeps = Join-Path (Join-Path $TargetDir 'debug') 'deps'
if (Test-Path -LiteralPath $debugDeps) {
    Get-ChildItem -LiteralPath $debugDeps -Filter '*.dSYM' -Force |
        ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force }
}
