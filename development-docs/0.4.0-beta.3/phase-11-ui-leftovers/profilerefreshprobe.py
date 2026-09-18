#!/usr/bin/env python3
"""
beta.3 Phase 11 item 8 (`P11-I8`) — does a profile edit reach the OTHER surfaces?

👤 Owner, 2026-08-25: *"I changed the avatar on another profile. It did take, but
it didn't refresh in all of the places. I closed it and reopened it and the new
avatar was there."*

⛔ SUBJECT. The surface under test is the **header browser's toolbar profile
button** — the one the owner said did not refresh. It is NOT the panel the edit
was made in: that one looks right either way, because it holds the value it just
wrote locally, which is exactly what made this defect look like a one-off.

⛔ LAYER. DOM, in the header browser: the button's `aria-label` (the profile name)
and the avatar's computed `background-color` (the profile colour). Both are what a
person actually sees. No log line is consulted.

The edit is sent from the **profile panel overlay's** browser, a different process
from the header, so "it refreshed" cannot be satisfied by the editing surface's own
local state.
"""
import re
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import omniboxprobe as P  # noqa: E402

PANEL_URL = "http://127.0.0.1:5137/profile-picker"

BTN = "document.querySelector('[aria-label^=\"Profile:\"]')"
AVATAR = BTN + " && " + BTN + ".querySelector('.MuiAvatar-root')"


def header_state(hdr):
    label = hdr.ev("%s ? %s.getAttribute('aria-label') : null" % (BTN, BTN))
    colour = hdr.ev(
        "(function(){var b=%s; if(!b) return null;"
        " var a=b.querySelector('.MuiAvatar-root'); if(!a) return null;"
        " return getComputedStyle(a).backgroundColor;})()" % BTN)
    return label, colour


def main():
    hdr = P.Session(P.pick_exact(P.HEADER_URL))
    panel = None
    try:
        for t in P.targets():
            if t["url"] == PANEL_URL:
                panel = P.Session(t)
                break
        if panel is None:
            print("profile panel overlay not created yet — opening it")
            hdr.ev("window.cefMessage.send('profile_panel_show', ['0'])")
            time.sleep(3)
            for t in P.targets():
                if t["url"] == PANEL_URL:
                    panel = P.Session(t)
                    break
        if panel is None:
            sys.exit("could not reach the profile panel overlay")

        print("SUBJECT: header=%s (read here)  editor=%s (edit sent from here)"
              % (hdr.url, panel.url))

        # Which profile is current? Ask the EDITOR, so the header is never the
        # source of the value we then check the header against.
        cur = panel.ev(
            "new Promise(function(r){window.onProfilesResult=function(d){r(d.currentProfileId)};"
            "window.cefMessage.send('profiles_get_all', [])})") if False else None
        # simpler and synchronous: the header's own label names the current profile
        label0, colour0 = header_state(hdr)
        print("BEFORE  header label=%r  avatar background=%r" % (label0, colour0))
        if not label0:
            sys.exit("header has no profile button — cannot measure")
        name = label0.replace("Profile: ", "")

        # pick a colour the header is definitely not showing
        target = "#c2185b" if "194, 24, 91" not in (colour0 or "") else "#00695c"
        want = tuple(int(target[i:i + 2], 16) for i in (1, 3, 5))
        print("EDIT    setting profile %r colour -> %s  (from the PANEL browser)"
              % (name, target))

        cur_id = panel.ev(
            "(function(){try{return JSON.parse(localStorage.getItem('hodos_current_profile'))}"
            "catch(e){return null}})()")
        # the id is not reliably in localStorage; fall back to the name, which the
        # C++ side keys on id — so ask the panel's own list.
        ids = panel.ev("JSON.stringify(Array.from(document.querySelectorAll('[data-profile-id]'))"
                       ".map(function(e){return e.getAttribute('data-profile-id')}))")
        print("        panel-visible profile ids: %s  localStorage current: %r" % (ids, cur_id))

        # C++ keys profiles_set_color on the profile ID. The current profile's id is
        # what the header is rendering, and ProfileManager exposes it to every
        # surface in profiles_result — so read it from the PANEL's own hook state by
        # asking C++ directly and letting the panel answer.
        got = {}

        def ask_panel_for_current():
            return panel.ev_async(
                "new Promise(function(res){var prev=window.onProfilesResult;"
                "window.onProfilesResult=function(d){window.onProfilesResult=prev;"
                "res(d.currentProfileId)};window.cefMessage.send('profiles_get_all', []);"
                "setTimeout(function(){res(null)},4000)})")

        pid = ask_panel_for_current()
        print("        current profile id, per the PANEL: %r" % pid)
        if not pid:
            sys.exit("could not resolve the current profile id from the panel")

        t0 = time.time()
        panel.ev("window.cefMessage.send('profiles_set_color', [%r, %r])" % (pid, target))

        seen = None
        while time.time() - t0 < 12:
            _, colour = header_state(hdr)
            if colour and colour != colour0:
                m = re.findall(r"\d+", colour)
                if len(m) >= 3 and tuple(int(x) for x in m[:3]) == want:
                    seen = round((time.time() - t0) * 1000)
                    break
            time.sleep(0.1)

        label1, colour1 = header_state(hdr)
        print("AFTER   header label=%r  avatar background=%r" % (label1, colour1))
        if seen is not None:
            print("RESULT: [GREEN] the header picked up the change in %d ms, "
                  "with no reload and no reopen." % seen)
            return 0
        print("RESULT: [RED] the header still shows the OLD colour 12 s after the edit. "
              "This is the owner's report: the edit took, other surfaces did not refresh.")
        return 1
    finally:
        hdr.close()
        if panel:
            panel.close()


if __name__ == "__main__":
    sys.exit(main())
