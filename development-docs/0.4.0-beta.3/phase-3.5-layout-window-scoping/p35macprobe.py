#!/usr/bin/env python3
"""
beta.3 macOS port of Phase 3.5 (`D-h2`) — overlay-follows-window probe.

THE QUESTION (relay round 2026-09-19f §3): on macOS every overlay anchors and
attaches to the process-global `g_main_window`, so a dropdown opened from a
torn-off window B opens over the PRIMARY window A. Round f measured 8/8 overlays
at A-relative coordinates whichever window asked.

⛔ LAYER. This reads the **AppKit/CoreGraphics window layer** —
`CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, ...)` gives
front-to-back z-order plus frames for a pid, with NO Accessibility grant needed.
Nothing here is a DOM fact. The overlays are OSR CEF windows, so their React
content is invisible to this probe by design: we measure WHERE the native window
landed, which is exactly the defect.

⛔ DEV ONLY. Every sample is filtered to the dev pid. The owner's INSTALLED
production browser runs under the same bundle id and holds CDP 9222; dev is 9322
(`cef_browser_shell_mac.mm :: main`, +100 under HODOS_DEV). The probe refuses to
run if it cannot tell the two apart.

ATTRIBUTION RULE (round f): windows A and B are placed so their x differs by a
known offset, and a right-anchored overlay's x tells you which window it was
anchored to. Never attribute by the keep-alive target URL — an overlay reused
across windows keeps its original URL.
"""
import json
import subprocess
import sys
import time
import urllib.request

import Quartz

DEV_CDP = 9322
PROD_CDP = 9222
DEV_BUNDLE_MARK = "cef-native/build/bin/HodosBrowser.app"


# ---------------------------------------------------------------- process ---

def dev_pid():
    """The dev SHELL pid, matched by bundle PATH.

    ⛔ Never match the bare process name: dev and installed-prod are both called
    "HodosBrowser" (CMakeLists OUTPUT_NAME), so a name match would return the
    owner's production browser. Helpers carry --type=, the shell does not.
    """
    out = subprocess.run(["ps", "-axo", "pid=,args="], capture_output=True,
                         text=True).stdout
    hits = []
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        pid, _, args = line.partition(" ")
        if DEV_BUNDLE_MARK in args and "--type=" not in args:
            hits.append(int(pid))
    if not hits:
        return None
    return min(hits)


# ----------------------------------------------------------------- window ---

def windows(pid):
    """One front-to-back sample of every on-screen window owned by `pid`.

    CGWindowListCopyWindowInfo returns windows in front-to-back order, and that
    order IS the measurement for the z-order half: "behind" and "hidden" look
    identical on screen and have different causes.
    """
    info = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID)
    rows = []
    z = 0
    for w in info:
        if w.get("kCGWindowOwnerPID") != pid:
            continue
        b = w.get("kCGWindowBounds", {})
        rows.append({
            "z": z,
            "num": w.get("kCGWindowNumber"),
            "name": w.get("kCGWindowName") or "",
            "layer": w.get("kCGWindowLayer"),
            "x": int(b.get("X", 0)), "y": int(b.get("Y", 0)),
            "w": int(b.get("Width", 0)), "h": int(b.get("Height", 0)),
        })
        z += 1
    return rows


def shells(pid):
    """The top-level browser windows (titled 'Hodos Browser'), front to back."""
    return [r for r in windows(pid) if r["name"] == "Hodos Browser"]


def overlays(pid):
    """Everything that is NOT a shell window and is not a zero-size stub.

    Overlay NSWindows are borderless and unnamed, so they are identified by
    exclusion and then attributed by FRAME, per the attribution rule above.
    """
    out = []
    for r in windows(pid):
        if r["name"] == "Hodos Browser":
            continue
        if r["w"] < 40 or r["h"] < 40:
            continue
        out.append(r)
    return out


def fmt(rows):
    return "\n".join(
        f"    z={r['z']:<2} layer={r['layer']:<4} {r['w']:>5}x{r['h']:<5} "
        f"@ ({r['x']:>5},{r['y']:>5})  {r['name']}"
        for r in rows)


# -------------------------------------------------------------------- CDP ---

def cdp_targets(port=DEV_CDP):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=5) as r:
        return json.load(r)


def headers(port=DEV_CDP):
    """Header browser targets — one per browser window.

    The header page is served at 127.0.0.1:5137 with no path (the shell's own
    UI). Overlays are also 5137 pages but carry a path (/menu, /omnibox, ...),
    so the bare-root test separates them.
    """
    out = []
    for t in cdp_targets(port):
        url = t.get("url", "")
        if t.get("type") != "page":
            continue
        if url.rstrip("/") in ("http://127.0.0.1:5137", "http://localhost:5137"):
            out.append(t)
    return out


class Conn:
    """Minimal CDP client. websocket-client is installed; requests is not."""

    def __init__(self, ws_url):
        import websocket
        self.ws = websocket.create_connection(ws_url, timeout=15)
        self.n = 0

    def call(self, method, params=None):
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": method,
                                 "params": params or {}}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == self.n:
                return msg

    def js(self, expr):
        r = self.call("Runtime.evaluate",
                      {"expression": expr, "awaitPromise": True,
                       "returnByValue": True, "userGesture": True})
        return r.get("result", {}).get("result", {}).get("value")

    def send_ipc(self, name, *args):
        """window.cefMessage.send(name, ...args) — the real IPC seam.

        ⭐ This is the same path the React click handler uses, so it is a valid
        instrument for the C++ side. It does NOT prove the React click target.
        """
        js_args = ", ".join(json.dumps(a) for a in args)
        sep = ", " if js_args else ""
        return self.js(
            f"(function(){{ if(!window.cefMessage||!window.cefMessage.send) return 'NO_BRIDGE';"
            f" window.cefMessage.send({json.dumps(name)}{sep}{js_args}); return 'SENT'; }})()")

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass


# ------------------------------------------------------------------ guards ---

def preflight():
    pid = dev_pid()
    if not pid:
        sys.exit("REFUSING: no DEV shell running (cef-native/build/bin/...). "
                 "Start it with cef-native/mac_build_run.sh")
    try:
        cdp_targets(DEV_CDP)
    except Exception as e:
        sys.exit(f"REFUSING: dev CDP {DEV_CDP} not answering ({e}). "
                 "A non-Default profile leaves the port at 0.")
    # Positive control on the dev/prod split: if 9322 and 9222 answer with the
    # same target set we are talking to the wrong browser.
    try:
        prod = cdp_targets(PROD_CDP)
        dev = cdp_targets(DEV_CDP)
        if prod and dev and {t["id"] for t in prod} == {t["id"] for t in dev}:
            sys.exit("REFUSING: 9322 and 9222 return identical targets — "
                     "that is the PRODUCTION browser.")
    except SystemExit:
        raise
    except Exception:
        pass  # prod not running is fine
    return pid


if __name__ == "__main__":
    pid = preflight()
    print(f"dev shell pid = {pid}")
    print("shell windows (front to back):")
    print(fmt(shells(pid)))
    print("overlay windows (front to back):")
    print(fmt(overlays(pid)))
    print("\nheader targets:")
    for t in headers():
        print(f"    {t['id']}  {t['url']}")
