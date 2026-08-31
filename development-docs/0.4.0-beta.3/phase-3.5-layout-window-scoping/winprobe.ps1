<#
.SYNOPSIS
  P3.5 SUBJECT probe: prove the two windows under test live in ONE dev process,
  and dump each top-level window's rect + DPI.

.DESCRIPTION
  The vacuous-test trap for this phase is testing two windows that are actually two
  PROCESSES (two different profiles). Such a pair passes every row while proving
  nothing, because each process has its own WindowManager and its own primary window.

  ⛔ Matches the DEV build by EXE PATH, never by process name. The owner's installed
  browser lives in %LOCALAPPDATA%\HodosBrowser and must never be touched, counted or
  signalled. This script only READS; it starts and stops nothing.

.OUTPUTS
  PASS  — exactly one non---type= (browser) process from build\bin\Release
  FAIL  — zero (nothing running) or more than one (two processes, not two windows)
#>
[CmdletBinding()]
param(
    # Substring of the dev build's exe path. Anything not matching is ignored entirely.
    [string]$DevPathMatch = 'build\bin\Release',

    # ⛔ WATCH MODE EXISTS BECAUSE THE ONE-SHOT MODE CANNOT SEE ITS OWN SUBJECT.
    # Every dropdown overlay is hidden on focus loss (HideAllOverlays via WM_ACTIVATEAPP,
    # cef_browser_shell.cpp :: HideAllOverlays). Clicking a PowerShell window to run this
    # script IS a focus loss, so a one-shot run always reports "no overlay" no matter what
    # was on screen a moment earlier. Same family as the CDP timing instrument in Phase 2:
    # sample continuously, never ask the observer to interact with the thing under test.
    #
    # Usage: start this FIRST, then switch to the browser and perform the action.
    [int]$WatchSeconds = 0,

    [int]$IntervalMs = 200
)

Set-StrictMode -Version Latest

Add-Type -Namespace P35 -Name Win -MemberDefinition @'
// ⛔ CharSet.Unicode is load-bearing on the two string calls. Without it the marshaller
// treats the W entry point as ANSI, .NET reads the UTF-16 buffer as single-byte, and every
// name truncates at the first embedded NUL -- "HodosBrowserWindow" reads back as "H".
// Same trap as FindWindowW; it produces plausible output, which is why it survives review.
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
public struct RECT { public int Left, Top, Right, Bottom; }
'@

$procs = @(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
           Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" })

if ($procs.Count -eq 0) {
    Write-Host "SUBJECT: FAIL - no DEV browser process found under *$DevPathMatch*." -ForegroundColor Red
    Write-Host "         (Any installed-build processes are deliberately ignored.)"
    exit 1
}

# The browser process is the one with no --type= switch. Renderers/utilities all carry one.
$browsers = @($procs | Where-Object { $_.CommandLine -notmatch '--type=' })

Write-Host "DEV processes under *$DevPathMatch*: $($procs.Count) total, $($browsers.Count) browser" -ForegroundColor Cyan
foreach ($b in $browsers) {
    Write-Host ("  pid {0}  {1}" -f $b.ProcessId, $b.CommandLine)
}

if ($browsers.Count -ne 1) {
    Write-Host "SUBJECT: FAIL - expected exactly ONE browser process, found $($browsers.Count)." -ForegroundColor Red
    Write-Host "         Two processes means two PROFILES, not two windows. Every row would" -ForegroundColor Red
    Write-Host "         pass while proving nothing. Use Ctrl+N to make the second window." -ForegroundColor Red
    exit 1
}

$pidOne = $browsers[0].ProcessId
Write-Host "SUBJECT: PASS - exactly one dev browser process (pid $pidOne)." -ForegroundColor Green
Write-Host ""

Add-Type -Namespace P35 -Name Enum -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
'@

# One sample of every matching top-level window in the subject process.
# ⭐ EnumWindows returns windows in Z-ORDER, topmost first, and that order is preserved
# here as the Z column. It is the whole point of watch mode: it distinguishes "window B
# was hidden" from "window B went BEHIND window A", which look identical on screen and
# have completely different causes.
function Get-Sample {
    param([int]$TargetPid)
    $sb  = New-Object System.Text.StringBuilder 512
    $cls = New-Object System.Text.StringBuilder 512
    $rows = New-Object System.Collections.Generic.List[object]
    $z = 0
    $cb = [P35.Enum+EnumProc]{
        param($h, $p)
        $wpid = 0
        [void][P35.Enum]::GetWindowThreadProcessId($h, [ref]$wpid)
        if ($wpid -ne $TargetPid) { return $true }
        [void]$cls.Clear(); [void][P35.Win]::GetClassNameW($h, $cls, 512)
        # ⛔ Filter by CLASS, never by size. An earlier version skipped windows narrower
        # than 200px and thereby hid the shell window itself when it was minimized -- the
        # instrument silently dropped its own subject and printed an empty, clean-looking
        # table. Class names are what distinguish shell / overlay / CEF internals.
        if ($cls.ToString() -notmatch '^(HodosBrowserWndClass|CEF.*OverlayWindow)$') { return $true }
        $script:z++
        # ⚠️ Invisible windows are KEPT (with Vis=False). Every overlay is pre-created
        # hidden at startup, so "absent" and "present but hidden" are different facts and
        # dropping the hidden ones would erase the distinction.
        [void]$sb.Clear(); [void][P35.Win]::GetWindowTextW($h, $sb, 512)
        $r = New-Object P35.Win+RECT
        [void][P35.Win]::GetWindowRect($h, [ref]$r)
        $rows.Add([pscustomobject]@{
            Z     = $script:z
            Hwnd  = ('0x{0:X}' -f [int64]$h)
            Class = $cls.ToString()
            Vis   = [P35.Win]::IsWindowVisible($h)
            Rect  = ('{0},{1} {2}x{3}' -f $r.Left, $r.Top, ($r.Right-$r.Left), ($r.Bottom-$r.Top))
            Dpi   = [P35.Win]::GetDpiForWindow($h)
        })
        return $true
    }
    $script:z = 0
    [void][P35.Enum]::EnumWindows($cb, [IntPtr]::Zero)
    return $rows
}

# Compact signature of a sample, used to print only when something actually CHANGES.
function Get-Signature {
    param($Rows)
    ($Rows | ForEach-Object { '{0}:{1}:{2}:{3}' -f $_.Z, $_.Class, $_.Vis, $_.Rect }) -join '|'
}

if ($WatchSeconds -le 0) {
    Write-Host "Top-level windows in pid ${pidOne}:" -ForegroundColor Cyan
    $rows = Get-Sample -TargetPid $pidOne
    $rows | Format-Table -AutoSize
    Write-Host "⚠️  One-shot mode CANNOT see a dropdown overlay. Overlays hide on focus loss, and" -ForegroundColor Yellow
    Write-Host "   clicking this PowerShell window to run the script IS a focus loss. To observe" -ForegroundColor Yellow
    Write-Host "   an overlay, use:  -WatchSeconds 30   and then switch to the browser." -ForegroundColor Yellow
} else {
    Write-Host "WATCH: sampling every ${IntervalMs}ms for ${WatchSeconds}s. Printing only on CHANGE." -ForegroundColor Cyan
    Write-Host "⭐ Switch to the browser NOW and perform the action. Do not click back here." -ForegroundColor Green
    Write-Host ""
    $deadline = (Get-Date).AddSeconds($WatchSeconds)
    $last = ''
    $n = 0
    while ((Get-Date) -lt $deadline) {
        $rows = Get-Sample -TargetPid $pidOne
        $sig  = Get-Signature -Rows $rows
        if ($sig -ne $last) {
            $n++
            $t = '{0:HH:mm:ss.fff}' -f (Get-Date)
            Write-Host "--- change #$n at $t ---" -ForegroundColor Magenta
            $rows | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
            $last = $sig
        }
        Start-Sleep -Milliseconds $IntervalMs
    }
    Write-Host "WATCH: done. $n state change(s) recorded." -ForegroundColor Cyan
    if ($n -le 1) {
        Write-Host "⚠️  0-1 changes means nothing happened while watching -- most likely the action" -ForegroundColor Yellow
        Write-Host "   was performed before the watch started, or after it ended. Re-run and act" -ForegroundColor Yellow
        Write-Host "   DURING the window. A quiet log is not evidence of a quiet browser." -ForegroundColor Yellow
    }
}

Write-Host "⚠️  Z = z-order, 1 = topmost. A rect of -32000,-32000 means MINIMIZED." -ForegroundColor Yellow
exit 0
