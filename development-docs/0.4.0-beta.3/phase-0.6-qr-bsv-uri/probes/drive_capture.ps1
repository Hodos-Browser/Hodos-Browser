# Phase 0.6 capture-path driver. Drives the real screen-capture selection overlay
# (class "HodosQRCapture" in QRScreenCapture.cpp) with a synthetic HARDWARE-level
# drag (SendInput) so the real quirc + ClassifyAndBuildJson run on the pixels under
# the selection. PostMessage(WM_LBUTTONDOWN) does NOT reach the overlay WndProc
# (synthetic window messages are dropped); SendInput injects into the system input
# queue and is indistinguishable from a real mouse.
#
# Usage:
#   -Front            : move the dev Hodos main window to the primary monitor + front, then exit.
#   (default)         : poll for the capture overlay, then drag-select the primary screen.
param(
  [switch]$Front,
  [int]$TimeoutMs = 9000
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class U32 {
  // CharSet.Unicode REQUIRED or the wide FindWindowW never matches the class name.
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);

  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
  [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint n, INPUT[] p, int cb);

  public const uint INPUT_MOUSE = 0;
  public const uint MOVE = 0x0001, ABSOLUTE = 0x8000, VIRTUALDESK = 0x4000, LEFTDOWN = 0x0002, LEFTUP = 0x0004;

  // Absolute mouse coords are normalized 0..65535 over the VIRTUAL desktop.
  public static void MoveAbs(int x, int y) {
    int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77);       // SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN
    int vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);       // SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN
    int nx = (int)(((double)(x - vx)) * 65535.0 / (vw - 1));
    int ny = (int)(((double)(y - vy)) * 65535.0 / (vh - 1));
    Send(MOVE | ABSOLUTE | VIRTUALDESK, nx, ny);
  }
  public static void Button(uint flag) { Send(flag, 0, 0); }
  static void Send(uint flags, int nx, int ny) {
    INPUT[] inp = new INPUT[1];
    inp[0].type = INPUT_MOUSE;
    inp[0].mi.dx = nx; inp[0].mi.dy = ny; inp[0].mi.dwFlags = flags;
    SendInput(1, inp, Marshal.SizeOf(typeof(INPUT)));
  }
}
"@

$SW_RESTORE = 9

if ($Front) {
  $p = Get-Process HodosBrowser -ErrorAction SilentlyContinue |
       Where-Object { $_.Path -like '*\cef-native\build\bin\Release\*' -and $_.MainWindowHandle -ne 0 } |
       Select-Object -First 1
  if (-not $p) { Write-Output 'NO_MAIN_WINDOW'; exit 1 }
  [U32]::ShowWindow($p.MainWindowHandle, $SW_RESTORE) | Out-Null
  [U32]::SetWindowPos($p.MainWindowHandle, [IntPtr]::Zero, 0, 0, 1600, 1000, 0x0040) | Out-Null # SWP_SHOWWINDOW
  [U32]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
  Write-Output ("FRONT_OK hwnd={0}" -f $p.MainWindowHandle)
  exit 0
}

# Poll for the capture overlay window.
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$h = [IntPtr]::Zero
while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
  $h = [U32]::FindWindowW('HodosQRCapture', $null)
  if ($h -ne [IntPtr]::Zero) { break }
  Start-Sleep -Milliseconds 100
}
if ($h -eq [IntPtr]::Zero) { Write-Output 'NO_CAPTURE_WINDOW'; exit 2 }
Write-Output ("CAPTURE_WINDOW hwnd={0} after={1}ms" -f $h, $sw.ElapsedMilliseconds)

# Drag-select a large region of the PRIMARY monitor (the QR is centered there).
$W = [U32]::GetSystemMetrics(0)   # SM_CXSCREEN (primary)
$H = [U32]::GetSystemMetrics(1)   # SM_CYSCREEN (primary)
$x1 = 60; $y1 = 60; $x2 = $W - 60; $y2 = $H - 60

Start-Sleep -Milliseconds 300     # let the overlay finish first paint
[U32]::MoveAbs($x1, $y1); Start-Sleep -Milliseconds 80
[U32]::Button([U32]::LEFTDOWN);   Start-Sleep -Milliseconds 90
[U32]::MoveAbs([int](($x1+$x2)/2), [int](($y1+$y2)/2)); Start-Sleep -Milliseconds 90
[U32]::MoveAbs($x2, $y2);         Start-Sleep -Milliseconds 90
[U32]::Button([U32]::LEFTUP)
Write-Output ("DRAG_DONE region=({0},{1})-({2},{3})" -f $x1,$y1,$x2,$y2)
