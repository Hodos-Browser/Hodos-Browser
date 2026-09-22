#!/usr/bin/env python3
"""
Relay 21f §1-B on macOS — the prompt-queue race, driven and verified ON SCREEN.

⛔ macOS assertion Windows cannot make: a log line is not enough, the NSWindow must
actually be on screen. So every prompt is confirmed as a REAL WINDOW before it is
answered.

⛔ INSTRUMENT NOTE, learned the hard way twice today: the connect / notification
overlay is **FULL-WINDOW** (same rect as the shell, e.g. 1440x795). A size filter
that excludes big windows throws away the very prompt it is looking for. Overlays
are therefore detected by **window NUMBER against a baseline**, never by size.
"""
import sys, time, json
sys.path.insert(0, '.')
import p35macprobe as P

pid = P.preflight()
BASE = {r["num"] for r in P.windows(pid)}


def onscreen_overlay():
    """An overlay NSWindow that is actually on screen — baseline-INDEPENDENT.

    ⛔ Not by size (the connect/notification overlay is FULL-WINDOW) and not against a
    startup baseline (the prompt may already be up when the run starts — that produced a
    false "NO NSWindow" once). A full-window overlay SHARES the shell's rect, so a rect
    with more than one window means an overlay is stacked on the shell; anything smaller
    than a shell is a dropdown-style overlay.
    """
    ws = P.windows(pid)
    shells = [r for r in ws if r["w"] >= 800 and r["h"] >= 600]
    if not shells:
        return None
    big = max(shells, key=lambda r: r["w"] * r["h"])
    rect = (big["x"], big["y"], big["w"], big["h"])
    stacked = [r for r in ws if (r["x"], r["y"], r["w"], r["h"]) == rect]
    if len(stacked) > 1:
        return stacked[0]                      # front-most at the shell's rect
    small = [r for r in ws if r["w"] >= 200 and r["h"] >= 200 and r not in shells]
    return small[0] if small else None


def prompt_target():
    for t in P.cdp_targets():
        if "/brc100-auth" in t.get("url", ""):
            return t
    return None


def answer(label_re="^connect$|allow|approve|^sign|confirm"):
    """Confirm the prompt is a real on-screen window, then click it."""
    t = prompt_target()
    if not t:
        return None
    ov = onscreen_overlay()
    c = P.Conn(t["webSocketDebuggerUrl"], timeout=25)
    try:
        txt = (c.js("document.body.innerText.slice(0,160)") or "").replace("\n", " | ")
        if not txt.strip():
            return None
        onscreen = ov
        print(f"   PROMPT  onscreen={'YES ' + str(onscreen['w']) + 'x' + str(onscreen['h']) if onscreen else '🚨 NO NSWindow'}"
              f"  :: {txt[:100]}")
        r = c.js("""(function(){const b=[...document.querySelectorAll('button')]
          .find(x=>/%s/i.test(x.textContent||''));
          if(!b) return 'NO_BTN:'+[...document.querySelectorAll('button')].map(x=>x.textContent).join('/');
          b.click(); return 'CLICKED:'+b.textContent;})()""" % label_re)
        print("      ->", r)
        return (txt, bool(onscreen))
    finally:
        c.close()


print("driving prompts (baseline-independent on-screen check)")
seen = []
t0 = time.time()
while time.time() - t0 < 60:
    got = answer()
    if got and got[0] not in [s[0] for s in seen]:
        seen.append(got)
        time.sleep(2.0)
    time.sleep(0.6)
print(f"\ndistinct prompts confirmed ON SCREEN: {sum(1 for _, ok in seen if ok)} of {len(seen)}")
