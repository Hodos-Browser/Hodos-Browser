<#
.SYNOPSIS
  P3.5-A3 measurement: the header-right-edge -> dropdown-right-edge gap, per window.

.DESCRIPTION
  ShowMenuOverlay computes  overlayX = headerRect.right - g_menu_icon_right_offset - panelWidth,
  so the gap between the header's right edge and the panel's right edge IS the icon offset
  in physical pixels. React sends the offset in CSS px (measured: 36 in BOTH windows) and
  simple_handler.cpp scales it with ScalePx(offset, g_hwnd) -- the PRIMARY window's DPI.

    pre-fix  : gap == 36 on both monitors            (identical => the defect)
    post-fix : gap == 36 at 100%, 45 at 125%         (36 * 1.25)

  Runs PER-MONITOR-DPI-AWARE; without that every rect below is divided by the scale factor
  and the whole measurement is silently wrong (see dpiprobe.ps1 / K7).
#>
[CmdletBinding()]
param([string]$OverlayClass = 'CEFMenuOverlayWindow', [string]$DevPathMatch = 'build\bin\Release',
      # MANDATORY for any row with a visible overlay (K8.3): overlays hide on focus loss,
      # and spawning this console IS a focus loss, so a one-shot run reports "no overlay"
      # no matter what was on screen. Start this FIRST, then trigger the dropdown.
      [int]$WatchSeconds = 0, [int]$IntervalMs = 250)
Set-StrictMode -Version Latest
Add-Type -Namespace GP -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@
[void][GP.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
# One CIM query has been seen to come back empty while the browser was busy, which
# would abort the watch before the action under test happens. Retry before failing --
# an instrument that gives up on a transient is an instrument that reports the wrong
# thing. It still FAILS on a real mismatch; it just does not fail on one bad sample.
$procs = @()
for ($try = 1; $try -le 3; $try++) {
  $procs = @(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
    Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" -and $_.CommandLine -notmatch '--type=' })
  if ($procs.Count -eq 1) { break }
  Write-Host "  (subject query attempt $try returned $($procs.Count); retrying)"
  Start-Sleep -Milliseconds 600
}
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - $($procs.Count) dev browser processes"; exit 1 }
$pidOne=$procs[0].ProcessId
Write-Host "SUBJECT: PASS - one dev browser process (pid $pidOne)"

$cls=New-Object System.Text.StringBuilder 512
function RectOf($h){ $r=New-Object GP.W+RECT; [void][GP.W]::GetWindowRect($h,[ref]$r); return $r }

function Sample-Once {
  $tops=New-Object System.Collections.Generic.List[object]
  $cb=[GP.W+EnumProc]{ param($h,$p)
    $wp=0; [void][GP.W]::GetWindowThreadProcessId($h,[ref]$wp); if($wp -ne $pidOne){return $true}
    [void]$cls.Clear(); [void][GP.W]::GetClassNameW($h,$cls,512)
    $c=$cls.ToString()
    if($c -notmatch '^(HodosBrowserWndClass|CEF.*OverlayWindow)$'){return $true}
    $tops.Add([pscustomobject]@{H=$h;Hwnd=('0x{0:X}' -f [int64]$h);Class=$c}); return $true }
  [void][GP.W]::EnumWindows($cb,[IntPtr]::Zero)

  $shells=@($tops|Where-Object{$_.Class -eq 'HodosBrowserWndClass'})
  $ovs=@($tops|Where-Object{$_.Class -eq $OverlayClass -and [GP.W]::IsWindowVisible($_.H)})
  Write-Output ""
  foreach($s in $shells){
    $sr=RectOf $s.H
    # The header is the TOPMOST-anchored child: same left/right as the shell, small height.
    $hdr=$null
    $ccb=[GP.W+EnumProc]{ param($h,$p)
      $r=New-Object GP.W+RECT; [void][GP.W]::GetWindowRect($h,[ref]$r)
      [void]$cls.Clear(); [void][GP.W]::GetClassNameW($h,$cls,512)
      if(($r.Bottom-$r.Top) -gt 0 -and ($r.Bottom-$r.Top) -lt 250 -and $r.Top -le ($sr.Top+8)){
        if(-not $script:hdrFound){ $script:hdrFound=[pscustomobject]@{H=$h;Cls=$cls.ToString();R=$r} }
      }
      return $true }
    $script:hdrFound=$null
    [void][GP.W]::EnumChildWindows($s.H,$ccb,[IntPtr]::Zero)
    $hdr=$script:hdrFound
    $hr = if($hdr){ $hdr.R.Right } else { $sr.Right }
    $src = if($hdr){ $hdr.Cls } else { 'SHELL (no header child found)' }
    Write-Output ("shell {0}  rect {1},{2} {3}x{4}  dpi={5}  headerRight={6}  [{7}]" -f `
      $s.Hwnd,$sr.Left,$sr.Top,($sr.Right-$sr.Left),($sr.Bottom-$sr.Top),[GP.W]::GetDpiForWindow($s.H),$hr,$src)
    foreach($o in $ovs){
      $orr=RectOf $o.H
      Write-Output ("    visible {0}  rect {1},{2} {3}x{4}   GAP(headerRight - overlayRight) = {5}" -f `
        $o.Class,$orr.Left,$orr.Top,($orr.Right-$orr.Left),($orr.Bottom-$orr.Top),($hr-$orr.Right))
    }
  }
  if($ovs.Count -eq 0){ Write-Output ("  (no VISIBLE $OverlayClass - trigger it first, then re-run)") }
}

if ($WatchSeconds -le 0) {
    Sample-Once
} else {
    Write-Host "WATCH: sampling every ${IntervalMs}ms for ${WatchSeconds}s. Printing only when a visible overlay appears or moves."
    $deadline=(Get-Date).AddSeconds($WatchSeconds); $last=''; $n=0
    while((Get-Date) -lt $deadline){
        $out = (Sample-Once | Out-String)
        if($out -notmatch 'no VISIBLE' -and $out -ne $last){
            $n++; Write-Host ("--- sample #$n at {0:HH:mm:ss.fff} ---" -f (Get-Date)); Write-Host $out; $last=$out
        }
        Start-Sleep -Milliseconds $IntervalMs
    }
    Write-Host "WATCH: done. $n sample(s) with a visible overlay."
    if($n -eq 0){ Write-Host "WARNING: never saw a visible $OverlayClass - the trigger fired outside the watch window." }
}
