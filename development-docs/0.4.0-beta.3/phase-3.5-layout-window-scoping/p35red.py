#!/usr/bin/env python3
"""
`D-h2` — drive every dropdown from BOTH windows and record where it landed.

THE ROW: an overlay opened from window B must anchor to B. Pre-fix every
`Calculate*OverlayFrame` call in cef_browser_shell_mac.mm passes the process
global `g_main_window`, so the answer is A-relative whichever window asked.

⭐ ATTRIBUTION IS BY FRAME, never by the overlay's target URL: a kept-alive
overlay reused from the other window keeps its ORIGINAL URL, so the URL would
report the window that opened it FIRST. The x coordinate cannot lie — A and B
are 600 pt apart.

Output columns:
  from_A / from_B   the overlay's NSWindow frame origin, as CoreGraphics sees it
  verdict           SAME  = the requesting window had no influence (the defect)
                    FOLLOWS = B's open is offset from A's (the fix)
"""
import json
import sys
import time

import p35macprobe as P

SETTLE = 0.9          # overlay create + show is async through CEF
HIDE_DEBOUNCE = 0.45  # WasCookiePanelJustHidden() etc. debounce at 300 ms

# (label, show IPC, args, hide IPC)
OVERLAYS = [
    ("menu",      "menu_show",            ["0"], "menu_hide"),
    ("profile",   "profile_panel_show",   ["0"], "profile_panel_hide"),
    ("download",  "download_panel_show",  ["0"], "download_panel_hide"),
    ("cookie",    "cookie_panel_show",    ["0"], "cookie_panel_hide"),
    ("bookmarks", "bookmarks_panel_show", ["0"], "bookmarks_panel_hide"),
    ("siteinfo",  "siteinfo_panel_show",  ["0"], "siteinfo_panel_hide"),
    ("tablist",   "tablist_panel_show",   ["0"], "tablist_panel_hide"),
    ("omnibox",   "omnibox_show",         ["a"], "omnibox_hide"),
]


def locate_windows():
    """Map each header CDP target to its shell window frame.

    ⭐ window.screenX/screenY on the HEADER page is the shell window's content
    origin and outerWidth/outerHeight its full frame — measured to match the
    CGWindowList row exactly. That is what identifies which shell is which
    without any Accessibility grant.
    """
    out = []
    for t in P.headers():
        c = P.Conn(t["webSocketDebuggerUrl"])
        geo = json.loads(c.js(
            "JSON.stringify({sx:window.screenX,sy:window.screenY,"
            "ow:window.outerWidth,oh:window.outerHeight})"))
        out.append({"target": t, "conn": c, **geo})
    # Window A is the PRIMARY: the one created first, which is the leftmost here
    # because the tear-off placed B at +600. Sort so A is index 0.
    out.sort(key=lambda w: w["sx"])
    return out


def open_and_sample(conn, show, args, pid, before_nums):
    conn.send_ipc(show, *args)
    time.sleep(SETTLE)
    rows = [r for r in P.windows(pid) if r["num"] not in before_nums]
    # The overlay is the new on-screen window. If the overlay already existed
    # (keep-alive) it is not "new", so fall back to any non-shell window that
    # moved — handled by the caller passing the shell nums only.
    return rows


def run():
    pid = P.preflight()
    wins = locate_windows()
    if len(wins) < 2:
        sys.exit("REFUSING: need TWO windows. Tear a tab off first "
                 "(tab_tearoff IPC), then re-run.")
    A, B = wins[0], wins[1]
    print(f"window A (primary): x={A['sx']} y={A['sy']} {A['ow']}x{A['oh']}")
    print(f"window B (torn off): x={B['sx']} y={B['sy']} {B['ow']}x{B['oh']}")
    dx = B["sx"] - A["sx"]
    print(f"A/B x separation = {dx} pt  (a B-anchored right overlay differs by this)\n")

    shell_nums = {r["num"] for r in P.windows(pid)
                  if r["w"] >= 800 and r["h"] >= 600}

    results = []
    for label, show, args, hide in OVERLAYS:
        row = {"overlay": label}
        for who, w in (("A", A), ("B", B)):
            # Hide first so every arm starts from the same state.
            w["conn"].send_ipc(hide)
            time.sleep(HIDE_DEBOUNCE)
            w["conn"].send_ipc(show, *args)
            time.sleep(SETTLE)
            cand = [r for r in P.windows(pid) if r["num"] not in shell_nums
                    and r["w"] >= 40 and r["h"] >= 40]
            if not cand:
                row[who] = None
            else:
                # Front-most non-shell window is the one just shown.
                c = cand[0]
                row[who] = (c["x"], c["y"], c["w"], c["h"])
            w["conn"].send_ipc(hide)
            time.sleep(HIDE_DEBOUNCE)
        a, b = row.get("A"), row.get("B")
        if a and b:
            row["verdict"] = "SAME" if a[:2] == b[:2] else f"MOVED by {b[0]-a[0]}"
        else:
            row["verdict"] = "NOT_SEEN"
        results.append(row)
        print(f"  {label:<10} fromA={row.get('A')}  fromB={row.get('B')}  -> {row['verdict']}")

    print("\n--- summary ---")
    same = sum(1 for r in results if r["verdict"] == "SAME")
    print(f"{same}/{len(results)} overlays ignored the requesting window")
    for w in wins:
        w["conn"].close()
    return results


if __name__ == "__main__":
    run()
