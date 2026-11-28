#Requires -Version 5.1
<#
.SYNOPSIS
    Scroll foreground-effect probe (A31): Part A driver.

.DESCRIPTION
    Starts Notepad as the foreground window F, then for each UI toolkit target
    starts the target, runs the Rust probe over three arrangements, closes the
    target, starts it fresh and runs the probe again. One JSON capture per
    target and session lands in -OutDir.

    Run it from an interactive console session. Build the probe first with
    build-probe.ps1 and the scratch targets with targets\build-targets.ps1.
    -Mode find only lists the vertically scrollable elements of each target.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Probe,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$Targets = 'winforms,wpf,win32,directui,xaml,chromium,chromium-div',
    [ValidateSet('find', 'run')][string]$Mode = 'run',
    [int]$Sessions = 2,
    [int]$Trials = 5,
    [string]$Arrangements = 'beside,covered,minimized',
    [ValidateSet('default', 'off')][string]$AutoSetFocus = 'default'
)

$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $PSCommandPath
$scratch = Join-Path $OutDir 'scratch'
New-Item -ItemType Directory -Force $OutDir, $scratch | Out-Null
$filesDir = Join-Path $scratch 'sfp-files300'

function Get-Tops {
    $text = (& $Probe tops) -join "`n"
    @((ConvertFrom-Json $text) | ForEach-Object { $_ })
}

function Wait-Top {
    param([string]$Class, [string]$Title, [int]$ProcessId = 0, [int]$Ms = 60000)
    $deadline = (Get-Date).AddMilliseconds($Ms)
    do {
        $hit = Get-Tops | Where-Object {
            $_.class -like $Class -and $_.title -like $Title -and ($ProcessId -eq 0 -or $_.pid -eq $ProcessId)
        } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 300
    } while ((Get-Date) -lt $deadline)
    Write-Host ("Wait-Top miss class=$Class pid=$ProcessId rows=" + @(Get-Tops).Count + ' same-class=' + @(Get-Tops | Where-Object { $_.class -like $Class }).Count)
    return $null
}

function Expand-TreeItems {
    param($Hwnd)
    Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public static class SfpTree {
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    const uint TVM_EXPAND = 0x1102, TVM_GETNEXTITEM = 0x110A;
    public static int ExpandAll(IntPtr top) {
        IntPtr tree = IntPtr.Zero;
        EnumChildWindows(top, (h, l) => { var s = new StringBuilder(64); GetClassName(h, s, 64); if (s.ToString() == "SysTreeView32") { tree = h; return false; } return true; }, IntPtr.Zero);
        if (tree == IntPtr.Zero) return -1;
        int n = 0;
        IntPtr root = SendMessage(tree, TVM_GETNEXTITEM, (IntPtr)0, IntPtr.Zero);
        for (IntPtr item = root; item != IntPtr.Zero; item = SendMessage(tree, TVM_GETNEXTITEM, (IntPtr)1, item)) {
            SendMessage(tree, TVM_EXPAND, (IntPtr)2, item);
            for (IntPtr kid = SendMessage(tree, TVM_GETNEXTITEM, (IntPtr)4, item); kid != IntPtr.Zero; kid = SendMessage(tree, TVM_GETNEXTITEM, (IntPtr)1, kid)) {
                SendMessage(tree, TVM_EXPAND, (IntPtr)2, kid); n++;
            }
        }
        return n;
    }
}
"@
    $deadline = (Get-Date).AddSeconds(20)
    do { Start-Sleep -Milliseconds 500; $count = [SfpTree]::ExpandAll([IntPtr][long]$Hwnd) } while ($count -lt 5 -and (Get-Date) -lt $deadline)
    Write-Host ('expanded tree items: ' + $count)
}

function Start-Target {
    param([string]$Name)
    switch ($Name) {
        'winforms' {
            $p = Start-Process (Join-Path $here 'targets\bin\SfpForms.exe') -PassThru
            return @{ window = (Wait-Top 'WindowsForms10*' 'SFP Forms' $p.Id); stop = @($p.Id)
                sel = @('--aid', 'lstRows'); items = 'Row 400|Row 300|Row 450|Row 250|Row 350' }
        }
        'wpf' {
            $p = Start-Process (Join-Path $here 'targets\bin\SfpWpf.exe') -PassThru
            return @{ window = (Wait-Top 'HwndWrapper*' 'SFP WPF' $p.Id); stop = @($p.Id)
                sel = @('--aid', 'lstRows'); items = 'Row 400|Row 300|Row 450|Row 250|Row 350' }
        }
        'win32' {
            $p = Start-Process mmc.exe -ArgumentList 'services.msc' -PassThru
            return @{ window = (Wait-Top 'MMCMainFrame' 'Services' $p.Id 40000); stop = @($p.Id)
                sel = @('--cls', 'SysListView32')
                items = 'Workstation|Windows Update|Windows Time|Windows Installer|Windows Event Log' }
        }
        'win32tree' {
            $p = Start-Process mmc.exe -ArgumentList 'devmgmt.msc' -PassThru
            $top = Wait-Top 'MMCMainFrame' 'Device Manager' $p.Id 60000
            if ($top) { Expand-TreeItems $top.hwnd }
            return @{ window = $top; stop = @($p.Id)
                sel = @('--cls', 'SysTreeView32'); items = '' }
        }
        { $_ -in 'directui', 'win32navtree' } {
            if (-not (Test-Path $filesDir)) {
                New-Item -ItemType Directory -Force $filesDir | Out-Null
                1..300 | ForEach-Object { Set-Content (Join-Path $filesDir ('file{0:D3}.txt' -f $_)) "x$_" }
            }
            (New-Object -ComObject Shell.Application).Open($filesDir)
            return @{ window = (Wait-Top 'CabinetWClass' '*sfp-files300*' 0 60000); stop = @(); close = 'explorer'
                sel = @('--cls', $(if ($Name -eq 'win32navtree') { 'SysTreeView32' } else { 'UIItemsView' }))
                items = $(if ($Name -eq 'win32navtree') { '' } else { 'file250|file200|file290|file150|file270' }) }
        }
        'xaml' {
            $before = @(Get-Process SystemSettings -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
            Start-Process 'ms-settings:about'
            $window = Wait-Top 'ApplicationFrameWindow' 'Settings' 0 40000
            Start-Sleep -Seconds 4
            $mine = @(Get-Process SystemSettings -ErrorAction SilentlyContinue |
                Where-Object { $before -notcontains $_.Id } | ForEach-Object { $_.Id })
            $core = Get-Tops | Where-Object { $_.class -eq 'Windows.UI.Core.CoreWindow' -and $mine -contains $_.pid } |
                Select-Object -First 1
            $sel = @()
            if ($core) { $sel = @('--root', $core.hwnd) }
            return @{ window = $window; stop = $mine; sel = $sel; items = ''; preexisting = $before }
        }
        { $_ -like 'chromium*' } {
            $profile = Join-Path $scratch 'chrome-profile'
            $page = 'file:///' + ((Join-Path $here 'targets\scroll.html') -replace '\\', '/')
            $chrome = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe').'(default)'
            $argv = @("--user-data-dir=`"$profile`"", '--no-first-run', '--no-default-browser-check',
                '--force-renderer-accessibility', '--window-size=720,520', '--new-window', $page)
            Start-Process $chrome -ArgumentList $argv | Out-Null
            return @{ window = (Wait-Top 'Chrome_WidgetWin_1' 'SFP Scroll Page*' 0 40000); stop = @(); close = 'chrome'
                profile = $profile; sel = @('--aid', $(if ($Name -eq 'chromium-div') { 'tall' } else { 'RootWebArea' })); items = '' }
        }
    }
    return $null
}

function Stop-Target {
    param($Info)
    foreach ($id in $Info.stop) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    if ($Info.close -eq 'explorer') {
        (New-Object -ComObject Shell.Application).Windows() |
            Where-Object { $_.LocationURL -like '*sfp-files300*' } | ForEach-Object { $_.Quit() }
    }
    if ($Info.close -eq 'chrome') {
        Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" |
            Where-Object { $_.CommandLine -like ('*' + $Info.profile + '*') } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    }
    Start-Sleep -Seconds 2
}

$notepad = Start-Process notepad.exe -PassThru
$f = Wait-Top 'Notepad' '*' $notepad.Id
if (-not $f) { 'FAIL no Notepad window'; exit 1 }
'F hwnd=' + $f.hwnd + ' pid=' + $f.pid

try {
    foreach ($name in ($Targets -split ',')) {
        $sessionCount = if ($Mode -eq 'find') { 1 } else { $Sessions }
        foreach ($session in 1..$sessionCount) {
            $info = Start-Target $name
            if (-not $info -or -not $info.window) {
                "$name s$session FAIL no target window"
                if ($info) { Stop-Target $info }
                continue
            }
            Start-Sleep -Seconds 3
            if ($Mode -eq 'find') {
                $out = Join-Path $OutDir "find-$name.json"
                $findArgs = @('find', '--t', $info.window.hwnd, '--out', $out) + @($info.sel | Where-Object { $_ })
                & $Probe @findArgs | Out-Null
                "$name find exit=$LASTEXITCODE out=$out"
            } else {
                $out = Join-Path $OutDir "$name-s$session.json"
                $argv = @('run', '--f', $f.hwnd, '--t', $info.window.hwnd, '--label', $name, '--session', $session,
                    '--trials', $Trials, '--arrangements', $Arrangements, '--auto-set-focus', $AutoSetFocus, '--out', $out) + $info.sel
                if ($info.items) { $argv += @('--item-names', $info.items) }
                $watch = [Diagnostics.Stopwatch]::StartNew()
                & $Probe @argv | Out-Null
                "$name s$session exit=$LASTEXITCODE seconds=$([int]$watch.Elapsed.TotalSeconds) out=$out"
            }
            Stop-Target $info
        }
    }
} finally {
    Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
}
'DONE'
