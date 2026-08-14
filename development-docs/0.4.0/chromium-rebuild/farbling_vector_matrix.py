#!/usr/bin/env python3
r"""farbling_vector_matrix.py — which §B VECTORS are actually perturbed?

Settles every ❓ and every CODE-READ row in FARBLING_DEFINITION_OF_DONE.md §B, on the
main thread of one farbled document, in a single browser session.

## The method, and the one thing it must not do

For each named vector V we hash V's output twice in a **farbled** arm and twice in a
**control** arm (same origin, Privacy Shield opted out ⇒ farbling off ⇒ native), then:

    farbled(V) != control(V)   ⇒  FARBLED   (the hook is live)
    farbled(V) == control(V)   ⇒  NATIVE    (nothing perturbs this path)

⛔ **The obvious version of this test is worthless, and here is the trap it falls into.**
"farbled == control" is ALSO what you get from a vector whose output is degenerate --
an all-zero PCM buffer, an analyser array of all -Infinity, a blank canvas. Multiplying
-Infinity by 1.0000002 is -Infinity, so a perfectly live hook reports NATIVE and we would
"discover" a gap that does not exist and patch code that was already correct. Three
controls stop that:

  * **DEGENERACY.** Every vector reports whether its own raw output actually varies
    (`nondegenerate`). A vector that is constant carries no fingerprint entropy, so a
    verdict on it is meaningless in EITHER direction and is reported as DEGENERATE, never
    as NATIVE.
  * **NOISE.** Every vector is measured twice per arm. If the two disagree, the vector is
    NOISY and gets no verdict -- a cross-arm difference would otherwise be indistinguishable
    from the instrument's own jitter. This is why the audio graph is OFFLINE: a realtime
    AudioContext is nondeterministic by construction and would make every audio row noise.
  * **POSITIVE + SIZE-GATE.** `getImageData` (known hooked) must come out FARBLED or the
    whole run is void -- that is the negative control for the harness itself, since a run
    with the feature simply off would report NATIVE for everything and look like a
    catastrophic discovery. `canvasLarge` (>65536 px, above the deliberate size gate) must
    come out NATIVE, proving "differs" is not something this rig produces for everything.

## Usage

    python3 farbling_vector_matrix.py --dev \
        --exe  ...\cef-native\build\bin\Release\HodosBrowser.exe \
        --data-root %APPDATA%\HodosBrowserDev

    # negative control: assert the rig goes RED when farbling is off in BOTH arms
    python3 farbling_vector_matrix.py ... --negative-control

Exit 0 = every vector accounted for and the two harness controls held.
Exit 1 = at least one vector is NATIVE (an unhooked surface).
Exit 2 = the run is void (a control failed, or a vector was noisy/degenerate/errored).
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# The Windows console is cp1252 by default and dies on the status glyphs mid-report --
# after the measurement, so the run is wasted rather than wrong. Degrade the glyphs
# instead of the run.
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
    kill_browser_by_path,
    launch_browser,
    read_settings,
    set_site_enabled,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402
from farbling_worker_probe import (  # noqa: E402
    optout_state,
    resolve_tab,
    snapshot_targets,
)

HOST = "example.com"
URL = "https://example.com/"

# ⛔ THE SMALL-CODOMAIN TRAP, and why these extra hosts exist.
#
# `deviceMemory` is farbled by DRAWING from {4, 8, 16, 32}. On a machine whose native
# value is 32, one draw in four returns 32 -- and the farbled value then equals the native
# value for a perfectly live hook. The first run of this harness hit exactly that and
# reported navigator.deviceMemory as an UNHOOKED VECTOR, which would have sent a build
# after a bug that does not exist.
#
# A hash-valued vector cannot do this: its codomain is 2^32 and a collision is not a
# thing that happens. Only the small-codomain SCALARS need the discriminator, so only
# they get one: the key is HMAC(profile_seed, registrable_domain), so measuring the same
# scalar on additional REGISTRABLE DOMAINS re-draws it independently. Agreement with
# native on one domain is a coin flip; agreement across four is 1 in 256.
#
# ⚠️ Distinct registrable domains, NOT subdomains -- example.com and www.example.com
# reduce to the same key and would re-draw nothing, turning this control into theatre.
COLLISION_HOSTS = ["example.org", "example.net", "iana.org"]
SCALAR_ROWS = ("deviceMemory", "cores")

# Rows are (name, §B label, expectation). The expectation is DOCUMENTATION of the prior,
# never an input to the verdict -- a row is judged purely on its measurement. It is
# printed beside the result so a surprise is visible rather than absorbed.
ROWS = [
    ("getImageData",        "Canvas 2D getImageData",              "FARBLED"),
    ("toDataURL",           "Canvas toDataURL",                    "FARBLED"),
    ("toBlob",              "Canvas toBlob (CODE-READ, test owed)", "FARBLED"),
    ("convertToBlob",       "OffscreenCanvas.convertToBlob (E3)",  "NATIVE"),
    ("readPixels",          "WebGL readPixels",                    "FARBLED"),
    ("getChannelData",      "AudioBuffer.getChannelData",          "FARBLED"),
    ("floatFreq",           "AnalyserNode.getFloatFrequencyData",  "FARBLED"),
    ("byteFreq",            "AnalyserNode.getByteFrequencyData (E4)",     "NATIVE"),
    ("floatTime",           "AnalyserNode.getFloatTimeDomainData (E4)",   "NATIVE"),
    ("byteTime",            "AnalyserNode.getByteTimeDomainData (E4)",    "NATIVE"),
    ("deviceMemory",        "navigator.deviceMemory",              "FARBLED"),
    ("cores",               "navigator.hardwareConcurrency",       "FARBLED"),
]

# Controls are judged, not merely printed.
POSITIVE_CONTROL = "getImageData"   # must be FARBLED or the run is void
SIZE_GATE_CONTROL = "canvasLarge"   # must be NATIVE in a valid run

PROBE_JS = r"""
(async function () {
  // ---- helpers ---------------------------------------------------------------------
  var FNV = function (bytes) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < bytes.length; i++) { h ^= (bytes[i] & 255); h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + (h >>> 0).toString(16)).slice(-8);
  };
  var FNVs = function (str) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < str.length; i++) { h ^= str.charCodeAt(i) & 255; h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + (h >>> 0).toString(16)).slice(-8);
  };
  // "Does this output actually carry entropy?" -- see the DEGENERACY control. A buffer
  // whose every element is identical cannot be shown to be farbled or native by ANY
  // comparison, so we must know before we interpret a match.
  var varies = function (arr) {
    if (!arr || arr.length < 2) return false;
    var first = arr[0];
    for (var i = 1; i < arr.length; i++) { if (arr[i] !== first) return true; }
    return false;
  };
  var DRAW = function (x, w, h) {
    x.fillStyle = '#f60'; x.fillRect(0, 0, w / 2, h / 3);
    x.fillStyle = '#069'; x.fillRect(10, 12, 60, 25);
    var g = x.createLinearGradient(0, 0, w, h);
    g.addColorStop(0, 'rgba(0,120,255,0.7)');
    g.addColorStop(1, 'rgba(255,0,90,0.35)');
    x.fillStyle = g; x.fillRect(0, h / 3, w, h - h / 3);
    x.strokeStyle = 'rgba(0,0,0,0.8)'; x.lineWidth = 3;
    x.beginPath(); x.arc(40, 25, 18, 0, Math.PI * 2); x.stroke();
  };
  // No text anywhere: font fallback can legitimately differ between realms and would
  // contaminate a comparison that is supposed to isolate farbling.
  var mkCanvas = function (w, h) {
    var c = document.createElement('canvas'); c.width = w; c.height = h;
    DRAW(c.getContext('2d'), w, h); return c;
  };
  var blobHash = async function (blob) {
    if (!blob) return null;
    var buf = await blob.arrayBuffer();
    return FNV(new Uint8Array(buf));
  };

  var out = {};
  var rec = function (name, hash, nondegenerate, err) {
    out[name] = { hash: hash === undefined ? null : hash,
                  nondegenerate: !!nondegenerate,
                  err: err ? String(err && err.message || err) : null };
  };
  var T = async function (name, fn) {
    try { await fn(); } catch (e) { rec(name, null, false, e); }
  };

  // ---- canvas ------------------------------------------------------------------------
  await T('getImageData', function () {
    var c = mkCanvas(200, 50);
    var d = c.getContext('2d').getImageData(0, 0, 200, 50).data;
    rec('getImageData', FNV(d), varies(d));
  });

  // Above the 65536-px size gate, so this must stay NATIVE in a valid run. It is the
  // proof that this rig does not simply report "differs" for everything.
  await T('canvasLarge', function () {
    var c = mkCanvas(400, 400);
    var d = c.getContext('2d').getImageData(0, 0, 400, 400).data;
    rec('canvasLarge', FNV(d), varies(d));
  });

  await T('toDataURL', function () {
    var s = mkCanvas(200, 50).toDataURL();
    rec('toDataURL', FNVs(s), s.length > 64);
  });

  await T('toBlob', async function () {
    var c = mkCanvas(200, 50);
    var blob = await new Promise(function (res) { c.toBlob(res); });
    var h = await blobHash(blob);
    rec('toBlob', h, !!(blob && blob.size > 64));
  });

  // E3. OffscreenCanvas exists on the main thread too, so this endpoint is measurable
  // here and does not have to wait for worker support to land.
  await T('convertToBlob', async function () {
    var oc = new OffscreenCanvas(200, 50);
    DRAW(oc.getContext('2d'), 200, 50);
    var blob = await oc.convertToBlob();
    var h = await blobHash(blob);
    rec('convertToBlob', h, !!(blob && blob.size > 64));
  });

  // ---- webgl -------------------------------------------------------------------------
  await T('readPixels', function () {
    var c = document.createElement('canvas'); c.width = 120; c.height = 60;
    var gl = c.getContext('webgl2') || c.getContext('webgl');
    if (!gl) { rec('readPixels', null, false, 'no webgl context'); return; }
    gl.clearColor(0.25, 0.5, 0.75, 1.0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.enable(gl.SCISSOR_TEST); gl.scissor(10, 10, 40, 20);
    gl.clearColor(0.9, 0.1, 0.4, 1.0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.disable(gl.SCISSOR_TEST);
    var px = new Uint8Array(120 * 60 * 4);
    gl.readPixels(0, 0, 120, 60, gl.RGBA, gl.UNSIGNED_BYTE, px);
    rec('readPixels', FNV(px), varies(px));
  });

  // ---- audio -------------------------------------------------------------------------
  // OFFLINE, deliberately. A realtime AudioContext renders a different number of frames
  // every run, so every audio row would come out NOISY and none of them would get a
  // verdict. Offline rendering is deterministic, which is the only reason cross-arm
  // comparison means anything here.
  var mkGraph = function () {
    var ctx = new OfflineAudioContext(1, 44100, 44100);
    var osc = ctx.createOscillator();
    osc.type = 'triangle'; osc.frequency.value = 10000;
    var comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -50; comp.knee.value = 40; comp.ratio.value = 12;
    comp.attack.value = 0; comp.release.value = 0.25;
    osc.connect(comp);
    var an = ctx.createAnalyser();
    an.fftSize = 2048;
    // smoothing carries state across blocks; zero it so the final read depends only on
    // the last block and not on how many blocks the renderer chose to run.
    an.smoothingTimeConstant = 0;
    comp.connect(an);
    an.connect(ctx.destination);
    osc.start(0);
    return { ctx: ctx, an: an };
  };

  await T('getChannelData', async function () {
    var g = mkGraph();
    var buf = await g.ctx.startRendering();
    var ch = buf.getChannelData(0);
    // Hash the float bits, not a rounded decimal: the perturbation is ~1e-7 relative and
    // any lossy stringification would erase exactly the thing under test.
    rec('getChannelData', FNV(new Uint8Array(ch.buffer, ch.byteOffset, ch.byteLength)),
        varies(ch));
  });

  var analyserRead = async function (name, kind) {
    var g = mkGraph();
    await g.ctx.startRendering();
    var arr, bytes;
    if (kind === 'floatFreq') { arr = new Float32Array(g.an.frequencyBinCount); g.an.getFloatFrequencyData(arr); }
    else if (kind === 'byteFreq') { arr = new Uint8Array(g.an.frequencyBinCount); g.an.getByteFrequencyData(arr); }
    else if (kind === 'floatTime') { arr = new Float32Array(g.an.fftSize); g.an.getFloatTimeDomainData(arr); }
    else { arr = new Uint8Array(g.an.fftSize); g.an.getByteTimeDomainData(arr); }
    bytes = new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);
    // -Infinity fills are the classic degenerate analyser result; varies() catches them
    // along with all-zero byte fills.
    rec(name, FNV(bytes), varies(arr));
  };
  await T('floatFreq', function () { return analyserRead('floatFreq', 'floatFreq'); });
  await T('byteFreq',  function () { return analyserRead('byteFreq', 'byteFreq'); });
  await T('floatTime', function () { return analyserRead('floatTime', 'floatTime'); });
  await T('byteTime',  function () { return analyserRead('byteTime', 'byteTime'); });

  // ---- navigator ---------------------------------------------------------------------
  // Scalars, so "varies" is meaningless; they are nondegenerate by construction as long
  // as they are numbers at all.
  await T('deviceMemory', function () {
    var v = navigator.deviceMemory;
    rec('deviceMemory', typeof v === 'number' ? String(v) : null, typeof v === 'number');
  });
  await T('cores', function () {
    var v = navigator.hardwareConcurrency;
    rec('cores', typeof v === 'number' ? String(v) : null, typeof v === 'number');
  });

  return JSON.stringify({ href: location.href, vectors: out });
})()
"""


def evaluate(port, chrome_ids, navigate, timeout=120):
    """Navigate if asked, then run PROBE_JS in the pinned tab and return the parsed value."""
    if navigate:
        t = resolve_tab(port, chrome_ids)
        ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
        try:
            ws.send(json.dumps({"id": 1, "method": "Page.navigate",
                                "params": {"url": URL}}))
        finally:
            ws.close()

    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        try:
            t = resolve_tab(port, chrome_ids)
            if HOST not in t.get("url", ""):
                continue
            ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=90)
            try:
                ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                                    "params": {"expression": PROBE_JS,
                                               "returnByValue": True,
                                               "awaitPromise": True}}))
                got, end = None, time.time() + 90
                while time.time() < end:
                    m = json.loads(ws.recv())
                    if m.get("id") == 2:
                        got = m
                        break
            finally:
                ws.close()
            if not got:
                continue
            res = got.get("result", {}).get("result", {})
            if "value" not in res:
                continue
            val = json.loads(res["value"])
            if HOST not in val["href"]:
                continue
            return val
        except SystemExit:
            raise
        except Exception:
            continue
    raise SystemExit(f"could not measure {URL} within {timeout}s")


SCALARS_JS = r"""
(function () {
  return JSON.stringify({
    href: location.href,
    deviceMemory: (typeof navigator.deviceMemory === 'number')
                    ? String(navigator.deviceMemory) : null,
    cores: (typeof navigator.hardwareConcurrency === 'number')
                    ? String(navigator.hardwareConcurrency) : null
  });
})()
"""


def measure_scalars(port, chrome_ids, host, timeout=90):
    """Read just the scalar vectors on `host`. Used only by the collision discriminator."""
    url = f"https://{host}/"
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
            if host not in t.get("url", ""):
                continue
            ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=40)
            try:
                ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                                    "params": {"expression": SCALARS_JS,
                                               "returnByValue": True}}))
                got, end = None, time.time() + 40
                while time.time() < end:
                    m = json.loads(ws.recv())
                    if m.get("id") == 2:
                        got = m
                        break
            finally:
                ws.close()
            if not got:
                continue
            res = got.get("result", {}).get("result", {})
            if "value" not in res:
                continue
            val = json.loads(res["value"])
            # Subject assertion: a redirect or an error page would otherwise be recorded
            # as this host's draw, and a wrong host is a wrong key.
            if host not in val.get("href", ""):
                continue
            return val
        except SystemExit:
            raise
        except Exception:
            continue
    return None


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
        raise SystemExit(f"CDP {port} never came up")
    time.sleep(args.settle)


def arm(args, port, pdir, farbling_on, label, collision_hosts=()):
    """One arm = boot + TWO evaluations, so intra-arm noise is visible before any
    cross-arm comparison is attempted."""
    boot(args, port, pdir, farbling_on)
    chrome_ids = snapshot_targets(port, args.settle)
    first = evaluate(port, chrome_ids, navigate=True)
    second = evaluate(port, chrome_ids, navigate=False)
    draws = {}
    for host in collision_hosts:
        got = measure_scalars(port, chrome_ids, host)
        if got:
            draws[host] = {k: got.get(k) for k in SCALAR_ROWS}
    eng = engine_version(port)
    oo = optout_state(pdir, HOST)
    print(f"\n  {label}: engine={eng}  on-disk opt-out for {HOST}={oo}")
    return {"a": first["vectors"], "b": second["vectors"], "engine": eng, "optout": oo,
            "draws": draws}


def collision_verdict(name, farbled, control):
    """For a small-codomain scalar that measured NATIVE on HOST: was that a genuine
    unhooked path, or one unlucky draw? Returns (resolved_state, note)."""
    native = (control["a"].get(name) or {}).get("hash")
    draws = farbled.get("draws") or {}
    seen = {host: vals.get(name) for host, vals in draws.items()}
    if not seen:
        return "NATIVE", "no other-domain draws collected; collision NOT ruled out"
    differing = [h for h, v in seen.items() if v is not None and v != native]
    shown = ", ".join(f"{h}={v}" for h, v in sorted(seen.items()))
    if differing:
        return ("FARBLED",
                f"collision on {HOST} only; native={native}, other domains: {shown}")
    return ("NATIVE",
            f"native={native} on {HOST} AND on every other domain ({shown})")




ALL_NAMES = [r[0] for r in ROWS] + [SIZE_GATE_CONTROL]


def classify(name, farbled, control):
    """One vector's verdict. Returns (state, detail). Never guesses."""
    fa, fb = farbled["a"].get(name), farbled["b"].get(name)
    ca, cb = control["a"].get(name), control["b"].get(name)
    if not fa or not ca:
        return "ERROR", "vector missing from a run"
    for tag, v in (("farbled", fa), ("control", ca)):
        if v.get("err"):
            return "ERROR", f"{tag}: {v['err']}"
        if v.get("hash") is None:
            return "ERROR", f"{tag}: no value"
    # NOISE first: without this a jittery vector's cross-arm difference reads as FARBLED.
    if fa["hash"] != fb["hash"]:
        return "NOISY", f"farbled arm differs run-to-run ({fa['hash']} vs {fb['hash']})"
    if ca["hash"] != cb["hash"]:
        return "NOISY", f"control arm differs run-to-run ({ca['hash']} vs {cb['hash']})"
    # DEGENERACY next: a constant output cannot be judged in either direction.
    if not (fa["nondegenerate"] and ca["nondegenerate"]):
        return "DEGENERATE", "output carries no entropy; no verdict is possible"
    if fa["hash"] != ca["hash"]:
        return "FARBLED", f"{ca['hash']} (native) -> {fa['hash']}"
    return "NATIVE", f"{fa['hash']} in both arms — nothing perturbs this path"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--negative-control", action="store_true",
                    help="run BOTH arms with farbling OFF. Every row must then come out "
                         "NATIVE and the positive control must FAIL — proving this "
                         "harness reports the feature's absence rather than its presence.")
    args = ap.parse_args()

    port = cdp_port_for(args.profile, args.dev)
    pdir = profile_dir(args.data_root, args.profile)
    original = optout_state(pdir, HOST)

    print("== farbling vector matrix (§B) ==")
    if args.negative_control:
        print("  ⚠️ NEGATIVE CONTROL: both arms farbling-OFF; expecting the run to go RED")

    try:
        farbled = arm(args, port, pdir, farbling_on=not args.negative_control,
                      label="arm 1 (farbled)" if not args.negative_control
                            else "arm 1 (NEG-CTL: farbling off)",
                      collision_hosts=COLLISION_HOSTS)
        control = arm(args, port, pdir, farbling_on=False, label="arm 2 (control/native)")
    finally:
        kill_browser_by_path(args.exe)
        if original is not None:
            set_site_enabled(pdir, HOST, not original)

    print("\n" + "=" * 78)
    if farbled["engine"] != control["engine"]:
        print(f"  REFUSED — arms on different engines: {farbled['engine']} vs "
              f"{control['engine']}")
        return 2
    print(f"  [PASS] subject: both arms on {farbled['engine']}")

    results = {n: classify(n, farbled, control) for n in ALL_NAMES}

    # Resolve small-codomain collisions BEFORE anything is reported, so the table never
    # shows a scalar as unhooked on the strength of one unlucky draw.
    collision_notes = {}
    for name in SCALAR_ROWS:
        if results.get(name, ("", ""))[0] == "NATIVE":
            state, note = collision_verdict(name, farbled, control)
            results[name] = (state, note)
            collision_notes[name] = (state, note)

    print("\n  HARNESS CONTROLS")
    pos_state = results[POSITIVE_CONTROL][0]
    size_state = results[SIZE_GATE_CONTROL][0]
    print(f"    positive  ({POSITIVE_CONTROL:<14}) = {pos_state:<11} "
          f"[must be FARBLED]")
    print(f"    size-gate ({SIZE_GATE_CONTROL:<14}) = {size_state:<11} "
          f"[must be NATIVE]")

    if args.negative_control:
        ok = (pos_state == "NATIVE")
        print("\n  NEGATIVE CONTROL: positive control came out %s — %s"
              % (pos_state, "as required (the rig sees the feature's absence)"
                            if ok else "⛔ UNEXPECTED"))
        return 0 if ok else 2

    void = False
    if pos_state != "FARBLED":
        print("\n  ⛔ RUN VOID — the known-hooked vector is not farbled. Farbling is off,")
        print("     or this build lacks the canvas patch. Every NATIVE below would be an")
        print("     artefact of that, not a finding.")
        void = True
    if size_state != "NATIVE":
        print("\n  ⛔ RUN VOID — the >65536px canvas was NOT native, so the size gate is")
        print("     not doing what the rest of this rig assumes.")
        void = True

    print("\n  VECTORS")
    print("    %-34s %-11s %-9s %s" % ("§B row", "measured", "expected", "detail"))
    print("    " + "-" * 92)
    natives = []
    unresolved = []
    for name, label, expect in ROWS:
        state, detail = results[name]
        flag = " " if state == expect else "*"
        print("  %s %-34s %-11s %-9s %s" % (flag, label[:34], state, expect, detail[:40]))
        if state == "NATIVE":
            natives.append(label)
        elif state in ("NOISY", "DEGENERATE", "ERROR"):
            unresolved.append((label, state, detail))
    print("\n    (* = measurement disagrees with the documented prior)")

    if collision_notes:
        print("\n  SMALL-CODOMAIN DISCRIMINATOR (re-drawn on other registrable domains)")
        for name, (state, note) in collision_notes.items():
            print(f"    {name}: resolved {state} — {note}")

    if void:
        return 2
    if unresolved:
        print("\n  ⚠️ NO VERDICT for %d row(s) — these are NOT 'native', they are unmeasured:"
              % len(unresolved))
        for label, state, detail in unresolved:
            print(f"     {label}: {state} — {detail}")
        return 2
    if natives:
        print("\n  ⛔ %d UNHOOKED VECTOR(S): %s" % (len(natives), ", ".join(natives)))
        return 1
    print("\n  ✅ every §B vector measured FARBLED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
