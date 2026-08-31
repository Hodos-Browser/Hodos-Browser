<#
.SYNOPSIS
  P3.5-Z5: what happens to the requesting window when its overlay is DISMISSED?

.DESCRIPTION
  Owner-reported 2026-08-31: with the wallet open in window B, clicking outside the wallet
  makes "window B minimize or go behind window A (vanishes)". Those two are visually
  identical and have different causes, so this probe reports BOTH discriminators:

    Z column  -> B below A  = went BEHIND
    Rect      -> -32000     = MINIMIZED

  The wallet has no mouse hook; it closes on WM_ACTIVATE(WA_INACTIVE) on its own HWND.
  Clicking window B's content is therefore an ACTIVATION change to B, which is what
  SetForegroundWindow reproduces here. If the foreground lock refuses it, the script says
  so rather than reporting a clean-looking wrong result.
#>
[CmdletBinding()]
param([int]$WaitSeconds = 12, [int]$AfterSeconds = 8, [string]$DevPathMatch = 'build\bin\Release')
Set-StrictMode -Version Latest
Add-Type -Namespace HP -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@
[void][HP.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
$procs=@(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
  Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like "*$DevPathMatch*" -and $_.CommandLine -notmatch '--type=' })
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - $($procs.Count) dev browser processes"; exit 1 }
$pidOne=$procs[0].ProcessId
Write-Host "SUBJECT: PASS - one dev browser process (pid $pidOne)"
$cls=New-Object System.Text.StringBuilder 512
function Get-Sample {
  $rows=New-Object System.Collections.Generic.List[object]; $script:z=0
  $cb=[HP.W+EnumProc]{ param($h,$p)
    $wp=0; [void][HP.W]::GetWindowThreadProcessId($h,[ref]$wp); if($wp -ne $pidOne){return $true}
    [void]$cls.Clear(); [void][HP.W]::GetClassNameW($h,$cls,512)
    if($cls.ToString() -notmatch '^(HodosBrowserWndClass|CEFWalletOverlayWindow)$'){return $true}
    $script:z++
    $r=New-Object HP.W+RECT; [void][HP.W]::GetWindowRect($h,[ref]$r)
    $rows.Add([pscustomobject]@{Z=$script:z;H=$h;Hwnd=('0x{0:X}' -f [int64]$h);Class=$cls.ToString()
      Vis=[HP.W]::IsWindowVisible($h);Min=[HP.W]::IsIconic($h)
      Rect=('{0},{1} {2}x{3}' -f $r.Left,$r.Top,($r.Right-$r.Left),($r.Bottom-$r.Top))})
    return $true }
  [void][HP.W]::EnumWindows($cb,[IntPtr]::Zero); return $rows }
function Dump { param($Label)
  Write-Host "--- $Label ---"
  Get-Sample | Format-Table Z,Hwnd,Class,Vis,Min,Rect -AutoSize | Out-String -Width 160 | Write-Host }

Write-Host "WAIT: $WaitSeconds s - open the WALLET in window B now. Do not touch this console."
Start-Sleep -Seconds $WaitSeconds
Dump "BEFORE dismiss (wallet should be visible)"
$s=Get-Sample
$shells=@($s|Where-Object{$_.Class -eq 'HodosBrowserWndClass'})
if($shells.Count -ne 2){Write-Host "FAIL - need 2 shell windows, found $($shells.Count)";exit 1}
$A=$shells|Where-Object{$_.Rect -like '0,0 *'}|Select-Object -First 1
$B=$shells|Where-Object{$_.Hwnd -ne $A.Hwnd}|Select-Object -First 1
Write-Host ("A={0}  B={1}" -f $A.Hwnd,$B.Hwnd)
Write-Host "DISMISS: activating window B (what clicking B's content does) ..."
$ok=[HP.W]::SetForegroundWindow($B.H)
$fg=[HP.W]::GetForegroundWindow()
Write-Host ("  SetForegroundWindow returned {0}; foreground is now 0x{1:X} (B={2})" -f $ok,[int64]$fg,$B.Hwnd)
if(-not $ok -or $fg -ne $B.H){ Write-Host "  WARNING: the foreground lock refused this. The result below is NOT the owner's action." }
for($i=1;$i -le [int]($AfterSeconds/2);$i++){ Start-Sleep -Seconds 2; Dump "AFTER dismiss +$($i*2)s" }
$f=Get-Sample
$fb=$f|Where-Object{$_.Hwnd -eq $B.Hwnd}; $fa=$f|Where-Object{$_.Hwnd -eq $A.Hwnd}
Write-Host ""
if($fb.Min -or $fb.Rect -like '-32000*'){ Write-Host "VERDICT: B is MINIMIZED." }
elseif($fb.Z -gt $fa.Z){ Write-Host ("VERDICT: B went BEHIND A (B Z{0} vs A Z{1}). Not minimized - rect is {2}." -f $fb.Z,$fa.Z,$fb.Rect) }
else { Write-Host ("VERDICT: B is still in front (B Z{0} vs A Z{1}) - not reproduced." -f $fb.Z,$fa.Z) }
