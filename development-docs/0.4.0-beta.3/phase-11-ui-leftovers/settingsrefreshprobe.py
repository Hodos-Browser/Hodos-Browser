#!/usr/bin/env python3
"""
beta.3 Phase 11 item 8 (`P11-I8`) — the FOURTH instance of the long-lived-surface
pattern: the header's search engine.

The ticket predicted this: *"Fixing only the avatar leaves the pattern in place and
guarantees a fourth instance."*

⛔ SUBJECT. What the address bar actually DOES with a non-URL query — i.e. the URL
the tab ends up at — not a stored setting and not a log line. A setting that is
written but not acted on is precisely the defect.

⛔ LAYER. The header browser's React state is invisible, so this reads the effect:
type a search term, press Enter, and see which engine the browser navigated to.
"""
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import omniboxprobe as P  # noqa: E402

ENGINES = {"duckduckgo": "duckduckgo.com", "google": "google.com", "bing": "bing.com"}


def current_tab_url():
    for t in P.targets():
        if "127.0.0.1:5137" not in t["url"]:
            return t["url"]
    for t in P.targets():
        if t["url"].endswith("/newtab"):
            return t["url"]
    return None


def search_and_read(hdr, term):
    """Type a non-URL into the address bar, press Enter, return where we landed."""
    hdr.ev("%s.focus()" % P.ADDR_SEL)
    time.sleep(0.3)
    for ch in term:
        P.type_char(hdr, ch)
    time.sleep(0.8)
    P.press(hdr, "Enter")
    t0 = time.time()
    while time.time() - t0 < 20:
        u = current_tab_url()
        if u and term in u:
            return u
        time.sleep(0.2)
    return current_tab_url()


def main():
    hdr = P.Session(P.pick_exact(P.HEADER_URL))
    # The editor must be a DIFFERENT browser from the header, or "it updated"
    # could be satisfied by the editing surface's own local state.
    # ⛔ The editor must be a browser that does NOT navigate during the run. The
    # first version of this probe used the TAB -- which `search_and_read` then
    # navigates to the search results, destroying the document the editor session
    # was attached to, so the `settings_set` after it went nowhere and the probe
    # reported a RED that was entirely its own. Use a keep-alive overlay instead.
    editor = None
    for want in ("/menu", "/privacy-shield", "/downloads", "/bookmarks"):
        for t in P.targets():
            if t["url"].endswith(want):
                editor = P.Session(t)
                break
        if editor:
            break
    try:
        if editor is None:
            sys.exit("no second browser to send settings_set from")
        print("SUBJECT: header=%s (measured)   editor=%s (edit sent from here)"
              % (hdr.url, editor.url))

        u0 = search_and_read(hdr, "hodosprobeaaa")
        eng0 = next((k for k, d in ENGINES.items() if d in (u0 or "")), None)
        print("BEFORE  searched -> %s   (engine: %s)" % ((u0 or "")[:70], eng0))
        if eng0 is None:
            sys.exit("could not identify the engine from %r" % u0)

        target = "google" if eng0 != "google" else "duckduckgo"
        print("EDIT    settings_set browser.searchEngine -> %r, from the OTHER browser"
              % target)
        sent = editor.ev(
            "(function(){if(!window.cefMessage){return 'NO BRIDGE'}"
            "window.cefMessage.send('settings_set', ['browser.searchEngine', %r]);"
            "return 'sent'})()" % target)
        if sent != "sent":
            sys.exit("the editor browser has no cefMessage bridge (%r) - the edit was "
                     "never sent, so a RED here would prove nothing" % sent)
        time.sleep(3)

        u1 = search_and_read(hdr, "hodosprobebbb")
        eng1 = next((k for k, d in ENGINES.items() if d in (u1 or "")), None)
        print("AFTER   searched -> %s   (engine: %s)" % ((u1 or "")[:70], eng1))

        # restore, whatever happened
        editor.ev("window.cefMessage.send('settings_set', ['browser.searchEngine', %r])" % eng0)
        time.sleep(1)
        print("        restored browser.searchEngine -> %r" % eng0)

        if eng1 == target:
            print("RESULT: [GREEN] the header acted on the new engine with no restart.")
            return 0
        print("RESULT: [RED] the header still searched with %r after the setting was "
              "changed to %r. The setting took; the long-lived surface never heard."
              % (eng1, target))
        return 1
    finally:
        hdr.close()
        if editor:
            editor.close()


if __name__ == "__main__":
    sys.exit(main())
