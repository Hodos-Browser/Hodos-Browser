#!/usr/bin/env python3
"""
beta.3 Phase 11 — omnibox cluster instrument (items 2 and 3).

⛔ LAYER DISCLOSURE (the rule that cost 2026-09-17/18 three false greens).
   This probe reads THREE layers and says which is which on every line:

     HWND  — Win32 IsWindowVisible() on the real CEFOmniboxOverlayWindow HWND.
             This is the layer the user's complaint lives in for item 2
             ("the omnibox is still on screen").
     DOM   — the HEADER browser's <input>.value. This is the layer item 3's
             complaint lives in ("the url takes a long time to load in the
             address box").
     IPC   — what we make React send. Driving is at the DOM/React layer:
             CDP key events fire React's onChange/onKeyDown exactly as typing
             does, and CDP mouse events fire the overlay's onClick exactly as
             clicking does. ⛔ It does NOT reproduce NATIVE focus routing.
             Nothing here claims anything about native input delivery.

⛔ SUBJECT. Dev browser only, CDP port 9322 (9222 is the installed build — the
   script refuses it). Header and ~14 overlays are all type:"page"; targets are
   named by exact URL, never by index.
"""

import ctypes
import ctypes.wintypes as wt
import json
import sys
import time
from urllib.request import urlopen

import websocket

DEV_PORT = 9322
DEV_PATH_MATCH = r"cef-native\build\bin\Release"
OMNIBOX_CLASS = "CEFOmniboxOverlayWindow"
HEADER_URL = "http://127.0.0.1:5137/"
OMNIBOX_URL = "http://127.0.0.1:5137/omnibox"

user32 = ctypes.WinDLL("user32", use_last_error=True)
EnumWindowsProc = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)


def dev_browser_pid():
    """⛔ Match the DEV build by EXE PATH, never by image name — the owner's
    installed browser shares the name (CLAUDE.md, feedback_never_kill_by_image_name)."""
    import subprocess
    ps = (
        "Get-CimInstance Win32_Process -Filter \"Name='HodosBrowser.exe'\" | "
        "Where-Object { $_.ExecutablePath -like '*%s*' -and $_.CommandLine -notmatch '--type=' } | "
        "Select-Object -ExpandProperty ProcessId" % DEV_PATH_MATCH
    )
    out = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                         capture_output=True, text=True).stdout.split()
    if len(out) != 1:
        sys.exit("SUBJECT FAIL: expected exactly one dev browser process, got %r" % out)
    return int(out[0])


def find_hwnd(pid, cls_name):
    found = []
    buf = ctypes.create_unicode_buffer(512)

    def cb(hwnd, _lp):
        wpid = wt.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(wpid))
        if wpid.value != pid:
            return True
        user32.GetClassNameW(hwnd, buf, 512)
        if buf.value == cls_name:
            found.append(hwnd)
        return True

    user32.EnumWindows(EnumWindowsProc(cb), 0)
    return found[0] if found else None


def is_visible(hwnd):
    return bool(user32.IsWindowVisible(hwnd))


# ---------------------------------------------------------------- CDP plumbing
def targets(port=DEV_PORT):
    if port == 9222:
        sys.exit("refusing to attach to 9222 — that is the installed browser")
    return json.loads(urlopen("http://127.0.0.1:%d/json/list" % port, timeout=5).read())


def pick_exact(url):
    hits = [t for t in targets() if t.get("url", "") == url]
    if len(hits) != 1:
        sys.exit("expected exactly one target with url %r, got %d" % (url, len(hits)))
    return hits[0]


class Session:
    """One persistent websocket per browser. Persistent because item 3 measures
    milliseconds and a fresh connect per sample would dominate the number."""

    def __init__(self, target):
        self.url = target["url"]
        self.ws = websocket.create_connection(target["webSocketDebuggerUrl"], timeout=10)
        self.n = 0

    def call(self, method, params=None):
        self.n += 1
        mid = self.n
        self.ws.send(json.dumps({"id": mid, "method": method, "params": params or {}}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == mid:
                if "error" in msg:
                    raise RuntimeError("%s -> %s" % (method, msg["error"]))
                return msg.get("result", {})

    def ev(self, js):
        r = self.call("Runtime.evaluate", {"expression": js, "returnByValue": True})
        # ⛔ A swallowed JS exception here would make the probe report "no
        # suggestions" whether or not any rendered -- an instrument that cannot
        # fail (working rule 7, trip-wire 4). Surface it instead.
        if "exceptionDetails" in r:
            raise RuntimeError("JS threw in %s: %s" % (self.url, json.dumps(r["exceptionDetails"])[:400]))
        return r.get("result", {}).get("value")

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass


VK = {"Enter": 13, "Escape": 27, "Tab": 9, "ArrowRight": 39, "End": 35,
      "ArrowDown": 40, "ArrowUp": 38, "Backspace": 8}


def type_char(sess, ch):
    """A real character keystroke: keyDown carrying text, then keyUp.
    This is what fires React's onChange on the address <input>."""
    # ⛔ vk MUST be 0 for punctuation. ord('.') is 46 = VK_DELETE, and Blink
    # then performs a DELETE editing command instead of inserting the character --
    # the probe silently typed a different string than it printed.
    vk = ord(ch.upper()) if ch.isalnum() else 0
    code = "Key" + ch.upper() if ch.isalpha() else ""
    common = {"key": ch, "code": code, "windowsVirtualKeyCode": vk,
              "nativeVirtualKeyCode": vk}
    sess.call("Input.dispatchKeyEvent", dict(type="keyDown", text=ch,
                                             unmodifiedText=ch, **common))
    sess.call("Input.dispatchKeyEvent", dict(type="keyUp", **common))


def press(sess, name):
    vk = VK[name]
    common = {"key": name, "code": name, "windowsVirtualKeyCode": vk,
              "nativeVirtualKeyCode": vk}
    sess.call("Input.dispatchKeyEvent", dict(type="rawKeyDown", **common))
    sess.call("Input.dispatchKeyEvent", dict(type="keyUp", **common))


ADDR_SEL = "document.querySelector('input[placeholder=\"Search or enter address\"]')"


# ------------------------------------------------------------------- sampling
import threading


class VisSampler(threading.Thread):
    """HWND-layer sampler. Records only CHANGES, with ms offsets from t0."""

    def __init__(self, hwnd, seconds, interval=0.02):
        super().__init__(daemon=True)
        self.hwnd, self.seconds, self.interval = hwnd, seconds, interval
        self.events = []
        self.t0 = None

    def run(self):
        self.t0 = time.time()
        last = None
        while time.time() - self.t0 < self.seconds:
            v = is_visible(self.hwnd)
            if v != last:
                self.events.append((round((time.time() - self.t0) * 1000), v))
                last = v
            time.sleep(self.interval)

    def fmt(self):
        return " ".join("%dms:%s" % (t, "VIS" if v else "hid") for t, v in self.events)


def settle(hdr, hwnd):
    """Return the surface to a known state: overlay hidden, address bar not focused."""
    hdr.ev("%s.blur()" % ADDR_SEL)
    hdr.ev("window.cefMessage.send('omnibox_hide', [])")
    time.sleep(0.6)
    return is_visible(hwnd)


# ---------------------------------------------------------------- experiments
def item2(gap_ms, text="127.0.0.1:5137/newtab", watch_s=4.0):
    """Item 2 — does the omnibox stay VISIBLE after Enter?

    The variable under test is ONLY the gap between the last keystroke and Enter.
    MainBrowserView's onChange schedules a 150 ms debounce that sends
    omnibox_update_query + omnibox_show; nothing cancels it on Enter/Escape/blur.
    gap < 150  -> the timer fires AFTER the hide.
    gap > 150  -> the timer has already fired and been superseded by the hide.
    """
    pid = dev_browser_pid()
    hwnd = find_hwnd(pid, OMNIBOX_CLASS)
    hdr = Session(pick_exact(HEADER_URL))
    try:
        if hwnd is None:
            hdr.ev("%s.focus()" % ADDR_SEL)   # onFocus sends omnibox_create
            time.sleep(1.2)
            hwnd = find_hwnd(pid, OMNIBOX_CLASS)
            if hwnd is None:
                sys.exit("no omnibox HWND after omnibox_create — cannot measure")
        print("SUBJECT: dev pid %d · omnibox HWND 0x%X · header target %s"
              % (pid, hwnd, hdr.url))
        print("pre-state (HWND layer): visible=%s" % settle(hdr, hwnd))

        hdr.ev("%s.focus()" % ADDR_SEL)       # onFocus selects all -> typing replaces
        time.sleep(0.25)
        for ch in text[:-1]:
            type_char(hdr, ch)
        time.sleep(1.2)                       # let the debounce fire and the overlay show
        before = is_visible(hwnd)
        print("after typing %r (HWND layer): visible=%s" % (text[:-1], before))

        s = VisSampler(hwnd, watch_s)
        s.start()
        type_char(hdr, text[-1])              # schedules a fresh 150 ms debounce
        time.sleep(gap_ms / 1000.0)
        press(hdr, "Enter")                   # sends navigate + omnibox_hide
        t_enter = round((time.time() - s.t0) * 1000)
        s.join()

        end = is_visible(hwnd)
        print("gap=%dms  enter at %dms  transitions: %s" % (gap_ms, t_enter, s.fmt() or "(none)"))
        print("RESULT (HWND layer): omnibox visible %.1fs after Enter = %s"
              % (watch_s - t_enter / 1000.0, end))
        return end
    finally:
        hdr.close()


def item2click(prefix, gap_ms, watch_s=4.0):
    """Item 2, the path the owner actually described: CLICK a suggestion.

    Same variable as item2 — the gap between the last keystroke and the click.
    The header's debounce is scheduled by typing and nothing cancels it, and the
    header never learns that the overlay's own click already hid the overlay."""
    pid = dev_browser_pid()
    hwnd = find_hwnd(pid, OMNIBOX_CLASS)
    hdr = Session(pick_exact(HEADER_URL))
    omni = Session(pick_exact(OMNIBOX_URL))
    try:
        print("SUBJECT: dev pid %d - omnibox HWND 0x%X" % (pid, hwnd))
        settle(hdr, hwnd)
        hdr.ev("%s.focus()" % ADDR_SEL)
        time.sleep(0.25)
        for ch in prefix[:-1]:
            type_char(hdr, ch)
        time.sleep(2.5)
        rows = json.loads(omni.ev(
            "JSON.stringify(Array.from(document.querySelectorAll('.MuiListItemButton-root'))"
            ".map(function(e){var r=e.getBoundingClientRect();"
            "return {t:e.innerText.split(String.fromCharCode(10)).join(' | '),"
            "x:r.left+r.width/2,y:r.top+r.height/2};}))") or "[]")
        if not rows:
            sys.exit("no suggestions for %r" % prefix)
        row = rows[0]
        print("clicking suggestion[0]: %s" % row["t"][:70])
        s = VisSampler(hwnd, watch_s)
        s.start()
        type_char(hdr, prefix[-1])          # schedules a fresh 150 ms debounce
        time.sleep(gap_ms / 1000.0)
        for typ in ("mousePressed", "mouseReleased"):
            omni.call("Input.dispatchMouseEvent", {"type": typ, "x": row["x"],
                                                   "y": row["y"], "button": "left",
                                                   "clickCount": 1})
        t_click = round((time.time() - s.t0) * 1000)
        s.join()
        print("gap=%dms  click at %dms  transitions: %s" % (gap_ms, t_click, s.fmt() or "(none)"))
        print("RESULT (HWND layer): omnibox visible at end = %s" % is_visible(hwnd))
        return is_visible(hwnd)
    finally:
        hdr.close(); omni.close()


def item3(prefix, watch_s=35.0):
    """Item 3 — how long after clicking a suggestion does the URL appear in the
    address bar?

    t0  = the CDP mouse-press on the suggestion row in the OMNIBOX browser.
          (Fires the same React onClick a real click fires. Native click routing
          is not reproduced and is not what is being timed.)
    tA  = the first sample at which the HEADER browser's <input>.value contains
          the clicked URL's host.       <- DOM layer, the user's complaint
    tB  = the first sample at which the TAB has actually navigated (its CDP
          target url changes).          <- for contrast: "the page went, the bar
                                            did not"
    """
    pid = dev_browser_pid()
    hwnd = find_hwnd(pid, OMNIBOX_CLASS)
    hdr = Session(pick_exact(HEADER_URL))
    omni = Session(pick_exact(OMNIBOX_URL))
    try:
        print("SUBJECT: dev pid %d · header %s · omnibox %s" % (pid, hdr.url, omni.url))
        if hwnd is None:
            hdr.ev("%s.focus()" % ADDR_SEL)
            time.sleep(1.2)
            hwnd = find_hwnd(pid, OMNIBOX_CLASS)
        settle(hdr, hwnd)

        # ⭐ Park the tab on the new-tab page first. Clicking a suggestion for the
        # site the tab is ALREADY on is a vacuous test -- the address bar would read
        # correctly without any of this working. On the NTP `toDisplayUrl` renders an
        # empty address bar, so "the clicked url appeared" cannot be satisfied by
        # anything that was already there.
        hdr.ev("window.cefMessage.send('navigate', ['http://127.0.0.1:5137/newtab'])")
        time.sleep(3.0)
        print("parked: address bar = %r" % hdr.ev("%s.value" % ADDR_SEL))

        before_tabs = {t["id"]: t["url"] for t in targets() if "5137/omnibox" not in t["url"]}

        hdr.ev("%s.focus()" % ADDR_SEL)
        time.sleep(0.25)
        for ch in prefix:
            type_char(hdr, ch)
        time.sleep(2.5)      # debounce + suggestion fetch + render

        rows = omni.ev(
            "JSON.stringify(Array.from(document.querySelectorAll('.MuiListItemButton-root'))"
            ".map(function(e){var r=e.getBoundingClientRect();"
            "return {t:e.innerText.split(String.fromCharCode(10)).join(' | '),x:r.left+r.width/2,y:r.top+r.height/2};}))")
        rows = json.loads(rows or "[]")
        if not rows:
            print("NO SUGGESTIONS for prefix %r — nothing to click. "
                  "(overlay HWND visible=%s)" % (prefix, is_visible(hwnd)))
            return None
        for i, r in enumerate(rows):
            print("  suggestion[%d] %s" % (i, r["t"][:90]))
        pickrow = rows[0]

        addr_before = hdr.ev("%s.value" % ADDR_SEL)
        # ⛔ tA must assert the CLICKED url appears, not merely that the value
        # CHANGED. 📏 Measured 2026-09-18 with the feature disabled: the bar changed
        # at 99 ms -- to the PREVIOUS page's url, because clicking the overlay blurs
        # the header input for ~18 ms and the tab-sync effect gets exactly one run,
        # with the pre-navigation url. A "did it change" probe scores that GREEN.
        want = pickrow["t"].split("|")[-1].strip().split("/")[0]
        print("address bar BEFORE click (DOM layer): %r   (expecting %r to appear)"
              % (addr_before, want))

        t0 = time.time()
        for typ in ("mousePressed", "mouseReleased"):
            omni.call("Input.dispatchMouseEvent", {
                "type": typ, "x": pickrow["x"], "y": pickrow["y"],
                "button": "left", "clickCount": 1})

        tA = tB = None
        newurl = None
        last_val = addr_before
        while time.time() - t0 < watch_s and (tA is None or tB is None):
            if tA is None:
                v = hdr.ev("%s.value" % ADDR_SEL)
                if v != last_val:
                    print("  %6dms  DOM  address bar -> %r"
                          % (round((time.time() - t0) * 1000), v))
                    last_val = v
                if v and want and want in v:
                    tA = round((time.time() - t0) * 1000)
                    newurl = v
            if tB is None:
                for t in targets():
                    if t["id"] in before_tabs and t["url"] != before_tabs[t["id"]] \
                            and "5137/" not in t["url"]:
                        tB = round((time.time() - t0) * 1000)
                        print("  %6dms  TAB  navigated -> %s" % (tB, t["url"][:80]))
                        break
            time.sleep(0.05)

        print("")
        print("MEASURED (t0 = suggestion click):")
        print("  tB  page navigated              : %s" % ("%d ms" % tB if tB is not None else "not observed"))
        print("  tA  CLICKED url in address bar  : %s" % ("%d ms" % tA if tA is not None else
                                                          "NOT WITHIN %.0f s" % watch_s))
        print("  address bar final value (DOM)   : %r" % hdr.ev("%s.value" % ADDR_SEL))
        print("  header input still focused?     : %s"
              % hdr.ev("document.activeElement === %s" % ADDR_SEL))
        print("  omnibox HWND visible?           : %s" % is_visible(hwnd))
        return tA, tB
    finally:
        hdr.close()
        omni.close()


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: omniboxprobe.py item2 <gap_ms> | item3 <prefix> | state")
    cmd = sys.argv[1]
    if cmd == "item2":
        item2(int(sys.argv[2]))
    elif cmd == "item2click":
        item2click(sys.argv[2], int(sys.argv[3]))
    elif cmd == "item3":
        item3(sys.argv[2])
    elif cmd == "state":
        pid = dev_browser_pid()
        h = find_hwnd(pid, OMNIBOX_CLASS)
        print("dev pid %d · omnibox hwnd %s · visible %s"
              % (pid, ("0x%X" % h) if h else None, is_visible(h) if h else None))
        for t in targets():
            print("  %-6s %s" % (t["type"], t["url"]))
    else:
        sys.exit("unknown command %r" % cmd)


if __name__ == "__main__":
    main()
