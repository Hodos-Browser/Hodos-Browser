#!/usr/bin/env python3
"""
`D-h2` z-order half + the window-close safety net.

⛔ WHY THIS IS SEPARATE from the geometry rows: relay round f measured the macOS
z-order symptom as WEAK — window A came in front of B on the FIRST CREATION of
menu/cookie/bookmarks/omnibox from B, but on 24 re-opens afterwards neither
window moved. So the row is only meaningful on a FRESH process where each
overlay's create path still runs. This script must be run right after launch.

⛔ ATTRIBUTION: "in front" is the CGWindowList index, front-to-back. That is the
discriminator round f used, and it separates "behind" from "hidden" — which look
identical on screen and have different causes.
"""
import json, sys, time
import p35macprobe as P

SETTLE = 1.1

# Only the four overlays that are actually addChildWindow: CHILDREN can move
# z-order; the other six attach to nothing (see the helper block comment).
CHILD_OVERLAYS = [
    ("menu",     "menu_show",         ["0"], "menu_hide"),
    ("cookie",   "cookie_panel_show", ["0"], "cookie_panel_hide"),
    ("omnibox",  "omnibox_show",      ["a"], "omnibox_hide"),
]


def shell_rows(pid):
    return [r for r in P.windows(pid) if r["w"] >= 800 and r["h"] >= 600]


def main():
    pid = P.preflight()
    wins = []
    for t in P.headers():
        c = P.Conn(t["webSocketDebuggerUrl"])
        g = json.loads(c.js("JSON.stringify({sx:window.screenX})"))
        wins.append((g["sx"], t, c))
    if len(wins) < 2:
        sys.exit("need two windows")
    wins.sort()
    (ax, _, connA), (bx, _, connB) = wins[0], wins[1]
    print(f"A at x={ax}, B at x={bx}\n")

    for label, show, args, hide in CHILD_OVERLAYS:
        before = shell_rows(pid)
        b_before = next((r["z"] for r in before if r["x"] == bx), None)
        a_before = next((r["z"] for r in before if r["x"] == ax), None)

        connB.send_ipc(show, *args)          # FIRST open, from window B
        time.sleep(SETTLE)

        after = shell_rows(pid)
        b_after = next((r["z"] for r in after if r["x"] == bx), None)
        a_after = next((r["z"] for r in after if r["x"] == ax), None)

        verdict = ("B STAYS IN FRONT" if (b_after is not None and a_after is not None
                                          and b_after < a_after)
                   else "🚨 A CAME IN FRONT OF B")
        print(f"  {label:<9} before: A.z={a_before} B.z={b_before}   "
              f"after: A.z={a_after} B.z={b_after}   -> {verdict}")

        connB.send_ipc(hide)
        time.sleep(0.5)

    connA.close(); connB.close()


if __name__ == "__main__":
    main()
