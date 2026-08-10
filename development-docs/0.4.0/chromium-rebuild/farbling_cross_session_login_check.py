#!/usr/bin/env python3
r"""farbling_cross_session_login_check.py — the §11 "cross-session login" acceptance row.

This is the row the persistent per-profile seed exists for. A per-SESSION seed would give
better unlinkability and would also silently break every site that binds a session to a
device fingerprint: the user logs in, quits, reopens, and is logged out. So the contract
is: **restart the browser, revisit a farbled site, and the login still works** — while the
farbled fingerprint for that origin comes back byte-identical.

## ⛔ The vacuous-pass trap this script refuses to walk into

The obvious sites to test are the ones you are already logged into. On this machine those
were **x.com and github.com** — and both are in `FingerprintProtection::IsAuthDomain`'s
allowlist, i.e. **not farbled at all**. Running the test there would have produced a
confident green that had nothing to do with farbling, because nothing was being farbled.
Same family as the three harnesses that died in this sprint.

So the exempt allowlist is parsed straight out of `FingerprintProtection.h` at runtime
(never copied here, so it cannot drift) and a target on it is a hard refusal, not a
warning.

## Controls

* **positive control on the login detector** — `--control-url` must be a URL that is
  known to be logged OUT (default: a SoundCloud page that redirects to /signin). If the
  detector cannot report "logged out" for a page that is logged out, its "logged in" on
  the real target means nothing.
* **negative control (`--negative-control`)** — rotates the profile seed between the two
  phases, i.e. simulates the per-session-seed world the persistent seed replaced, and
  asserts this harness goes RED on the "fingerprint survived the restart" half. The seed
  is restored afterwards.

## What this needs from a human, once

A login on a **farbled** (non-exempt) site in the profile under test. This script does not
create accounts. If the target reads as logged out it exits non-zero telling you so — it
never downgrades that to a pass.

## Usage

    # one-time: log in to some non-exempt site in the dev profile by hand, then:
    python farbling_cross_session_login_check.py \
        --exe "C:\...\build\bin\Release\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" --profile Default --dev \
        --url "https://www.reddit.com/settings" --host reddit.com

    python farbling_cross_session_login_check.py ... --negative-control
"""

import argparse
import json
import os
import re
import secrets
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import websocket  # noqa: E402  (websocket-client)

from farbling_seed_rotation_check import (  # noqa: E402
    kill_browser_by_path,
    launch_browser,
    measure,
    read_settings,
    resolve_tab,
    set_profile_seed,
    snapshot_targets,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for  # noqa: E402

HEADER = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.dirname(os.path.abspath(__file__))))),
    "cef-native", "include", "core", "FingerprintProtection.h")

LOGGED_OUT_JS = r"""
(() => {
  const text = (document.body ? document.body.innerText : '').slice(0, 6000);
  return JSON.stringify({
    href: location.href,
    title: (document.title || '').slice(0, 90),
    // Deliberately crude and deliberately explained: a login wall is a page that either
    // redirected somewhere with 'login'/'signin' in the URL, or is asking for credentials.
    // Anything subtler would be site-specific, and site-specific detectors rot silently.
    urlWall: /\/(login|signin|sign_in|sign-in|auth)\b/i.test(location.pathname + location.search),
    textWall: /(sign in|sign-in|log in|log-in|create account|continue with)/i.test(text)
  });
})()
"""


def exempt_hosts():
    """Parse the auth allowlist out of the C++ header, so this can never drift from it."""
    try:
        with open(HEADER, "r", encoding="utf-8", errors="replace") as fh:
            src = fh.read()
    except OSError:
        return None
    block = re.search(r"authDomains\[\]\s*=\s*\{(.*?)\};", src, re.S)
    if not block:
        return None
    return {m.lower() for m in re.findall(r'"([^"]+)"', block.group(1))}


def is_exempt(host, allow):
    """`IsAuthDomain` matches the FULL committed host exactly (OQ3 — deliberately not
    collapsed to eTLD+1), so mirror that, but also check the www./bare siblings, because a
    target that redirects to its sibling would land on an exempt host mid-test."""
    h = host.lower()
    cands = {h, "www." + h, h[4:] if h.startswith("www.") else h}
    return sorted(c for c in cands if c in allow)


def probe_login(port, excluded, url, host, timeout=90):
    t = resolve_tab(port, excluded)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": url}}))
    finally:
        ws.close()
    deadline = time.time() + timeout
    seen = None
    while time.time() < deadline:
        time.sleep(3.0)
        t = resolve_tab(port, excluded)
        seen = t.get("url", "")
        if host in seen:
            break
    time.sleep(6.0)
    t = resolve_tab(port, excluded)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                            "params": {"expression": LOGGED_OUT_JS,
                                       "returnByValue": True, "awaitPromise": True}}))
        while True:
            m = json.loads(ws.recv())
            if m.get("id") == 2:
                break
    finally:
        ws.close()
    v = json.loads(m["result"]["result"]["value"])
    v["loggedOut"] = bool(v["urlWall"] or v["textWall"])
    return v


def boot(args, port):
    for attempt in range(1, 4):
        kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            return snapshot_targets(port, settle=args.settle)
        print("    launch attempt %d did not bring up CDP %d; retrying" % (attempt, port))
    raise SystemExit("CDP %d never came up after 3 launch attempts" % port)


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--url", required=True,
                    help="a URL that is only reachable when logged in")
    ap.add_argument("--host", required=True)
    ap.add_argument("--control-url", default="https://soundcloud.com/you/likes",
                    help="a URL known to be LOGGED OUT — the detector's positive control")
    ap.add_argument("--control-host", default="soundcloud.com")
    ap.add_argument("--negative-control", action="store_true")
    args = ap.parse_args()

    pdir = os.path.join(args.data_root, args.profile)
    port = cdp_port_for(args.profile, args.dev)

    # ---- refuse the vacuous pass BEFORE launching anything ---------------------------
    allow = exempt_hosts()
    if allow is None:
        return fail("could not parse the auth allowlist from %s — without it this script "
                    "cannot tell a farbled target from an exempt one, and an exempt "
                    "target would pass this test while farbling nothing." % HEADER)
    hit = is_exempt(args.host, allow)
    if hit:
        return fail(
            "%s is on FingerprintProtection::IsAuthDomain's allowlist (%s), so it is "
            "NOT farbled. A login there survives a restart whether or not the seed is "
            "persistent, so this test would pass while measuring nothing. Pick a "
            "non-exempt site." % (args.host, ", ".join(hit)))
    print("[guard] %s is not in the %d-entry auth allowlist -> it is farbled. Good."
          % (args.host, len(allow)))

    original_seed = read_settings(pdir).get("profileSeed")
    if not original_seed:
        return fail("no profileSeed in %s" % pdir)

    try:
        # ---- phase 1 -----------------------------------------------------------------
        print("\n--- phase 1: before restart -------------------------------------")
        excluded = boot(args, port)

        ctrl = probe_login(port, excluded, args.control_url, args.control_host)
        print("    login-detector control (%s): loggedOut=%s  %s"
              % (args.control_host, ctrl["loggedOut"], ctrl["href"][:70]))
        if not ctrl["loggedOut"]:
            return fail(
                "the login detector's positive control did not read as logged out on %s. "
                "Either that account is now logged in (pick another control URL) or the "
                "detector cannot detect a login wall — in which case its verdict on %s "
                "would be worthless." % (args.control_host, args.host))
        print("    positive control OK: the detector can report 'logged out'.")

        before = probe_login(port, excluded, args.url, args.host)
        print("    target (%s): loggedOut=%s  %s"
              % (args.host, before["loggedOut"], before["href"][:70]))
        if before["loggedOut"]:
            return fail(
                "not logged in to %s, so there is no session to carry across a restart. "
                "This is NOT a pass and is NOT a farbling failure — log in by hand in "
                "profile %r and re-run. (Do not substitute an auth-exempt site; the "
                "guard above will refuse it.)" % (args.host, args.profile))

        fp_before = measure(port, excluded, args.url, args.host)
        print("    fingerprint: canvas=%s webgl=%s audio=%s"
              % (fp_before["small"], fp_before["glSmall"], fp_before["audio"]))

        # ---- restart -----------------------------------------------------------------
        if args.negative_control:
            kill_browser_by_path(args.exe)
            rotated = secrets.token_hex(32)
            set_profile_seed(pdir, rotated)
            print("\n[negative control] profile seed rotated to %s... — this is the "
                  "per-session-seed world the persistent seed replaced" % rotated[:16])

        print("\n--- phase 2: after restart --------------------------------------")
        excluded = boot(args, port)
        after = probe_login(port, excluded, args.url, args.host)
        print("    target (%s): loggedOut=%s  %s"
              % (args.host, after["loggedOut"], after["href"][:70]))
        fp_after = measure(port, excluded, args.url, args.host)
        print("    fingerprint: canvas=%s webgl=%s audio=%s"
              % (fp_after["small"], fp_after["glSmall"], fp_after["audio"]))
    finally:
        if args.negative_control:
            kill_browser_by_path(args.exe)
            set_profile_seed(pdir, original_seed)
            print("[negative control] restored the original profile seed")

    # ---- verdict ---------------------------------------------------------------------
    print("\n== verdict ==")
    failures = []

    def check(label, ok, detail):
        print("  %s %-46s %s" % ("PASS" if ok else "FAIL", label, detail))
        if not ok:
            failures.append(label)

    stable = all(fp_before[k] == fp_after[k] for k in ("small", "glSmall", "audio"))
    if args.negative_control:
        check("fingerprint CHANGED across restart (control)", not stable,
              "canvas %s -> %s" % (fp_before["small"], fp_after["small"]))
        print("  ---- login state after a seed rotation, recorded not asserted: %s ----"
              % ("still logged in" if not after["loggedOut"] else "LOGGED OUT"))
        print()
        if failures:
            print("RESULT: NEGATIVE CONTROL FAILED — rotating the profile seed did not "
                  "change the fingerprint, so this harness is not measuring the seed and "
                  "its green run proves nothing.")
            return 1
        print("RESULT: negative control OK — with the seed rotated the fingerprint does "
              "move across a restart, i.e. this harness does go RED when the seed stops "
              "being persistent.")
        return 0

    check("still logged in after restart", not after["loggedOut"], after["href"][:70])
    check("fingerprint identical across restart", stable,
          "canvas %s, webgl %s, audio %s"
          % (fp_after["small"], fp_after["glSmall"], fp_after["audio"]))
    print()
    if failures:
        print("RESULT: FAIL — %s" % ", ".join(failures))
        return 1
    print("RESULT: PASS — the session survived a real browser restart on a farbled "
          "origin, and the farbled fingerprint came back byte-identical.")
    return 0


def fail(msg):
    print("RESULT: FAIL -- %s" % msg)
    return 1


if __name__ == "__main__":
    sys.exit(main())
