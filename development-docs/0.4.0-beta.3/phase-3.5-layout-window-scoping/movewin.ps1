<#
.SYNOPSIS
  Move the SECONDARY dev shell window onto a chosen monitor, so P3.5-A3 can be run.
  Reads and moves only; identifies the dev process by exe path.
  Runs PER-MONITOR-DPI-AWARE -- see dpiprobe.ps1 for why that is load-bearing.
#>
[CmdletBinding()]
param([int]$X = -1800, [int]$Y = 100, [int]$W = 1500, [int]$H = 900,
      [string]$DevPathMatch = 'build\bin\Release')
Set-StrictMode -Version Latest
Add-Type -Namespace MV -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
[DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@
[void][MV.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
$procs=@(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
  Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" -and $_.CommandLine -notmatch '--type=' })
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - $($procs.Count) dev browser processes"; exit 1 }
$pidOne=$procs[0].ProcessId
$sh=New-Object System.Collections.Generic.List[object]
$cls=New-Object System.Text.StringBuilder 512
$cb=[MV.W+EnumProc]{ param($h,$p)
  $wp=0; [void][MV.W]::GetWindowThreadProcessId($h,[ref]$wp); if($wp -ne $pidOne){return $true}
  [void]$cls.Clear(); [void][MV.W]::GetClassNameW($h,$cls,512)
  if($cls.ToString() -ne 'HodosBrowserWndClass'){return $true}
  $r=New-Object MV.W+RECT; [void][MV.W]::GetWindowRect($h,[ref]$r)
  $sh.Add([pscustomobject]@{H=$h;Hwnd=('0x{0:X}' -f [int64]$h);L=$r.Left;T=$r.Top;R=$r.Right;B=$r.Bottom;Dpi=[MV.W]::GetDpiForWindow($h)})
  return $true }
[void][MV.W]::EnumWindows($cb,[IntPtr]::Zero)
if($sh.Count -ne 2){ Write-Host "FAIL - need 2 shell windows, found $($sh.Count). Use Ctrl+N."; exit 1 }
$A=$sh|Where-Object{$_.L -eq 0 -and $_.T -eq 0}|Select-Object -First 1
if(-not $A){ $A=$sh[1] }
$B=$sh|Where-Object{$_.Hwnd -ne $A.Hwnd}|Select-Object -First 1
Write-Host ("A={0} {1},{2} {3}x{4} dpi={5}" -f $A.Hwnd,$A.L,$A.T,($A.R-$A.L),($A.B-$A.T),$A.Dpi)
Write-Host ("B={0} {1},{2} {3}x{4} dpi={5}  -> moving to {6},{7} {8}x{9}" -f $B.Hwnd,$B.L,$B.T,($B.R-$B.L),($B.B-$B.T),$B.Dpi,$X,$Y,$W,$H)
[void][MV.W]::SetWindowPos($B.H,[IntPtr]0,$X,$Y,$W,$H,0x0010)   # SWP_NOACTIVATE
Start-Sleep -Milliseconds 900
$r=New-Object MV.W+RECT; [void][MV.W]::GetWindowRect($B.H,[ref]$r)
Write-Host ("B now  {0},{1} {2}x{3} dpi={4}" -f $r.Left,$r.Top,($r.Right-$r.Left),($r.Bottom-$r.Top),[MV.W]::GetDpiForWindow($B.H))
