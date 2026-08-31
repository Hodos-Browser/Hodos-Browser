<#
.SYNOPSIS
  P3.5 follow-ups to zexp.ps1, still outside the product: (4) a non-ownership alternative,
  and (5) the R-CLOSE lifetime hazard the re-own fix would create — measured BEFORE any
  code is written, because P3.5-Z3 is the row most likely to fail.

  ARM 4  keep the overlay owned by A, then put B back on top afterwards. If this holds,
         the phase can close Z1 without touching overlay LIFETIME at all.
  ARM 5  re-own the overlay to B, then destroy B. Does the overlay HWND survive?
         ⛔ DESTRUCTIVE to window B by design — that is the measurement.
#>
[CmdletBinding()]
param([string]$DevPathMatch = 'build\bin\Release', [string]$OverlayClass = 'CEFMenuOverlayWindow')
Set-StrictMode -Version Latest

Add-Type -Namespace ZX2 -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int idx);
[DllImport("user32.dll", EntryPoint="SetWindowLongPtrW")] public static extern IntPtr SetWindowLongPtr(IntPtr h, int idx, IntPtr v);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@

$HWND_TOPMOST=[IntPtr](-1); $HWND_TOP=[IntPtr]0
$SWP_NOACTIVATE=0x0010; $SWP_SHOWWINDOW=0x0040; $SWP_NOMOVE=0x0002; $SWP_NOSIZE=0x0001
$GWLP_HWNDPARENT=-8; $SW_HIDE=0; $WM_CLOSE=0x0010

$procs=@(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
  Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" -and $_.CommandLine -notmatch '--type=' })
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - $($procs.Count) dev browser processes" -ForegroundColor Red; exit 1 }
$pidOne=$procs[0].ProcessId
Write-Host "SUBJECT: PASS - one dev browser process (pid $pidOne)" -ForegroundColor Green

function Get-Sample {
  $cls=New-Object System.Text.StringBuilder 512
  $rows=New-Object System.Collections.Generic.List[object]; $script:z=0
  $cb=[ZX2.W+EnumProc]{ param($h,$p)
    $wpid=0; [void][ZX2.W]::GetWindowThreadProcessId($h,[ref]$wpid)
    if ($wpid -ne $pidOne) { return $true }
    [void]$cls.Clear(); [void][ZX2.W]::GetClassNameW($h,$cls,512)
    if ($cls.ToString() -notmatch '^(HodosBrowserWndClass|CEF.*OverlayWindow)$') { return $true }
    $script:z++
    $r=New-Object ZX2.W+RECT; [void][ZX2.W]::GetWindowRect($h,[ref]$r)
    $rows.Add([pscustomobject]@{ Z=$script:z; Hwnd=('0x{0:X}' -f [int64]$h); Class=$cls.ToString()
      Vis=[ZX2.W]::IsWindowVisible($h); Rect=('{0},{1} {2}x{3}' -f $r.Left,$r.Top,($r.Right-$r.Left),($r.Bottom-$r.Top)); H=$h })
    return $true }
  [void][ZX2.W]::EnumWindows($cb,[IntPtr]::Zero); return $rows
}
function Show-Z { param([string]$Label)
  $s=Get-Sample
  $sh=@($s|Where-Object{$_.Class -eq 'HodosBrowserWndClass'}); $ov=@($s|Where-Object{$_.Class -eq $OverlayClass})
  $line=($sh|ForEach-Object{'{0}@Z{1}' -f $_.Hwnd,$_.Z}) -join '  '
  $o = if($ov.Count){'{0}@Z{1} Vis={2}' -f $ov[0].Hwnd,$ov[0].Z,$ov[0].Vis} else {'absent'}
  Write-Host ("  {0,-26} shells: {1}   overlay: {2}" -f $Label,$line,$o) }

$s=Get-Sample
$sh=@($s|Where-Object{$_.Class -eq 'HodosBrowserWndClass'}); $ovr=@($s|Where-Object{$_.Class -eq $OverlayClass})
if ($sh.Count -ne 2){Write-Host "FAIL - need 2 shell windows, found $($sh.Count)" -ForegroundColor Red; exit 1}
if ($ovr.Count -ne 1){Write-Host "FAIL - overlay $OverlayClass absent" -ForegroundColor Red; exit 1}
$A=$sh|Where-Object{$_.Rect -like '0,0 *'}|Select-Object -First 1
$B=$sh|Where-Object{$_.Hwnd -ne $A.Hwnd}|Select-Object -First 1
$OV=$ovr[0]
Write-Host ("A={0}  B={1}  overlay={2}" -f $A.Hwnd,$B.Hwnd,$OV.Hwnd); Write-Host ""
$r=New-Object ZX2.W+RECT; [void][ZX2.W]::GetWindowRect($OV.H,[ref]$r)
$ox=$r.Left;$oy=$r.Top;$ow=$r.Right-$r.Left;$oh=$r.Bottom-$r.Top

# ---- ARM 4 -------------------------------------------------------------------
Write-Host "ARM 4  owner stays A; restore B on top after the show" -ForegroundColor Cyan
[void][ZX2.W]::ShowWindow($OV.H,$SW_HIDE); Start-Sleep -Milliseconds 250
[void][ZX2.W]::SetWindowLongPtr($OV.H,$GWLP_HWNDPARENT,$A.H)
[void][ZX2.W]::SetWindowPos($B.H,$HWND_TOP,0,0,0,0,($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_NOACTIVATE))
Start-Sleep -Milliseconds 250
Show-Z 'before show'
[void][ZX2.W]::SetWindowPos($OV.H,$HWND_TOPMOST,$ox,$oy,$ow,$oh,($SWP_NOACTIVATE -bor $SWP_SHOWWINDOW))
Start-Sleep -Milliseconds 300
Show-Z 'after  show (drops)'
[void][ZX2.W]::SetWindowPos($B.H,$HWND_TOP,0,0,0,0,($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_NOACTIVATE))
Start-Sleep -Milliseconds 400
Show-Z 'after  B->HWND_TOP'
Write-Host ""

# ---- ARM 5 -------------------------------------------------------------------
Write-Host "ARM 5  R-CLOSE: overlay owned by B, then B is destroyed" -ForegroundColor Cyan
[void][ZX2.W]::ShowWindow($OV.H,$SW_HIDE); Start-Sleep -Milliseconds 250
[void][ZX2.W]::SetWindowLongPtr($OV.H,$GWLP_HWNDPARENT,$B.H)
Write-Host ("  overlay owner now = 0x{0:X} (B={1})" -f [int64][ZX2.W]::GetWindowLongPtr($OV.H,$GWLP_HWNDPARENT),$B.Hwnd)
Write-Host ("  IsWindow(overlay) before closing B : {0}" -f [ZX2.W]::IsWindow($OV.H))
[void][ZX2.W]::PostMessageW($B.H,$WM_CLOSE,[IntPtr]::Zero,[IntPtr]::Zero)
Start-Sleep -Seconds 3
Write-Host ("  IsWindow(B)       after closing B  : {0}" -f [ZX2.W]::IsWindow($B.H))
Write-Host ("  IsWindow(overlay) after closing B  : {0}  <== THE ANSWER" -f [ZX2.W]::IsWindow($OV.H)) -ForegroundColor Yellow
if ([ZX2.W]::IsWindow($OV.H)) {
    [void][ZX2.W]::SetWindowLongPtr($OV.H,$GWLP_HWNDPARENT,$A.H)
    [void][ZX2.W]::ShowWindow($OV.H,$SW_HIDE)
    Write-Host ("  RESTORED overlay owner to A ({0})" -f $A.Hwnd) -ForegroundColor Green
} else {
    Write-Host "  overlay DESTROYED with B - g_menu_overlay_hwnd is now DANGLING in the product." -ForegroundColor Red
}
Show-Z 'final'
