#!/usr/bin/env python3
r"""farbling_psl_linkability_check.py — do unrelated sites on shared hosting share a key?

Settles §H11, which until now was a CODE READ and nothing more.

## The claim under test

The farbling key is `HMAC(profile_seed, RegistrableDomain(host))`, and
`FarblingPolicy::RegistrableDomain` is hand-rolled: it treats the last TWO labels as the
registrable domain unless the last two are in `MultiLabelSuffixes()`, a table that is
**ccTLD-only** (`co.uk`, `com.au`, `co.jp`, …). It contains no modern hosting suffix —
no `github.io`, `workers.dev`, `pages.dev`, `vercel.app`, `netlify.app`.

⇒ **If that is true in the running build, `alice.github.io` and `bob.github.io` both reduce
to `github.io` and therefore share one farbling key.** Two unrelated sites would then see
IDENTICAL farbled values, and either could use that to recognise a visitor on the other —
which is precisely the **C-2 unlinkability** contract.

⚠️ This is a *keying* question, not a P4f question. It predates every worker and subframe
change and would have been true of the very first C2 build.

## Why the controls decide it and a bare comparison would not

"Two sites produced the same hash" is also what you get from a browser where farbling is
off (both native), or from a harness that measured the same tab twice. So:

  * **NATIVE control** — every farbled value must differ from the native value measured on
    the same build. If they match native, farbling is simply off and nothing below means
    anything.
  * **SEPARATION control** — two genuinely different registrable domains (`example.com` vs
    `example.org`) MUST produce different hashes. This proves the rig can *see* a key
    difference at all. Without it, "the two github.io hosts match" is unfalsifiable: a
    broken harness returns identical hashes for everything.
  * **SUBJECT control** — each measurement asserts `location.href` is on the host we asked
    for, so a redirect (very common on shared hosting) cannot be recorded as that host.

Only with both controls green does "same hash" mean "same key".

## Usage

    python3 farbling_psl_linkability_check.py --dev \
        --exe ...\cef-native\build\bin\Release\HodosBrowser.exe \
        --data-root %APPDATA%\HodosBrowserDev

Exit 0 = shared-hosting hosts are correctly SEPARATED (hypothesis refuted).
Exit 1 = they SHARE a key (hypothesis confirmed — a C-2 weakness).
Exit 2 = inconclusive (a control failed, or too few hosts loaded).
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
    cef_version,
    kill_browser_by_path,
    launch_browser,
    require_engine,
    set_site_enabled,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402
from farbling_worker_probe import resolve_tab, snapshot_targets  # noqa: E402
from farbling_iframe_check import _rpc  # noqa: E402
from farbling_widget_regression_check import CANVAS_JS  # noqa: E402

# Two unrelated sites under one shared-hosting suffix. Both are long-lived org pages
# rather than personal repos, so the test does not rot the first time someone deletes a
# demo. Neither is on the IsAuthDomain allowlist.
SHARED_SUFFIX_PAIRS = [
    # ⚠️ Deep paths, not bare org roots: https://microsoft.github.io/ and
    # https://google.github.io/ both 404 to a "Site not found" page, which is why the
    # first run of this check returned None for both and reached no verdict.
    ("github.io", "https://squidfunk.github.io/mkdocs-material/", "squidfunk.github.io",
                  "https://microsoft.github.io/monaco-editor/", "microsoft.github.io"),
]

# Genuinely distinct registrable domains: the separation control.
CONTROL_A = ("https://example.com/", "example.com")
CONTROL_B = ("https://example.org/", "example.org")

# ---------------------------------------------------------------------------------------
# ⛔ THE SHELL SUBJECT ASSERTION -- and why --expect-cef is NOT it for this check.
#
# H11 is an APP-SIDE fix (cef-native/src/core/FarblingPolicy.cpp). The engine is
# unchanged across it: same fork pin, same CEF_VERSION, same framework LC_UUID. So
# --expect-cef -- the correct subject assertion for every P4e/P4f harness, because those
# DID change the engine -- cannot tell a fixed shell from an unfixed one here. Measured
# on macOS 2026-08-16: the pre-fix and post-fix runs printed the identical engine string
# `150.0.43-7871.3576+g9ccef04` and returned opposite verdicts.
#
# We still call require_engine() (it catches a stale app-bundle framework and keeps this
# harness consistent with its siblings), but the discriminator for H11 is the SHELL
# BINARY. The fix is literally a string table, so the suffixes are observable in the
# linked image -- verify by CONTENT, never by a build exit code.
SHELL_MARKERS = ["myshopify.com", "workers.dev", "vercel.app", "notion.site"]
# A multi-label suffix that was in the table long before H11. If this is ABSENT the
# instrument did not really read the binary, and the absence of the markers above would
# be meaningless -- the classic "wrong subject looks like a missing feature" shape.
SHELL_POSITIVE_CONTROL = "police.uk"


def shell_carries_h11_fix(exe):
    """Does the shell under test carry the shared-hosting suffix table?

    Returns (verdict, detail) where verdict is True / False / None (= cannot tell).
    Deliberately does NOT refuse on False: running this harness against a pre-fix build
    is the legitimate way to capture the RED baseline. It refuses to GUESS, which is a
    different thing.
    """
    try:
        with open(exe, "rb") as fh:
            blob = fh.read()
    except OSError as exc:
        return None, "could not read the shell binary (%s)" % exc
    if SHELL_POSITIVE_CONTROL.encode() not in blob:
        return None, ("positive control %r absent -- the scan did not genuinely read the "
                      "binary, so it cannot report on the H11 markers either"
                      % SHELL_POSITIVE_CONTROL)
    found = [m for m in SHELL_MARKERS if m.encode() in blob]
    if len(found) == len(SHELL_MARKERS):
        return True, "all %d shared-hosting markers present" % len(SHELL_MARKERS)
    if not found:
        return False, ("none of the %d shared-hosting markers present (positive control "
                       "%r found, so this is a real absence) -- PRE-FIX shell"
                       % (len(SHELL_MARKERS), SHELL_POSITIVE_CONTROL))
    return None, ("only %d of %d markers present (%s) -- partial table, not a state this "
                  "build should ever be in" % (len(found), len(SHELL_MARKERS),
                                               ", ".join(found)))


def goto_and_measure(port, excluded, url, want_host, timeout=75):
    """Navigate, then read the canvas hash — asserting the subject on the way out."""
    tab = resolve_tab(port, excluded)
    _rpc(tab["webSocketDebuggerUrl"], "Page.navigate", {"url": url}, msg_id=1)
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        tab = resolve_tab(port, excluded)
        if want_host not in tab.get("url", ""):
            continue
        got = _rpc(tab["webSocketDebuggerUrl"], "Runtime.evaluate",
                   {"expression": CANVAS_JS, "returnByValue": True},
                   msg_id=2, wait=40)
        if not got:
            continue
        res = got.get("result", {}).get("result", {})
        if "value" not in res:
            continue
        val = json.loads(res["value"])
        # SUBJECT control: shared hosting redirects constantly; a redirect recorded as the
        # requested host would be a silently wrong key attribution.
        if want_host not in (val.get("href") or ""):
            continue
        return val.get("canvas")
    return None


def boot(args, port, pdir, hosts, farbling_on):
    kill_browser_by_path(args.exe)
    for h in hosts:
        set_site_enabled(pdir, h, farbling_on)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    time.sleep(args.settle)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--expect-cef", default=None,
                    help="refuse to run unless CEF_VERSION contains this "
                         "(e.g. +g9ccef04). Catches a stale app-bundle framework; it "
                         "canNOT tell a fixed shell from an unfixed one -- H11 is "
                         "app-side and does not move the engine.")
    args = ap.parse_args()

    port = cdp_port_for(args.profile, args.dev)
    pdir = profile_dir(args.data_root, args.profile)

    all_hosts = [CONTROL_A[1], CONTROL_B[1]]
    for _, _, h1, _, h2 in SHARED_SUFFIX_PAIRS:
        all_hosts += [h1, h2]

    print("== shared-hosting key separation (§H11) ==")
    print("   engine %s" % cef_version())
    # Hard refusal BEFORE a browser is launched. Only ever turns green into red.
    require_engine(args.exe, expect=args.expect_cef, label="psl-linkability")
    shell_ok, shell_why = shell_carries_h11_fix(args.exe)
    print("   shell : %s -- %s"
          % ({True: "H11 suffix table PRESENT",
              False: "H11 suffix table ABSENT",
              None: "H11 suffix table UNDETERMINED"}[shell_ok], shell_why))

    try:
        # --- farbled arm ---------------------------------------------------------------
        boot(args, port, pdir, all_hosts, farbling_on=True)
        excluded = snapshot_targets(port, args.settle)
        farb = {}
        for url, host in [CONTROL_A, CONTROL_B]:
            farb[host] = goto_and_measure(port, excluded, url, host)
        for _, u1, h1, u2, h2 in SHARED_SUFFIX_PAIRS:
            farb[h1] = goto_and_measure(port, excluded, u1, h1)
            farb[h2] = goto_and_measure(port, excluded, u2, h2)

        # --- native arm ----------------------------------------------------------------
        boot(args, port, pdir, all_hosts, farbling_on=False)
        excluded = snapshot_targets(port, args.settle)
        native = goto_and_measure(port, excluded, *CONTROL_A)
    finally:
        kill_browser_by_path(args.exe)
        for h in all_hosts:
            set_site_enabled(pdir, h, True)

    print("\n  native (farbling off): %s" % native)
    for h, v in farb.items():
        print("  farbled  %-26s %s" % (h, v))

    print("\n  CONTROLS")
    if not native:
        print("    ⚠️ NO VERDICT — could not read the native value.")
        return 2
    unfarbled = [h for h, v in farb.items() if v is not None and v == native]
    if unfarbled:
        print("    ⛔ these hosts read NATIVE, so farbling is not on for them: %s"
              % ", ".join(unfarbled))
        print("       No verdict — they may be exempt, or the feature is off.")
        return 2
    print("    [PASS] every host differs from native (farbling is on)")

    a, b = farb.get(CONTROL_A[1]), farb.get(CONTROL_B[1])
    if not a or not b:
        print("    ⚠️ NO VERDICT — a separation-control host did not load.")
        return 2
    if a == b:
        print("    ⛔ SEPARATION CONTROL FAILED — example.com and example.org, which are")
        print("       different registrable domains, produced the SAME hash (%s)." % a)
        print("       The rig cannot see a key difference, so nothing below is meaningful.")
        return 2
    print("    [PASS] separation: example.com (%s) != example.org (%s)" % (a, b))

    print("\n  RESULT")
    rc = 0
    for suffix, _, h1, _, h2 in SHARED_SUFFIX_PAIRS:
        v1, v2 = farb.get(h1), farb.get(h2)
        if not v1 or not v2:
            print("    ⚠️ %s — NO VERDICT (%s=%s, %s=%s); one host did not load."
                  % (suffix, h1, v1, h2, v2))
            rc = max(rc, 2)
            continue
        if v1 == v2:
            print("    ⛔ %s — SHARED KEY CONFIRMED. %s and %s both read %s."
                  % (suffix, h1, h2, v1))
            print("       Two unrelated sites see identical farbled values, so either can")
            print("       recognise a visitor on the other. This is a C-2 unlinkability")
            print("       weakness. It PREDATES P4f and is a keying issue, not a worker one.")
            rc = max(rc, 1)
        else:
            print("    ✅ %s — separated (%s=%s, %s=%s). Hypothesis refuted for this suffix."
                  % (suffix, h1, v1, h2, v2))

    # ATTRIBUTION cross-check. A separated result is only evidence FOR THE H11 FIX if the
    # shell under test actually contains it. Green against a pre-fix shell would mean the
    # separation came from somewhere else entirely -- the harness would be measuring
    # something other than what it claims, and reporting a pass either way.
    if rc == 0 and shell_ok is False:
        print("\n    ⛔ ATTRIBUTION FAILURE — the hosts separated, but the shell under")
        print("       test does NOT carry the H11 suffix table. The pass cannot be")
        print("       attributed to the fix, so it is not evidence that the fix works.")
        print("       Find out what actually separated them before trusting this.")
        return 2
    if rc == 0 and shell_ok is None:
        print("\n    ⚠️ the pass could not be attributed to the H11 fix: %s" % shell_why)
    return rc


if __name__ == "__main__":
    sys.exit(main())
