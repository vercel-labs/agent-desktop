#Requires -Version 5.1

Set-StrictMode -Version 2.0

function Invoke-WindowOpsScenario {
    <# Window commands on the fixture's own top-level window. Each effect is read back through GetWindowRect, never from the command's ok. No helper in NativeDesktop.psm1 reads minimized or maximized state, so minimize and maximize assert that the rect moved, and restore asserts that the rect returned to its pre-maximize value. #>
    [CmdletBinding()]
    param()
    Register-Legs -Names @('window-resize', 'window-move', 'window-maximize', 'window-restore', 'window-minimize')
    $tolerancePx = 3

    Enter-Stage -Lock DesktopLease -Body {
        $windowId = Get-WindowId -Where { $_['title'] -eq 'AgentDeskFixture' } -TimeoutSeconds 10
        if (-not $windowId) { throw 'window ops: could not resolve the fixture main window id' }
        $handle = ConvertTo-NativeWindowHandle -WindowId $windowId
        $original = Get-NativeWindowRect -WindowHandle $handle
        $beforeMaximize = $null

        try {
            try {
                $before = Get-NativeWindowRect -WindowHandle $handle
                $width = ($before.Right - $before.Left) - 40
                $height = ($before.Bottom - $before.Top) - 30
                $envelope = Invoke-AgentDesktop -Arguments @('resize-window', '--window-id', $windowId, '--width', [string]$width, '--height', [string]$height)
                Assert-Envelope -Envelope $envelope -Ok
                $deadline = [System.Diagnostics.Stopwatch]::StartNew()
                $settled = $false
                $rect = $null
                while ($deadline.Elapsed.TotalSeconds -lt 10 -and -not $settled) {
                    $rect = Get-NativeWindowRect -WindowHandle $handle
                    $settled = ([Math]::Abs(($rect.Right - $rect.Left) - $width) -le $tolerancePx) -and ([Math]::Abs(($rect.Bottom - $rect.Top) - $height) -le $tolerancePx)
                    if (-not $settled) { Start-Sleep -Milliseconds 150 }
                }
                if (-not $settled) { throw "window-resize: the native rect is $($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top) within 10s of resize-window to ${width}x${height}" }
                Add-Pass -Leg 'window-resize'
            } catch {
                Add-Fail -Leg 'window-resize' -Reason $_.Exception.Message
            }

            try {
                $before = Get-NativeWindowRect -WindowHandle $handle
                $x = $before.Left + 40
                $y = $before.Top + 30
                $envelope = Invoke-AgentDesktop -Arguments @('move-window', '--window-id', $windowId, '--x', [string]$x, '--y', [string]$y)
                Assert-Envelope -Envelope $envelope -Ok
                $deadline = [System.Diagnostics.Stopwatch]::StartNew()
                $settled = $false
                $rect = $null
                while ($deadline.Elapsed.TotalSeconds -lt 10 -and -not $settled) {
                    $rect = Get-NativeWindowRect -WindowHandle $handle
                    $settled = ([Math]::Abs($rect.Left - $x) -le $tolerancePx) -and ([Math]::Abs($rect.Top - $y) -le $tolerancePx)
                    if (-not $settled) { Start-Sleep -Milliseconds 150 }
                }
                if (-not $settled) { throw "window-move: the native rect origin is $($rect.Left),$($rect.Top) within 10s of move-window to $x,$y" }
                Add-Pass -Leg 'window-move'
            } catch {
                Add-Fail -Leg 'window-move' -Reason $_.Exception.Message
            }

            try {
                $beforeMaximize = Get-NativeWindowRect -WindowHandle $handle
                $envelope = Invoke-AgentDesktop -Arguments @('maximize', '--window-id', $windowId)
                Assert-Envelope -Envelope $envelope -Ok
                $deadline = [System.Diagnostics.Stopwatch]::StartNew()
                $grown = $false
                $rect = $null
                while ($deadline.Elapsed.TotalSeconds -lt 10 -and -not $grown) {
                    $rect = Get-NativeWindowRect -WindowHandle $handle
                    $grown = (($rect.Right - $rect.Left) -gt (($beforeMaximize.Right - $beforeMaximize.Left) + $tolerancePx)) -and (($rect.Bottom - $rect.Top) -gt (($beforeMaximize.Bottom - $beforeMaximize.Top) + $tolerancePx))
                    if (-not $grown) { Start-Sleep -Milliseconds 150 }
                }
                if (-not $grown) { throw "window-maximize: the native rect is still $($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top) within 10s of maximize" }
                Add-Pass -Leg 'window-maximize'
            } catch {
                Add-Fail -Leg 'window-maximize' -Reason $_.Exception.Message
            }

            try {
                if ($null -eq $beforeMaximize) { throw 'window-restore: no pre-maximize rect was captured' }
                $envelope = Invoke-AgentDesktop -Arguments @('restore', '--window-id', $windowId)
                Assert-Envelope -Envelope $envelope -Ok
                $deadline = [System.Diagnostics.Stopwatch]::StartNew()
                $settled = $false
                $rect = $null
                while ($deadline.Elapsed.TotalSeconds -lt 10 -and -not $settled) {
                    $rect = Get-NativeWindowRect -WindowHandle $handle
                    $settled = ([Math]::Abs($rect.Left - $beforeMaximize.Left) -le $tolerancePx) -and ([Math]::Abs($rect.Top - $beforeMaximize.Top) -le $tolerancePx) -and ([Math]::Abs($rect.Right - $beforeMaximize.Right) -le $tolerancePx) -and ([Math]::Abs($rect.Bottom - $beforeMaximize.Bottom) -le $tolerancePx)
                    if (-not $settled) { Start-Sleep -Milliseconds 150 }
                }
                if (-not $settled) { throw "window-restore: the native rect $($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom) did not return to the pre-maximize rect $($beforeMaximize.Left),$($beforeMaximize.Top),$($beforeMaximize.Right),$($beforeMaximize.Bottom) within 10s" }
                Add-Pass -Leg 'window-restore'
            } catch {
                Add-Fail -Leg 'window-restore' -Reason $_.Exception.Message
            }

            try {
                $before = Get-NativeWindowRect -WindowHandle $handle
                $envelope = Invoke-AgentDesktop -Arguments @('minimize', '--window-id', $windowId)
                Assert-Envelope -Envelope $envelope -Ok
                $deadline = [System.Diagnostics.Stopwatch]::StartNew()
                $moved = $false
                $rect = $null
                while ($deadline.Elapsed.TotalSeconds -lt 10 -and -not $moved) {
                    $rect = Get-NativeWindowRect -WindowHandle $handle
                    $moved = ([Math]::Abs($rect.Left - $before.Left) -gt $tolerancePx) -or ([Math]::Abs($rect.Top - $before.Top) -gt $tolerancePx) -or ([Math]::Abs($rect.Right - $before.Right) -gt $tolerancePx) -or ([Math]::Abs($rect.Bottom - $before.Bottom) -gt $tolerancePx)
                    if (-not $moved) { Start-Sleep -Milliseconds 150 }
                }
                if (-not $moved) { throw "window-minimize: the native rect $($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom) never moved from $($before.Left),$($before.Top),$($before.Right),$($before.Bottom) within 10s of minimize" }
                Add-Pass -Leg 'window-minimize'
            } catch {
                Add-Fail -Leg 'window-minimize' -Reason $_.Exception.Message
            }
        } finally {
            $restored = Invoke-AgentDesktop -Arguments @('restore', '--window-id', $windowId)
            Assert-Envelope -Envelope $restored -Ok
            $sized = Invoke-AgentDesktop -Arguments @('resize-window', '--window-id', $windowId, '--width', [string]($original.Right - $original.Left), '--height', [string]($original.Bottom - $original.Top))
            Assert-Envelope -Envelope $sized -Ok
            $placed = Invoke-AgentDesktop -Arguments @('move-window', '--window-id', $windowId, '--x', [string]$original.Left, '--y', [string]$original.Top)
            Assert-Envelope -Envelope $placed -Ok
            $deadline = [System.Diagnostics.Stopwatch]::StartNew()
            $settled = $false
            $rect = $null
            while ($deadline.Elapsed.TotalSeconds -lt 10 -and -not $settled) {
                $rect = Get-NativeWindowRect -WindowHandle $handle
                $settled = ([Math]::Abs($rect.Left - $original.Left) -le $tolerancePx) -and ([Math]::Abs($rect.Top - $original.Top) -le $tolerancePx) -and ([Math]::Abs($rect.Right - $original.Right) -le $tolerancePx) -and ([Math]::Abs($rect.Bottom - $original.Bottom) -le $tolerancePx)
                if (-not $settled) { Start-Sleep -Milliseconds 150 }
            }
            if (-not $settled) { throw "window ops cleanup: the native rect $($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom) did not return to the original rect $($original.Left),$($original.Top),$($original.Right),$($original.Bottom) within 10s" }
        }
    }
}
