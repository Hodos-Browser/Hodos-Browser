#!/usr/bin/env python3
"""
macOS Phase 3 (#5 half) — does fullscreen act on the window that ASKED?

⛔ ATTRIBUTION IS BY `kCGWindowNumber`, CAPTURED BEFORE THE ACTION — **not** by frame.
A first version of this probe identified the windows by their x origin and reported
RED on a build where the fix worked: a window that enters fullscreen moves to x=0,
which is also the primary's x, so the two become indistinguishable EXACTLY when the
fix succeeds. The window number is stable across the transition; the frame is not.
⭐ Same family as the ClampOverlayToScreen trap in relay round k §4: a discriminator
that collapses in the success case is not a discriminator.

⚠️ A window in native fullscreen gets its own Space, so the OTHER window drops out of
`kCGWindowListOptionOnScreenOnly` entirely. "Absent" therefore means "not on the active
Space", not "closed" — which is itself evidence about which window went fullscreen.
"""
import json, sys, time
import p35macprobe as P


def by_number(pid):
    return {r["num"]: r for r in P.windows(pid) if r["w"] >= 800 and r["h"] >= 600}


def show(pid, tag, ids):
    cur = by_number(pid)
    parts = []
    for name, num in ids.items():
        r = cur.get(num)
        parts.append(f"{name}: " + (f"{r['w']}x{r['h']} @x={r['x']}" if r else "OFF-SPACE"))
    print(f"  {tag:<12} " + " | ".join(parts))
    return cur


def verdict(pid, ids, base, asked):
    cur = by_number(pid)
    grew = []
    for name, num in ids.items():
        b, c = base.get(num), cur.get(num)
        if c and b and c["h"] > b["h"]:
            grew.append(name)
        # A window that vanished while the OTHER grew is on another Space.
    if not grew:
        # Native fullscreen: the fullscreened window may be the only one left on-screen.
        present = [n for n, num in ids.items() if num in cur]
        if len(present) == 1:
            grew = present
    if not grew:
        print("  => could not attribute")
        return
    who = grew[0]
    print(f"  => the window that went fullscreen is {who}"
          f"  {'✅ correct — ' + asked + ' asked' if who == asked else '🚨 WRONG — ' + asked + ' asked'}")


def main():
    pid = P.preflight()
    conns, ids = {}, {}
    for t in P.headers():
        c = P.Conn(t["webSocketDebuggerUrl"])
        sx = json.loads(c.js("JSON.stringify({sx:window.screenX})"))["sx"]
        conns[sx] = c
    xs = sorted(conns)
    nums = by_number(pid)
    for name, x in (("A", xs[0]), ("B", xs[1])):
        match = [n for n, r in nums.items() if r["x"] == x]
        ids[name] = match[0]
    A, B = conns[xs[0]], conns[xs[1]]
    print(f"A = window #{ids['A']} at x={xs[0]} (primary)")
    print(f"B = window #{ids['B']} at x={xs[1]} (torn off)\n")

    print("ROW 1 — MENU fullscreen driven from window B's header")
    base = show(pid, "before", ids)
    B.send_ipc("menu_action", "fullscreen")
    time.sleep(7)
    show(pid, "after", ids)
    verdict(pid, ids, base, "B")
    B.send_ipc("menu_action", "fullscreen")
    time.sleep(7)
    show(pid, "restored", ids)

    print("\nROW 2 — CONTENT fullscreen from a TAB in window B")
    btab = None
    for t in P.cdp_targets():
        u = t.get("url", "")
        if t.get("type") != "page" or "5137" not in u or u.rstrip("/").endswith("5137"):
            continue
        c = P.Conn(t["webSocketDebuggerUrl"])
        try:
            g = json.loads(c.js("JSON.stringify({sx:window.screenX,ih:window.innerHeight})"))
            if g["sx"] == xs[1] and g["ih"] > 400:
                btab = c
                break
        except Exception:
            pass
        c.close()
    if not btab:
        print("  (no tab in window B — skipped)")
        return
    base = show(pid, "before", ids)
    print("  enter:", btab.js("document.documentElement.requestFullscreen()"
                              ".then(()=>'ok').catch(e=>'ERR '+e.message)"))
    time.sleep(5)
    show(pid, "after", ids)
    verdict(pid, ids, base, "B")
    # Whose header got hidden is the second, independent discriminator.
    for name, x in (("A", xs[0]), ("B", xs[1])):
        print(f"    header {name}: innerHeight={conns[x].js('window.innerHeight')}")
    print("  exit :", btab.js("document.exitFullscreen().then(()=>'ok').catch(e=>'ERR '+e.message)"))
    time.sleep(5)
    show(pid, "restored", ids)
    btab.close()
    for c in conns.values():
        c.close()


if __name__ == "__main__":
    main()
