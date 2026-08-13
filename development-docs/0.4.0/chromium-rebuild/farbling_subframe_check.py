#!/usr/bin/env python3
r"""farbling_subframe_check.py — W3 Step 0: is a SAME-ORIGIN subframe farbled?

Settles the ROUND 2026-08-13a §W3 claim, which was reasoned from fork source and explicitly
NOT measured:

    CefFrameImpl::MaybeApplyHodosFarblingKey:  if (frame_->Parent() != nullptr) { return; }

If that bail really covers every subframe, then a page defeats farbling in three lines —
create a same-origin `about:blank` iframe and read canvas / WebGL / audio / navigator from
its `contentWindow`. That is a **bypass**, not a coverage gap, and it is a different claim
from the already-measured cross-site-iframe gap (`farbling_iframe_check.py`).

## Why this needs its own harness

`farbling_iframe_check.py` attaches to the iframe's **own CDP target**, which only exists
because a cross-origin iframe is an OOPIF in its own process. A same-origin `about:blank`
child has **no separate target** — it shares the parent's process and realm boundary — so
that harness structurally cannot see this case. Here the child is reached through
`contentWindow`, which is also precisely how the hypothesised attack reaches it.

The child is measured by `w.eval(MEASURE_JS)`, i.e. the SAME measurement source executed in
the child's own realm, so `document` / `navigator` / `OfflineAudioContext` resolve to the
child's. Reusing the source verbatim is deliberate: a re-implementation could differ from
the parent's measurement in some detail and manufacture a difference that is not farbling.

## ⛔ Subject assertion

The child's `location.href` MUST read `about:blank`. Without that, a failed injection would
silently measure the PARENT twice, the two would agree, and the harness would report
"subframes are farbled" — a false exoneration, which is the worst direction to be wrong in.

## Controls (both halves, per TESTING.md §15)

  * **size-gate control** — `large` (>=65536px canvas) and `glLarge` (>=262144B readPixels)
    sit outside the farbling gates and must be IDENTICAL parent vs child. If one moves,
    the two realms are not comparable and nothing below means anything.
  * **native baseline** — phase 2 re-measures with the host hard-bypassed per-site, giving
    real native values to compare against, so an unfarbled child can be distinguished from
    a differently-keyed one.
  * **negative control** — in phase 2 the parent/child difference must VANISH. A harness
    that reports a difference in both phases is measuring realm noise, not farbling.

## Three distinguishable outcomes

    child == parent (and parent != native)   -> SUBFRAMES ARE FARBLED   -> §W3 is WRONG
    child == native (and parent != native)   -> SUBFRAME UNFARBLED      -> §W3 CONFIRMED, bypass
    child != parent and child != native      -> keyed some third way    -> investigate

Usage:

    python farbling_subframe_check.py \
        --exe ".../HodosBrowser.app/Contents/MacOS/HodosBrowser" \
        --data-root "~/Library/Application Support/HodosBrowserDev" --dev
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import websocket  # noqa: F401
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402
    MEASURE_JS,
    engine_version,
    kill_browser_by_path,
    launch_browser,
    read_settings,
    resolve_tab,
    set_site_enabled,
    snapshot_targets,
    wait_for_cdp,
    write_settings,
)
from farbling_iframe_check import _rpc  # noqa: E402
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402

HOST = "example.com"
FARBLED_FIELDS = ["small", "glSmall", "audio", "deviceMemory", "cores"]
GATE_CONTROLS = ["large", "glLarge"]

# Parent and child measured with ONE source, in two realms. `w.eval` runs in the child's
# global, so every API the measurement touches is the child's.
PROBE_JS = r"""
(async function () {
  var SRC = %s;
  var parent = JSON.parse(await (0, eval)(SRC));

  var old = document.getElementById('hodos_subframe_probe');
  if (old) { old.remove(); }
  var f = document.createElement('iframe');
  f.id = 'hodos_subframe_probe';
  document.body.appendChild(f);          // about:blank, same origin, synchronously usable

  var w = f.contentWindow;
  var child = null, err = null;
  try {
    child = JSON.parse(await w.eval(SRC));
  } catch (e) {
    err = String(e && e.message || e);
  }
  return JSON.stringify({parent: parent, child: child, childError: err});
})()
"""


def measure_pair(port, excluded, timeout=90):
    t = resolve_tab(port, excluded)
    _rpc(t["webSocketDebuggerUrl"], "Page.navigate",
         {"url": "https://%s/" % HOST}, msg_id=1)

    expr = PROBE_JS % json.dumps(MEASURE_JS)
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        t = resolve_tab(port, excluded)
        if HOST not in t.get("url", ""):
            continue
        got = _rpc(t["webSocketDebuggerUrl"], "Runtime.evaluate",
                   {"expression": expr, "returnByValue": True, "awaitPromise": True},
                   msg_id=2, wait=60)
        if not got:
            continue
        res = got.get("result", {}).get("result", {})
        if "value" not in res:
            continue
        return json.loads(res["value"])
    raise SystemExit("could not measure %s within %ss" % (HOST, timeout))


def boot(args, bypass):
    pdir = profile_dir(args.data_root, args.profile)
    port = cdp_port_for(args.profile, args.dev)
    kill_browser_by_path(args.exe)
    set_site_enabled(pdir, HOST, not bypass)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    time.sleep(args.settle)
    return port, pdir


def show(tag, v):
    if not v:
        print("    %-22s <none>" % tag)
        return
    print("    %-22s canvas=%s/%s webgl=%s/%s audio=%s mem=%s cores=%s"
          % (tag, v["small"], v["large"], v["glSmall"], v["glLarge"],
             v["audio"], v["deviceMemory"], v["cores"]))


def same(a, b, fields):
    return all(a.get(f) == b.get(f) for f in fields)


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
    args = ap.parse_args()

    pdir0 = profile_dir(args.data_root, args.profile)
    preserved = read_settings(pdir0)

    try:
        print("=== phase 1 — farbling ON for %s ===" % HOST)
        port, pdir = boot(args, bypass=False)
        excluded = snapshot_targets(port)
        p1 = measure_pair(port, excluded)
        show("parent (top frame)", p1["parent"])
        show("child (about:blank)", p1["child"])
        if p1.get("childError"):
            print("    child error: %s" % p1["childError"])

        print("\n=== phase 2 — %s hard-bypassed per-site (NATIVE baseline + neg control) ===" % HOST)
        port, pdir = boot(args, bypass=True)
        excluded = snapshot_targets(port)
        p2 = measure_pair(port, excluded)
        show("parent (native)", p2["parent"])
        show("child (native)", p2["child"])
    finally:
        write_settings(pdir0, preserved)
        print("\nrestored fingerprint_settings.json")

    par, ch, nat, nch = p1["parent"], p1["child"], p2["parent"], p2["child"]

    print("\n" + "=" * 78)
    print("CONTROLS")
    ok = True

    if ch is None:
        print("  [FAIL] child was never measured — cannot conclude anything")
        return 1

    href = ch.get("href", "")
    if href == "about:blank":
        print("  [PASS] SUBJECT: child href is 'about:blank'  (not the parent)")
    else:
        print("  [FAIL] SUBJECT: child href is %r — measured the wrong realm" % href)
        ok = False

    if same(par, ch, GATE_CONTROLS):
        print("  [PASS] size-gate control identical parent vs child   large=%s glLarge=%s"
              % (par["large"], par["glLarge"]))
    else:
        print("  [FAIL] size-gate control MOVED — realms not comparable  %s/%s vs %s/%s"
              % (par["large"], par["glLarge"], ch["large"], ch["glLarge"]))
        ok = False

    if not same(par, nat, FARBLED_FIELDS):
        print("  [PASS] farbling is active at all      parent != native")
    else:
        print("  [FAIL] parent == native — farbling was OFF; nothing below is meaningful")
        ok = False

    if nch is not None and same(nat, nch, FARBLED_FIELDS):
        print("  [PASS] NEGATIVE CONTROL: with the host bypassed, parent == child")
    else:
        print("  [FAIL] NEGATIVE CONTROL: parent != child even with farbling off "
              "⇒ measuring realm noise, not farbling")
        ok = False

    print("\nVERDICT")
    child_is_parent = same(par, ch, FARBLED_FIELDS)
    child_is_native = same(ch, nat, FARBLED_FIELDS)

    if not ok:
        print("  ⛔ CONTROLS FAILED — no verdict. Fix the harness before believing any result.")
        return 1

    if child_is_parent:
        print("  ✅ SUBFRAMES ARE FARBLED — §W3 is WRONG.")
        print("     The same-origin child carries the parent's farbled values.")
        print("     Scope line stays 'main frame + same-site frames'; P4e shrinks to cross-site.")
        rc = 0
    elif child_is_native:
        print("  ⛔ §W3 CONFIRMED — the same-origin subframe is UNFARBLED.")
        print("     A page reads NATIVE values through its own about:blank iframe.")
        print("     This is a BYPASS, not a coverage gap: farbling is defeated in 3 lines of JS.")
        print("     Shipped scope line is 'MAIN FRAME ONLY'.")
        for f in FARBLED_FIELDS:
            if par.get(f) != ch.get(f):
                print("       %-14s parent=%-10s child=%-10s native=%s"
                      % (f, par.get(f), ch.get(f), nat.get(f)))
        rc = 2
    else:
        print("  ⚠️ THIRD OUTCOME — child matches neither the parent nor native.")
        print("     Keyed some other way; investigate before acting.")
        for f in FARBLED_FIELDS:
            print("       %-14s parent=%-10s child=%-10s native=%s"
                  % (f, par.get(f), ch.get(f), nat.get(f)))
        rc = 3

    print("\n  engine %s" % engine_version(cdp_port_for(args.profile, args.dev)))
    return rc


if __name__ == "__main__":
    sys.exit(main())
