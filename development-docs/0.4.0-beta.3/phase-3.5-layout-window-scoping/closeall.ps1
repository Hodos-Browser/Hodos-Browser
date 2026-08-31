# P3.5-A4: close every shell window, secondaries first, then the last one -- which is the
# only path on which ShutdownApplication (and therefore SaveSession) runs.
Set-StrictMode -Version Latest
Add-Type -Namespace CA -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
public delegate bool EnumProc(IntPtr h, IntPtr p);
public struct RECT { public int Left, Top, Right, Bottom; }
'@
$procs=@(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
  Where-Object { $_.ExecutablePath -and $_.ExecutablePath -like '*cef-native\bin*' -or ($_.ExecutablePath -like '*cef-native\build\bin\Release*' -and $_.CommandLine -notmatch '--type=') })
$procs=@($procs | Where-Object { $_.CommandLine -notmatch '--type=' })
if ($procs.Count -ne 1) { Write-Host "SUBJECT: FAIL - $($procs.Count) dev browser processes"; exit 1 }
$pidOne=$procs[0].ProcessId
$cls=New-Object System.Text.StringBuilder 512
$shells=New-Object System.Collections.Generic.List[object]
$cb=[CA.W+EnumProc]{ param($h,$p)
  $wp=0; [void][CA.W]::GetWindowThreadProcessId($h,[ref]$wp); if($wp -ne $pidOne){return $true}
  [void]$cls.Clear(); [void][CA.W]::GetClassNameW($h,$cls,512)
  if($cls.ToString() -ne 'HodosBrowserWndClass'){return $true}
  $r=New-Object CA.W+RECT; [void][CA.W]::GetWindowRect($h,[ref]$r)
  $shells.Add([pscustomobject]@{H=$h;Hwnd=('0x{0:X}' -f [int64]$h);Rect=('{0},{1}' -f $r.Left,$r.Top)}); return $true }
[void][CA.W]::EnumWindows($cb,[IntPtr]::Zero)
Write-Host ("shell windows: " + (($shells | ForEach-Object { $_.Hwnd }) -join ', '))
foreach($s in $shells){
  Write-Host ("  WM_CLOSE -> {0}" -f $s.Hwnd)
  [void][CA.W]::PostMessageW($s.H,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)
  Start-Sleep -Seconds 4
}
Start-Sleep -Seconds 4
$left=@(Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" | Where-Object { $_.ExecutablePath -like '*cef-native\build\bin\Release*' })
Write-Host ("dev processes remaining: " + $left.Count)
