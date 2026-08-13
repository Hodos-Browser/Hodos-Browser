#!/usr/bin/env python3
r"""farbling_subframe_check.py — are ORIGIN-INHERITING child realms farbled?

Settles the ROUND 2026-08-13a §W3 claim, which was reasoned from fork source and explicitly
NOT measured:

    CefFrameImpl::MaybeApplyHodosFarblingKey:  if (frame_->Parent() != nullptr) { return; }

If that bail really covers every subframe, then a page defeats farbling in three lines —
create a same-origin `about:blank` iframe and read canvas / WebGL / audio / navigator from
its `contentWindow`. That is a **bypass**, not a coverage gap, and it is a different claim
from the already-measured cross-site-iframe gap (`farbling_iframe_check.py`).

Measured 2026-08-13 (Mac, f910e19): CONFIRMED for the iframe vector. This harness is now the
**regression test for the P4e fix** — after the patch the same rows must flip to "farbled".

## Two vectors, because they are the same bug in two containers

    --vector iframe   same-origin `about:blank` CHILD FRAME, read via contentWindow
    --vector popup    same-origin `about:blank` POPUP  (window.open()), read via the handle

⛔ **The popup vector exists because the P4e design does not fix it for free.** The patch
resolves a subframe's key from its top frame; a `window.open()` popup **is** a top frame,
whose committed URL is `about:blank`, so the registry lookup misses and it fails closed to
native — exactly the §W3 bypass in a different container. A run that only exercises the
iframe row would go green on a patch that leaves the popup wide open.

Both vectors share ONE parent measurement per phase and one boot pair, so the two children
are compared against the same baseline under identical conditions.

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

⚠️ For the popup vector there is a second way to get a false green: `window.open()` may be
blocked or intercepted by the shell, in which case there is no child at all. That is
reported as UNREACHABLE — a distinct outcome from "farbled". It is NOT a pass, because it
says the harness could not test the claim, not that the claim is false.

## Controls (both halves, per TESTING.md §15)

  * **size-gate control** — `large` (>=65536px canvas) and `glLarge` (>=262144B readPixels)
    sit outside the farbling gates and must be IDENTICAL parent vs child. If one moves,
    the two realms are not comparable and nothing below means anything.
  * **native baseline** — phase 2 re-measures with the host hard-bypassed per-site, giving
    real native values to compare against, so an unfarbled child can be distinguished from
    a differently-keyed one.
  * **negative control** — in phase 2 the parent/child difference must VANISH. A harness
    that reports a difference in both phases is measuring realm noise, not farbling.

## Three distinguishable outcomes, per vector

    child == parent (and parent != native)   -> CHILD IS FARBLED   -> fixed / §W3 wrong
    child == native (and parent != native)   -> CHILD UNFARBLED    -> the bypass is live
    child != parent and child != native      -> keyed some third way -> investigate

Exit code is the worst outcome across the selected vectors: 0 all farbled, 1 controls
failed (no verdict), 2 a bypass is live, 3 a third outcome, 4 a vector was unreachable.

Usage:

    python farbling_subframe_check.py \
        --exe ".../HodosBrowser.app/Contents/MacOS/HodosBrowser" \
        --data-root "~/Library/Application Support/HodosBrowserDev" --dev \
        [--vector both|iframe|popup]
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

# Parent and both children measured with ONE source, in three realms. `w.eval` runs in the
# child's global, so every API the measurement touches is the child's.
#
# %(src)s = the MEASURE_JS source as a JS string literal
# %(iframe)s / %(popup)s = "true"/"false", so a single-vector run does not open the other
# realm at all (an unused popup would steal focus and perturb the timing of nothing useful).
PROBE_JS = r"""
(async function () {
  var SRC = %(src)s;
  var out = {parent: null, iframe: null, popup: null,
             iframeError: null, popupError: null, popupOpened: false};

  out.parent = JSON.parse(await (0, eval)(SRC));

  if (%(iframe)s) {
    var old = document.getElementById('hodos_subframe_probe');
    if (old) { old.remove(); }
    var f = document.createElement('iframe');
    f.id = 'hodos_subframe_probe';
    document.body.appendChild(f);        // about:blank, same origin, synchronously usable
    try {
      out.iframe = JSON.parse(await f.contentWindow.eval(SRC));
    } catch (e) {
      out.iframeError = String(e && e.message || e);
    }
  }

  if (%(popup)s) {
    var w = null;
    try {
      // No URL: the popup stays on about:blank and inherits this origin, which is the
      // whole point -- it is a TOP frame with no HTTP URL for the registry to key on.
      w = window.open('', 'hodos_popup_probe', 'width=420,height=320');
    } catch (e) {
      out.popupError = 'window.open threw: ' + String(e && e.message || e);
    }
    out.popupOpened = !!w;
    if (!w) {
      if (!out.popupError) {
        out.popupError = 'window.open returned null (blocked or intercepted by the shell)';
      }
    } else {
      try {
        out.popup = JSON.parse(await w.eval(SRC));
      } catch (e) {
        // A popup the shell re-hosts in another process is not scriptable from here.
        // That is a real finding, not a harness failure -- report it verbatim.
        out.popupError = String(e && e.message || e);
      }
      try { w.close(); } catch (e) { /* best effort */ }
    }
  }

  return JSON.stringify(out);
})()
"""


def measure_all(port, excluded, vectors, timeout=90):
    """One page visit, one parent baseline, both child realms."""
    t = resolve_tab(port, excluded)
    _rpc(t["webSocketDebuggerUrl"], "Page.navigate",
         {"url": "https://%s/" % HOST}, msg_id=1)

    expr = PROBE_JS % {
        "src": json.dumps(MEASURE_JS),
        "iframe": "true" if "iframe" in vectors else "false",
        "popup": "true" if "popup" in vectors else "false",
    }
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        t = resolve_tab(port, excluded)
        if HOST not in t.get("url", ""):
            continue
        got = _rpc(t["webSocketDebuggerUrl"], "Runtime.evaluate",
                   # userGesture matters for the popup vector: without it window.open is
                   # blocked outright and the run reports UNREACHABLE for the wrong reason.
                   {"expression": expr, "returnByValue": True,
                    "awaitPromise": True, "userGesture": True},
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


VECTOR_LABEL = {
    "iframe": "same-origin about:blank IFRAME (contentWindow)",
    "popup": "same-origin about:blank POPUP (window.open)",
}


def verdict_for(vector, par, ch, nat, nch, err, opened):
    """Controls then verdict for one child realm. Returns an exit code."""
    print("\n" + "=" * 78)
    print("VECTOR: %s" % VECTOR_LABEL[vector])

    if ch is None:
        # Distinguish "could not test" from "tested and it is fine". A popup the shell
        # blocks or re-hosts out-of-process is not a pass -- it is an untested claim.
        print("  ⚠️ UNREACHABLE — no child measurement.")
        if vector == "popup" and not opened:
            print("     window.open() did not yield a scriptable handle.")
            print("     If the shell intercepts popups this bypass may not exist here —")
            print("     but this run did NOT establish that. Confirm before relying on it.")
        print("     error: %s" % (err or "<none reported>"))
        return 4

    ok = True
    print("  CONTROLS")

    href = ch.get("href", "")
    if href == "about:blank":
        print("    [PASS] SUBJECT: child href is 'about:blank'  (not the parent)")
    else:
        print("    [FAIL] SUBJECT: child href is %r — measured the wrong realm" % href)
        ok = False

    if same(par, ch, GATE_CONTROLS):
        print("    [PASS] size-gate control identical parent vs child   large=%s glLarge=%s"
              % (par["large"], par["glLarge"]))
    else:
        print("    [FAIL] size-gate control MOVED — realms not comparable  %s/%s vs %s/%s"
              % (par["large"], par["glLarge"], ch["large"], ch["glLarge"]))
        ok = False

    if not same(par, nat, FARBLED_FIELDS):
        print("    [PASS] farbling is active at all      parent != native")
    else:
        print("    [FAIL] parent == native — farbling was OFF; nothing below is meaningful")
        ok = False

    if nch is not None and same(nat, nch, FARBLED_FIELDS):
        print("    [PASS] NEGATIVE CONTROL: with the host bypassed, parent == child")
    else:
        print("    [FAIL] NEGATIVE CONTROL: parent != child even with farbling off "
              "⇒ measuring realm noise, not farbling")
        ok = False

    if not ok:
        print("  ⛔ CONTROLS FAILED — no verdict for this vector.")
        return 1

    print("  VERDICT")
    if same(par, ch, FARBLED_FIELDS):
        print("    ✅ FARBLED — the child carries the parent's farbled values.")
        return 0
    if same(ch, nat, FARBLED_FIELDS):
        print("    ⛔ UNFARBLED — this child reads NATIVE values. The bypass is LIVE.")
        for f in FARBLED_FIELDS:
            if par.get(f) != ch.get(f):
                print("       %-14s parent=%-10s child=%-10s native=%s"
                      % (f, par.get(f), ch.get(f), nat.get(f)))
        return 2
    print("    ⚠️ THIRD OUTCOME — child matches neither the parent nor native.")
    print("       Keyed some other way; investigate before acting.")
    for f in FARBLED_FIELDS:
        print("       %-14s parent=%-10s child=%-10s native=%s"
              % (f, par.get(f), ch.get(f), nat.get(f)))
    return 3


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
    ap.add_argument("--vector", choices=["both", "iframe", "popup"], default="both",
                    help="which origin-inheriting child realm(s) to test")
    args = ap.parse_args()

    vectors = ["iframe", "popup"] if args.vector == "both" else [args.vector]

    pdir0 = profile_dir(args.data_root, args.profile)
    preserved = read_settings(pdir0)

    try:
        print("=== phase 1 — farbling ON for %s ===" % HOST)
        port, pdir = boot(args, bypass=False)
        excluded = snapshot_targets(port)
        p1 = measure_all(port, excluded, vectors)
        show("parent (top frame)", p1["parent"])
        for v in vectors:
            show("child (%s)" % v, p1[v])
            if p1.get(v + "Error"):
                print("    %s error: %s" % (v, p1[v + "Error"]))

        print("\n=== phase 2 — %s hard-bypassed per-site (NATIVE baseline + neg control) ==="
              % HOST)
        port, pdir = boot(args, bypass=True)
        excluded = snapshot_targets(port)
        p2 = measure_all(port, excluded, vectors)
        show("parent (native)", p2["parent"])
        for v in vectors:
            show("child (%s, native)" % v, p2[v])
    finally:
        write_settings(pdir0, preserved)
        print("\nrestored fingerprint_settings.json")

    codes = [verdict_for(v, p1["parent"], p1[v], p2["parent"], p2[v],
                         p1.get(v + "Error"), p1.get("popupOpened", False))
             for v in vectors]

    # Worst outcome wins, in severity order rather than numeric order: controls failing (1)
    # means we learned nothing at all, which is worse than a known-live bypass (2).
    print("\n" + "=" * 78)
    for v, c in zip(vectors, codes):
        print("  %-8s -> %s" % (v, {0: "FARBLED (pass)", 1: "CONTROLS FAILED",
                                    2: "UNFARBLED (bypass live)", 3: "THIRD OUTCOME",
                                    4: "UNREACHABLE"}[c]))
    for severity in (1, 2, 3, 4, 0):
        if severity in codes:
            rc = severity
            break

    print("\n  engine %s" % engine_version(cdp_port_for(args.profile, args.dev)))
    return rc


if __name__ == "__main__":
    sys.exit(main())
