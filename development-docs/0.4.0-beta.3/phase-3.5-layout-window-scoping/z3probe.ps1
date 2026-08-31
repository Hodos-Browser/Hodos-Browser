<#
.SYNOPSIS
  P3.5-Z3 (R-CLOSE): with a dropdown open in window B, destroy B. Does the overlay that
  window A still needs survive, and does g_menu_overlay_hwnd stay valid?

.DESCRIPTION
  ARM 5 of zexp2.ps1 measured, BEFORE any code existed, that an overlay re-owned to B is
  DESTROYED when B closes. The fix that shipped deliberately does NOT re-own -- it leaves
  ownership on the primary and instead asserts B's z-order after the show -- so this row is
  expected to pass by construction. Expected-to-pass is not measured, which is why it runs.

  Reports the overlay HWND before and after, and whether it is still a window.
#>
[CmdletBinding()]
param([string]$OverlayClass = 'CEFMenuOverlayWindow', [string]$DevPathMatch = 'build\bin\Release',
      # The strict form of this row needs the dropdown VISIBLE when B is destroyed. Spawning
      # this console is a focus loss and hides every overlay (K8.3), so the console must exist
      # BEFORE the dropdown is opened: start with -WaitSeconds N, open the dropdown in B during
      # the wait, and the probe then closes B with the overlay still on screen.
      [int]$WaitSeconds = 0)
Set-StrictMode -Version Latest
Add-Type -Namespace Z3 -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int idx);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@
[void][Z3.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
$GWLP_HWNDPARENT=-8; $WM_CLOSE=0x0010
$procs=@(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
  Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" -and $_.CommandLine -notmatch '--type=' })
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - $($procs.Count) dev browser processes"; exit 1 }
$pidOne=$procs[0].ProcessId
Write-Host "SUBJECT: PASS - one dev browser process (pid $pidOne)"
if ($WaitSeconds -gt 0) {
  Write-Host "WAIT: $WaitSeconds s - open the dropdown in window B NOW, without touching this console."
  Start-Sleep -Seconds $WaitSeconds
}
$cls=New-Object System.Text.StringBuilder 512
function Get-Sample {
  $rows=New-Object System.Collections.Generic.List[object]; $script:z=0
  $cb=[Z3.W+EnumProc]{ param($h,$p)
    $wp=0; [void][Z3.W]::GetWindowThreadProcessId($h,[ref]$wp); if($wp -ne $pidOne){return $true}
    [void]$cls.Clear(); [void][Z3.W]::GetClassNameW($h,$cls,512)
    if($cls.ToString() -notmatch '^(HodosBrowserWndClass|CEF.*OverlayWindow)$'){return $true}
    $script:z++
    $r=New-Object Z3.W+RECT; [void][Z3.W]::GetWindowRect($h,[ref]$r)
    $rows.Add([pscustomobject]@{Z=$script:z;H=$h;Hwnd=('0x{0:X}' -f [int64]$h);Class=$cls.ToString()
      Vis=[Z3.W]::IsWindowVisible($h);Rect=('{0},{1} {2}x{3}' -f $r.Left,$r.Top,($r.Right-$r.Left),($r.Bottom-$r.Top))})
    return $true }
  [void][Z3.W]::EnumWindows($cb,[IntPtr]::Zero); return $rows }

$s=Get-Sample
$shells=@($s|Where-Object{$_.Class -eq 'HodosBrowserWndClass'})
$ov=@($s|Where-Object{$_.Class -eq $OverlayClass})
if($shells.Count -ne 2){Write-Host "FAIL - need 2 shell windows, found $($shells.Count)";exit 1}
if($ov.Count -ne 1){Write-Host "FAIL - $OverlayClass absent";exit 1}
$A=$shells|Where-Object{$_.Rect -like '0,0 *'}|Select-Object -First 1
$B=$shells|Where-Object{$_.Hwnd -ne $A.Hwnd}|Select-Object -First 1
$OV=$ov[0]
Write-Host ("A={0}  B={1}  overlay={2} Vis={3} {4}" -f $A.Hwnd,$B.Hwnd,$OV.Hwnd,$OV.Vis,$OV.Rect)
Write-Host ("overlay OWNER = 0x{0:X}   (A={1}, B={2})" -f [int64][Z3.W]::GetWindowLongPtr($OV.H,$GWLP_HWNDPARENT),$A.Hwnd,$B.Hwnd)
Write-Host ("IsWindow(overlay) BEFORE closing B : {0}" -f [Z3.W]::IsWindow($OV.H))
Write-Host "closing window B ..."
[void][Z3.W]::PostMessageW($B.H,$WM_CLOSE,[IntPtr]::Zero,[IntPtr]::Zero)
Start-Sleep -Seconds 4
Write-Host ("IsWindow(B)       AFTER  closing B : {0}" -f [Z3.W]::IsWindow($B.H))
$alive=[Z3.W]::IsWindow($OV.H)
Write-Host ("IsWindow(overlay) AFTER  closing B : {0}   <== P3.5-Z3" -f $alive)
if($alive){ Write-Host "Z3: overlay SURVIVED - g_*_overlay_hwnd is still valid." }
else      { Write-Host "Z3: overlay DESTROYED with B - the fix traded a z-order bug for a lifetime bug." }
Get-Sample | Format-Table Z,Hwnd,Class,Vis,Rect -AutoSize | Out-String -Width 160 | Write-Host
