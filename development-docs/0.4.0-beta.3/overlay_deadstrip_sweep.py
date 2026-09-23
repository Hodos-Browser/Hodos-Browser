#!/usr/bin/env python3
# beta.3 2026-09-23 — macOS overlay dead-strip sweep (relay 23b). Real CGEventPost clicks (needs
# Terminal Accessibility). Per dropdown overlay: click its TRANSPARENT area (expect: closes),
# inert CONTENT (expect: stays open), OUTSIDE (expect: closes). Negative control: build without
# OverlayHitsContent => every transparent click STAYS OPEN (measured 2026-09-23, 7/7).
# Run from phase-3.5-layout-window-scoping/ (imports p35macprobe). Dev browser on CDP 9322.
import sys, time, json
sys.path.insert(0, "/Users/matt/Hodos-Browser/development-docs/0.4.0-beta.3/phase-3.5-layout-window-scoping")
import Quartz, p35macprobe as P
pid = P.preflight()
OVERLAYS = [
  ("cookie/privacy", "cookie_panel_show", ["200", "zanaadu.com"], "/privacy-shield"),
  ("downloads",      "download_panel_show", ["150"], "/downloads"),
  ("profile",        "profile_panel_show", ["100"], "/profile-picker"),
  ("menu",           "menu_show", ["30"], "/menu"),
  ("bookmarks",      "bookmarks_panel_show", ["300", "https://zanaadu.com/", "Zanaadu"], "/bookmarks"),
  ("tab-list",       "tablist_panel_show", ["300"], "/tab-list"),
  ("site-info",      "siteinfo_panel_show", ["120", "zanaadu.com", "secure"], "/site-info"),
]
def click(x, y):
    for kind in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(None, kind, (x, y), Quartz.kCGMouseButtonLeft))
        time.sleep(0.12)
def small_windows():
    return {r["num"]: r for r in P.windows(pid) if r["w"] < 800}
MEASURE = r"""JSON.stringify((()=>{
 const W=innerWidth,H=innerHeight; let bottom=0, right=0;
 for (const e of document.body.querySelectorAll('*')) { const r=e.getBoundingClientRect(), cs=getComputedStyle(e);
   if (r.width>0 && r.height>0 && cs.visibility!=='hidden' && cs.display!=='none') { bottom=Math.max(bottom,r.bottom); right=Math.max(right,r.right);} }
 const bare = e => !e || e===document.body || e===document.documentElement || e.id==='root';
 let transp=null, best=-1;
 for (let y=4;y<H-4;y+=6) for (let x=4;x<W-4;x+=6) { if (bare(document.elementFromPoint(x,y))) {
   let d=1e9; for (const e of document.body.querySelectorAll('*')) { const r=e.getBoundingClientRect(); if(!r.width||!r.height||bare(e)) continue;
     const dx=Math.max(r.left-x,0,x-r.right), dy=Math.max(r.top-y,0,y-r.bottom); d=Math.min(d,Math.hypot(dx,dy)); }
   if (d>best) { best=d; transp=[x,y]; } } }
 let content=null;
 outer: for (let y=Math.round(bottom*0.4);y<bottom;y+=5) for (let x=Math.round(right*0.3);x<right;x+=5) { const e=document.elementFromPoint(x,y); if (bare(e) || !/^(P|SPAN|H[1-6]|DIV)$/.test(e.tagName) || !e.textContent.trim()) continue;
   let a=e, inert=true; while(a && a!==document.body){ const cs=getComputedStyle(a);
     if (/^(BUTTON|A|INPUT|SELECT|TEXTAREA|LABEL|SVG|IMG)$/i.test(a.tagName) || a.getAttribute('role') || cs.cursor==='pointer') {inert=false;break;}  /* not a.onclick: React's root carries a noop onclick */ a=a.parentElement; }
   if (inert) { content=[x,y,e.tagName]; break outer; } }
 return {W,H,contentBottom:Math.round(bottom),contentRight:Math.round(right),transp,transpClearance:Math.round(best),content};})())"""
def open_it(ipc, args):
    before = set(small_windows())
    h = P.Conn(P.headers()[0]["webSocketDebuggerUrl"]); h.send_ipc(ipc, *args); time.sleep(1.8)
    new = [r for n, r in small_windows().items() if n not in before]
    return new[0] if new else None
results = []
for name, ipc, args, route in OVERLAYS:
    row = {"overlay": name}
    for trial in ("transparent", "content", "outside"):
        w = open_it(ipc, args)
        if not w: row[trial] = "NEVER OPENED"; continue
        t = [t for t in P.cdp_targets() if t["url"].split("?")[0].endswith(route)]
        m = json.loads(P.Conn(t[0]["webSocketDebuggerUrl"]).js(MEASURE)) if t else None
        row.setdefault("geom", f'window {w["w"]}x{w["h"]}, content to {m["contentRight"]}x{m["contentBottom"]}' if m else "no CDP target")
        if trial == "transparent":
            if not m or not m["transp"] or m["transpClearance"] < 12:
                row[trial] = f"n/a (no transparent area, clearance {m and m['transpClearance']})"; click(w["x"]-120, w["y"]+10); time.sleep(1); continue
            pt = (w["x"]+m["transp"][0], w["y"]+m["transp"][1])
        elif trial == "content":
            if not m or not m["content"]: row[trial] = "n/a (no inert content point)"; click(w["x"]-120, w["y"]+10); time.sleep(1); continue
            pt = (w["x"]+m["content"][0], w["y"]+m["content"][1])
        else:
            pt = (w["x"]-120, w["y"]+10) if w["x"] > 200 else (w["x"]+w["w"]+120, w["y"]+10)
        click(*pt); time.sleep(1.2)
        still = w["num"] in small_windows()
        row[trial] = f'{"STAYED OPEN" if still else "closed"} @ {pt}'
        if still: click(w["x"]-120 if w["x"]>200 else w["x"]+w["w"]+120, w["y"]+10); time.sleep(1)
    results.append(row); print(json.dumps(row), flush=True)
