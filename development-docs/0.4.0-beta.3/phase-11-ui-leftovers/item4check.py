#!/usr/bin/env python3
"""
beta.3 Phase 11 item 4 (`P11-I4`) — keyboard conformance check.

SUBJECT, stated per row rather than assumed:
  the HEADER browser's <input> — its .value and whether it is document.activeElement
  the OMNIBOX overlay HWND — IsWindowVisible(), the Win32 layer, because "the
  dropdown is still on screen" is an HWND fact and not a DOM one.

⛔ What this does NOT prove: native keyboard delivery. CDP key events reach the
renderer directly; they do not travel OS -> HWND -> CEF. Every row here is about
what the address bar CONTAINS and where DOM focus IS after each key, which is
exactly what the harness asked for ("not that no error occurred").
"""
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import omniboxprobe as P  # noqa: E402

hdr = P.Session(P.pick_exact(P.HEADER_URL))
omni = P.Session(P.pick_exact(P.OMNIBOX_URL))
pid = P.dev_browser_pid()
hwnd = P.find_hwnd(pid, P.OMNIBOX_CLASS)
print("SUBJECT: dev pid %d  omnibox HWND 0x%X  header %s" % (pid, hwnd, hdr.url))

FAILED = []


def state():
    return (hdr.ev("%s.value" % P.ADDR_SEL),
            hdr.ev("document.activeElement === %s" % P.ADDR_SEL),
            hdr.ev("document.activeElement.tagName"),
            P.is_visible(hwnd))


def show(tag):
    v, f, el, vis = state()
    print("    %-26s value=%-30r inBar=%-5s activeEl=%-7s dropdown=%s"
          % (tag, v, f, el, "UP" if vis else "down"))
    return v, f, el, vis


def check(label, cond, detail=""):
    print("    %s %s %s" % ("PASS" if cond else "FAIL", label, detail))
    if not cond:
        FAILED.append(label)


def fresh(prefix, settle_s=2.5):
    P.settle(hdr, hwnd)
    hdr.ev("%s.focus()" % P.ADDR_SEL)
    time.sleep(0.25)
    for ch in prefix:
        P.type_char(hdr, ch)
    time.sleep(settle_s)


def park():
    """Put the tab on a page whose URL is nothing like the suggestions, so
    'restored the page URL' and 'restored the typed text' cannot be confused."""
    P.settle(hdr, hwnd)
    hdr.ev("window.cefMessage.send('navigate', ['https://teragun.com/'])")
    time.sleep(5)


park()
page_url = hdr.ev("%s.value" % P.ADDR_SEL)
print("parked on %r\n" % page_url)

print("R1 — Tab traverses the suggestion list (Chrome/Firefox/Vivaldi all agree)")
fresh("exam")
show("typed 'exam'")
P.press(hdr, "Tab"); time.sleep(0.5)
v1, f1, _, vis1 = show("Tab x1")
P.press(hdr, "Tab"); time.sleep(0.5)
v2, f2, _, vis2 = show("Tab x2")
check("R1a Tab keeps focus in the address bar", f1 and f2)
check("R1b Tab fills the bar with a suggestion", v1 != "exam" and bool(v1))
check("R1c a second Tab moves to the NEXT suggestion", v2 != v1, "%r -> %r" % (v1, v2))
check("R1d the dropdown stays up while traversing", vis1 and vis2)

print("\nR2 — Shift+Tab traverses back up")
P.press(hdr, "Tab", shift=True); time.sleep(0.5)
v3, f3, _, _ = show("Shift+Tab")
check("R2a Shift+Tab steps back", v3 == v1, "%r == %r" % (v3, v1))
check("R2b focus still in the address bar", f3)

print("\nR3 — Tab with NO dropdown must keep its normal job (move focus out)")
P.settle(hdr, hwnd)
hdr.ev("%s.focus()" % P.ADDR_SEL)
time.sleep(0.4)
show("focused, nothing typed")
P.press(hdr, "Tab"); time.sleep(0.5)
_, f4, el4, _ = show("Tab")
check("R3 Tab leaves the address bar when there is no dropdown",
      (not f4) and el4 != "INPUT", "activeEl=%s" % el4)

print("\nR4 — Escape rung 1: close the dropdown, give back what the USER typed, stay put")
fresh("exam")
P.press(hdr, "Tab"); time.sleep(0.5)
show("Tab (bar now holds a suggestion)")
P.press(hdr, "Escape"); time.sleep(0.6)
v5, f5, _, vis5 = show("Escape x1")
check("R4a dropdown closed", not vis5)
check("R4b the user's typed text is back", v5 == "exam", "value=%r" % v5)
check("R4c focus stays in the address bar", f5)
check("R4d it did NOT jump straight to the page URL", v5 != page_url)

print("\nR5 — Escape rung 2: restore the page URL and leave the bar")
P.press(hdr, "Escape"); time.sleep(0.8)
v6, f6, _, vis6 = show("Escape x2")
check("R5a page URL restored", v6 == page_url, "value=%r want %r" % (v6, page_url))
check("R5b focus left the address bar", not f6)
check("R5c dropdown still down", not vis6)

print("\nR6 — REGRESSION: blur now hides the dropdown; does clicking a suggestion still work?")
park()
fresh("exam")
import json  # noqa: E402
rows = json.loads(omni.ev(
    "JSON.stringify(Array.from(document.querySelectorAll('.MuiListItemButton-root'))"
    ".map(function(e){var r=e.getBoundingClientRect();"
    "return {t:e.innerText.split(String.fromCharCode(10)).join(' | '),"
    "x:r.left+r.width/2,y:r.top+r.height/2};}))") or "[]")
print("    clicking: %s" % rows[0]["t"][:60])
t0 = time.time()
for typ in ("mousePressed", "mouseReleased"):
    omni.call("Input.dispatchMouseEvent", {"type": typ, "x": rows[0]["x"],
                                           "y": rows[0]["y"], "button": "left",
                                           "clickCount": 1})
got = None
while time.time() - t0 < 20:
    v = hdr.ev("%s.value" % P.ADDR_SEL)
    if v and "example.com" in v:
        got = round((time.time() - t0) * 1000)
        break
    time.sleep(0.05)
navigated = any("example.com" in t["url"] for t in P.targets())
show("after the click")
check("R6a the click still navigates", navigated)
check("R6b item 3 still green (clicked URL in the bar)", got is not None,
      "%s ms" % got if got else "NOT within 20 s")

print("\nR7 — REGRESSION: Enter still navigates")
park()
P.settle(hdr, hwnd)
hdr.ev("%s.focus()" % P.ADDR_SEL)
time.sleep(0.3)
for ch in "example.org":
    P.type_char(hdr, ch)
time.sleep(1.0)
P.press(hdr, "Enter")
ok = False
t0 = time.time()
while time.time() - t0 < 20:
    v = hdr.ev("%s.value" % P.ADDR_SEL)
    if v and "example.org" in v and v != "example.org":
        ok = round((time.time() - t0) * 1000)
        break
    time.sleep(0.05)
show("after Enter")
check("R7 Enter navigates and the bar shows the URL", bool(ok), "%s ms" % ok)

print("\n%s  (%d failure(s))" % ("ALL ROWS PASS" if not FAILED else "FAILED: " + ", ".join(FAILED),
                                 len(FAILED)))
hdr.close()
omni.close()
sys.exit(1 if FAILED else 0)
