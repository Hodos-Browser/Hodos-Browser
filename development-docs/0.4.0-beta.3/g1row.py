#!/usr/bin/env python3
"""
HUMAN_TEST_QUEUE `G1` — an overlay whose OWNER WINDOW closes.

Tear a tab into window B, open a dropdown IN B (so `D-h2` parents it to B),
close B with its real close button, then open that dropdown in A.

⛔ THE PRECONDITION IS THE ASSERTION. A first attempt at this row on 2026-09-19
printed "GREEN — overlay survived B's close" against a window that had NEVER
CLOSED: AXIsProcessTrusted was false, so the synthetic click was dropped, and the
test could not fail. Every step below is checked before the next one runs.
"""
import json, sys, time
import Quartz
import p35macprobe as P

pid = P.preflight()


def shells():
    return {r["num"]: r for r in P.windows(pid) if r["w"] >= 800 and r["h"] >= 600}


def overlay():
    return [r for r in P.windows(pid) if r["w"] == 400 and r["h"] == 500]


def click(x, y):
    for kind in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        Quartz.CGEventPost(Quartz.kCGHIDEventTap,
                           Quartz.CGEventCreateMouseEvent(None, kind, (x, y),
                                                          Quartz.kCGMouseButtonLeft))
        time.sleep(0.12)


# --- identify A and B by window NUMBER (stable; frames move) ---------------
conns = {}
for t in P.headers():
    c = P.Conn(t["webSocketDebuggerUrl"], timeout=45)
    conns[json.loads(c.js("JSON.stringify({sx:window.screenX})"))["sx"]] = c
xs = sorted(conns)
if len(xs) < 2:
    sys.exit("REFUSING: need two windows")
s = shells()
A_num = [n for n, r in s.items() if r["x"] == xs[0]][0]
B_num = [n for n, r in s.items() if r["x"] == xs[1]][0]
print(f"A = #{A_num} @x={xs[0]}   B = #{B_num} @x={xs[1]}")

# --- 1. open the cookie panel FROM B ---------------------------------------
conns[xs[1]].send_ipc("cookie_panel_show", "0")
time.sleep(1.5)
ov = overlay()
print(f"1. dropdown opened from B: {ov[0] if ov else 'NONE'}")
assert ov, "REFUSING: the dropdown never opened — nothing to test"

# --- 2. close B with a REAL click on its close button ----------------------
b = shells()[B_num]
target = (b["x"] + 20, b["y"] + 20)
print(f"2. clicking B's close button at {target}")
click(*target)
time.sleep(4)

# ⛔ THE GATE. If B is still here the row is void, not green.
still = shells()
if B_num in still:
    sys.exit(f"🚨 VOID — window B (#{B_num}) did NOT close: {still[B_num]}. "
             f"The click did not land; this row proves nothing.")
print(f"   ✅ B actually closed (shells now: {sorted(still)})")

# --- 3. THE ROW: does that dropdown still work in A? -----------------------
conns[xs[0]].send_ipc("cookie_panel_hide")
time.sleep(0.6)
conns[xs[0]].send_ipc("cookie_panel_show", "0")
time.sleep(1.8)
ov2 = overlay()
print(f"3. reopened from A: {ov2[0] if ov2 else 'NONE'}")
print("\nVERDICT:", "✅ GREEN — the overlay survived its owner window closing and reopens in A"
      if ov2 else "🚨 RED — the overlay is gone after its owner window closed")
for c in conns.values():
    try: c.close()
    except Exception: pass
