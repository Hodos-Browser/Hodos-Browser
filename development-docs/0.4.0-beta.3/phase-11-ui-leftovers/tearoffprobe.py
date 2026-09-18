#!/usr/bin/env python3
"""
beta.3 Phase 11 item 5 (`P11-I5`) — tear-off window overlay sweep.

THE QUESTION (owner, 2026-09-15): *"typing in a torn-off tab's address bar makes
that window disappear."* Phase 3.5 measured and fixed exactly this shape for
Ctrl+N windows (K9: the overlay pulled the PRIMARY window forward, so the
requesting window went BEHIND it and read as "vanished"; fix =
`OwnOverlayToRequestingWindow`). It was never measured on a torn-off window.

⛔ LAYER. This reads the **Win32 window layer**: EnumWindows z-order, plus
IsWindowVisible and IsIconic on every Hodos shell and overlay HWND in the dev
process. "Went behind" and "was minimised" look identical on screen and have
different causes, so both discriminators are printed. Nothing here is a DOM fact.

⭐ WHY IT CAN BE DRIVEN AT ALL: tear-off has an IPC (`tab_tearoff` ->
simple_handler.cpp), so the window can be created without the drag gesture that
`SendInput` cannot deliver here. ⚠️ That means this proves the WINDOW-OWNERSHIP
half, not the drag. A real drag also moves the mouse and changes activation.
"""
import ctypes
import ctypes.wintypes as wt
import json
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import omniboxprobe as P  # noqa: E402

user32 = ctypes.WinDLL("user32", use_last_error=True)
EnumWindowsProc = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

SHELL_CLASS = "HodosBrowserWndClass"
OVERLAY_CLASSES = [
    "CEFOmniboxOverlayWindow", "CEFMenuOverlayWindow", "CEFCookiePanelOverlayWindow",
    "CEFDownloadPanelOverlayWindow", "CEFProfilePanelOverlayWindow",
    "CEFSiteInfoPanelOverlayWindow", "CEFTabListPanelOverlayWindow",
    "CEFBookmarksPanelOverlayWindow", "CEFTabMenuOverlayWindow",
    "CEFWalletOverlayWindow", "CEFSettingsMenuOverlayWindow",
    "CEFSettingsOverlayWindow", "CEFNotificationOverlayWindow",
]


class RECT(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                ("right", ctypes.c_long), ("bottom", ctypes.c_long)]


def sample(pid):
    """One z-ordered sample. EnumWindows returns TOPMOST FIRST and that order is
    the whole point: it separates 'hidden' from 'went behind'."""
    rows = []
    cls = ctypes.create_unicode_buffer(512)
    z = [0]

    def cb(hwnd, _lp):
        wpid = wt.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(wpid))
        if wpid.value != pid:
            return True
        user32.GetClassNameW(hwnd, cls, 512)
        name = cls.value
        if name != SHELL_CLASS and name not in OVERLAY_CLASSES:
            return True
        z[0] += 1
        r = RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(r))
        rows.append({
            "z": z[0], "hwnd": hwnd, "cls": name,
            "vis": bool(user32.IsWindowVisible(hwnd)),
            "min": bool(user32.IsIconic(hwnd)),
            "rect": "%d,%d %dx%d" % (r.left, r.top, r.right - r.left, r.bottom - r.top),
        })
        return True

    user32.EnumWindows(EnumWindowsProc(cb), 0)
    return rows


def shells(rows):
    return [r for r in rows if r["cls"] == SHELL_CLASS]


def dump(rows, label):
    print("  --- %s ---" % label)
    for r in rows:
        print("      z%-2d %-30s vis=%-5s min=%-5s %s"
              % (r["z"], r["cls"], r["vis"], r["min"], r["rect"]))


# \u2b50 The 3.5 write-up predicted its fix "generalises beyond the omnibox" and
# admitted it was "not yet tested on a second overlay". This is that test: every
# dropdown-style overlay that OwnOverlayToRequestingWindow covers, driven from the
# TORN-OFF window's header. IPC name -> the window class it should raise.
SWEEP = [
    ("omnibox",   None,                              "CEFOmniboxOverlayWindow"),
    ("menu",      ("menu_show", "[]"),               "CEFMenuOverlayWindow"),
    ("cookie",    ("cookie_panel_show", "['0','example.com']"),
                                                     "CEFCookiePanelOverlayWindow"),
    ("download",  ("download_panel_show", "['0']"),  "CEFDownloadPanelOverlayWindow"),
    ("profile",   ("profile_panel_show", "['0']"),   "CEFProfilePanelOverlayWindow"),
    ("siteinfo",  ("siteinfo_panel_show", "['0','example.com','secure']"),
                                                     "CEFSiteInfoPanelOverlayWindow"),
    ("tablist",   ("tablist_panel_show", "['0']"),   "CEFTabListPanelOverlayWindow"),
    ("bookmarks", ("bookmarks_panel_show", "['0']"), "CEFBookmarksPanelOverlayWindow"),
]


def split_windows(rows):
    """Move the two shell windows so their rects DO NOT OVERLAP.

    \u26d4 THIS IS LOAD-BEARING, and the first version of this probe did not do it.
    Ownership is attributed from the overlay's origin, and window A (0,0 1920w)
    and the torn-off B (800,380 1820w) OVERLAP heavily. Every overlay anchored to
    a window's RIGHT edge -- menu 1605, profile 1485, download 1435, cookie 1365 --
    has an origin past B's left edge, so all four were attributed to B while the
    user had typed in A, and the probe printed four confident REDs that were
    entirely its own. Non-overlapping windows make the attribution exact.
    """
    sh = shells(rows)
    if len(sh) != 2:
        return
    sh = sorted(sh, key=lambda r: r["hwnd"])
    SWP = 0x0004 | 0x0010  # NOACTIVATE | NOZORDER
    for i, r in enumerate(sh):
        user32.SetWindowPos(r["hwnd"], 0, i * 960, 0, 940, 1000, SWP)
    time.sleep(1.5)


def owner_of(rows, cls="CEFOmniboxOverlayWindow"):
    """Which shell window does the visible omnibox belong to?

    \u26d4 Derived from GEOMETRY, not from the z-order we are about to judge.
    ShowOmniboxOverlay places the dropdown at `mainRect.left + ScalePx(160, hwnd)`,
    so the overlay's origin sits inside exactly one shell window's rect. Using the
    z-order to decide ownership and then judging the z-order would be circular.
    """
    omni = next((r for r in rows if r["cls"] == cls and r["vis"]), None)
    if omni is None:
        return None, None
    ox = int(omni["rect"].split(",")[0])
    # With split_windows() applied the shell rects are disjoint, so "the window
    # whose horizontal span contains the overlay origin" is exact rather than a
    # nearest-left guess.
    for sh in shells(rows):
        left = int(sh["rect"].split(",")[0])
        width = int(sh["rect"].split(" ")[1].split("x")[0])
        if left <= ox < left + width:
            return sh, omni
    return None, omni


def verdict(rows, owner):
    """The assertion is about the window the user TYPED IN, not about a fixed
    window.

    \u26d4 The first version of this asserted that the torn-off window B must stay
    in front no matter which header was driven -- so typing in window A, where A
    coming forward is CORRECT, scored as a reproduction. Same family as the
    item-3 probe that asserted "the value changed" instead of "the value is the
    one we expect": an assertion that does not name the right subject reports a
    defect that is not there.
    """
    if owner is None:
        return "SKIP: that overlay never became visible - nothing to judge"
    if owner["min"] or owner["rect"].startswith("-32000"):
        return "RED: the window typed in was MINIMISED"
    if not owner["vis"]:
        return "RED: the window typed in was HIDDEN"
    others = [r for r in shells(rows) if r["hwnd"] != owner["hwnd"]]
    if others and owner["z"] > min(o["z"] for o in others):
        return ("RED: the window typed in went BEHIND the other shell window "
                "(z%d vs z%d)" % (owner["z"], min(o["z"] for o in others)))
    return "OK: the window typed in stayed visible and in front (z%d)" % owner["z"]


def main():
    pid = P.dev_browser_pid()
    print("SUBJECT: dev browser pid %d (matched by EXE PATH, not image name)" % pid)

    base = sample(pid)
    dump(base, "baseline")
    if len(shells(base)) != 1:
        sys.exit("expected exactly ONE shell window to start from, found %d. "
                 "Close extra windows first." % len(shells(base)))
    hwnd_a = shells(base)[0]["hwnd"]

    hdr = P.Session(P.pick_exact(P.HEADER_URL))
    try:
        # Need 2+ tabs: the last tab in a window cannot be torn off.
        tabs = json.loads(hdr.ev_async(
            "new Promise(function(r){window.addEventListener('message',function h(e){"
            "if(e.data&&e.data.type==='tab_list_response'){window.removeEventListener('message',h);"
            "r(e.data.data)}});window.cefMessage.send('get_tab_list')})") or "{}")
        n = len(tabs.get("tabs", []))
        print("  tabs open: %d" % n)
        while n < 2:
            hdr.ev("window.cefMessage.send('tab_create', 'http://127.0.0.1:5137/newtab')")
            time.sleep(2.5)
            n += 1
        tabs = json.loads(hdr.ev_async(
            "new Promise(function(r){window.addEventListener('message',function h(e){"
            "if(e.data&&e.data.type==='tab_list_response'){window.removeEventListener('message',h);"
            "r(e.data.data)}});window.cefMessage.send('get_tab_list')})"))
        victim = tabs["tabs"][-1]["id"]
        print("  tearing off tab %d via the tab_tearoff IPC" % victim)
        hdr.ev("window.cefMessage.send('tab_tearoff', %d, 900, 400)" % victim)
        time.sleep(4)
    finally:
        hdr.close()

    split_windows(sample(pid))
    after_tear = sample(pid)
    dump(after_tear, "after tear-off (windows moved side by side, see split_windows)")
    sh = shells(after_tear)
    if len(sh) != 2:
        sys.exit("tear-off did not produce a second shell window (found %d)" % len(sh))
    hwnd_b = [r["hwnd"] for r in sh if r["hwnd"] != hwnd_a][0]
    print("  window A = 0x%X (original), window B = 0x%X (torn off)" % (hwnd_a, hwnd_b))

    # Window B's header is a SECOND browser at the same URL, so it can only be
    # addressed by target id — the whole reason p35drive grew the '#' form.
    heads = [t for t in P.targets() if t["url"] == P.HEADER_URL]
    print("  header targets now: %d (B's is the one that did not exist before)" % len(heads))
    if len(heads) != 2:
        sys.exit("expected 2 header targets after tear-off, found %d" % len(heads))

    results = []
    for t in heads:
        s = P.Session(t)
        try:
            # Identify this header's window ONCE, with the omnibox, then sweep the
            # rest of the overlays from the same header.
            s.ev("%s.focus()" % P.ADDR_SEL)
            time.sleep(0.3)
            for ch in "exam":
                P.type_char(s, ch)
            time.sleep(2.0)
            rows = sample(pid)
            owner, _ = owner_of(rows)
            if owner is None:
                print("  header %s: the omnibox never appeared - cannot identify its "
                      "window, skipping" % t["id"][:8])
                s.ev("window.cefMessage.send('omnibox_hide', [])")
                continue
            which = "B (torn off)" if owner["hwnd"] == hwnd_b else "A (original)"
            print("\n  === header %s drives window %s ===" % (t["id"][:8], which))
            print("      omnibox   %s" % verdict(rows, owner))
            results.append((which, "omnibox", verdict(rows, owner)))
            s.ev("window.cefMessage.send('omnibox_hide', [])")
            s.ev("%s.blur()" % P.ADDR_SEL)
            time.sleep(0.8)

            for name, ipc, cls in SWEEP:
                if ipc is None:
                    continue
                s.ev("window.cefMessage.send(%r, %s)" % (ipc[0], ipc[1]))
                time.sleep(1.6)
                rows = sample(pid)
                own2, ov = owner_of(rows, cls)
                v = verdict(rows, own2)
                mis = ""
                if own2 is not None and own2["hwnd"] != owner["hwnd"]:
                    mis = "  [!] it opened over the OTHER window"
                print("      %-9s %s%s" % (name, v, mis))
                results.append((which, name, v + mis))
                # every dropdown has its own hide IPC; the generic one is a no-op here
                for hide in ("menu_hide", "cookie_panel_hide", "download_panel_hide",
                             "profile_panel_hide", "siteinfo_panel_hide",
                             "tablist_panel_hide", "bookmarks_panel_hide"):
                    s.ev("window.cefMessage.send(%r, [])" % hide)
                time.sleep(0.8)
        finally:
            s.close()

    print("")
    covered = {w for (w, _, _) in results}
    bad = [r for r in results if r[2].startswith("RED") or "[!]" in r[2]]
    skipped = [r for r in results if r[2].startswith("SKIP")]
    if "B (torn off)" not in covered:
        print("VERDICT: INCOMPLETE - the torn-off window was never driven.")
        return 2
    print("rows: %d judged, %d skipped (overlay never appeared), %d red"
          % (len(results) - len(skipped), len(skipped), len(bad)))
    for r in skipped:
        print("   SKIPPED  window %s / %s" % (r[0], r[1]))
    if bad:
        print("VERDICT: [RED] REPRODUCED:")
        for r in bad:
            print("   window %s / %s: %s" % r)
        return 1
    print("VERDICT: [GREEN] NOT REPRODUCED on any overlay that appeared, including")
    print("         on the torn-off window. Each overlay opened over the window")
    print("         that asked for it, and that window stayed visible and in front.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
