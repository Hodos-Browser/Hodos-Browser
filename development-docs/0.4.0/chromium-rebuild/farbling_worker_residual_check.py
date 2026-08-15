#!/usr/bin/env python3
r"""farbling_worker_residual_check.py — is release-note residual #3 accurate?

§D.1 residual 3 says shared (R9) and service (R10) workers are not covered. That line is
queued for a PUBLIC PRIVACY STATEMENT and it is currently a **CODE READ** — the exact
position D5 was in before `farbling_d5_residual_check.py` measured it and found it
materially WIDER than described. This harness measures it instead.

## What is actually being asked

Post-P4f the §A picture is: R7 dedicated and R8 nested workers are KEYED; R9 shared and
R10 service workers are ⏸️ owner-signed deferrals. So the claim under test is:

    a SharedWorkerGlobalScope reads NATIVE values while, in the same document,
    a DedicatedWorkerGlobalScope reads the TOP FRAME's FARBLED values.

## ⛔ Why "the shared worker reads native" is worthless on its own

It is satisfied by at least four things that are not the finding:

  1. the shared worker never started (CSP, blob: restriction, construction threw);
  2. the probe threw inside the worker and we defaulted a null to "native";
  3. the whole build is unfarbled (a stale binary, farbling globally off, the
     wrong browser) — the pre-P4f world, in which EVERY worker reads native;
  4. the page itself is not farbled, so "native" is simply the only value there is.

⭐ **The discriminator is the DEDICATED worker, measured in the same document, in the
same run, by the same probe.** Post-P4f it must come out FARBLED. That single arm kills
(2), (3) and (4) at once: a probe that can read a farbled value out of one worker realm
is a probe that works, on a build that farbles, in a document that is farbled. (1) is
killed separately by requiring the shared worker to return a live payload carrying its
own `self.location.origin`.

This is the same shape as `farbling_d5_residual_check.py`'s phase-2 arm, and it is
mandatory for the same reason: without it the run confirms the prior rather than testing
it.

## R10 service workers — read the CAVEAT before quoting any result

A service worker cannot be registered from a `blob:` URL; it needs a same-origin script
served over a secure context. We cannot serve one:

  ⛔ `CefFrameImpl::MaybeApplyHodosFarblingKey` returns early for host `127.0.0.1`,
     `localhost` and `[::1]` — UNCONDITIONALLY for main frames, not just for the Hodos UI
     port (`frame_impl.cc`, the internal-UI fast path). So a page served from a local
     server is never farbled, its top-frame reference equals native, and every realm
     verdict under it is VOID by construction. A local fixture cannot settle R10.

So R10 is attempted only opportunistically: if a visited site has registered its own
service worker, its CDP target is attached and the probe is evaluated there. If no such
target exists, R10 is reported NOT MEASURED, with the reason — never as a pass.

## Usage

    python3 farbling_worker_residual_check.py --dev \
        --exe ...HodosBrowser.app/Contents/MacOS/HodosBrowser \
        --data-root ~/Library/Application\ Support/HodosBrowserDev \
        --expect-cef +g9ccef04

    python3 farbling_worker_residual_check.py ... --negative-control

Exit 0 = the residual is exactly as described (R9 unfarbled, R7 farbled, controls green).
Exit 1 = the residual is NOT as described — the release-note line must change.
Exit 2 = void: a control failed, or nothing could be measured.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

try:
    import websocket  # noqa: F401
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    require_engine,
    kill_browser_by_path,
    launch_browser,
    set_site_enabled,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402
from farbling_worker_probe import (  # noqa: E402
    optout_state,
    page_targets,
    resolve_tab,
    snapshot_targets,
)
from farbling_iframe_check import _rpc  # noqa: E402

import urllib.request  # noqa: E402

HOST = "example.com"
URL = "https://example.com/"

# Geometry only -- no text. Font fallback can legitimately differ between a document and
# a worker global scope, which would contaminate the comparison this harness exists to
# make. Identical source is used in EVERY realm so the only variable is the realm.
PROBE_BODY = r"""
(function () {
  var FNV = function (b) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < b.length; i++) { h ^= (b[i] & 255); h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + (h >>> 0).toString(16)).slice(-8);
  };
  var out = { canvas: null, cores: null, mem: null, origin: null, err: null, has: {} };
  try {
    out.has.OffscreenCanvas = (typeof OffscreenCanvas !== 'undefined');
    out.has.navigator = (typeof navigator !== 'undefined');
    try { out.origin = String(self.location.origin); } catch (e) { out.origin = null; }
    if (out.has.OffscreenCanvas) {
      var c = new OffscreenCanvas(200, 50);
      var x = c.getContext('2d');
      x.fillStyle = '#f60'; x.fillRect(0, 0, 100, 20);
      x.fillStyle = '#069'; x.fillRect(10, 12, 60, 25);
      var g = x.createLinearGradient(0, 0, 200, 50);
      g.addColorStop(0, 'rgba(0,120,255,0.7)');
      g.addColorStop(1, 'rgba(255,0,90,0.35)');
      x.fillStyle = g; x.fillRect(0, 20, 200, 30);
      x.strokeStyle = 'rgba(0,0,0,0.8)'; x.lineWidth = 3;
      x.beginPath(); x.arc(40, 25, 18, 0, Math.PI * 2); x.stroke();
      out.canvas = FNV(x.getImageData(0, 0, 200, 50).data);
    }
    if (out.has.navigator) {
      out.cores = (typeof navigator.hardwareConcurrency === 'number')
                    ? navigator.hardwareConcurrency : null;
      out.mem = (typeof navigator.deviceMemory === 'number')
                    ? navigator.deviceMemory : null;
    }
  } catch (e) { out.err = String((e && e.message) || e); }
  return out;
})
"""

# main thread reference + dedicated worker + shared worker, one evaluate.
DRIVER_JS = r"""
(async function () {
  var SRC = %(probe)s;
  var out = { main: null, dedicated: null, shared: null, errs: {} };

  // ---- main-thread reference (the FARBLED value every realm is judged against) -------
  try { out.main = eval('(' + SRC + ')')(); }
  catch (e) { out.errs.main = String((e && e.message) || e); }

  var runWorker = function (kind) {
    return new Promise(function (resolve) {
      var body, url, w, done = false;
      var finish = function (v) { if (!done) { done = true; resolve(v); } };
      setTimeout(function () { finish({ __timeout: true }); }, 25000);
      try {
        if (kind === 'dedicated') {
          body = 'self.onmessage=function(ev){try{self.postMessage(eval("("+ev.data+")")());}'
               + 'catch(e){self.postMessage({err:String(e&&e.message||e),has:{}});}};';
          url = URL.createObjectURL(new Blob([body], {type: 'text/javascript'}));
          w = new Worker(url);
          w.onmessage = function (e) { finish(e.data); };
          w.onerror = function (e) { finish({ __error: String(e.message || 'worker error') }); };
          w.postMessage(SRC);
        } else {
          // SharedWorkerGlobalScope. Reached over the port, which is also the only way
          // a page can talk to it at all -- so a payload arriving here proves the realm
          // genuinely ran, which kills the "it never started" reading of a native result.
          body = 'self.onconnect=function(ev){var p=ev.ports[0];'
               + 'p.onmessage=function(m){try{p.postMessage(eval("("+m.data+")")());}'
               + 'catch(e){p.postMessage({err:String(e&&e.message||e),has:{}});}};p.start();};';
          url = URL.createObjectURL(new Blob([body], {type: 'text/javascript'}));
          w = new SharedWorker(url);
          w.port.onmessage = function (e) { finish(e.data); };
          w.onerror = function (e) { finish({ __error: String(e.message || 'sharedworker error') }); };
          w.port.start();
          w.port.postMessage(SRC);
        }
      } catch (e) { finish({ __error: String((e && e.message) || e) }); }
    });
  };

  out.dedicated = await runWorker('dedicated');
  try {
    out.shared = (typeof SharedWorker === 'undefined')
      ? { __error: 'SharedWorker is not defined in this build' }
      : await runWorker('shared');
  } catch (e) { out.shared = { __error: String((e && e.message) || e) }; }

  return JSON.stringify(out);
})()
"""


def boot(args, port, pdir, farbling_on):
    kill_browser_by_path(args.exe)
    set_site_enabled(pdir, HOST, farbling_on)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    time.sleep(args.settle)


def navigate(port, excluded, url, want, timeout=90):
    t = resolve_tab(port, excluded)
    _rpc(t["webSocketDebuggerUrl"], "Page.navigate", {"url": url}, msg_id=1)
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(1.5)
        t = resolve_tab(port, excluded)
        if want in t.get("url", ""):
            time.sleep(1.5)
            return True
    return False


def sw_targets(port):
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=10) as r:
            return [t for t in json.load(r) if t.get("type") == "service_worker"]
    except Exception:
        return []


def probe_service_workers(port):
    """R10, opportunistic: evaluate the probe inside any LIVE service-worker target.

    Never synthesises a pass. No target ⇒ NOT MEASURED, with the reason.
    """
    found = []
    for t in sw_targets(port):
        ws_url = t.get("webSocketDebuggerUrl")
        if not ws_url:
            continue
        got = _rpc(ws_url, "Runtime.evaluate",
                   {"expression": "(" + PROBE_BODY.strip() + ")()",
                    "returnByValue": True, "awaitPromise": True}, msg_id=7, wait=45)
        val = ((got or {}).get("result", {}).get("result", {}) or {}).get("value")
        found.append({"url": t.get("url"), "value": val})
    return found


def arm(args, port, pdir, farbling_on, label):
    boot(args, port, pdir, farbling_on)
    excluded = snapshot_targets(port, args.settle)
    if not navigate(port, excluded, URL, HOST):
        raise SystemExit("could not reach %s" % URL)
    t = resolve_tab(port, excluded)
    got = _rpc(t["webSocketDebuggerUrl"], "Runtime.evaluate",
               {"expression": DRIVER_JS % {"probe": json.dumps(PROBE_BODY.strip())},
                "returnByValue": True, "awaitPromise": True, "userGesture": True},
               msg_id=2, wait=180)
    raw = ((got or {}).get("result", {}).get("result", {}) or {}).get("value")
    if not raw:
        raise SystemExit("driver returned nothing in %s" % label)
    data = json.loads(raw)
    data["engine"] = engine_version(port)
    data["optout"] = optout_state(pdir, HOST)
    data["service_workers"] = probe_service_workers(port)
    print("\n  %s: engine=%s  on-disk opt-out for %s=%s"
          % (label, data["engine"], HOST, data["optout"]))
    for k in ("main", "dedicated", "shared"):
        v = data.get(k) or {}
        if v.get("__timeout"):
            print("    %-10s TIMED OUT" % k)
        elif v.get("__error"):
            print("    %-10s ERROR: %s" % (k, v["__error"]))
        else:
            print("    %-10s canvas=%s cores=%s mem=%s origin=%s"
                  % (k, v.get("canvas"), v.get("cores"), v.get("mem"), v.get("origin")))
    return data


def val(d, key):
    v = (d or {}).get(key) or {}
    if v.get("__timeout") or v.get("__error") or v.get("err"):
        return None
    return v.get("canvas")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--negative-control", action="store_true",
                    help="both arms farbling-OFF. main == dedicated == shared == native, "
                         "so the DISCRIMINATOR (dedicated != main-native) must FAIL and "
                         "the run must go void — proving this rig reports the feature's "
                         "absence rather than its presence.")
    ap.add_argument("--expect-cef", default=None,
                    help="REFUSE unless CEF_VERSION contains this (e.g. +g9ccef04).")
    args = ap.parse_args()

    port = cdp_port_for(args.profile, args.dev)
    pdir = profile_dir(args.data_root, args.profile)
    original = optout_state(pdir, HOST)

    print("== farbling worker residual check (§D.1 residual 3: R9/R10) ==")
    require_engine(args.exe, expect=args.expect_cef, label="worker residual subject")
    if args.negative_control:
        print("  ⚠️ NEGATIVE CONTROL: both arms farbling-OFF; the run MUST go void")

    try:
        farbled = arm(args, port, pdir,
                      farbling_on=not args.negative_control,
                      label="arm 1 (farbled)" if not args.negative_control
                            else "arm 1 (NEG-CTL: farbling off)")
        control = arm(args, port, pdir, farbling_on=False, label="arm 2 (control/native)")
    finally:
        try:
            kill_browser_by_path(args.exe)
            set_site_enabled(pdir, HOST, not original.get("optout", False)
                             if isinstance(original, dict) else True)
        except Exception:
            pass

    print("\n" + "=" * 78)
    problems, void = [], []

    if farbled["engine"] != control["engine"]:
        void.append("arms ran on different engines (%s vs %s)"
                    % (farbled["engine"], control["engine"]))
    else:
        print("  [PASS] subject: both arms on %s" % farbled["engine"])

    main_f, main_c = val(farbled, "main"), val(control, "main")
    ded_f = val(farbled, "dedicated")
    shared_f, shared_c = val(farbled, "shared"), val(control, "shared")

    if not main_f or not main_c:
        void.append("no main-thread reference in one of the arms")
    elif main_f == main_c and not args.negative_control:
        void.append("the top frame is NOT farbled in the farbled arm (%s == %s) — "
                    "nothing below can be judged" % (main_f, main_c))
    elif not args.negative_control:
        print("  [PASS] top frame IS farbled: %s (native) -> %s" % (main_c, main_f))

    # ---- THE DISCRIMINATOR ------------------------------------------------------------
    if ded_f is None:
        void.append("the dedicated worker produced no value — the discriminator that "
                    "makes a 'native' shared-worker result mean anything is missing")
    elif args.negative_control:
        if ded_f == main_c:
            print("  [PASS] neg-control: dedicated worker == native (%s); the "
                  "discriminator correctly FAILS with the feature off" % ded_f)
            void.append("negative control: no farbling anywhere, so nothing is judgeable "
                        "(this is the REQUIRED outcome)")
        else:
            problems.append("neg-control: dedicated worker %s != native %s with farbling "
                            "OFF — the rig reports farbling that is not there" % (ded_f, main_c))
    elif ded_f == main_f:
        print("  [PASS] DISCRIMINATOR: dedicated worker carries the top frame's farbled "
              "value (%s) — the probe works, on a build that farbles workers" % ded_f)
    elif ded_f == main_c:
        problems.append("DISCRIMINATOR FAILED: the dedicated worker reads NATIVE (%s). "
                        "R7 is supposed to be CLOSED by P4f — either this is not a P4f "
                        "binary, or R7 has regressed. No R9 conclusion is possible."
                        % ded_f)
    else:
        problems.append("dedicated worker %s matches neither farbled %s nor native %s"
                        % (ded_f, main_f, main_c))

    # ---- the finding itself ------------------------------------------------------------
    r9 = "NOT MEASURED"
    if not args.negative_control and ded_f and ded_f == main_f:
        if shared_f is None:
            sv = (farbled.get("shared") or {})
            r9 = "NOT MEASURED (%s)" % (sv.get("__error") or sv.get("err")
                                        or "timed out / no payload")
            problems.append("R9 could not be measured: %s" % r9)
        elif shared_f == main_c:
            r9 = "UNFARBLED (native) — residual #3 is ACCURATE for shared workers"
            print("  [PASS] R9 shared worker reads NATIVE %s while the dedicated worker "
                  "in the SAME document reads farbled %s" % (shared_f, ded_f))
            if (farbled.get("shared") or {}).get("origin") != "https://example.com":
                problems.append("R9 subject check: shared worker origin is %r, not the "
                                "page's — it may not be the realm we think"
                                % (farbled.get("shared") or {}).get("origin"))
        elif shared_f == main_f:
            r9 = "KEYED — residual #3 is WRONG for shared workers"
            problems.append("R9 shared worker carries the FARBLED value (%s). The "
                            "release-note line says shared workers are not covered; "
                            "measured, they are. The line must change." % shared_f)
        else:
            r9 = "INVESTIGATE"
            problems.append("R9 shared worker %s matches neither farbled %s nor native %s "
                            "— possibly keyed on its OWN origin, the wrong-model outcome"
                            % (shared_f, main_f, main_c))

    print("\n  R9 shared worker : %s" % r9)
    if shared_c is not None and shared_f is not None:
        print("     (control arm shared worker = %s; farbled arm = %s)" % (shared_c, shared_f))

    sws = farbled.get("service_workers") or []
    if not sws:
        print("  R10 service worker: NOT MEASURED — no live service-worker target existed.")
        print("     A SW needs a same-origin secure script URL. The only origin we can")
        print("     serve is localhost, which MaybeApplyHodosFarblingKey skips")
        print("     unconditionally, so a local fixture yields an unfarbled top frame and")
        print("     a VOID comparison. ⛔ Not a pass — R10 stays ⏸️/unmeasured.")
    else:
        for s in sws:
            print("  R10 service worker @ %s -> %s" % (s["url"], s["value"]))

    print()
    for v in void:
        print("  ⚠️ VOID: %s" % v)
    for p in problems:
        print("  ⛔ %s" % p)

    if args.negative_control:
        ok = bool(void) and not problems
        print("\n  RESULT: negative control %s" % ("BEHAVED (run correctly void)" if ok
                                                   else "DID NOT behave"))
        return 0 if ok else 2
    if void:
        return 2
    if problems:
        return 1
    print("\n  RESULT: residual #3 is ACCURATE as written for shared workers;")
    print("          R10 service workers remain unmeasured (see above).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
