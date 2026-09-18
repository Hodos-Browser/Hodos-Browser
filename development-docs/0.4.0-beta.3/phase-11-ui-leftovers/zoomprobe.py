#!/usr/bin/env python3
"""
beta.3 Phase 11 item 7, route 1 (`P11-I7a`) — Ctrl+wheel must zoom the PAGE and
not the browser chrome.

⛔ SUBJECT / LAYER. `window.devicePixelRatio`, read in each browser's own document.
It is `deviceScaleFactor × pageZoom`, so on a machine whose monitor scale is not
changing it moves if and only if that browser's page zoom moved. Not a log line,
not a stored preference — the thing that actually determines how big the UI draws.

⭐ THE POSITIVE CONTROL IS FREE AND IT IS THE PRODUCT ITSELF. The same CDP wheel
event is sent to a TAB, which SHOULD zoom, and to the HEADER, which should not.
One run, both directions, no control build:
    tab    zooms  -> the instrument can reach the zoom path at all
    header doesn't -> the guard is what stopped it
⛔ If the tab does NOT zoom, the run proves nothing about the header and says so,
because then the wheel never reached the zoom path in the first place.

⚠️ WHAT THIS DOES NOT PROVE. CDP injects into the browser-process input pipeline;
it is not a physical wheel. If it turns out CDP cannot reach the zoom path at all,
this row is a human one (HUMAN_TEST_QUEUE W10) and the script says so rather than
reporting a green.
"""
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import omniboxprobe as P  # noqa: E402


def dpr(sess):
    return sess.ev("window.devicePixelRatio")


def ctrl_wheel(sess, clicks):
    """Ctrl + wheel, the gesture under test. modifiers=2 is Ctrl in CDP."""
    for _ in range(abs(clicks)):
        sess.call("Input.dispatchMouseEvent", {
            "type": "mouseWheel", "x": 200, "y": 40,
            "deltaX": 0, "deltaY": -120 if clicks > 0 else 120,
            "modifiers": 2,
        })
        time.sleep(0.25)


def measure(sess, label, clicks=3):
    before = dpr(sess)
    ctrl_wheel(sess, clicks)
    time.sleep(1.2)
    after = dpr(sess)
    moved = (before != after)
    print("  %-26s devicePixelRatio %s -> %s   %s"
          % (label, before, after, "ZOOMED" if moved else "unchanged"))
    return before, after, moved


def reset(sess):
    # Ctrl+0 equivalent: wheel back the other way until it settles, then report.
    ctrl_wheel(sess, -6)
    time.sleep(0.8)


def guard_fires(sess):
    """The half that IS provable from here: does our listener run and CANCEL a
    ctrl+wheel?

    ⛔ This is a DOM-layer fact, and it is NOT the same claim as "the chrome did
    not zoom". A synthetic event is untrusted and would never have zoomed anyway.
    What it establishes is that the guard is installed, reached, and cancels --
    which is the only half that lives in our code. Chromium's side (a consumed
    wheel never reaches HandleWheelEvent) is established by source reading, and
    the on-screen result is HUMAN_TEST_QUEUE W10.

    ⭐ It also catches the specific trap that would make this silently useless:
    a PASSIVE listener's preventDefault() is discarded. If someone converts this
    to a JSX onWheel, `ctrl` below flips to False and this row goes red.
    """
    return sess.ev(
        "(function(){"
        " var ev=new WheelEvent('wheel',{deltaY:-120,ctrlKey:true,cancelable:true,bubbles:true});"
        " window.dispatchEvent(ev);"
        " var plain=new WheelEvent('wheel',{deltaY:-120,ctrlKey:false,cancelable:true,bubbles:true});"
        " window.dispatchEvent(plain);"
        " return JSON.stringify({ctrl:ev.defaultPrevented, plain:plain.defaultPrevented});"
        "})()")


def origin_sweep():
    """⭐ The row the owner's own testing forced into existence.

    📏 Owner, 2026-09-18: zoom behaves correctly on a real site and wrongly on
    localhost. Chromium stores page zoom per-ORIGIN, and the header, all ~14
    overlays AND the internal pages a user opens as TABS (/newtab, /settings-page,
    /browser-data, /wallet-panel) are all served from 127.0.0.1:5137 -- so
    Ctrl+scrolling the NEW TAB PAGE zoomed the entire browser chrome with it.
    Guarding only the header could never have caught that: the user is not over
    the header when it happens.

    The assertion is a SPLIT, not a single value, because either half alone is
    satisfiable by a bug: a guard that blocks nothing passes "web content still
    zooms", and a guard that blocks everything passes "our UI does not".
    """
    print("ORIGIN SWEEP - ctrl+wheel must be cancelled on every page WE serve,")
    print("               and on NO web content:")
    ok, ours_n, web_n = True, 0, 0
    for t in P.targets():
        if not t["url"].startswith("http"):
            continue
        ours = "127.0.0.1:5137" in t["url"]
        try:
            s = P.Session(t)
            c = s.ev("(function(){var e=new WheelEvent('wheel',{deltaY:-120,"
                     "ctrlKey:true,cancelable:true,bubbles:true});"
                     "window.dispatchEvent(e);return e.defaultPrevented})()")
            s.close()
        except Exception:
            print("   %-46s (unreachable)" % t["url"][:46])
            continue
        good = (c == ours)
        ok = ok and good
        ours_n += 1 if ours else 0
        web_n += 0 if ours else 1
        print("   %-46s ours=%-5s cancelled=%-5s %s"
              % (t["url"][:46], ours, c, "ok" if good else "<<< WRONG"))
    if web_n == 0:
        print("   ⛔ NO web content was open -- the sweep cannot show the guard is")
        print("      scoped rather than global. Open a real site and re-run.")
        return False
    print("   -> %s  (%d of ours, %d web)"
          % ("OK" if ok else "RED", ours_n, web_n))
    return ok


def main():
    hdr = P.Session(P.pick_exact(P.HEADER_URL))
    tab = None
    for t in P.targets():
        if t["url"].endswith("/newtab") or "127.0.0.1:5137" not in t["url"]:
            tab = P.Session(t)
            break
    try:
        if tab is None:
            sys.exit("no tab browser to use as the positive control")
        print("SUBJECT: header=%s   tab=%s" % (hdr.url, tab.url))
        print("")
        print("POSITIVE CONTROL - the tab SHOULD zoom (this is the behaviour we keep):")
        _, _, tab_moved = measure(tab, "tab, ctrl+wheel")
        reset(tab)
        print("")
        print("UNDER TEST - the header must NOT zoom:")
        _, _, hdr_moved = measure(hdr, "header, ctrl+wheel")
        print("")

        sweep_ok = origin_sweep()
        print("")
        import json as _j
        g = _j.loads(guard_fires(hdr))
        t = _j.loads(guard_fires(tab))
        print("GUARD, at the DOM layer (provable from here):")
        print("  header: ctrl+wheel cancelled = %-5s | plain wheel cancelled = %-5s"
              % (g["ctrl"], g["plain"]))
        print("  tab   : ctrl+wheel cancelled = %-5s | plain wheel cancelled = %-5s"
              % (t["ctrl"], t["plain"]))
        guard_ok = g["ctrl"] and not g["plain"] and not t["ctrl"] and sweep_ok
        print("  -> %s" % ("OK: the header cancels ctrl+wheel ONLY, and the tab cancels nothing"
                           if guard_ok else
                           "RED: guard missing, too broad, or leaking into tabs"))
        print("")

        if not tab_moved:
            print("VERDICT: [PARTIAL] %s" % ("the guard is verified at the DOM layer"
                                             if guard_ok else "GUARD FAILED - see above"))
            print("         ⛔ But the TAB did not zoom either, so this CDP wheel never")
            print("         reached Chromium's zoom path -- a synthetic wheel is not a")
            print("         native one. The end-to-end claim ('the toolbar no longer")
            print("         grows') is NOT established here: HUMAN_TEST_QUEUE W10.")
            return 0 if guard_ok else 1
        if hdr_moved:
            print("VERDICT: [RED] the header zoomed. The guard did not hold.")
            return 1
        print("VERDICT: [GREEN] the tab zoomed and the header did not, in the same run,")
        print("         with the same event. That is exactly the split we want.")
        return 0
    finally:
        try:
            reset(hdr)
        except Exception:
            pass
        hdr.close()
        if tab:
            tab.close()


if __name__ == "__main__":
    sys.exit(main())
