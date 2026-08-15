#!/usr/bin/env python3
r"""farbling_worker_probe.py — are IN-PROCESS DEDICATED WORKERS farbled?

Settles the claim in MAC_WINDOWS_RELAY.md ROUND 2026-08-10 (Windows) §5, which was
explicitly flagged as *reasoned from the code, not yet measured*.

MEASURED ANSWER (macOS, fork c63654654, 2026-08-10): **NO — they are unfarbled.**
A dedicated worker returns byte-identical native values for canvas, hardwareConcurrency
and deviceMemory while the main thread of the SAME document is farbled.

## The mechanism this measures

`HodosSessionCache` is a `Supplement<ExecutionContext>`. The only place a key is ever
installed is:

    CefFrameImpl::MaybeApplyHodosFarblingKey()      (frame_impl.cc, at OnContextCreated)
      -> blink_glue::SetHodosFarblingKey(WebLocalFrame*, key32, enabled)
           -> local_frame->DomWindow()              (a LocalDOMWindow)
             -> HodosSessionCache::From(*window).SetOriginKey(...)

A `DedicatedWorkerGlobalScope` is a DIFFERENT ExecutionContext. `From()` on it creates a
fresh Supplement, which by construction has `has_key_ == false`, so `FarblingEnabled()` is
false and every patched API falls through to the native value. There is no worker-start
hook anywhere in libcef.

## Why this needs a control, and why the control is the OPT-OUT and not an exempt origin

Main thread and worker reach canvas by different objects, so a raw main-vs-worker hash
difference is ambiguous: it could be the missing key, or an ordinary rendering difference
between the two paths. Two things remove the ambiguity:

1. **Both sides use OffscreenCanvas**, so the same Blink code path
   (`BaseRenderingContext2D`) renders both and only the ExecutionContext differs.
2. **The control run is the SAME ORIGIN with farbling switched off.** Then neither side is
   farbled, so main and worker MUST agree. If they disagree there, this probe is measuring
   a rendering difference and cannot speak to farbling at all.

⛔ **Do NOT use an auth-exempt origin as the control.** The obvious choice, github.com,
has a Content-Security-Policy that blocks `blob:` workers — the worker never starts, the
control silently yields nothing, and the run is inconclusive. Measured 2026-08-10. The
per-site Privacy Shield opt-out keeps the origin (and its CSP) fixed and changes only the
one variable under test, which is what a control is supposed to do.

No text is drawn: font fallback can legitimately differ in a worker, which would
contaminate the control. Geometry only.

`hardwareConcurrency` / `deviceMemory` come along for free: C6 patches `NavigatorBase`,
the shared base of both `Navigator` and `WorkerNavigator`, so those are farbled in a
worker too *if the key is present*.

## Usage

    # ONE command, both arms, controls asserted (preferred):
    python3 farbling_worker_probe.py --auto \
        --exe  ...\cef-native\build\bin\Release\HodosBrowser.exe \
        --data-root %APPDATA%\HodosBrowserDev --dev

    # or drive the arms yourself against an already-running browser:
    python3 farbling_worker_probe.py --mode farbled --data-root ... [--profile Default]
    python3 farbling_worker_probe.py --mode control --data-root ...   # after opting out
                                                                      # AND restarting

The second arm to complete prints the verdict, comparing against the sidecar next to this
script. `--auto` runs both, restores the profile's original opt-out state afterwards, and
kills only processes under `--exe`'s own directory.

## ⛔ Two subject assertions, added 2026-08-14, and why neither is optional

Both are instances of the failure this project keeps repeating: a harness that produces a
plausible number about the wrong thing.

1. **ENGINE.** The port is just an int. The owner's INSTALLED browser runs from
   `%LOCALAPPDATA%` and holds CDP **9222**; a dev build with `--dev` holds **9322**. One
   wrong flag measures the shipped pre-P4e engine and reports a RED that says nothing
   about the build under test — the same class of defect as the harness that drove an
   OVERLAY and manufactured an intermittent bug. Every arm now records
   `/json/version`'s Browser string, and the verdict REFUSES to compare two arms that
   were not the same engine.

2. **THE OPT-OUT ACTUALLY LANDED.** The control arm's whole meaning is "farbling is off
   for this origin". If the settings edit silently fails -- and it silently fails for the
   most natural hand-edit, a bare `false` instead of `{"enabled": false}` -- then BOTH
   arms are farbled runs, `main == worker` holds in both for the ordinary reason, and the
   probe prints a confident INCONCLUSIVE about the wrong thing. Each arm now reads
   `fingerprint_settings.json` back OFF DISK after the browser is up and records what it
   found; the verdict refuses if the recorded state contradicts the arm's declared mode.

Neither assertion can turn a red into a green. They only ever refuse to render a verdict,
which is the correct direction for a control.
"""

import argparse
import json
import os
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import websocket  # websocket-client
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

# Reuse the proven implementations rather than growing a second copy of each: these are
# the ones the release gate runs, and every one of them carries a comment about a specific
# way its naive version was wrong (path-matching kills, the {"enabled": false} object
# shape, the profile-derived CDP port).
from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    require_engine,
    kill_browser_by_path,
    launch_browser,
    read_settings,
    set_site_enabled,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402


TARGET_URL = "https://example.com/"
TARGET_HOST = "example.com"
SIDECAR = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                       ".farbling_worker_probe_state.json")

VECTORS = ("canvas", "cores", "mem")


# Drawn identically on both sides. 200x50 = 10,000 px, inside the <65536px canvas gate,
# so it is farbled on a farbled origin. Geometry only -- no text, no fonts.
PROBE_JS = r"""
(async function () {
  var DRAW = function (canvas) {
    var x = canvas.getContext('2d');
    x.fillStyle = '#f60'; x.fillRect(0, 0, 100, 20);
    x.fillStyle = '#069'; x.fillRect(10, 12, 60, 25);
    var g = x.createLinearGradient(0, 0, 200, 50);
    g.addColorStop(0, 'rgba(0,120,255,0.7)');
    g.addColorStop(1, 'rgba(255,0,90,0.35)');
    x.fillStyle = g; x.fillRect(0, 20, 200, 30);
    x.strokeStyle = 'rgba(0,0,0,0.8)'; x.lineWidth = 3;
    x.beginPath(); x.arc(40, 25, 18, 0, Math.PI * 2); x.stroke();
    return x.getImageData(0, 0, canvas.width, canvas.height).data;
  };
  var FNV = function (bytes) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < bytes.length; i++) { h ^= bytes[i]; h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + (h >>> 0).toString(16)).slice(-8);
  };

  var mainHash = null, mainErr = null;
  try {
    mainHash = FNV(DRAW(new OffscreenCanvas(200, 50)));
  } catch (e) { mainErr = String(e && e.message || e); }

  // Blob URL -> same-origin, IN-PROCESS dedicated worker. This is precisely the case the
  // relay says is unfarbled, as distinct from the OOP shared/service workers of P4e.
  var src = '(' + (function () {
    self.onmessage = function (ev) {
      var DRAW = eval('(' + ev.data.draw + ')');
      var FNV = eval('(' + ev.data.fnv + ')');
      var out = { hash: null, err: null, cores: null, mem: null };
      try { out.hash = FNV(DRAW(new OffscreenCanvas(200, 50))); }
      catch (e) { out.err = String(e && e.message || e); }
      try {
        out.cores = (typeof navigator.hardwareConcurrency === 'number')
          ? navigator.hardwareConcurrency : null;
        out.mem = (typeof navigator.deviceMemory === 'number')
          ? navigator.deviceMemory : null;
      } catch (e) {}
      self.postMessage(out);
    };
  }).toString() + ')()';

  var wr = await new Promise(function (resolve) {
    var w, done = false;
    var finish = function (v) {
      if (!done) { done = true; try { w && w.terminate(); } catch (e) {} resolve(v); }
    };
    setTimeout(function () { finish({ hash: null, err: 'timeout', cores: null, mem: null }); }, 15000);
    try {
      var url = URL.createObjectURL(new Blob([src], { type: 'text/javascript' }));
      w = new Worker(url);
      w.onmessage = function (ev) { finish(ev.data); };
      // A CSP that forbids blob: workers lands here with an essentially empty event --
      // that is what rules out an auth-exempt origin as the control page.
      w.onerror = function (ev) {
        finish({ hash: null, err: 'workererror (CSP blocking blob: worker?): '
                 + (ev && ev.message || 'no message'), cores: null, mem: null });
      };
      w.postMessage({ draw: DRAW.toString(), fnv: FNV.toString() });
    } catch (e) {
      finish({ hash: null, err: 'spawn: ' + String(e && e.message || e), cores: null, mem: null });
    }
  });

  return JSON.stringify({
    href: location.href,
    main: { canvas: mainHash, err: mainErr,
            cores: (typeof navigator.hardwareConcurrency === 'number')
                     ? navigator.hardwareConcurrency : null,
            mem: (typeof navigator.deviceMemory === 'number') ? navigator.deviceMemory : null },
    worker: { canvas: wr.hash, err: wr.err, cores: wr.cores, mem: wr.mem }
  });
})()
"""


def page_targets(port):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=10) as r:
        return [t for t in json.load(r) if t.get("type") == "page"]


def snapshot_targets(port, settle):
    """Same subject discipline as farbling_seed_rotation_check.py — three harnesses died
    by driving an overlay. Pin exactly one tab and exclude everything else."""
    time.sleep(settle)
    chrome, tabs = set(), []
    for t in page_targets(port):
        url = t.get("url", "")
        if "127.0.0.1:5137" in url and "/newtab" not in url:
            chrome.add(t["id"])
        else:
            tabs.append(t)
    if not tabs:
        raise SystemExit("browser came up with no tab target at all")
    pinned = next((t for t in tabs if "/newtab" in t.get("url", "")), tabs[0])
    excluded = set(chrome) | {t["id"] for t in tabs if t["id"] != pinned["id"]}
    print(f"    excluded {len(chrome)} chrome target(s) + {len(tabs) - 1} extra tab(s)")
    return excluded


def resolve_tab(port, excluded_ids):
    cands = [t for t in page_targets(port) if t["id"] not in excluded_ids]
    if not cands:
        raise SystemExit("no candidate page target -- the pinned tab disappeared")
    if len(cands) == 1:
        return cands[0]
    raise SystemExit("ambiguous tab target among %d candidates (%s)."
                     % (len(cands), ", ".join(t.get("url", "?")[:60] for t in cands)))


def measure(port, chrome_ids, url, want_host, timeout=90):
    t = resolve_tab(port, chrome_ids)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": url}}))
    finally:
        ws.close()

    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        try:
            t = resolve_tab(port, chrome_ids)
            if want_host not in t.get("url", ""):
                continue
            ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=40)
            try:
                ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                                    "params": {"expression": PROBE_JS,
                                               "returnByValue": True,
                                               "awaitPromise": True}}))
                got, end = None, time.time() + 40
                while time.time() < end:
                    m = json.loads(ws.recv())
                    if m.get("id") == 2:
                        got = m
                        break
            finally:
                ws.close()
            if not got or "value" not in got.get("result", {}).get("result", {}):
                continue
            val = json.loads(got["result"]["result"]["value"])
            if want_host not in val["href"]:
                continue
            return val
        except SystemExit:
            raise
        except Exception:
            continue
    raise SystemExit(f"could not measure {url} within {timeout}s")


def optout_state(pdir, host):
    """Read the on-disk Privacy Shield state for `host`. True == farbling OPTED OUT.

    Deliberately mirrors the loader's shape check rather than trusting the key's presence:
    a bare `false` is silently ignored by the browser and leaves farbling ON, so a probe
    that merely saw `host in siteSettings` would report an opt-out that does not exist.
    Returns None when the file is unreadable, which is distinct from "present and on".
    """
    doc = read_settings(pdir)
    if not doc:
        return None
    sites = doc.get("siteSettings")
    if not isinstance(sites, dict):
        return False
    entry = sites.get(host)
    if isinstance(entry, dict):
        return entry.get("enabled") is False
    return False


def show(mode, v):
    print(f"\n  {mode.upper()} run  ({v['href'][:50]})")
    print(f"    engine={v.get('engine')}   on-disk opt-out for {TARGET_HOST}="
          f"{v.get('optout')}")
    for side in ("main", "worker"):
        s = v[side]
        line = (f"    {side:<6} canvas={s['canvas']}  cores={s['cores']}  mem={s['mem']}")
        if s.get("err"):
            line += f"   ERR={s['err']}"
        print(line)


def verdict(farbled, control):
    print("\n== verdict ==")

    # --- subject assertions, BEFORE any value is compared ------------------------------
    # These run first on purpose. Everything below is a comparison of two numbers, and a
    # comparison of two numbers from the wrong browser is worse than no number at all.
    eng_f, eng_c = farbled.get("engine"), control.get("engine")
    if eng_f != eng_c:
        print(f"  REFUSED — the two arms ran against DIFFERENT engines:")
        print(f"    farbled: {eng_f}")
        print(f"    control: {eng_c}")
        print("  Almost always the installed browser on 9222 vs the dev build on 9322.")
        return 2
    if not eng_f or eng_f == "unknown":
        print("  REFUSED — could not read the engine version; subject unverified.")
        return 2
    print(f"  [PASS] subject: both arms on {eng_f}")

    if farbled.get("optout") is not False:
        print(f"  REFUSED — the 'farbled' arm found on-disk opt-out="
              f"{farbled.get('optout')} for {TARGET_HOST}; it must be False.")
        print("  That arm was not a farbled run, whatever it measured.")
        return 2
    if control.get("optout") is not True:
        print(f"  REFUSED — the 'control' arm found on-disk opt-out="
              f"{control.get('optout')} for {TARGET_HOST}; it must be True.")
        print("  The opt-out never landed (a bare `false` instead of {\"enabled\": false}")
        print("  is silently ignored), so this 'control' is a second farbled run and")
        print("  main==worker in it would prove nothing.")
        return 2
    print(f"  [PASS] control really is off / farbled really is on for {TARGET_HOST}")

    for name, v in (("control", control), ("farbled", farbled)):
        for side in ("main", "worker"):
            if v[side]["canvas"] is None:
                print(f"  INCONCLUSIVE — {name} run's {side} canvas read failed: "
                      f"{v[side].get('err')}")
                return 2

    # 1. Control: with farbling off, the two paths must agree, or the probe is measuring
    #    a rendering difference rather than a farbling gap.
    if control["main"]["canvas"] != control["worker"]["canvas"]:
        print(f"  INCONCLUSIVE — CONTROL FAILED: with farbling off, main "
              f"({control['main']['canvas']}) != worker ({control['worker']['canvas']}).")
        print("  The two OffscreenCanvas paths do not render identically, so nothing")
        print("  below can distinguish a missing key from that difference.")
        return 2
    native = control["main"]["canvas"]
    print(f"  [PASS] control: farbling OFF -> main == worker == {native} "
          f"(the two paths agree, so a difference means farbling)")

    # 2. The main thread must actually be farbled, or there is nothing to compare against.
    if farbled["main"]["canvas"] == native:
        print("  INCONCLUSIVE — the main thread is NOT farbled in the farbled run "
              "(feature off, or this build lacks C3).")
        return 2
    print(f"  [PASS] main thread is farbled: {native} (native) -> "
          f"{farbled['main']['canvas']} (farbled)")

    # 3. The question.
    if farbled["worker"]["canvas"] == farbled["main"]["canvas"]:
        print("\n  RESULT: worker IS farbled — it shares the document's key.")
        print("  Relay §5's reading does NOT hold; in-process workers are covered.")
        rc = 0
    elif farbled["worker"]["canvas"] == native:
        print("\n  RESULT: worker is NOT farbled — it returns the native value.")
        print("  Confirms relay §5: a DedicatedWorkerGlobalScope gets a key-less")
        print("  Supplement and fails closed to native.")
        rc = 1
    else:
        print("\n  UNEXPECTED: the worker hash matches neither the farbled main-thread")
        print("  value nor the native value. Investigate before reporting.")
        rc = 2

    print("\n  navigator (C6 patches NavigatorBase, shared by Navigator and WorkerNavigator):")
    print(f"    control  main=({control['main']['mem']}, {control['main']['cores']})   "
          f"worker=({control['worker']['mem']}, {control['worker']['cores']})")
    print(f"    farbled  main=({farbled['main']['mem']}, {farbled['main']['cores']})   "
          f"worker=({farbled['worker']['mem']}, {farbled['worker']['cores']})")
    if (farbled["worker"]["cores"] == control["worker"]["cores"]
            and farbled["worker"]["mem"] == control["worker"]["mem"]
            and (farbled["main"]["cores"], farbled["main"]["mem"])
                != (control["main"]["cores"], control["main"]["mem"])):
        print("    -> the worker's navigator values are native while the main thread's")
        print("       are farbled: the same finding, on a second vector.")
    return rc


def run_arm(mode, port, pdir, settle):
    """One arm against an already-running browser. Stamps the two subject facts."""
    print(f"\n== farbling worker probe ({mode} run) ==")
    chrome_ids = snapshot_targets(port, settle)
    v = measure(port, chrome_ids, TARGET_URL, TARGET_HOST)
    # Read the settings back AFTER the browser is up: what matters is the state the
    # running browser loaded, not what we believe we wrote.
    v["engine"] = engine_version(port)
    v["optout"] = optout_state(pdir, TARGET_HOST) if pdir else None
    show(mode, v)
    return v


def load_state():
    if os.path.exists(SIDECAR):
        try:
            return json.load(open(SIDECAR))
        except Exception:
            pass
    return {}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=("farbled", "control"),
                    help="'control' means example.com is CURRENTLY opted out of Privacy "
                         "Shield and the browser has been restarted since.")
    ap.add_argument("--auto", action="store_true",
                    help="run BOTH arms end to end (needs --exe); restores the profile's "
                         "original opt-out state afterwards.")
    ap.add_argument("--exe", help="required by --auto; also selects what may be killed")
    ap.add_argument("--data-root",
                    help="e.g. %%APPDATA%%\\HodosBrowserDev — needed for the opt-out "
                         "assertion, and required by --auto")
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--port", type=int, default=None,
                    help="default: derived from --profile/--dev (Default+dev = 9322)")
    ap.add_argument("--settle", type=float, default=6.0)
    ap.add_argument("--expect-cef", default=None,
                    help="REFUSE to run unless CEF_VERSION contains this (e.g. +g9ccef04). "
                         "engine_version() cannot tell P4e from P4f — both are Chromium "
                         "150.0.7871.187 — so this is the only subject assertion that "
                         "identifies WHICH build was measured.")
    args = ap.parse_args()

    if not args.auto and not args.mode:
        ap.error("one of --mode or --auto is required")

    if args.exe:
        require_engine(args.exe, expect=args.expect_cef, label="worker probe subject")
    elif args.expect_cef:
        raise SystemExit("--expect-cef needs --exe to tie the header to the loaded binary")

    port = args.port if args.port is not None else cdp_port_for(args.profile, args.dev)
    pdir = profile_dir(args.data_root, args.profile) if args.data_root else None

    if not args.auto:
        if pdir is None:
            print("  ⚠️ no --data-root: the opt-out assertion cannot run and the verdict")
            print("     will REFUSE. Pass --data-root, or use --auto.")
        state = load_state()
        state[args.mode] = run_arm(args.mode, port, pdir, args.settle)
        json.dump(state, open(SIDECAR, "w"), indent=2)
        other = "control" if args.mode == "farbled" else "farbled"
        if other not in state:
            print(f"\n  saved. Now do the '{other}' run ("
                  f"{'opt example.com out and restart' if other == 'control' else 'remove the opt-out and restart'}).")
            return 3
        return verdict(state["farbled"], state["control"])

    # --- auto: both arms, one command -------------------------------------------------
    if not args.exe or not args.data_root:
        ap.error("--auto needs --exe and --data-root")

    # Remember the caller's original state so a probe run never leaves a profile altered.
    original_optout = optout_state(pdir, TARGET_HOST)
    state = {}
    try:
        for mode, want_optout in (("farbled", False), ("control", True)):
            kill_browser_by_path(args.exe)
            set_site_enabled(pdir, TARGET_HOST, not want_optout)
            for attempt in range(1, 4):
                if attempt > 1:
                    kill_browser_by_path(args.exe)
                launch_browser(args.exe, args.dev, args.profile)
                if wait_for_cdp(port):
                    break
            else:
                raise SystemExit(f"CDP {port} never came up")
            state[mode] = run_arm(mode, port, pdir, args.settle)
        json.dump(state, open(SIDECAR, "w"), indent=2)
        return verdict(state["farbled"], state["control"])
    finally:
        kill_browser_by_path(args.exe)
        if original_optout is not None:
            set_site_enabled(pdir, TARGET_HOST, not original_optout)


if __name__ == "__main__":
    sys.exit(main())
