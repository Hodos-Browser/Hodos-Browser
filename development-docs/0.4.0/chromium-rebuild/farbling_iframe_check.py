#!/usr/bin/env python3
r"""farbling_iframe_check.py — the §7 "cross-site iframe difference" row, as a DIAGNOSTIC.

The row asks: does a third-party origin embedded under two different first parties get
DIFFERENT farbled values? That is the first-party-keying contract (Brave's model, and what
C2 implements by keying on the registrable domain of the **main-frame** navigation URL).

## Why this is written as a diagnostic and not a pass/fail assertion

There are three distinguishable outcomes, and only one of them is "working". A bare
"A != B" assertion cannot tell the other two apart, and the difference matters:

    iframe_A == its own parent's farbled value -> FIRST-PARTY KEYING LIVE      (the contract)
    iframe_A == iframe_B == native_X           -> iframe UNFARBLED             (a coverage gap)
    iframe_A == iframe_B == farbled_X          -> keyed on the IFRAME's origin (wrong model)

The second and third are both "equal", and a harness that only checked for difference would
report them identically — while they call for completely different responses. So this script
measures a native baseline and a top-level farbled baseline for the third-party origin and
names which of the three it found.

## ⛔ 2026-08-13 — "iframe_A != iframe_B" was NOT strong enough, and has been replaced

The pass condition used to be simply that the third party got different values under the two
parents. **The wrong model satisfies that too:** a build keying on the *iframe's own origin*
also produces two different values under two different parents, because the key varies per
iframe instance. So the loose form went green on precisely the model P4e exists to avoid, and
it could not separate outcome 1 from outcome 3.

The row now asserts the thing it actually means: **each cross-site iframe carries ITS OWN
PARENT'S farbled values.** Each parent is therefore also measured as a top-level page, in the
same session so the profile seed is identical.

Two guards come with it (relay §Z1), because `child == parent` is trivially true when both
are broken the same way:
  * each parent's top-level measurement must itself differ from native — otherwise nothing
    is farbled and the match is vacuous;
  * the two parents must differ from each other — otherwise the assertion cannot discriminate.

⚠️ **The gap outcome was the measured reality until P4e.** The renderer bailed on every
subframe (`if (frame_->Parent() != nullptr) return;`), so no subframe ever received a key and
all of them failed closed to native. Post-P4e the key is delivered via
`CefBrowserFrame::ResolveTopFrameHost`, and seeing the gap outcome again is a **regression**,
not a known limitation.

## Subject

Cross-origin iframes surface as their own CDP target with `type == "iframe"`, so this
measures the iframe's real main world directly — no isolated world, no same-origin-policy
workaround, and no risk of reading the parent by mistake. The three origins are distinct
registrable domains that all permit framing (no X-Frame-Options).

## Controls

  * the >=65536px canvas and >=262144B readPixels sit outside the farbling size gates and
    must be IDENTICAL everywhere; if one moves, nothing below is comparable
  * **same-parent repeat** — the iframe measured twice under the SAME parent must be
    identical, which is what makes a cross-parent difference meaningful rather than noise
  * a top-level measurement of the third-party origin, both farbled and hard-bypassed,
    to name the outcome

## Usage

    python farbling_iframe_check.py \
        --exe "C:\...\HodosBrowser.exe" --data-root "%APPDATA%\HodosBrowserDev" --dev
"""

import argparse
import json
import os
import shutil
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import websocket
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402
    MEASURE_JS,
    engine_version,
    kill_browser_by_path,
    launch_browser,
    measure,
    read_settings,
    resolve_tab,
    settings_path,
    snapshot_targets,
    wait_for_cdp,
    write_settings,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402

PARENT_A = "example.com"
PARENT_B = "example.net"
THIRD = "example.org"          # framed by both; a distinct registrable domain from either

FARBLED_FIELDS = ["small", "glSmall", "audio", "deviceMemory", "cores"]
GATE_CONTROLS = ["large", "glLarge"]


def _rpc(ws_url, method, params=None, msg_id=1, wait=25):
    ws = websocket.create_connection(ws_url, timeout=30)
    try:
        ws.send(json.dumps({"id": msg_id, "method": method, "params": params or {}}))
        end = time.time() + wait
        while time.time() < end:
            m = json.loads(ws.recv())
            if m.get("id") == msg_id:
                return m
    finally:
        ws.close()
    return None


def targets(port):
    with urllib.request.urlopen("http://127.0.0.1:%d/json/list" % port, timeout=10) as fh:
        return json.loads(fh.read().decode("utf-8", "replace"))


def measure_iframe(port, excluded, parent_host, timeout=90):
    """Load the parent, inject the third-party iframe, then measure INSIDE the iframe by
    attaching to its own CDP target.

    ⚠️ `excluded` is passed in, never re-snapshotted here. Snapshotting with a short settle
    catches only some of the ~14 overlays, and `resolve_tab` then sees several candidates
    and aborts as ambiguous — correctly, but it looks like a harness failure rather than
    the caller's mistake. One snapshot per launch, after a full settle."""
    t = resolve_tab(port, excluded)
    _rpc(t["webSocketDebuggerUrl"], "Page.navigate",
         {"url": "https://%s/" % parent_host}, msg_id=1)

    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        t = resolve_tab(port, excluded)
        if parent_host not in t.get("url", ""):
            continue
        # Remove any previous probe iframe first: a stale one from the other parent would
        # be measured instead, and would make the two parents agree for the worst reason.
        inject = ("(function(){var o=document.getElementById('hodosprobe');"
                  "if(o)o.remove();var f=document.createElement('iframe');"
                  "f.id='hodosprobe';f.src='https://%s/?p=%s';"
                  "document.body.appendChild(f);return 'ok';})()" % (THIRD, parent_host))
        _rpc(t["webSocketDebuggerUrl"], "Runtime.evaluate",
             {"expression": inject, "returnByValue": True}, msg_id=2)
        break
    else:
        raise SystemExit("parent %s never loaded" % parent_host)

    # Find the iframe target. Keyed on the ?p= marker so we cannot pick up a leftover.
    marker = "?p=%s" % parent_host
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        for tg in targets(port):
            if tg.get("type") == "iframe" and THIRD in tg.get("url", "") \
                    and marker in tg.get("url", ""):
                got = _rpc(tg["webSocketDebuggerUrl"], "Runtime.evaluate",
                           {"expression": MEASURE_JS, "returnByValue": True,
                            "awaitPromise": True}, msg_id=3)
                if not got:
                    continue
                res = got.get("result", {}).get("result", {})
                if "value" not in res:
                    continue
                val = json.loads(res["value"])
                # SUBJECT assertion: we must be inside the third party, not the parent.
                if THIRD not in val.get("href", "") or marker not in val.get("href", ""):
                    continue
                return val
    raise SystemExit("could not measure the %s iframe under %s within %ss"
                     % (THIRD, parent_host, timeout))


def boot(args, bypass_hosts):
    pdir = profile_dir(args.data_root, args.profile)
    port = cdp_port_for(args.profile, args.dev)
    kill_browser_by_path(args.exe)

    doc = read_settings(pdir)
    sites = doc.get("siteSettings")
    if not isinstance(sites, dict):
        sites = {}
    for h in (PARENT_A, PARENT_B, THIRD):
        sites.pop(h, None)
    for h in bypass_hosts:
        sites[h] = {"enabled": False}
    doc["siteSettings"] = sites
    write_settings(pdir, doc)

    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    return port


def show(label, v):
    print("    %-26s canvas=%s/%s webgl=%s/%s audio=%s mem=%s cores=%s"
          % (label, v["small"], v["large"], v["glSmall"], v["glLarge"],
             v["audio"], v["deviceMemory"], v["cores"]))


def same(a, b):
    return all(a.get(f) == b.get(f) for f in FARBLED_FIELDS)


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
    ap.add_argument("--timeout", type=float, default=90.0)
    args = ap.parse_args()

    pdir = profile_dir(args.data_root, args.profile)
    path = settings_path(pdir)
    backup = None
    if os.path.exists(path):
        backup = path + ".iframe-backup"
        shutil.copy2(path, backup)

    try:
        print("=== phase 1 — third party as a TOP-LEVEL page (farbled + native) ======")
        port = boot(args, bypass_hosts=[])
        eng = engine_version(port)
        excl = snapshot_targets(port, settle=8.0)
        top_farbled = measure(port, excl, "https://%s/" % THIRD, THIRD, timeout=args.timeout)
        show("top-level farbled", top_farbled)

        port = boot(args, bypass_hosts=[THIRD])
        excl = snapshot_targets(port, settle=8.0)
        top_native = measure(port, excl, "https://%s/" % THIRD, THIRD, timeout=args.timeout)
        show("top-level NATIVE", top_native)

        print("\n=== phase 2 — the same third party in an IFRAME, two parents ========")
        port = boot(args, bypass_hosts=[])
        excl = snapshot_targets(port, settle=8.0)
        a1 = measure_iframe(port, excl, PARENT_A, timeout=args.timeout)
        show("iframe under %s" % PARENT_A, a1)
        a2 = measure_iframe(port, excl, PARENT_A, timeout=args.timeout)
        show("iframe under %s (again)" % PARENT_A, a2)
        b1 = measure_iframe(port, excl, PARENT_B, timeout=args.timeout)
        show("iframe under %s" % PARENT_B, b1)

        # The S3 STRENGTHENING (relay 2026-08-13c §Y5 / §Z1). Each parent measured as a
        # TOP-LEVEL page in this same session, so its first-party key is the same one its
        # iframes should be receiving. Same boot on purpose -- a different launch could
        # carry a different profile seed and the comparison would be meaningless.
        pa_top = measure(port, excl, "https://%s/" % PARENT_A, PARENT_A, timeout=args.timeout)
        show("top-level %s" % PARENT_A, pa_top)
        pb_top = measure(port, excl, "https://%s/" % PARENT_B, PARENT_B, timeout=args.timeout)
        show("top-level %s" % PARENT_B, pb_top)
    finally:
        kill_browser_by_path(args.exe)
        if backup and os.path.exists(backup):
            shutil.copy2(backup, path)
            os.remove(backup)
            print("\nrestored %s" % path)

    print("\n================ DIAGNOSIS (engine %s) ================" % eng)
    ok = True

    broken = [f for f in GATE_CONTROLS
              if len({x.get(f) for x in (top_farbled, top_native, a1, a2, b1,
                                         pa_top, pb_top)}) != 1]
    if broken:
        print("  *** SIZE-GATE CONTROL MOVED (%s) — nothing below is comparable."
              % ",".join(broken))
        return 1
    print("  size-gate controls held everywhere                     OK")

    if not same(a1, a2):
        print("  *** same-parent repeat DIFFERED — the iframe measurement is not stable, "
              "so a cross-parent difference would prove nothing.")
        return 1
    print("  same-parent repeat identical (stability control)       OK")

    if not same(top_farbled, top_native):
        print("  top-level farbling is active for %-14s        OK" % THIRD)
    else:
        print("  *** top-level %s measured the same farbled and hard-bypassed — farbling "
              "is not active at all here; the iframe verdict below is meaningless." % THIRD)
        return 1

    print()

    # ⛔ "a1 != b1" IS NOT SUFFICIENT, and was the row's assertion until 2026-08-13.
    # A build that keys on the IFRAME'S OWN ORIGIN also yields different values under the
    # two parents, so the loose form goes green on precisely the model we do not want --
    # it cannot separate outcome 1 from outcome 3. The direct test of first-party keying
    # is that the child carries ITS PARENT'S value.
    #
    # ⚠️ Paired with an "actually farbled" guard, per relay §Z1: if a future regression
    # made parent and child BOTH fail closed to native, `child == parent` would be
    # trivially true and the row would go green with farbling entirely dead. Native
    # values are machine-wide -- they do not depend on the host -- so top_native is a
    # valid native baseline for every host measured here.
    # The guard applies to the PARENTS, not the children. A child equal to native is the
    # informative coverage-gap outcome reported below -- swallowing it here would delete
    # the diagnosis this harness exists to make. What must hold is that the reference
    # values are genuinely farbled, otherwise "child == parent" proves nothing.
    dead = [n for n, v in (("top-level " + PARENT_A, pa_top),
                           ("top-level " + PARENT_B, pb_top))
            if same(v, top_native)]
    if dead:
        print("  *** REFERENCE NOT FARBLED: %s equal(s) the native baseline, so a"
              % ", ".join(dead))
        print("      'child == parent' match would be trivially true. No verdict.")
        return 1

    if same(pa_top, pb_top):
        print("  *** THE TWO PARENTS PRODUCED THE SAME FARBLED VALUES, so 'child carries its")
        print("      own parent's key' cannot distinguish them. No verdict.")
        return 1
    print("  the two first parties are keyed differently (reference)  OK")

    matches_own_parent = same(a1, pa_top) and same(b1, pb_top)

    if matches_own_parent:
        print("  VERDICT: FIRST-PARTY KEYING LIVE — each cross-site iframe carries ITS OWN")
        print("           PARENT'S farbled values (a1==%s-top, b1==%s-top), and the two"
              % (PARENT_A, PARENT_B))
        print("           parents differ. Row PASSES on the strong assertion.")
        if same(a1, b1):
            print("  *** ...but a1 == b1, which contradicts first-party keying. Both parents")
            print("      resolving to one key would do this. Investigate before trusting.")
            ok = False
    elif same(a1, top_native):
        print("  VERDICT: CROSS-SITE IFRAME IS UNFARBLED (equals the native baseline).")
        print("           A COVERAGE GAP, not a keying bug: the renderer bailed on every")
        print("           subframe, so it never received a key and failed closed to native.")
        print("           This is the EXPECTED PRE-P4e RESULT. Post-P4e it is a REGRESSION —")
        print("           the key is delivered by CefBrowserFrame::ResolveTopFrameHost, so")
        print("           seeing it again means the subframe path is not reaching that.")
        ok = False
    elif same(a1, top_farbled):
        print("  VERDICT: KEYED ON THE IFRAME'S OWN ORIGIN, not the top frame. This is a")
        print("           real keying BUG, not a coverage gap: a third-party tracker would")
        print("           get the SAME value across every site that embeds it, which is")
        print("           precisely the cross-site linkage farbling exists to prevent.")
        print("           ⚠️ NOTE this outcome also satisfies the OLD 'a1 != b1' assertion,")
        print("           which is exactly why that form was replaced.")
        ok = False
    else:
        print("  VERDICT: each iframe is farbled, but NOT with its own parent's key.")
        print("             a1 == %s-top ? %s" % (PARENT_A, same(a1, pa_top)))
        print("             b1 == %s-top ? %s" % (PARENT_B, same(b1, pb_top)))
        print("           A partial match points at one path resolving the top frame and the")
        print("           other not — check the opener walk and the memo key before anything")
        print("           else. Do not interpret as a pass.")
        ok = False

    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
