Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);

  [StructLayout(LayoutKind.Sequential)] public struct PROPERTYKEY { public Guid fmtid; public uint pid; }
  [ComImport, Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface IPropertyStore {
    int GetCount(out uint c);
    int GetAt(uint i, out PROPERTYKEY k);
    int GetValue(ref PROPERTYKEY k, [In, Out] PropVariant v);
    int SetValue(ref PROPERTYKEY k, [In] PropVariant v);
    int Commit();
  }
  [StructLayout(LayoutKind.Sequential)] public class PropVariant : IDisposable {
    ushort vt; ushort r1; ushort r2; ushort r3; IntPtr p; IntPtr p2;
    public string AsString() { return (vt == 31) ? Marshal.PtrToStringUni(p) : null; }
    public void Dispose() { PropVariantClear(this); }
  }
  [DllImport("ole32.dll")] public static extern int PropVariantClear([In,Out] PropVariant v);
  [DllImport("shell32.dll")] public static extern int SHGetPropertyStoreForWindow(IntPtr h, ref Guid riid, out IPropertyStore s);

  public static string GetAumid(IntPtr hwnd) {
    Guid iid = new Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99");
    IPropertyStore ps;
    int hr = SHGetPropertyStoreForWindow(hwnd, ref iid, out ps);
    if (hr != 0 || ps == null) return "<hr=0x" + hr.ToString("X8") + ">";
    PROPERTYKEY k = new PROPERTYKEY();
    k.fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");
    k.pid = 5;
    PropVariant pv = new PropVariant();
    hr = ps.GetValue(ref k, pv);
    string s = (hr == 0) ? pv.AsString() : null;
    pv.Dispose();
    Marshal.ReleaseComObject(ps);
    return string.IsNullOrEmpty(s) ? "<NONE>" : s;
  }
}
"@
$results = @()
$cb = [W+EnumProc]{
  param($h,$l)
  if ([W]::IsWindowVisible($h)) {
    $sb = New-Object System.Text.StringBuilder 512
    [void][W]::GetWindowTextW($h, $sb, 512)
    $title = $sb.ToString()
    if ($title.Length -gt 0) {
      [uint32]$procId = 0
      [void][W]::GetWindowThreadProcessId($h, [ref]$procId)
      $path = ""
      try { $path = (Get-Process -Id $procId -ErrorAction Stop).Path } catch {}
      if ($path -and ($path -match 'HodosBrowser' -or $path -match 'chrome\.exe$' -or $path -match 'brave\.exe$')) {
        $script:results += [pscustomobject]@{ PID=$procId; Title=$title.Substring(0,[Math]::Min(38,$title.Length)); AUMID=[W]::GetAumid($h); Path=$path }
      }
    }
  }
  return $true
}
[void][W]::EnumWindows($cb, [IntPtr]::Zero)
$results | Format-Table -AutoSize -Wrap
