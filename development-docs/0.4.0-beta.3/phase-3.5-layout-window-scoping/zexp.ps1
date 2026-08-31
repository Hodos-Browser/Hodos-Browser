<#
.SYNOPSIS
  P3.5 Z-order MECHANISM experiment — runs entirely OUTSIDE the product, no rebuild.

.DESCRIPTION
  K9.3 records the Z-order drop as a CANDIDATE cause ("all 14 overlays are owned by the
  primary window") and says the decisive experiment is the fix itself. It is not: the
  whole show path reduces to one Win32 call —

      SetWindowPos(overlay, HWND_TOPMOST, x,y,w,h, SWP_NOACTIVATE|SWP_SHOWWINDOW)

  (📏 simple_app.cpp :: ShowMenuOverlay — no SetFocus, no SetForegroundWindow). So the
  hypothesis can be tested from outside the process, against the LIVE overlay HWNDs,
  before a single line of product code is written:

    ARM 1  show the overlay while it is still owned by A   -> expect A to rise above B
    ARM 2  re-own it to B (GWLP_HWNDPARENT), show again    -> expect B to STAY above A
    ARM 3  restore ownership to A, show again              -> expect the drop to RETURN

  ⭐ ARM 3 is the negative control: without it, ARM 2 proves only that something changed.

⛔ DEV BUILD ONLY. Windows are resolved from the one dev process (matched by exe path).
   The installed browser is never enumerated.
#>
[CmdletBinding()]
param(
    [string]$DevPathMatch = 'build\bin\Release',
    [string]$OverlayClass = 'CEFMenuOverlayWindow'
)
Set-StrictMode -Version Latest

Add-Type -Namespace ZX -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int idx);
[DllImport("user32.dll", EntryPoint="SetWindowLongPtrW")] public static extern IntPtr SetWindowLongPtr(IntPtr h, int idx, IntPtr v);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@

$HWND_TOPMOST  = [IntPtr](-1)
$HWND_TOP      = [IntPtr]0
$SWP_NOACTIVATE = 0x0010; $SWP_SHOWWINDOW = 0x0040
$SWP_NOMOVE = 0x0002; $SWP_NOSIZE = 0x0001
$GWLP_HWNDPARENT = -8
$SW_HIDE = 0

$procs = @(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
           Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" -and $_.CommandLine -notmatch '--type=' })
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - expected 1 dev browser process, found $($procs.Count)" -ForegroundColor Red; exit 1 }
$pidOne = $procs[0].ProcessId
Write-Host "SUBJECT: PASS - one dev browser process (pid $pidOne)" -ForegroundColor Green

function Get-Sample {
    $cls = New-Object System.Text.StringBuilder 512
    $rows = New-Object System.Collections.Generic.List[object]
    $script:z = 0
    $cb = [ZX.W+EnumProc]{
        param($h, $p)
        $wpid = 0; [void][ZX.W]::GetWindowThreadProcessId($h, [ref]$wpid)
        if ($wpid -ne $pidOne) { return $true }
        [void]$cls.Clear(); [void][ZX.W]::GetClassNameW($h, $cls, 512)
        if ($cls.ToString() -notmatch '^(HodosBrowserWndClass|CEF.*OverlayWindow)$') { return $true }
        $script:z++
        $r = New-Object ZX.W+RECT; [void][ZX.W]::GetWindowRect($h, [ref]$r)
        $rows.Add([pscustomobject]@{
            Z = $script:z; Hwnd = ('0x{0:X}' -f [int64]$h); Class = $cls.ToString()
            Vis = [ZX.W]::IsWindowVisible($h)
            Rect = ('{0},{1} {2}x{3}' -f $r.Left, $r.Top, ($r.Right-$r.Left), ($r.Bottom-$r.Top))
            H = $h
        })
        return $true
    }
    [void][ZX.W]::EnumWindows($cb, [IntPtr]::Zero)
    return $rows
}

function Show-Z {
    param([string]$Label)
    $s = Get-Sample
    $shells  = @($s | Where-Object { $_.Class -eq 'HodosBrowserWndClass' })
    $ov      = @($s | Where-Object { $_.Class -eq $OverlayClass })
    $line = ($shells | ForEach-Object { '{0}@Z{1}' -f $_.Hwnd, $_.Z }) -join '  '
    $ovl  = if ($ov.Count) { '{0}@Z{1} Vis={2} {3}' -f $ov[0].Hwnd, $ov[0].Z, $ov[0].Vis, $ov[0].Rect } else { 'absent' }
    Write-Host ("  {0,-26} shells: {1}   overlay: {2}" -f $Label, $line, $ovl)
}

$s = Get-Sample
$shells = @($s | Where-Object { $_.Class -eq 'HodosBrowserWndClass' })
$ovr    = @($s | Where-Object { $_.Class -eq $OverlayClass })
if ($shells.Count -ne 2) { Write-Host "FAIL - need exactly 2 shell windows, found $($shells.Count). Use Ctrl+N." -ForegroundColor Red; exit 1 }
if ($ovr.Count -ne 1)    { Write-Host "FAIL - overlay class $OverlayClass not found." -ForegroundColor Red; exit 1 }

# A = the primary = the one created first. EnumWindows gives Z-order, not age, so identify
# A by rect: the primary is the maximised 0,0 window in this rig. Print both and let the
# reader check; do not silently guess.
$A = $shells | Where-Object { $_.Rect -like '0,0 *' } | Select-Object -First 1
$B = $shells | Where-Object { $_.Hwnd -ne $A.Hwnd }   | Select-Object -First 1
$OV = $ovr[0]
Write-Host ("A (primary, 0,0) = {0}   B (secondary) = {1}   overlay = {2}" -f $A.Hwnd, $B.Hwnd, $OV.Hwnd)
$origOwner = [ZX.W]::GetWindowLongPtr($OV.H, $GWLP_HWNDPARENT)
Write-Host ("overlay current owner = 0x{0:X}  (A={1})" -f [int64]$origOwner, $A.Hwnd)
Write-Host ""

# Overlay geometry to re-apply on every show, so only the OWNER differs between arms.
$r = New-Object ZX.W+RECT; [void][ZX.W]::GetWindowRect($OV.H, [ref]$r)
$ox=$r.Left; $oy=$r.Top; $ow=$r.Right-$r.Left; $oh=$r.Bottom-$r.Top

function Reset-Baseline {
    [void][ZX.W]::ShowWindow($OV.H, $SW_HIDE)
    Start-Sleep -Milliseconds 250
    # Put B above A without activating either — the state a user is in before clicking.
    [void][ZX.W]::SetWindowPos($B.H, $HWND_TOP, 0,0,0,0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_NOACTIVATE))
    Start-Sleep -Milliseconds 250
}
function Do-Show {
    [void][ZX.W]::SetWindowPos($OV.H, $HWND_TOPMOST, $ox, $oy, $ow, $oh, ($SWP_NOACTIVATE -bor $SWP_SHOWWINDOW))
    Start-Sleep -Milliseconds 400
}

foreach ($arm in @(
    @{ n='ARM 1  owner = A (as shipped)'; owner=$A.H },
    @{ n='ARM 2  owner = B (candidate)';  owner=$B.H },
    @{ n='ARM 3  owner = A (neg control)';owner=$A.H }
)) {
    Write-Host $arm.n -ForegroundColor Cyan
    Reset-Baseline
    [void][ZX.W]::SetWindowLongPtr($OV.H, $GWLP_HWNDPARENT, $arm.owner)
    Show-Z 'before show'
    Do-Show
    Show-Z 'after  show'
    Write-Host ""
}

# Always leave the process as we found it.
[void][ZX.W]::ShowWindow($OV.H, $SW_HIDE)
[void][ZX.W]::SetWindowLongPtr($OV.H, $GWLP_HWNDPARENT, $origOwner)
$now = [ZX.W]::GetWindowLongPtr($OV.H, $GWLP_HWNDPARENT)
Write-Host ("RESTORED overlay owner to 0x{0:X} (was 0x{1:X})" -f [int64]$now, [int64]$origOwner) -ForegroundColor Green
