<#
.SYNOPSIS
  Per-monitor DPI, measured from a DPI-AWARE process.

.DESCRIPTION
  NOTE: K7 ("all three monitors read 96 dpi -> no mixed-DPI rig exists") is suspect, and this
  script exists to settle it. A process that is not per-monitor-DPI-aware is LIED TO by
  Windows: GetDpiForWindow/GetDpiForMonitor return 96 for every monitor, and monitor rects
  come back divided by the scale factor. A 1920x1200 laptop panel at 125% then reports as
  1536x960 @ 96dpi -- which is exactly what K7 recorded for DISPLAY24.

  Same failure family as winprobe.ps1's two bugs (K8): plausible output, wrong data.
  SetProcessDpiAwarenessContext must be called BEFORE any monitor or window query.
#>
Set-StrictMode -Version Latest

Add-Type -Namespace DP -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
[DllImport("user32.dll")] public static extern IntPtr GetThreadDpiAwarenessContext();
[DllImport("user32.dll")] public static extern uint GetAwarenessFromDpiAwarenessContext(IntPtr ctx);
[DllImport("shcore.dll")] public static extern int GetDpiForMonitor(IntPtr hmon, int type, out uint dx, out uint dy);
[DllImport("user32.dll")] public static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr clip, MonEnumProc cb, IntPtr data);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool GetMonitorInfoW(IntPtr hmon, ref MONITORINFOEX mi);
public delegate bool MonEnumProc(IntPtr hmon, IntPtr hdc, ref RECT r, IntPtr data);
public struct RECT { public int Left, Top, Right, Bottom; }
[StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
public struct MONITORINFOEX {
    public int cbSize; public RECT rcMonitor; public RECT rcWork; public uint dwFlags;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string szDevice;
}
'@

# DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = (HANDLE)-4. Must precede every query below.
$ok = [DP.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
$aw = [DP.W]::GetAwarenessFromDpiAwarenessContext([DP.W]::GetThreadDpiAwarenessContext())
Write-Host ("SetProcessDpiAwarenessContext(PER_MONITOR_V2) = {0}; thread awareness = {1} (0=UNAWARE 1=SYSTEM 2=PER_MONITOR)" -f $ok, $aw)
if ($aw -ne 2) { Write-Host "STOP - NOT per-monitor aware - every number below would be a lie. Stopping." -ForegroundColor Red; exit 1 }
Write-Host ""

$rows = New-Object System.Collections.Generic.List[object]
$cb = [DP.W+MonEnumProc]{
    param($hmon, $hdc, [ref]$r, $data)
    $mi = New-Object DP.W+MONITORINFOEX
    $mi.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($mi)
    [void][DP.W]::GetMonitorInfoW($hmon, [ref]$mi)
    $dx = 0; $dy = 0
    [void][DP.W]::GetDpiForMonitor($hmon, 0, [ref]$dx, [ref]$dy)   # 0 = MDT_EFFECTIVE_DPI
    $rows.Add([pscustomobject]@{
        Device  = $mi.szDevice
        Primary = [bool]($mi.dwFlags -band 1)
        Rect    = ('{0},{1} {2}x{3}' -f $mi.rcMonitor.Left, $mi.rcMonitor.Top,
                   ($mi.rcMonitor.Right-$mi.rcMonitor.Left), ($mi.rcMonitor.Bottom-$mi.rcMonitor.Top))
        Dpi     = $dx
        Scale   = ('{0}%' -f [int](100 * $dx / 96))
    })
    return $true
}
[void][DP.W]::EnumDisplayMonitors([IntPtr]::Zero, [IntPtr]::Zero, $cb, [IntPtr]::Zero)
$rows | Format-Table -AutoSize | Out-String -Width 200 | Write-Host

$distinct = @($rows | Select-Object -ExpandProperty Dpi | Sort-Object -Unique)
if ($distinct.Count -gt 1) {
    Write-Host ("MIXED-DPI RIG: PRESENT - {0} distinct DPI values {1}. P3.5-A3 is runnable." -f $distinct.Count, ($distinct -join ', ')) -ForegroundColor Green
} else {
    Write-Host ("MIXED-DPI RIG: ABSENT - every monitor at {0} dpi. P3.5-A3 needs a scale change first." -f $distinct[0]) -ForegroundColor Yellow
}
