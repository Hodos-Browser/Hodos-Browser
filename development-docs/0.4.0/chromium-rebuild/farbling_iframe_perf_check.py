#!/usr/bin/env python3
r"""farbling_iframe_perf_check.py — P4e D4 gate: what does per-frame farbling cost?

## Why `farbling_perf_check.py` cannot answer this, and must not be reused for it

That harness times `getImageData` / `readPixels` in microseconds per CALL. It measures the
cost of perturbing a readback buffer. P4e's regression is somewhere else entirely: a
BLOCKING sync browser round-trip at FRAME CREATION, on the first-paint path. Reusing the
canvas harness here would produce a confident green number that is structurally incapable
of seeing the thing it was pointed at -- the same defect that produced three worthless
farbling harnesses already (CLAUDE.md, "NEGATIVE CONTROL").

So this measures frame creation, not pixel readback.

## What is actually being measured

    t0 = performance.now()
    for i in 0..N:  create iframe, append it, force its V8 context
    t1 = performance.now()

Forcing the context (`contentWindow.eval('1')`) is load-bearing. Appending an iframe
creates the frame, but the child's V8 context may not be built until script touches it --
and OnContextCreated is exactly where the farbling pull happens. Without the eval, the
loop can complete having triggered zero pulls and report a beautiful flat line. It is also
precisely what the bypass does, so the measured path is the attacked path.

## ⛔ The pre-change baseline can only be taken ONCE

Before P4e, subframes take an early return and make NO browser call at all, so today's
number is the FLOOR -- the best this can ever be. Record it before rebuilding; after the
rebuild the old engine no longer exists to re-measure. Use `--save-baseline` now and
`--baseline` after.

## Controls

  * **N-scaling (sensitivity)** -- the metric MUST grow with N across the sweep. If cost
    at N=200 is not clearly above cost at N=1, the harness is not resolving frame-creation
    cost and every number below it is noise. This is the control that makes a flat
    post-change result meaningful rather than merely comforting.
  * **memo effectiveness (post-change only)** -- per-frame cost at high N must not exceed
    per-frame cost at N=1 by more than the budget. Without the D4 memo every frame pays
    its own blocking IPC, so per-frame cost stays flat-high; with it, the first frame pays
    and the rest amortise. A per-frame cost that RISES with N means the memo is not
    working, whatever the totals look like.
  * **repetition** -- each N is measured `--reps` times and the MEDIAN is taken, because a
    single first-run measurement on a cold renderer is dominated by lazy initialisation.

⚠️ "Farbling off" is NOT a valid negative control for this metric, and pretending it is
would be the same class of error the file is written to avoid. With a site hard-bypassed
the shell still files an entry (`enabled=false`) and the renderer still pulls, so the IPC
cost is unchanged. The controls that DO discriminate here are N-scaling and the
per-frame-vs-N shape above.

Usage:

    # BEFORE the rebuild -- do this first, it cannot be repeated
    python farbling_iframe_perf_check.py --exe ... --data-root ... --dev \
        --save-baseline p4e_iframe_perf_baseline.json

    # AFTER the rebuild
    python farbling_iframe_perf_check.py --exe ... --data-root ... --dev \
        --baseline p4e_iframe_perf_baseline.json [--max-delta-us 50]
"""

import argparse
import json
import os
import platform
import statistics
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import websocket  # noqa: F401
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    kill_browser_by_path,
    launch_browser,
    resolve_tab,
    snapshot_targets,
    wait_for_cdp,
)
from farbling_iframe_check import _rpc  # noqa: E402
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402

HOST = "example.com"
DEFAULT_SWEEP = [1, 10, 50, 200]

# Returns total elapsed ms for creating N frames, each with a live V8 context.
# Frames are removed afterwards so repeated reps start from the same DOM size; the
# removal is OUTSIDE the timed region.
BUILD_JS = r"""
(function () {
  var N = %(n)d;
  var made = [];
  var sink = 0;
  var t0 = performance.now();
  for (var i = 0; i < N; i++) {
    var f = document.createElement('iframe');
    document.body.appendChild(f);
    // Force the child's V8 context -- this is where the farbling pull fires, and
    // without it the loop can trigger none of them.
    try { sink += f.contentWindow.eval('1'); } catch (e) { sink += 0; }
    made.push(f);
  }
  var t1 = performance.now();
  for (var j = 0; j < made.length; j++) { made[j].remove(); }
  return JSON.stringify({n: N, ms: t1 - t0, sink: sink});
})()
"""


def boot(args):
    port = cdp_port_for(args.profile, args.dev)
    kill_browser_by_path(args.exe)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    time.sleep(args.settle)
    return port


def goto_host(port, excluded, timeout=90):
    t = resolve_tab(port, excluded)
    _rpc(t["webSocketDebuggerUrl"], "Page.navigate",
         {"url": "https://%s/" % HOST}, msg_id=1)
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        t = resolve_tab(port, excluded)
        if HOST in t.get("url", ""):
            return t
    raise SystemExit("could not load %s within %ss" % (HOST, timeout))


class Cdp:
    """ONE persistent CDP connection for the whole sweep.

    ⚠️ Not a style preference. The shared `_rpc` helper opens and closes a websocket per
    call, which is fine for the two-call harnesses it was written for. This sweep issues
    ~20 evaluates in quick succession against the same target, and that connect/close
    churn intermittently leaves a fresh connection unserviced until the 30 s socket
    timeout fires -- reproduced twice here as a hang on the very first N. One connection
    held open for the run removes the race and is faster besides.
    """

    def __init__(self, ws_url, timeout=120):
        self.ws = websocket.create_connection(ws_url, timeout=timeout)
        self._id = 1000

    def evaluate(self, expression):
        self._id += 1
        want = self._id
        self.ws.send(json.dumps({"id": want, "method": "Runtime.evaluate",
                                 "params": {"expression": expression,
                                            "returnByValue": True}}))
        # Drain unsolicited events until our own reply arrives.
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == want:
                return msg

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass


def run_one(cdp, n):
    got = cdp.evaluate(BUILD_JS % {"n": n})
    res = (got or {}).get("result", {}).get("result", {})
    if "value" not in res:
        raise SystemExit("iframe-build probe returned nothing for N=%d: %s"
                         % (n, json.dumps(got)[:300]))
    return json.loads(res["value"])


def measure(port, excluded, sweep, reps):
    tab = goto_host(port, excluded)
    cdp = Cdp(tab["webSocketDebuggerUrl"])
    out = {}
    try:
        # One discarded warm-up per run: the first iframe in a fresh renderer pays lazy
        # initialisation that has nothing to do with farbling, and folding it into N=1
        # would inflate the very number the whole comparison pivots on.
        run_one(cdp, 1)
        for n in sweep:
            samples = []
            for _ in range(reps):
                samples.append(run_one(cdp, n)["ms"])
                time.sleep(0.3)
            ms = statistics.median(samples)
            out[n] = {"total_ms": ms, "per_frame_us": (ms * 1000.0) / n,
                      "samples_ms": samples}
            print("    N=%-4d  total %8.2f ms   per frame %8.1f us   (reps %s)"
                  % (n, ms, out[n]["per_frame_us"],
                     ", ".join("%.1f" % s for s in samples)))
    finally:
        cdp.close()
    return out


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
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--sweep", default=",".join(str(n) for n in DEFAULT_SWEEP))
    ap.add_argument("--save-baseline", help="write results here (do this BEFORE rebuilding)")
    ap.add_argument("--baseline", help="compare against a previously saved baseline")
    ap.add_argument("--max-delta-us", type=float, default=50.0,
                    help="per-frame budget, microseconds, vs the baseline (default 50)")
    args = ap.parse_args()

    sweep = [int(x) for x in args.sweep.split(",") if x.strip()]
    if len(sweep) < 2:
        sys.exit("need at least two N values for the scaling control")

    profile_dir(args.data_root, args.profile)  # validates the path shape
    port = boot(args)
    try:
        eng = engine_version(port)
        excluded = snapshot_targets(port)
        print("=== iframe-creation cost on %s (engine %s) ===" % (HOST, eng))
        results = measure(port, excluded, sweep, args.reps)
    finally:
        kill_browser_by_path(args.exe)

    lo, hi = sweep[0], sweep[-1]
    print("\n" + "=" * 78)
    print("CONTROLS")
    ok = True

    # Sensitivity: if total cost does not rise with N, this harness is not measuring
    # frame creation and nothing below means anything.
    if results[hi]["total_ms"] > results[lo]["total_ms"]:
        print("    [PASS] N-scaling: total cost rises with N   (%.2f ms @N=%d -> %.2f ms @N=%d)"
              % (results[lo]["total_ms"], lo, results[hi]["total_ms"], hi))
    else:
        print("    [FAIL] N-scaling: cost did NOT rise with N — harness is not resolving")
        print("           frame-creation cost. Do not read the numbers below.")
        ok = False

    # Memo shape: per-frame cost must not climb with N. It climbing means each frame is
    # paying its own blocking round trip, i.e. the D4 memo is absent or not hitting.
    pf_lo, pf_hi = results[lo]["per_frame_us"], results[hi]["per_frame_us"]
    if pf_hi <= pf_lo + args.max_delta_us:
        print("    [PASS] per-frame cost does not climb with N   (%.1f us @N=%d -> %.1f us @N=%d)"
              % (pf_lo, lo, pf_hi, hi))
    else:
        print("    [FAIL] per-frame cost CLIMBS with N   (%.1f us @N=%d -> %.1f us @N=%d)"
              % (pf_lo, lo, pf_hi, hi))
        print("           Every frame appears to be paying its own blocking IPC.")
        ok = False

    rc = 0 if ok else 1

    if args.baseline:
        with open(args.baseline, encoding="utf-8") as fh:
            base = json.load(fh)
        print("\nBASELINE COMPARISON  (baseline engine %s, this engine %s)"
              % (base.get("engine", "?"), eng))
        if base.get("machine") != platform.node():
            print("    ⚠️ baseline was recorded on %r, this is %r — cross-machine"
                  % (base.get("machine"), platform.node()))
            print("       numbers are not comparable; treat this as advisory only.")
        worst = 0.0
        for n in sweep:
            b = base.get("results", {}).get(str(n))
            if not b:
                print("    N=%-4d  no baseline entry — skipped" % n)
                continue
            delta = results[n]["per_frame_us"] - b["per_frame_us"]
            worst = max(worst, delta)
            print("    N=%-4d  %8.1f -> %8.1f us/frame   delta %+8.1f us"
                  % (n, b["per_frame_us"], results[n]["per_frame_us"], delta))
        if worst <= args.max_delta_us:
            print("    [PASS] worst per-frame delta %+.1f us within budget %.1f us"
                  % (worst, args.max_delta_us))
        else:
            print("    [FAIL] worst per-frame delta %+.1f us EXCEEDS budget %.1f us"
                  % (worst, args.max_delta_us))
            rc = 1

    if args.save_baseline:
        payload = {
            "engine": eng,
            "machine": platform.node(),
            "platform": platform.platform(),
            "reps": args.reps,
            "results": {str(n): {"total_ms": results[n]["total_ms"],
                                 "per_frame_us": results[n]["per_frame_us"]}
                        for n in sweep},
        }
        with open(args.save_baseline, "w", encoding="utf-8") as fh:
            json.dump(payload, fh, indent=2)
        print("\n  baseline written to %s" % args.save_baseline)
        print("  ⛔ This is the PRE-CHANGE floor and cannot be re-measured after the")
        print("     rebuild. Keep it with the branch.")

    return rc


if __name__ == "__main__":
    sys.exit(main())
