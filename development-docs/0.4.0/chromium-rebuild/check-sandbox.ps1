# Prove the Chromium sandbox is actually engaged rather than silently disabled.
# A sandboxed Chromium child runs at UNTRUSTED integrity (S-1-16-0); an unsandboxed
# one inherits the parent's MEDIUM (S-1-16-8192). Absence of --no-sandbox on the
# command line proves nothing on its own, so read the token.
#
# Defaults to the dev build. Point -Path at an INSTALLED build to verify the sandbox
# on a real user install (the only test that proves it for production):
#   .\check-sandbox.ps1 -Path "$env:LOCALAPPDATA\HodosBrowser"
#
# PASS = every renderer UNTRUSTED and the GPU LOW. Expect ~12-14 renderers on a
# fully-loaded window; ZERO renderers is the classic "sandbox on but broken" signature.
#
# Watch the AppContainer column. It is empty today, which is why LPAC ACLs on the
# program directory are currently unnecessary (verified 2026-08-05 by removing them).
# If a future Chromium bump ever makes renderers AppContainer processes, that column
# flips to "yes" and LPAC ACLs become REQUIRED in both the build and the installer.
param(
    [string]$Path = 'C:\Users\archb\Hodos-Browser\cef-native\build\bin\Release'
)

$src = @"
using System;
using System.Runtime.InteropServices;
public static class Tok {
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("advapi32.dll", SetLastError=true)]
    public static extern bool OpenProcessToken(IntPtr h, uint access, out IntPtr tok);
    [DllImport("advapi32.dll", SetLastError=true)]
    public static extern bool GetTokenInformation(IntPtr tok, int cls, IntPtr buf, int len, out int ret);
    [DllImport("advapi32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    public static extern bool ConvertSidToStringSidW(IntPtr sid, out IntPtr str);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr p);

    const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    const uint TOKEN_QUERY = 0x0008;
    const int TokenIntegrityLevel = 25;
    const int TokenIsAppContainer = 29;

    public static string Integrity(int pid) {
        IntPtr h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (h == IntPtr.Zero) return "‹no access›";
        IntPtr tok;
        if (!OpenProcessToken(h, TOKEN_QUERY, out tok)) { CloseHandle(h); return "‹no token›"; }
        int need;
        GetTokenInformation(tok, TokenIntegrityLevel, IntPtr.Zero, 0, out need);
        IntPtr buf = Marshal.AllocHGlobal(need);
        string result = "‹error›";
        if (GetTokenInformation(tok, TokenIntegrityLevel, buf, need, out need)) {
            IntPtr sid = Marshal.ReadIntPtr(buf);   // TOKEN_MANDATORY_LABEL.Label.Sid
            IntPtr str;
            if (ConvertSidToStringSidW(sid, out str)) {
                result = Marshal.PtrToStringUni(str);
                LocalFree(str);
            }
        }
        Marshal.FreeHGlobal(buf);

        // AppContainer flag (LPAC shows up here)
        int ac = 0;
        IntPtr b2 = Marshal.AllocHGlobal(4);
        int n2;
        if (GetTokenInformation(tok, TokenIsAppContainer, b2, 4, out n2)) ac = Marshal.ReadInt32(b2);
        Marshal.FreeHGlobal(b2);

        CloseHandle(tok); CloseHandle(h);
        return result + (ac != 0 ? "  +AppContainer" : "");
    }
}
"@
Add-Type -TypeDefinition $src -Language CSharp

$labels = @{
    'S-1-16-0'     = 'UNTRUSTED  <- sandboxed'
    'S-1-16-4096'  = 'LOW        <- sandboxed'
    'S-1-16-8192'  = 'MEDIUM     <- NOT sandboxed'
    'S-1-16-12288' = 'HIGH       <- NOT sandboxed'
}

Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" |
  Where-Object { $_.ExecutablePath -like "$Path*" } |
  ForEach-Object {
      $type = if ($_.CommandLine -match '--type=([a-z-]+)') { $Matches[1] } else { 'browser' }
      $int  = [Tok]::Integrity($_.ProcessId)
      $sid  = ($int -split '  ')[0]
      $desc = if ($labels.ContainsKey($sid)) { $labels[$sid] } else { $sid }
      [PSCustomObject]@{
          PID = $_.ProcessId
          Type = $type
          Integrity = $desc
          AppContainer = if ($int -match 'AppContainer') { 'yes' } else { '' }
          NoSandboxFlag = if ($_.CommandLine -match '--no-sandbox') { 'PRESENT' } else { '' }
      }
  } | Sort-Object Type, PID | Format-Table -AutoSize
