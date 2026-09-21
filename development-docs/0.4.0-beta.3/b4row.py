#!/usr/bin/env python3
"""
HUMAN_TEST_QUEUE `B4` — overlap two windows, open a dropdown in the SECOND with a
REAL CLICK, and check that second window does not drop behind the first.

⛔ This is the half `D-h2` could not close. Round k measured the GEOMETRY with the
IPC and had to report the z-order one-sided, because macOS dropped synthetic
clicks (AXIsProcessTrusted false). With the grant, the gesture is real.

⛔ DISCRIMINATOR, from Phase 3.5 `Z1`: the **z index**, not visibility. "Behind"
and "minimised" look identical on screen and have different causes, so the rect is
asserted unchanged as well.
"""
import json, sys, time
import Quartz
import p35macprobe as P

pid = P.preflight()


def shells():
    return {r["num"]: r for r in P.windows(pid) if r["w"] >= 800 and r["h"] >= 600}


def click(x, y):
    for k in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        Quartz.CGEventPost(Quartz.kCGHIDEventTap,
                           Quartz.CGEventCreateMouseEvent(None, k, (x, y),
                                                          Quartz.kCGMouseButtonLeft))
        time.sleep(0.12)


conns = {}
for t in P.headers():
    c = P.Conn(t["webSocketDebuggerUrl"], timeout=45)
    g = json.loads(c.js("JSON.stringify({sx:window.screenX,sy:window.screenY})"))
    conns[g["sx"]] = (c, g)
xs = sorted(conns)
if len(xs) < 2:
    sys.exit("REFUSING: need two windows")
s = shells()
A = [n for n, r in s.items() if r["x"] == xs[0]][0]
B = [n for n, r in s.items() if r["x"] == xs[1]][0]
print(f"A = #{A} @x={xs[0]}   B = #{B} @x={xs[1]}  (they overlap)")

cB, gB = conns[xs[1]]
btn = json.loads(cB.js("""(function(){
  const b=[...document.querySelectorAll('button,[role=button]')]
    .find(e=>/privacy shield/i.test(e.getAttribute('aria-label')||e.title||''));
  if(!b) return 'null';
  const r=b.getBoundingClientRect();
  return JSON.stringify({x:Math.round(r.x+r.width/2), y:Math.round(r.y+r.height/2)});
})()"""))
sx, sy = gB["sx"] + btn["x"], gB["sy"] + btn["y"]

# Make B the front window the way a user would: click its own toolbar area first.
before = shells()
zB, zA = before[B]["z"], before[A]["z"]
rectB = (before[B]["x"], before[B]["y"], before[B]["w"], before[B]["h"])
print(f"before: B.z={zB} A.z={zA}   B rect={rectB}")
if zB > zA:
    sys.exit("REFUSING: B is already behind A before the click — wrong starting state")

print(f"clicking B's Privacy Shield icon at ({sx},{sy})")
click(sx, sy)
time.sleep(2.5)

after = shells()
if B not in after:
    sys.exit(f"🚨 VOID — window B vanished from the on-screen list entirely")
zB2, zA2 = after[B]["z"], after[A]["z"]
rectB2 = (after[B]["x"], after[B]["y"], after[B]["w"], after[B]["h"])
ov = [r for r in P.windows(pid) if r["w"] == 400 and r["h"] == 500]
print(f"after : B.z={zB2} A.z={zA2}   B rect={rectB2}")
print(f"dropdown: {ov[0] if ov else 'NONE — the click did not open it'}")

if not ov:
    sys.exit("🚨 VOID — no dropdown opened, so the row tested nothing")
print("\nVERDICT:",
      "✅ GREEN — B stayed in front of A and its rect is unchanged"
      if (zB2 < zA2 and rectB2 == rectB)
      else f"🚨 RED — B.z={zB2} vs A.z={zA2}, rect {rectB} -> {rectB2}")
for c, _ in conns.values():
    try: c.close()
    except Exception: pass
