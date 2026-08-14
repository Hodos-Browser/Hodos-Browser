#!/usr/bin/env python3
r"""farbling_d5_residual_check.py — is D5's residual REAL, and how big is it?

Settles a claim that has been carried as a **code read** since relay round 2026-08-13c §Y4
and was never measured on either platform, while simultaneously being queued for a
user-facing release note:

    "a third-party frame on an auth-exempt top frame inherits enabled=false and sees
     NATIVE values. That is correct and required (Turnstile on a login page), but it is a
     residual bypass and should be written down rather than discovered."

Writing a public statement about a privacy limitation on the strength of a code read is
exactly the pattern that produced this project's false greens. This harness measures it.

## ⛔ Why "the child reads native" is NOT sufficient on its own

Before P4e, EVERY subframe read native, because the renderer bailed on all of them. So
"child under an exempt parent is native" is satisfied by a build where cross-site keying is
simply broken — the pre-P4e world — and that has nothing to do with exemption inheritance.

The measurement therefore needs a **discriminator arm**: the SAME third party, under the
SAME parent, with the parent NOT exempt. If that child carries the parent's farbled values,
then cross-site keying is live, and only then does the exempt arm's native result mean
"inherited the exemption" rather than "subframes are broken".

    exempt parent      -> child NATIVE            \  together: D5 residual is REAL
    non-exempt parent  -> child == parent farbled /

    both native                                   -> P4e cross-site keying REGRESSED;
                                                     the D5 question is not answerable
                                                     from this run

## The exemption lever

`OnBeforeBrowse` collapses three inputs into one `enabled` bit (global toggle, IsAuthDomain,
per-site Privacy Shield opt-out). D5 is about that bit being inherited by subframes, not
about which input produced it, so this drives the **per-site opt-out** rather than an
IsAuthDomain host.

That is deliberate and it is the stronger choice:
  * a real auth-exempt host (github.com, x.com, paypal.com...) sends CSP/frame-ancestors and
    controls its own markup, so injecting a third-party iframe into it is unreliable and
    failures would be indistinguishable from the effect under test;
  * example.com/example.org are already the S3 pair and are known to frame each other;
  * IsAuthDomain is EXACT-HOST matched (`lower == auth`), so it carries its own foot-guns
    that are irrelevant to the inheritance question.

## Controls

  * **subject**   -- inherited from measure_iframe(): the child's own href must contain both
                     the third-party host and the ?p=<parent> marker, so a stale probe or a
                     silently-failed injection cannot be measured as the child.
  * **size gate** -- `large` / `glLarge` sit outside the farbling gates and must be identical
                     across every realm measured. If one moves, the realms are not comparable.
  * **farbling is live at all** -- the non-exempt parent must differ from native, or every
                     "== native" below is vacuous.
  * **native baseline** -- taken in-session from the exempt parent's own top-level read.

Exit codes: 0 residual confirmed (expected), 1 controls failed / no verdict,
2 cross-site keying regressed, 3 some third outcome worth investigating.

Usage:

    python farbling_d5_residual_check.py \
        --exe ".../HodosBrowser.app/Contents/MacOS/HodosBrowser" \
        --data-root "~/Library/Application Support/HodosBrowserDev" --dev
"""

import argparse
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    measure,
    settings_path,
    snapshot_targets,
)
from farbling_cross_profile_check import profile_dir  # noqa: E402
from farbling_iframe_check import (  # noqa: E402
    FARBLED_FIELDS,
    GATE_CONTROLS,
    PARENT_A,
    THIRD,
    boot,
    measure_iframe,
    same,
    show,
)


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
        backup = path + ".d5-backup"
        shutil.copy2(path, backup)

    try:
        # --- phase 1: the D5 arm. Parent hard-bypassed => enabled=false for the top frame.
        print("=== phase 1 — parent %s EXEMPT (per-site opt-out) ==================" % PARENT_A)
        port = boot(args, bypass_hosts=[PARENT_A])
        eng = engine_version(port)
        excl = snapshot_targets(port, settle=8.0)
        exempt_parent = measure(port, excl, "https://%s/" % PARENT_A, PARENT_A,
                                timeout=args.timeout)
        show("parent (exempt)", exempt_parent)
        exempt_child = measure_iframe(port, excl, PARENT_A, timeout=args.timeout)
        show("child %s in it" % THIRD, exempt_child)

        # --- phase 2: the discriminator. Same parent, same child, NOT exempt.
        print("\n=== phase 2 — parent %s NOT exempt (discriminator) ================" % PARENT_A)
        port = boot(args, bypass_hosts=[])
        excl = snapshot_targets(port, settle=8.0)
        farbled_parent = measure(port, excl, "https://%s/" % PARENT_A, PARENT_A,
                                 timeout=args.timeout)
        show("parent (farbled)", farbled_parent)
        farbled_child = measure_iframe(port, excl, PARENT_A, timeout=args.timeout)
        show("child %s in it" % THIRD, farbled_child)
    finally:
        if backup and os.path.exists(backup):
            shutil.copy2(backup, path)
            os.remove(backup)
            print("\nrestored fingerprint_settings.json")

    native = exempt_parent  # an exempt top frame is a true native pass-through

    print("\n" + "=" * 78)
    print("CONTROLS")
    ok = True

    gate_ref = {f: exempt_parent.get(f) for f in GATE_CONTROLS}
    for label, v in (("exempt child", exempt_child),
                     ("farbled parent", farbled_parent),
                     ("farbled child", farbled_child)):
        if any(v.get(f) != gate_ref[f] for f in GATE_CONTROLS):
            print("    [FAIL] size-gate control moved on %s — realms not comparable" % label)
            ok = False
    if ok:
        print("    [PASS] size-gate control identical in all 4 realms   large=%s glLarge=%s"
              % (gate_ref["large"], gate_ref["glLarge"]))

    if same(farbled_parent, native):
        print("    [FAIL] farbling is not active at all — the non-exempt parent reads native;")
        print("           every '== native' verdict below would be vacuous.")
        ok = False
    else:
        print("    [PASS] farbling is active at all     non-exempt parent != native")

    rc = 0
    print("\nVERDICT")
    if not ok:
        print("    ⛔ CONTROLS FAILED — no verdict.")
        rc = 1
    elif not same(farbled_child, farbled_parent):
        if same(farbled_child, native):
            print("    ⛔ REGRESSION — the cross-site child is NATIVE even under a NON-exempt")
            print("       parent. P4e cross-site keying is not working, so this run cannot")
            print("       answer the D5 question at all.")
            rc = 2
        else:
            print("    ⚠️ THIRD OUTCOME — the child under a non-exempt parent matches neither")
            print("       its parent nor native. Investigate before reading the D5 arm.")
            rc = 3
    elif same(exempt_child, native):
        print("    ✅ D5 RESIDUAL CONFIRMED — and it is a real, measured residual bypass.")
        print("       Under a NON-exempt parent the child carries the parent's farbled key")
        print("       (%s), so cross-site keying is live. Under an EXEMPT parent the same"
              % farbled_parent["small"])
        print("       child reads NATIVE (%s) — the exemption is inherited by the subframe."
              % exempt_child["small"])
        print("       ⇒ any third party framed by an auth-exempt top frame reads true native")
        print("         values. This needs a release-note line.")
    elif same(exempt_child, farbled_parent):
        print("    ⚠️ D5 RESIDUAL NOT PRESENT — the child under an exempt parent is FARBLED.")
        print("       The exemption is not inherited. That contradicts the §Y4 code read and")
        print("       means the queued release-note line would have been WRONG.")
        rc = 3
    else:
        print("    ⚠️ THIRD OUTCOME on the exempt arm — child matches neither native nor the")
        print("       farbled parent. Investigate.")
        rc = 3

    print("\n    fields compared: %s" % ", ".join(FARBLED_FIELDS))
    for f in FARBLED_FIELDS:
        print("      %-14s exempt-child=%-10s native=%-10s farbled-parent=%-10s farbled-child=%s"
              % (f, exempt_child.get(f), native.get(f),
                 farbled_parent.get(f), farbled_child.get(f)))
    print("\n  engine %s" % eng)
    return rc


if __name__ == "__main__":
    sys.exit(main())
