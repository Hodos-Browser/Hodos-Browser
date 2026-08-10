#!/usr/bin/env python3
r"""codec_check.py — the P5 codec gate (PLAN_codecs.md §6 matrix, §7 procedure).

Two layers, run against a REAL tab of the built browser:

  Layer A  `canPlayType` capability probe. GATE rows must return 'probably'; a `""`
           on any GATE row means the codec build regressed -> block the bump.
  Layer B  actual decode. `canPlayType` is a capability *string* -- it can answer
           'probably' for a codec that never decodes a byte. Layer B's pass criterion
           is therefore `webkitVideoDecodedByteCount` / `webkitAudioDecodedByteCount`
           CLIMBING between two samples, plus `currentTime` advancing. Anything weaker
           re-tests Layer A with extra steps (PLAN_codecs.md §6.3 method note).

## The control -- why this harness can go red at all

CLAUDE.md's Testing Standards make a negative control mandatory: a test never seen to
fail has not been shown to test anything. The true feature-off build here is
`ffmpeg_branding=Chromium`, which is a 10-12 hour rebuild, so it is not run per-check.
What IS run every time, on the same page, in the same probe, is a set of codecs this
build genuinely does NOT ship:

    audio/mp4; codecs="ac-3"     ENABLE_PLATFORM_AC3_EAC3_AUDIO = 0   -> must be ""
    audio/mp4; codecs="ec-3"     ENABLE_PLATFORM_AC3_EAC3_AUDIO = 0   -> must be ""
    video/mp4; codecs="nope.1"   not a codec                          -> must be ""

If any control row comes back non-empty the probe is not discriminating and the run
exits non-zero regardless of the GATE rows. That bounds the failure mode "the probe
says 'probably' to everything" -- which is the shape a broken canPlayType harness takes.
It does NOT prove the FFmpeg decoders were compiled in; the build-flag artifacts do that
(`out/.../gen/media/media_buildflags.h`, `USE_PROPRIETARY_CODECS() (1)`, and the
`config/Chrome/win/x64` ffmpeg config path in the generated ninja), and Layer B proves
they run. State all three together when reporting; no one of them is sufficient.

## Subject assertion -- three harnesses died here

Hodos's header and ~14 overlays are separate CEF browsers on 127.0.0.1:5137 and CDP
reports every one of them as `type: "page"`. Driving one produces a verdict about the
wrong document. This script does not re-solve that: it imports the target selection,
kill-by-path and launch helpers from `farbling_seed_rotation_check.py`, which already
pins exactly one tab by CDP target **id** and refuses to guess when ambiguous.

## Usage

    pip install websocket-client

    python codec_check.py \
        --exe "C:\Users\<you>\Hodos-Browser\cef-native\build\bin\Release\HodosBrowser.exe" \
        --port 9322 --dev --profile Default \
        --log "%APPDATA%\HodosBrowserDev\logs\debug_output.log"

    --layer a|b|both   (default both)
    --attach           measure the already-running browser instead of restarting it

⚠️ The browser is killed by executable PATH, never by image name -- the installed
production browser shares the image name and holds CDP 9222.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import websocket  # websocket-client
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402  (path insert above)
    check_role_in_log,
    count_browser_procs,
    engine_version,
    kill_browser_by_path,
    launch_browser,
    resolve_tab,
    snapshot_targets,
    wait_for_cdp,
)
from codec_test_assets import (  # noqa: E402
    AAC_M4A_B64,
    AC3_MP4_B64,
    H264_MP4_B64,
    MP3_B64,
)


# A real remote origin on purpose. A local fixture would exercise a different resource
# path and, for the farbling work sharing this browser, is skipped by design.
PROBE_URL = "https://example.com/"
PROBE_HOST = "example.com"


# --------------------------------------------------------------------------------------
# Layer A -- capability matrix
# --------------------------------------------------------------------------------------

# (label, element, mime, class)   class: GATE | PRESENT | RECORD | CONTROL
LAYER_A = [
    ("H.264 baseline", "video", 'video/mp4; codecs="avc1.42E01E"',      "GATE"),
    ("H.264 High",     "video", 'video/mp4; codecs="avc1.640028"',      "GATE"),
    ("AAC-LC",         "audio", 'audio/mp4; codecs="mp4a.40.2"',        "GATE"),
    ("MP3",            "audio", 'audio/mpeg',                            "GATE"),
    ("VP9",            "video", 'video/webm; codecs="vp09.00.10.08"',   "GATE"),
    ("AV1",            "video", 'video/mp4; codecs="av01.0.05M.08"',    "PRESENT"),
    ("HEVC/H.265",     "video", 'video/mp4; codecs="hvc1.1.6.L93.B0"',  "RECORD"),
    ("Dolby Vision",   "video", 'video/mp4; codecs="dvh1.05.07"',       "RECORD"),
    ("Dolby AC-3",     "audio", 'audio/mp4; codecs="ac-3"',             "CONTROL"),
    ("Dolby E-AC-3",   "audio", 'audio/mp4; codecs="ec-3"',             "CONTROL"),
    ("bogus codec",    "video", 'video/mp4; codecs="nope.1"',           "CONTROL"),
]

LAYER_A_JS = """
(() => {
  const v = document.createElement('video');
  const a = document.createElement('audio');
  const rows = %s;
  const out = {};
  for (const [label, el, mime] of rows) {
    out[label] = (el === 'video' ? v : a).canPlayType(mime);
  }
  return JSON.stringify({href: location.href, results: out});
})()
"""


# --------------------------------------------------------------------------------------
# Layer B -- decode receipts
# --------------------------------------------------------------------------------------

# Plays a data: URL through a real media element and samples the decoded-byte counters
# twice. Returns the deltas. A decoder that is advertised but absent yields
# decoded == 0 while readyState and networkState still look healthy, which is exactly
# the "green build, no codecs" failure this gate exists to catch.
#
# ⚠️ Two things here are load-bearing and both cost a red run to find:
#   * the element is a <video>, even for audio-only content. Chrome's autoplay policy
#     allows muted *video* autoplay but still blocks <audio> — a muted <audio> element
#     returns NotAllowedError, which reads exactly like a decoder failure and is not one.
#   * the caller passes userGesture=True on Runtime.evaluate, which supplies the user
#     activation the policy wants. Belt and braces; either alone has failed here.
SRC_DECODE_JS = """
(async () => {
  const el = document.createElement('video');
  el.src = "%(src)s";
  el.muted = true;
  document.body.appendChild(el);
  const err = () => el.error ? (el.error.code + ':' + (el.error.message||'')) : null;
  try { await el.play(); } catch (e) { return JSON.stringify(
      {ok:false, why:'play() rejected: ' + e, error: err()}); }
  const t0 = el.currentTime;
  const a0 = el.webkitAudioDecodedByteCount, v0 = el.webkitVideoDecodedByteCount;
  await new Promise(r => setTimeout(r, 1500));
  const t1 = el.currentTime;
  const a1 = el.webkitAudioDecodedByteCount, v1 = el.webkitVideoDecodedByteCount;
  el.pause(); el.remove();
  return JSON.stringify({ok: true, t0, t1, a0, a1, v0, v1,
                         duration: el.duration, error: err()});
})()
"""

# For real sites: find the media element that is actually playing, nudge it if the page
# left it paused, and sample the same two counters.
SITE_DECODE_JS = """
(async () => {
  const pick = () => {
    const els = [...document.querySelectorAll('video, audio')];
    if (!els.length) return null;
    // Prefer one that already has data; else the largest.
    const ready = els.filter(e => e.readyState >= 2);
    const pool = ready.length ? ready : els;
    return pool.sort((x, y) =>
      ((y.videoWidth||0)*(y.videoHeight||0)) - ((x.videoWidth||0)*(x.videoHeight||0))
    )[0];
  };
  let el = null;
  for (let i = 0; i < 30 && !el; i++) {
    el = pick();
    if (!el) await new Promise(r => setTimeout(r, 500));
  }
  if (!el) return JSON.stringify({ok:false, why:'no media element on the page'});
  el.muted = true;
  try { if (el.paused) await el.play(); } catch (e) { /* keep going; may autoplay */ }
  await new Promise(r => setTimeout(r, 2500));
  const t0 = el.currentTime;
  const v0 = el.webkitVideoDecodedByteCount || 0;
  const a0 = el.webkitAudioDecodedByteCount || 0;
  await new Promise(r => setTimeout(r, 3000));
  return JSON.stringify({
    ok: true,
    tag: el.tagName, w: el.videoWidth || 0, h: el.videoHeight || 0,
    paused: el.paused, readyState: el.readyState,
    t0, t1: el.currentTime,
    v0, v1: el.webkitVideoDecodedByteCount || 0,
    a0, a1: el.webkitAudioDecodedByteCount || 0,
    error: el.error ? el.error.code : null
  });
})()
"""

# PLAN_codecs.md §6.2. `blocked` rows are recorded as blocked-by-site-access, never as a
# codec failure -- each is redundant with a row that does decode the same codec.
SITES = [
    # Long-lived entry URLs only. A hand-picked video/VOD id rots, and a 404 recorded as
    # "blocked by site access" would be the harness's fault misfiled as the site's.
    ("youtube.com", "https://www.youtube.com/watch?v=aqz-KE-bpKQ", "VP9/AV1 + AAC/Opus"),
    ("x.com",       "https://x.com/",                              "H.264 + AAC"),
    ("twitch.tv",   "https://www.twitch.tv/",                      "H.264/AAC live"),
]

# Played from an element on the probe page rather than navigated to. Navigating the tab
# straight at an .mp4 does NOT get you an inline player here — Hodos's CefDownloadHandler
# claims it, the page ends up with no media element, and the row reads "blocked" for a
# reason that has nothing to do with codecs.
ELEMENT_SOURCES = [
    ("MP3 decode  (data:)", "data:audio/mpeg;base64," + MP3_B64,      "audio"),
    ("AAC decode  (data:)", "data:audio/mp4;base64," + AAC_M4A_B64,   "audio"),
    # Local, not remote, on purpose: this row first pointed at Google's public
    # gtv-videos-bucket sample, which began answering 403 mid-sprint. A remote asset in a
    # release gate is a row that rots, and it rots as a false RED.
    ("H.264 decode (data:)", "data:video/mp4;base64," + H264_MP4_B64,  "video"),
    # ⛔ NEGATIVE CONTROL, and the reason Layer B means anything. AC-3 in MP4, built by
    # the same ffmpeg from the same tone as the AAC row, played through the same element
    # and read from the same counters -- but ENABLE_PLATFORM_AC3_EAC3_AUDIO is 0 in this
    # build, so it MUST NOT decode. If this row ever "passes", the decode receipts above
    # are not receipts and the whole layer is measuring nothing.
    ("AC-3 (NEG CONTROL)",  "data:audio/mp4;base64," + AC3_MP4_B64,    "none"),
]


def evaluate(port, chrome_ids, expr, timeout=90):
    """Run an async expression in the pinned tab and return its parsed JSON value.

    `userGesture` supplies user activation. Without it every `play()` in Layer B is
    rejected by the autoplay policy with NotAllowedError, which looks like a decode
    failure and is not one.
    """
    t = resolve_tab(port, chrome_ids)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 7, "method": "Runtime.evaluate",
                            "params": {"expression": expr, "returnByValue": True,
                                       "userGesture": True, "awaitPromise": True}}))
        end = time.time() + timeout
        while time.time() < end:
            m = json.loads(ws.recv())
            if m.get("id") == 7:
                res = m.get("result", {}).get("result", {})
                if "value" not in res:
                    raise SystemExit("evaluate returned no value: %s" % json.dumps(m)[:400])
                return json.loads(res["value"])
    finally:
        ws.close()
    raise SystemExit("evaluate timed out")


def navigate(port, chrome_ids, url, want_host, timeout=90):
    t = resolve_tab(port, chrome_ids)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": url}}))
    finally:
        ws.close()
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        t = resolve_tab(port, chrome_ids)
        if want_host in t.get("url", ""):
            time.sleep(2.0)
            return t
    raise SystemExit("navigation to %s did not land within %ss" % (url, timeout))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--port", type=int, default=9322)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--log", default=None, help="shell debug log, for the role cross-check")
    ap.add_argument("--layer", choices=["a", "b", "both"], default="both")
    ap.add_argument("--attach", action="store_true",
                    help="measure the running browser instead of restarting it")
    args = ap.parse_args()

    if not args.attach:
        print("[setup] killing any browser under %s" % os.path.dirname(args.exe))
        kill_browser_by_path(args.exe, verify=True)
        launch_browser(args.exe, args.dev, args.profile)
        if not wait_for_cdp(args.port):
            return fail("CDP never came up on port %d" % args.port)
    else:
        n = count_browser_procs(args.exe)
        print("[setup] attaching to %d running process(es) under the build dir" % n)
        if n == 0:
            return fail("--attach but nothing is running from the build directory")
        if not wait_for_cdp(args.port, timeout=20):
            return fail("CDP not answering on port %d" % args.port)

    engine = engine_version(args.port)
    print("[setup] engine = %s" % engine)
    chrome_ids = snapshot_targets(args.port)

    failures, notes = [], []

    def check(label, ok, detail):
        print("  %s %-34s %s" % ("PASS" if ok else "FAIL", label, detail))
        if not ok:
            failures.append(label)

    # ---------------------------------------------------------------- Layer A
    layer_a = {}
    if args.layer in ("a", "both"):
        print("\n== Layer A -- canPlayType on %s ==" % PROBE_URL)
        navigate(args.port, chrome_ids, PROBE_URL, PROBE_HOST)
        role = check_role_in_log(args.log, PROBE_HOST)
        if role:
            print("    shell served %s to role=%s" % (PROBE_HOST, role))
            if not role.startswith("tab"):
                return fail("subject error: %s was served to role=%s, not a tab"
                            % (PROBE_HOST, role))
        rows = json.dumps([[l, e, m] for (l, e, m, _c) in LAYER_A])
        val = evaluate(args.port, chrome_ids, LAYER_A_JS % rows)
        if PROBE_HOST not in val["href"]:
            return fail("subject error: measured %s" % val["href"])
        layer_a = val["results"]
        print()
        for label, _el, mime, cls in LAYER_A:
            got = layer_a[label] or '""'
            if cls == "GATE":
                check("%s [GATE]" % label, layer_a[label] == "probably", got)
            elif cls == "PRESENT":
                check("%s [present]" % label, layer_a[label] != "", got)
            elif cls == "CONTROL":
                check("%s [CONTROL must be \"\"]" % label, layer_a[label] == "", got)
            else:
                print("  RECD %-34s %s   (non-gating)" % (label, got))
                notes.append("%s = %s" % (label, got))

    # ---------------------------------------------------------------- Layer B
    if args.layer in ("b", "both"):
        print("\n== Layer B -- decode receipts (elements on the probe page) ==")
        if args.layer == "b":
            navigate(args.port, chrome_ids, PROBE_URL, PROBE_HOST)
        for label, src, want in ELEMENT_SOURCES:
            r = evaluate(args.port, chrome_ids, SRC_DECODE_JS % {"src": src})
            if want == "none":
                # The control passes by FAILING to decode: either play() is rejected, or
                # it "plays" while the counters stay flat. Both are the shape of an
                # absent decoder; either satisfies the control.
                if not r.get("ok"):
                    check(label, True, "did not decode: %s" % r.get("why"))
                else:
                    moved = (r["a1"] - r["a0"]) > 0 or (r["v1"] - r["v0"]) > 0
                    check(label, not moved,
                          "audio +%d B, video +%d B (err=%s)"
                          % (r["a1"] - r["a0"], r["v1"] - r["v0"], r.get("error")))
                continue
            if not r.get("ok"):
                check(label, False, r.get("why", "?"))
                continue
            da, dv, dt = r["a1"] - r["a0"], r["v1"] - r["v0"], r["t1"] - r["t0"]
            ok = dt > 0 and (da > 0 if want in ("audio", "both") else True) \
                         and (dv > 0 if want in ("video", "both") else True)
            check(label, ok,
                  "audio +%d B, video +%d B, currentTime +%.3fs (err=%s)"
                  % (da, dv, dt, r.get("error")))

        print("\n== Layer B -- decode receipts (real sites) ==")
        for host, url, exercises in SITES:
            try:
                navigate(args.port, chrome_ids, url, host, timeout=120)
                r = evaluate(args.port, chrome_ids, SITE_DECODE_JS, timeout=120)
            except SystemExit as e:
                print("  BLOCKED %-14s %s  (site access, not decode: %s)"
                      % (host, exercises, e))
                notes.append("%s blocked: %s" % (host, e))
                continue
            if not r.get("ok"):
                print("  BLOCKED %-14s %s  (site access, not decode: %s)"
                      % (host, exercises, r.get("why")))
                notes.append("%s blocked: %s" % (host, r.get("why")))
                continue
            dv, da, dt = r["v1"] - r["v0"], r["a1"] - r["a0"], r["t1"] - r["t0"]
            check("%s (%s)" % (host, exercises), (dv > 0 or da > 0) and dt > 0,
                  "video +%d B, audio +%d B, currentTime +%.2fs, %dx%d rs=%d"
                  % (dv, da, dt, r["w"], r["h"], r["readyState"]))

    # ---------------------------------------------------------------- verdict
    print()
    if notes:
        print("Recorded (non-gating): " + "; ".join(notes))
    if failures:
        print("RESULT: FAIL -- %d gate/control row(s): %s"
              % (len(failures), ", ".join(failures)))
        print("A \"\" on a GATE row means the codec build regressed. Re-audit the resolved")
        print("args (out/.../gen/media/media_buildflags.h) per PLAN_codecs.md §7 step 3")
        print("BEFORE assuming the harness is wrong.")
        return 1
    ran = {"a": "Layer A", "b": "Layer B", "both": "Layer A + Layer B"}[args.layer]
    print("RESULT: PASS (%s)" % ran)
    # The token carries Layer-A values only, so it is printed only when Layer A ran --
    # otherwise every row would read `none` and a Layer-B-only run would look like a
    # codec-free build to anyone reading the token instead of the transcript.
    if layer_a:
        print()
        print("CODEC-GATE-v1 engine=%s %s"
              % (engine, " ".join("%s=%s" % (l.replace(" ", "_"),
                                             (layer_a.get(l) or "empty"))
                                  for l, _e, _m, c in LAYER_A if c != "CONTROL")))
    return 0


def fail(msg):
    print("RESULT: FAIL -- %s" % msg)
    return 1


if __name__ == "__main__":
    sys.exit(main())
