#!/usr/bin/env python3
r"""drm_check.py — Q4 Spike-1 steps 3+4, re-runnable on any build (P5 DRM-1).

Answers, from artifacts rather than assumption:

  1. Is a Widevine CDM actually on disk for this profile, and which version?
  2. What robustness tiers does `requestMediaKeySystemAccess('com.widevine.alpha')`
     actually offer on THIS binary?

Tier (2) is the whole DRM question. Chromium enables CDM host verification at runtime only
when valid VMP `.sig` files sit beside the executable. So the ladder below IS the VMP
evidence — a refusal on a real title is confirmation, not the measurement.

⚠️ Measured 2026-08-10 on `c63654654`, and it is NOT where the plan docs said it would be:
software `SW_SECURE_DECODE` **is** granted (and negotiated back as such). The refusal line
sits at **`distinctiveIdentifier: required`** and at every hardware tier. Premium services
require a distinctive identifier, so the conclusion (no premium DRM without VMP) is
unchanged — but the *reason to cite* is the identifier, not a SW_SECURE_DECODE cap.

## The control is built in, and that is not an accident

This probe walks a ladder that must answer BOTH ways on a correct build:

    (unspecified) / SW_SECURE_CRYPTO / SW_SECURE_DECODE  -> expected GRANTED
    distinctiveIdentifier: required, and every HW_ tier   -> expected REFUSED

plus a bogus key system that must be refused. A probe that returned "granted" for
everything, or "refused" for everything, would be indistinguishable from a broken probe —
the ladder having a known YES *and* a known NO on the same page, in the same call, is what
makes either answer mean anything. The run fails if the ladder is uniform in either
direction, even if the uniform answer is the "good" one.

## What this does NOT do

Amazon, Netflix and Spotify need real accounts, so the site matrix (Spike-1 step 5) stays
owner-gated. `--bitmovin` adds the one premium-ish data point that needs no account: the
Bitmovin Widevine demo, which fetches a real licence from a real licence server, so it
distinguishes "EME resolves" from "a licence is actually granted and content decrypts".

## Usage

    python drm_check.py --exe "C:\...\build\bin\Release\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" --profile Default --dev [--attach]
        [--bitmovin]
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import websocket  # noqa: E402

from farbling_seed_rotation_check import (  # noqa: E402
    count_browser_procs,
    engine_version,
    kill_browser_by_path,
    launch_browser,
    resolve_tab,
    snapshot_targets,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for  # noqa: E402
from codec_check import PROBE_HOST, PROBE_URL, evaluate, navigate  # noqa: E402

# (label, keySystem, videoRobustness, distinctiveIdentifier, expectation)
#
# ⚠️ The audio robustness is pinned to SW_SECURE_CRYPTO for every rung and is NOT varied
# with the video one. Audio has no `SW_SECURE_DECODE` tier at all, so asking for it makes
# the whole configuration invalid and the request is refused for a reason that has nothing
# to do with attestation. A probe that varies both together therefore reports a phantom
# "capped below SW_SECURE_DECODE" on a build that grants it — see the note in
# `Q4_widevine_amazon_drm.md` §7 about the 2026-08-05 result that does not reproduce.
#
# The real attestation signal on this build is the `distinctiveIdentifier: required` rung.
LADDER = [
    ("unspecified",         "com.widevine.alpha", "",                 "optional", "GRANT"),
    ("SW_SECURE_CRYPTO",    "com.widevine.alpha", "SW_SECURE_CRYPTO", "optional", "GRANT"),
    ("SW_SECURE_DECODE",    "com.widevine.alpha", "SW_SECURE_DECODE", "optional", "GRANT"),
    ("+ distinctiveId REQ", "com.widevine.alpha", "SW_SECURE_DECODE", "required", "REFUSE"),
    ("HW_SECURE_CRYPTO",    "com.widevine.alpha", "HW_SECURE_CRYPTO", "optional", "REFUSE"),
    ("HW_SECURE_DECODE",    "com.widevine.alpha", "HW_SECURE_DECODE", "optional", "REFUSE"),
    ("HW_SECURE_ALL",       "com.widevine.alpha", "HW_SECURE_ALL",    "optional", "REFUSE"),
    ("bogus key system",    "com.hodos.notreal",  "",                 "optional", "REFUSE"),
]

EME_JS = """
(async () => {
  const rows = %s;
  const out = [];
  for (const [label, ks, robustness, distinctiveId] of rows) {
    const cfg = [{
      initDataTypes: ['cenc'],
      distinctiveIdentifier: distinctiveId,
      videoCapabilities: [{
        contentType: 'video/mp4; codecs="avc1.42E01E"',
        robustness: robustness
      }],
      audioCapabilities: [{
        contentType: 'audio/mp4; codecs="mp4a.40.2"',
        robustness: robustness ? 'SW_SECURE_CRYPTO' : ''
      }]
    }];
    let access = false, keys = false, err = null, negotiated = null;
    try {
      const a = await navigator.requestMediaKeySystemAccess(ks, cfg);
      access = true;
      const g = a.getConfiguration();
      negotiated = (g.videoCapabilities && g.videoCapabilities[0])
        ? g.videoCapabilities[0].robustness : null;
      try { await a.createMediaKeys(); keys = true; }
      catch (e) { err = 'createMediaKeys: ' + e.name; }
    } catch (e) { err = e.name; }
    out.push({label, access, keys, err, negotiated});
  }
  return JSON.stringify({href: location.href, secure: window.isSecureContext, rows: out});
})()
"""

BITMOVIN_JS = """
(async () => {
  const els = [...document.querySelectorAll('video')];
  const el = els.sort((a,b) => (b.videoWidth*b.videoHeight)-(a.videoWidth*a.videoHeight))[0];
  if (!el) return JSON.stringify({ok:false, why:'no video element'});
  el.muted = true;
  try { if (el.paused) await el.play(); } catch (e) {}
  await new Promise(r => setTimeout(r, 4000));
  const v0 = el.webkitVideoDecodedByteCount || 0, t0 = el.currentTime;
  await new Promise(r => setTimeout(r, 4000));
  return JSON.stringify({
    ok: true, v0, v1: el.webkitVideoDecodedByteCount || 0,
    t0, t1: el.currentTime, err: el.error ? el.error.code : null,
    keys: !!el.mediaKeys, keySystem: el.mediaKeys ? el.mediaKeys.keySystem : null
  });
})()
"""


def find_cdm(data_root, profile):
    """Spike-1 step 3 — the CDM is downloaded PER PROFILE by the component updater."""
    found = []
    for root in (os.path.join(data_root, profile), data_root):
        base = os.path.join(root, "WidevineCdm")
        if not os.path.isdir(base):
            continue
        for version in sorted(os.listdir(base)):
            dll = os.path.join(base, version, "_platform_specific", "win_x64",
                               "widevinecdm.dll")
            if os.path.exists(dll):
                found.append((version, dll, os.path.getsize(dll)))
    return found


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
    ap.add_argument("--attach", action="store_true")
    ap.add_argument("--bitmovin", action="store_true",
                    help="also drive the Bitmovin Widevine demo (real licence server)")
    args = ap.parse_args()

    port = cdp_port_for(args.profile, args.dev)
    failures = []

    def check(label, ok, detail):
        print("  %s %-34s %s" % ("PASS" if ok else "FAIL", label, detail))
        if not ok:
            failures.append(label)

    # ---- step 3: CDM on disk ---------------------------------------------------------
    print("== Spike-1 step 3 — Widevine CDM on disk ==")
    cdms = find_cdm(args.data_root, args.profile)
    for version, path, size in cdms:
        print("   %-14s %8.1f KB  %s" % (version, size / 1024.0, path))
    check("CDM present for profile %r" % args.profile, bool(cdms),
          "%d version(s)" % len(cdms))

    # ⚠️ widevinecdm.dll.sig is GOOGLE's signature on THEIR CDM. It is not VMP attestation
    # of OUR binaries, and mistaking one for the other is the single easiest way to
    # conclude we are VMP-signed when we are not.
    exe_dir = os.path.dirname(os.path.abspath(args.exe))
    vmp = [f for f in os.listdir(exe_dir) if f.endswith(".sig")]
    print("   VMP .sig files beside the executable: %s"
          % (", ".join(vmp) if vmp else "NONE (expected — we do not VMP-sign)"))

    # ---- step 4: EME robustness ladder -----------------------------------------------
    if not args.attach:
        for attempt in range(1, 4):
            kill_browser_by_path(args.exe)
            launch_browser(args.exe, args.dev, args.profile)
            if wait_for_cdp(port):
                break
        else:
            return fail("CDP %d never came up" % port)
    else:
        if count_browser_procs(args.exe) == 0:
            return fail("--attach but nothing is running from the build directory")
        if not wait_for_cdp(port, timeout=20):
            return fail("CDP not answering on %d" % port)

    print("\n== Spike-1 step 4 — EME robustness ladder (engine %s) =="
          % engine_version(port))
    excluded = snapshot_targets(port, settle=args.settle)
    navigate(port, excluded, PROBE_URL, PROBE_HOST)
    rows = json.dumps([[l, k, r, d] for (l, k, r, d, _e) in LADDER])
    val = evaluate(port, excluded, EME_JS % rows, timeout=120)
    if PROBE_HOST not in val["href"]:
        return fail("subject error: measured %s" % val["href"])
    # EME requires a secure context; on an insecure origin every row refuses and the whole
    # ladder reads as "capped at nothing", which looks like a catastrophic DRM failure.
    check("secure context", val["secure"], val["href"])

    print()
    granted = set()
    for (label, _ks, _rb, _di, expect), got in zip(LADDER, val["rows"]):
        state = "GRANT" if got["keys"] else ("ACCESS-ONLY" if got["access"] else "REFUSE")
        if got["keys"]:
            granted.add(label)
        check("%-18s expect %-6s" % (label, expect), state == expect,
              "%-6s negotiated=%-16r %s"
              % (state, got.get("negotiated"), got["err"] or ""))

    # ---- the built-in control --------------------------------------------------------
    print()
    uniform = len(granted) == 0 or len(granted) == len(LADDER)
    check("ladder discriminates (has a YES and a NO)", not uniform,
          "%d granted of %d" % (len(granted), len(LADDER)))
    if uniform:
        print("   A ladder that answers the same way to every rung cannot tell a")
        print("   robustness cap from a broken probe. Fix the probe before reading")
        print("   anything into the result.")

    # ---- optional: real licence server ------------------------------------------------
    if args.bitmovin:
        print("\n== free premium data point — Bitmovin Widevine demo (real licence) ==")
        try:
            navigate(port, excluded, "https://bitmovin.com/demos/drm", "bitmovin.com",
                     timeout=120)
            r = evaluate(port, excluded, BITMOVIN_JS, timeout=120)
        except SystemExit as e:
            print("   BLOCKED (site access, not DRM): %s" % e)
            r = None
        if r and r.get("ok"):
            dv, dt = r["v1"] - r["v0"], r["t1"] - r["t0"]
            # JSON.stringify DROPS keys whose value is `undefined`, so a present-but-
            # undefined `keySystem` simply is not in the dict. Use .get, not [].
            print("   decoded +%d B, currentTime +%.2fs, mediaKeys=%s (%s), err=%s"
                  % (dv, dt, r.get("keys"), r.get("keySystem"), r.get("err")))
            print("   -> %s" % ("DECRYPTED AND DECODED: an unattested L3 CDM does get a "
                                "licence from a real server" if dv > 0 and dt > 0 else
                                "did NOT decode — licence refused or playback blocked"))
        elif r:
            print("   BLOCKED (site access, not DRM): %s" % r.get("why"))

    print()
    if failures:
        print("RESULT: FAIL — %s" % ", ".join(failures))
        return 1
    print("RESULT: PASS — CDM present; granted: %s. The refusal line sits at "
          "`distinctiveIdentifier: required` and at every HW_ tier — that is the "
          "attestation/VMP signature. Steps 5-6 (Amazon/Netflix/Spotify) need real "
          "accounts." % ", ".join(sorted(granted)))
    return 0


def fail(msg):
    print("RESULT: FAIL -- %s" % msg)
    return 1


if __name__ == "__main__":
    sys.exit(main())
