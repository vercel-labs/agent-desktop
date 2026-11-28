#Requires -Version 5.1
<#
.SYNOPSIS
    Scroll foreground-effect probe (A31): Part B, the shipped commands end to end.

.DESCRIPTION
    For each row, starts the WPF scratch target fresh, keeps Notepad in the
    foreground, takes a fresh snapshot with the binary under test, then runs
    one command through the probe's `cli` bracket. Each row runs twice.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Probe,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [Parameter(Mandatory = $true)][string]$Rows
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $PSCommandPath
New-Item -ItemType Directory -Force $OutDir | Out-Null

function Get-Tops { $text = (& $Probe tops) -join "`n"; @((ConvertFrom-Json $text) | ForEach-Object { $_ }) }
function Wait-Top {
    param([string]$Class, [string]$Title, [int]$ProcessId)
    foreach ($i in 1..200) {
        $hit = Get-Tops | Where-Object { $_.class -like $Class -and $_.title -like $Title -and $_.pid -eq $ProcessId } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 300
    }
    return $null
}
function Find-Node {
    param($Node, [scriptblock]$Match)
    if (& $Match $Node) { return $Node }
    foreach ($child in @($Node.children)) {
        if ($child) { $hit = Find-Node $child $Match; if ($hit) { return $hit } }
    }
    return $null
}

$notepad = Start-Process notepad.exe -PassThru
$f = Wait-Top 'Notepad' '*' $notepad.Id
try {
    foreach ($spec in ($Rows -split ';')) {
        $id, $binary, $command, $value, $mode = $spec -split '\|'
        foreach ($run in 1, 2) {
            $target = Start-Process (Join-Path $here 'targets\bin\SfpWpf.exe') -PassThru
            $t = Wait-Top 'HwndWrapper*' 'SFP WPF' $target.Id
            Start-Sleep -Seconds 2
            $env:AGENT_DESKTOP_HOME = Join-Path $OutDir ('home-' + [IO.Path]::GetFileName((Split-Path -Parent $binary)))
            $snapshotText = (& $binary snapshot --app SfpWpf) -join "`n"
            $snapshot = $null
            try { $snapshot = ConvertFrom-Json $snapshotText } catch { }
            $list = $null
            if ($snapshot -and $snapshot.ok) { $list = Find-Node $snapshot.data.tree { param($n) ($n.role -eq 'list' -or $n.role -eq 'listbox') -and $n.ref_id } }
            if (-not $list) {
                "$id run$run NO LIST REF snapshot_ok=$($snapshot.ok) code=$($snapshot.error.code) text=$($snapshotText.Substring(0, [Math]::Min(300, $snapshotText.Length)))"
                Stop-Process -Id $target.Id -Force
                continue
            }
            $argv = @($command, $list.ref_id)
            if ($command -eq 'scroll') { $argv += @('--direction', 'down') } else { $argv += $value }
            if ($mode -eq 'headed') { $argv += '--headed' }
            $argsFile = Join-Path $OutDir "$id-run$run.args.json"
            [IO.File]::WriteAllText($argsFile, (ConvertTo-Json @($argv) -Compress), (New-Object System.Text.UTF8Encoding $false))
            $out = Join-Path $OutDir "$id-run$run.json"
            & $Probe cli --f $f.hwnd --t $t.hwnd --exe $binary --args-file $argsFile --aid lstRows --label $id --out $out | Out-Null
            $doc = Get-Content $out -Raw | ConvertFrom-Json
            "$id run$run exit=$LASTEXITCODE ok=$($doc.envelope.ok) code=$($doc.envelope.error.code) changed=$($doc.row.changed_at_any_time -join ',') fgEvents=$($doc.row.winevents.foreground) percent=$($doc.row.before.v_percent)>$($doc.row.after.v_percent) selected=$($doc.selection_after -join ',')"
            Stop-Process -Id $target.Id -Force
            Start-Sleep -Seconds 1
        }
    }
} finally {
    Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
}
'DONE'
