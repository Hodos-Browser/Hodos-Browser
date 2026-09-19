#!/usr/bin/env python3
"""
beta.3 Phase 11 item 1 (`P11-I1`) — WHERE DOES NATIVE KEYBOARD FOCUS LAND AT STARTUP?

The item-1 write-up says, in as many words:

    "Establish where native keyboard focus actually lands at startup -- which HWND,
     and which CEF browser believes it has focus -- BEFORE changing anything.
     That measurement does not exist yet."

This is that measurement. It reads the Win32 layer only and makes no claim about
what the user sees.

⛔ WHY IT IS NOT `document.activeElement`. That is DOM focus: which element gets
keys ONCE THEY ARRIVE at a browser. Native focus decides whether they arrive at
all. Three confident greens died in this phase by reading the first and reporting
the second.

What it prints, per Win32:
  GetForegroundWindow   the top-level window the OS is sending input to
  GUITHREADINFO.hwndFocus  the HWND with keyboard focus on the browser UI thread
                        -- this is the one that matters, and it is per-thread,
                        which is why GetFocus() from our own thread would return
                        nothing useful
Then it maps that HWND back to a Hodos window class, and to the CEF child window
of each browser, so the answer is a name rather than a number.
"""
import ctypes
import ctypes.wintypes as wt
import sys

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import omniboxprobe as P  # noqa: E402
import tearoffprobe as T  # noqa: E402

user32 = ctypes.WinDLL("user32", use_last_error=True)
EnumWindowsProc = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
EnumChildProc = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)


class GUITHREADINFO(ctypes.Structure):
    _fields_ = [("cbSize", wt.DWORD), ("flags", wt.DWORD),
                ("hwndActive", wt.HWND), ("hwndFocus", wt.HWND),
                ("hwndCapture", wt.HWND), ("hwndMenuOwner", wt.HWND),
                ("hwndMoveSize", wt.HWND), ("hwndCaret", wt.HWND),
                ("rcCaret", wt.RECT)]


def cls_of(hwnd):
    b = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, b, 256)
    return b.value


def title_of(hwnd):
    b = ctypes.create_unicode_buffer(256)
    user32.GetWindowTextW(hwnd, b, 256)
    return b.value


def describe(hwnd):
    if not hwnd:
        return "(none)"
    return "0x%X  class=%s  title=%r" % (hwnd, cls_of(hwnd), title_of(hwnd)[:40])


def walk_children(root, depth=0, out=None):
    """CEF's own browser window is a CHILD of the HWND we created. The chain is
    what tells us whether focus is on OUR window or on CEF's child inside it."""
    if out is None:
        out = []

    def cb(h, _lp):
        out.append((depth, h))
        walk_children(h, depth + 1, out)
        return True

    user32.EnumChildWindows(root, EnumChildProc(cb), 0)
    return out


def main():
    pid = P.dev_browser_pid()
    print("SUBJECT: dev browser pid %d\n" % pid)

    fg = user32.GetForegroundWindow()
    print("GetForegroundWindow : %s" % describe(fg))
    fg_pid = wt.DWORD()
    fg_tid = user32.GetWindowThreadProcessId(fg, ctypes.byref(fg_pid))
    print("   belongs to pid %d %s" % (fg_pid.value,
                                       "<-- THE DEV BROWSER" if fg_pid.value == pid
                                       else "<-- NOT the dev browser"))
    print("")

    # The UI thread of the dev browser: take it from its main shell window.
    shells = [r for r in T.sample(pid) if r["cls"] == "HodosBrowserWndClass"]
    if not shells:
        sys.exit("no shell window found")
    shell = shells[0]["hwnd"]
    tid = user32.GetWindowThreadProcessId(shell, None)

    gti = GUITHREADINFO()
    gti.cbSize = ctypes.sizeof(GUITHREADINFO)
    ok = user32.GetGUIThreadInfo(tid, ctypes.byref(gti))
    print("GetGUIThreadInfo on the browser UI thread (tid %d): %s" % (tid, bool(ok)))
    if ok:
        print("   hwndActive  : %s" % describe(gti.hwndActive))
        print("   hwndFocus   : %s   <-- KEYS GO HERE" % describe(gti.hwndFocus))
        print("   hwndCaret   : %s" % describe(gti.hwndCaret))
    print("")

    print("Window tree under the shell (which child is CEF's?):")
    print("   root  %s" % describe(shell))
    for depth, h in walk_children(shell)[:14]:
        mark = ""
        if ok and h == gti.hwndFocus:
            mark = "   <<< hwndFocus"
        if ok and h == gti.hwndCaret:
            mark += "   <<< hwndCaret"
        print("   %s%s%s" % ("  " * (depth + 1), describe(h), mark))
    print("")

    print("DOM focus, for contrast - a DIFFERENT layer, do not conflate:")
    for t in P.targets():
        if not t["url"].startswith("http"):
            continue
        try:
            s = P.Session(t)
            print("   %-34s activeElement=%-28s hasFocus=%s"
                  % (t["url"][-32:],
                     s.ev("document.activeElement?document.activeElement.tagName+"
                          "'/'+(document.activeElement.placeholder||''):'none'"),
                     s.ev("document.hasFocus()")))
            s.close()
        except Exception:
            pass


if __name__ == "__main__":
    main()
