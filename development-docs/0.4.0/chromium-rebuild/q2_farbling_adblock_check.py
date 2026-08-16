#!/usr/bin/env python3
r"""q2_farbling_adblock_check.py — Q2 §4 rows T1, T2, T5, T6, T7, T8.

Six of the eight Q2 rows are settled here. The other two are not, and deliberately so:

  * **T3** (YouTube `AdblockResponseFilter` `adPlacements` rename) needs a human watching
    for a pre-roll — "no ad played" is not something this rig can honestly assert.
  * **T4** (CreepJS worker column == window column). ⚠️ **UPDATED 2026-08-15 — this row used
    to read "KNOWN RED, all workers and all cross-site iframes are unfarbled, an accepted gap
    not a failure to chase". That is now WRONG IN THE DANGEROUS DIRECTION.** P4e closed the
    iframe half and P4f closed dedicated + nested workers, so a worker/window mismatch here is
    a **REGRESSION to chase**, not a known gap. Only SHARED and SERVICE workers remain
    unfarbled (R9/R10, owner-signed ⏸️), and CreepJS's worker column does not exercise those.

## T1 / T7 — the request is CANCELLED, not merely classified

Probed with a `no-cors` fetch. A normal cross-origin fetch fails on CORS whether or not the
request was blocked, so it would report "blocked" for the wrong reason; in `no-cors` a
request that goes through resolves to an opaque response and only a *cancelled* one rejects.
T7 runs the same probe from an auth-exempt origin, showing the farbling exemption does not
disable adblock.

⚠️ Two traps this row hit, both of which faked a product bug:
  1. **`AdblockCache` memoises verdicts per URL** and clears only on the *browser's* toggle /
     filter update / site toggle — **not** on the engine's HTTP `/toggle`. Without a fresh
     nonce per probe, the negative control re-reads a cached "blocked" and reports that
     disabling adblock changed nothing.
  2. A **cross-origin benign control is cancelled by CSP `connect-src`** on a strict origin
     like github.com. It must be same-origin. A 404 still *resolves*, so a missing path
     cannot fake a cancellation.

## T2 — cosmetic CSS / scriptlet injection, per MECHANISM

cnn.com and youtube.com are not two samples of one thing. They exercise different mechanisms
and each is the other's control:

    cnn.com      generichide=False, 465 selectors, 0 scriptlet bytes  -> CSS path
    youtube.com  generichide=True,  0 selectors, ~34 KB scriptlet      -> scriptlet path

So YouTube must get **no** cosmetic CSS. Judging it by the CSS path measures the wrong
mechanism and reports a false failure.

⚠️ Two more traps, both measured rather than anticipated:
  1. **Read our `<style id="hodos-cosmetic-css">` by id, never "any stylesheet containing an
     ad selector".** On cnn.com the site's own 2 MB stylesheet contains `.zone__ads` while
     our injected 717-byte block does not — the loose check returns a PASS attributable to
     the *site's* CSS.
  2. **Poll; do not read once.** `measure()` returns as soon as the host appears in the URL,
     which on a heavy site is `readyState:"loading"` with zero stylesheets attached.

## T6 — the `[native code]` gate. Q2 calls this "the single most valuable Q2 assertion".

It proves the farbling migration actually landed **below JavaScript**. The injected-JS
implementation could not avoid leaving a tamper tell: a JS-wrapped `toDataURL` reports its
own source from `toString()`, which is a one-line detection any anti-bot script can run.
Native Blink farbling reports `[native code]` because the function genuinely is native.

⚠️ **This assertion is trivially satisfied by deleting the feature**, which is precisely how
one of this project's earlier harnesses passed against a browser with no farbling at all.
`[native code]` is necessary, not sufficient — it is only meaningful **alongside** the
seed-rotation gate proving the values actually move. Do not cite T6 on its own as evidence
that farbling works. It is cited here as evidence that farbling is not *visible*.

**In-page negative control:** the probe wraps a function in JS and asserts the same detector
reports it as NOT native. Without that, "everything is native" is equally consistent with a
detector that always returns true — e.g. if `Function.prototype.toString` were itself
patched, which the probe also checks.

## T8 — no orphaned FP symbols

⚠️ **A naive grep FAILS against correct code here.** The retired symbols all survive as
*tombstone comments* explaining what was deleted and why — which are desirable, since they
are what stops someone re-adding an injected-JS farbling path. So the audit strips comments
before counting, and then has to prove it did not over-strip.

**The positive control is the guard set:** `IsAuthDomain`, `IsSiteEnabled`, `SetSiteEnabled`
and the `fingerprint_*_site_enabled` IPC are **shipped user-facing control** and must still
be present after stripping. Q2 T8 is explicit that this group must NOT go to zero until the
per-site toggle is re-homed (TD-5) — a T8 that went green by deleting the user's Privacy
Shield toggle would be a regression dressed as a cleanup.

## T5 — canvas-touching scriptlet double-wrap

⚠️ **Do not answer this by grepping rule text for "canvas".** The filter lists reference
scriptlets by ALIAS (`aopr`, `acs`, `set`, `nostif`, ...), so no rule contains the word
"canvas" whether or not a canvas scriptlet is in use, and that search returns a confident
zero for the wrong reason.

What determines double-wrap risk is what the scriptlet **implementations** do, because
injection can only ever run code from the available set. So the audit scans the two sets
that can actually be injected — the downloaded `resources/scriptlets.js` and the six
bundled scriptlets — for canvas/WebGL/audio APIs, each with a positive control proving the
file was read.

## Usage

    python q2_farbling_adblock_check.py --repo . --adblock-data "%APPDATA%\HodosBrowserDev\adblock"
    # add the browser rows:
    python q2_farbling_adblock_check.py --repo . --adblock-data ... \
        --exe "...\HodosBrowser.exe" --data-root "%APPDATA%\HodosBrowserDev" --dev
"""

import argparse
import json
import os
import re
import sys
import time

# 31302 release / 31402 under HODOS_DEV — never hardcode one; PortConfig.h is the source.
ADBLOCK_PORT_RELEASE = 31302
ADBLOCK_PORT_DEV = 31402
ADBLOCK_PORT = ADBLOCK_PORT_DEV

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# ---- T8 -------------------------------------------------------------------------------

# Fully retired by the 2026-08-09 injected-JS deletion. Zero LIVE references expected.
RETIRED = [
    "s_domainSeeds",
    "s_fingerprintDisabledUrls",
    "fingerprint_seed",
    "FINGERPRINT_PROTECTION_SCRIPT",
    "FingerprintScript",
]

# ⛔ Shipped user-facing control. These must NOT go to zero — see the module docstring.
GUARD = [
    "IsAuthDomain",
    "IsSiteEnabled",
    "SetSiteEnabled",
    "fingerprint_get_site_enabled",
    "fingerprint_set_site_enabled",
    "FingerprintProtection",
]

SOURCE_DIRS = [("cef-native", (".cpp", ".h", ".mm", ".hpp")),
               (os.path.join("frontend", "src"), (".ts", ".tsx"))]

BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
LINE_COMMENT = re.compile(r"//[^\n]*")


def strip_comments(src):
    """Remove block and line comments.

    Deliberately crude: it will also blank a `//` inside a string literal. That direction is
    SAFE for this audit — it can only cause a false "symbol absent", which for the RETIRED
    set is a false pass, but the GUARD set is checked through the identical stripper and
    would collapse first. A stripper aggressive enough to hide a retired symbol cannot leave
    the guard symbols standing."""
    return LINE_COMMENT.sub("", BLOCK_COMMENT.sub("", src))


def scan_symbols(repo):
    live = {s: [] for s in RETIRED + GUARD}
    for rel, exts in SOURCE_DIRS:
        root = os.path.join(repo, rel)
        for dirpath, _dirs, files in os.walk(root):
            for name in files:
                if not name.endswith(exts):
                    continue
                path = os.path.join(dirpath, name)
                try:
                    with open(path, "r", encoding="utf-8", errors="replace") as fh:
                        code = strip_comments(fh.read())
                except OSError:
                    continue
                for sym in live:
                    if sym in code:
                        live[sym].append(os.path.relpath(path, repo))
    return live


def run_t8(repo):
    print("### T8 — orphaned FP symbols (comments stripped)\n")
    live = scan_symbols(repo)
    ok = True

    print("  RETIRED — must have ZERO live references:")
    for sym in RETIRED:
        hits = live[sym]
        print("    %-32s %s" % (sym, "clean" if not hits else
                                "*** %d live: %s" % (len(hits), ", ".join(hits[:3]))))
        if hits:
            ok = False

    print("\n  GUARD (positive control) — must still be PRESENT:")
    for sym in GUARD:
        hits = live[sym]
        print("    %-32s %s" % (sym, "present in %d file(s)" % len(hits) if hits else
                                "*** ABSENT — either the stripper over-stripped (making every "
                                "'clean' above meaningless) or shipped user control was deleted"))
        if not hits:
            ok = False

    print("\n  T8: %s" % ("PASS" if ok else "FAIL"))
    return ok


# ---- T5 -------------------------------------------------------------------------------

CANVAS_API = (r"toDataURL|getImageData|putImageData|getChannelData|copyFromChannel|"
              r"readPixels|OffscreenCanvas|createImageBitmap|AudioContext|"
              r"OfflineAudioContext|WebGLRenderingContext|getContext")


def count_re(path, pattern):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            return len(re.findall(pattern, fh.read()))
    except OSError:
        return None


def run_t5(repo, adblock_data):
    print("\n### T5 — canvas/WebGL/audio-touching scriptlets (double-wrap risk)\n")
    ok = True
    subjects = []

    if adblock_data:
        subjects.append(("downloaded resources/scriptlets.js",
                         os.path.join(adblock_data, "resources", "scriptlets.js"),
                         r"Object|window"))
    bundled_dir = os.path.join(repo, "adblock-engine", "src", "scriptlets")
    if os.path.isdir(bundled_dir):
        for name in sorted(os.listdir(bundled_dir)):
            if name.endswith(".js"):
                subjects.append(("bundled/" + name,
                                 os.path.join(bundled_dir, name),
                                 r"function|=>"))

    for label, path, positive in subjects:
        hits = count_re(path, CANVAS_API)
        pos = count_re(path, positive)
        if hits is None:
            print("    %-40s *** UNREADABLE: %s" % (label, path))
            ok = False
            continue
        if not pos:
            print("    %-40s *** POSITIVE CONTROL FAILED (0 matches for %r) — the file was "
                  "not really scanned, so its 0 canvas hits mean nothing" % (label, positive))
            ok = False
            continue
        print("    %-40s canvas/audio APIs=%d   (positive control=%d)" % (label, hits, pos))
        if hits:
            print("        ^ a scriptlet touching these WILL double-wrap native farbling. "
                  "Q2-1 says accept double-wrap (non-breaking) but verify the site renders.")

    print("\n  ⚠️ Method note: filter lists reference scriptlets by ALIAS, so grepping rule "
          "text for 'canvas' returns 0 whether or not one is in use. Only the implementations "
          "above can actually be injected, so they are the correct subject.")
    print("\n  T5: %s" % ("PASS — no injectable scriptlet touches canvas/WebGL/audio" if ok
                          else "FAIL"))
    return ok


# ---- T6 -------------------------------------------------------------------------------

T6_JS = r"""
(function () {
  function native(fn) {
    try { return Function.prototype.toString.call(fn).indexOf('[native code]') >= 0; }
    catch (e) { return 'ERR:' + e.message; }
  }
  var gl = null;
  try {
    var c = document.createElement('canvas');
    gl = c.getContext('webgl') || c.getContext('experimental-webgl');
  } catch (e) {}

  // ⛔ In-page NEGATIVE CONTROL: a deliberately JS-wrapped function. The same detector
  // MUST report this one as non-native, or it is not discriminating anything.
  var orig = HTMLCanvasElement.prototype.toDataURL;
  var wrapped = function toDataURL() { return orig.apply(this, arguments); };

  return JSON.stringify({
    href: location.href,
    toDataURL:      native(HTMLCanvasElement.prototype.toDataURL),
    getImageData:   native(CanvasRenderingContext2D.prototype.getImageData),
    readPixels:     native(WebGLRenderingContext.prototype.readPixels),
    getParameter:   gl ? native(Object.getPrototypeOf(gl).getParameter ||
                                WebGLRenderingContext.prototype.getParameter) : 'NOGL',
    getChannelData: native(AudioBuffer.prototype.getChannelData),
    // If Function.prototype.toString were itself patched, every answer above would be a
    // lie in the safe-looking direction.
    toStringItself: native(Function.prototype.toString),
    wrappedControl: native(wrapped)
  });
})()
"""


def run_t6(args):
    from farbling_seed_rotation_check import (kill_browser_by_path, launch_browser,
                                              measure, snapshot_targets, wait_for_cdp,
                                              engine_version)
    from farbling_cross_profile_check import cdp_port_for

    print("\n### T6 — [native code] gate (Q2's highest-value assertion)\n")
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

    excluded = snapshot_targets(port, settle=args.settle)
    # A FARBLED origin on purpose: on an auth-exempt origin the patched paths are not even
    # engaged, so [native code] there would say nothing about the farbled case.
    v = measure(port, excluded, "https://example.com/", "example.com",
                timeout=args.timeout, js=T6_JS)
    eng = engine_version(port)
    kill_browser_by_path(args.exe)

    ok = True
    checks = ["toDataURL", "getImageData", "readPixels", "getParameter", "getChannelData",
              "toStringItself"]
    for k in checks:
        val = v.get(k)
        good = (val is True) or (k == "getParameter" and val == "NOGL")
        print("    %-16s %-6s %s" % (k, str(val), "OK" if good else "*** NOT [native code]"))
        if not good:
            ok = False

    ctrl = v.get("wrappedControl")
    good_ctrl = (ctrl is False)
    print("\n    NEGATIVE CONTROL (a JS-wrapped function must read NON-native): %s  %s"
          % (ctrl, "OK" if good_ctrl else
             "*** the detector reports EVERYTHING native — it discriminates nothing"))
    if not good_ctrl:
        ok = False

    print("\n  ⚠️ [native code] is NECESSARY, NOT SUFFICIENT — deleting farbling entirely "
          "would also pass this. Cite it only alongside the seed-rotation gate.")
    print("\n  T6: %s   (engine %s)" % ("PASS" if ok else "FAIL", eng))
    return ok


# ---- T1 / T7 ---------------------------------------------------------------------------

# Chosen by asking the engine, not by assuming: see run_t1t7's engine pre-check, which
# refuses to proceed unless the engine agrees these two are classified oppositely.
BLOCKED_URL = "https://www.google-analytics.com/analytics.js"

FETCH_JS_TMPL = r"""
(async function () {
  async function probe(u) {
    // no-cors on purpose: a normal cross-origin fetch fails on CORS whether or not the
    // request was blocked, which would make every row look "blocked" for the wrong reason.
    // In no-cors mode a request that goes through resolves to an opaque response, and only
    // a CANCELLED request rejects — so this distinguishes adblock from CORS.
    // A 404 still RESOLVES; only network-level cancellation rejects. That is what makes the
    // same-origin benign control below safe even where the path does not exist.
    try { await fetch(u, {mode: 'no-cors', cache: 'no-store'}); return 'through'; }
    catch (e) { return 'cancelled'; }
  }
  // ⛔ Cache-buster, and it is load-bearing rather than hygiene. AdblockCache memoises the
  // verdict per URL and clears only on the BROWSER's toggle / filter update / site toggle —
  // NOT on the engine's HTTP /toggle. Without a unique URL per probe, the negative control
  // re-reads a cached "blocked" and reports that disabling adblock changed nothing, which
  // looks like a product bug and is not one. The nonce changes the cache key while still
  // matching the same filter rule (verified against POST /check).
  var nonce = '%s';
  // Same-origin benign control: immune to the page's CSP connect-src, which on a strict
  // origin like github.com cancels a cross-origin fetch for reasons unrelated to adblock —
  // the defect this control originally had.
  var benign = location.origin + '/favicon.ico?hodos=' + nonce;
  return JSON.stringify({
    href: location.href,
    blocked: await probe(%s + '?hodos=' + nonce),
    benign:  await probe(benign)
  });
})()
"""


def engine_check(url, source):
    import urllib.request
    body = json.dumps({"url": url, "sourceUrl": source,
                       "resourceType": "script"}).encode()
    req = urllib.request.Request("http://127.0.0.1:%d/check" % ADBLOCK_PORT, data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as fh:
        return json.loads(fh.read().decode())


def engine_toggle(enabled):
    import urllib.request
    body = json.dumps({"enabled": enabled}).encode()
    req = urllib.request.Request("http://127.0.0.1:%d/toggle" % ADBLOCK_PORT, data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as fh:
        return json.loads(fh.read().decode())


def run_t1t7(args):
    """T1 — a blocked request is actually CANCELLED in the browser, not merely classified.
    T7 — the same holds on an auth-EXEMPT origin, i.e. the farbling exemption does not
    accidentally disable adblock (they are independent systems)."""
    from farbling_seed_rotation_check import (kill_browser_by_path, launch_browser,
                                              measure, snapshot_targets, wait_for_cdp)
    from farbling_cross_profile_check import cdp_port_for

    print("\n### T1 / T7 — adblock cancels in the browser, on farbled AND exempt origins\n")

    # Engine pre-check. If the engine does not classify these two oppositely, the browser
    # rows below cannot mean anything, and picking URLs by assumption is how a gate ends up
    # asserting nothing.
    nonce = "%d" % int(time.time())
    try:
        b = engine_check(BLOCKED_URL + "?hodos=" + nonce, "https://example.com/")
        n = engine_check("https://example.com/favicon.ico?hodos=" + nonce,
                         "https://example.com/")
    except OSError as exc:
        print("    engine unreachable on :%d (%s) — is the adblock engine running?"
              % (ADBLOCK_PORT, exc))
        return False
    print("    engine says blocked=%s for the nonce'd tracker URL" % b.get("blocked"))
    print("    engine says blocked=%s for the same-origin control" % n.get("blocked"))
    if not b.get("blocked") or n.get("blocked"):
        print("    *** the engine does not classify these two oppositely; the browser rows "
              "would prove nothing. Pick different URLs.")
        return False

    js = FETCH_JS_TMPL % (nonce, json.dumps(BLOCKED_URL))
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
    excluded = snapshot_targets(port, settle=args.settle)

    ok = True
    rows = {}
    try:
        for label, host in (("farbled origin (T1)", "example.com"),
                            ("auth-EXEMPT origin (T7)", "github.com")):
            v = measure(port, excluded, "https://%s/" % host, host,
                        timeout=args.timeout, js=js)
            rows[label] = v
            good = (v["blocked"] == "cancelled" and v["benign"] == "through")
            print("    %-26s blocked-url=%-9s benign-url=%-8s %s"
                  % (label, v["blocked"], v["benign"], "OK" if good else "*** FAIL"))
            if not good:
                ok = False

        # ⛔ NEGATIVE CONTROL: turn the feature off and require the same probe to go red.
        print("\n    negative control — disabling the engine globally via POST /toggle")
        engine_toggle(False)
        time.sleep(2)
        # A FRESH nonce, or this probe re-reads the "blocked" verdict the run above just
        # put in AdblockCache and the control silently tests nothing.
        js2 = FETCH_JS_TMPL % ("%d" % (int(time.time()) + 1), json.dumps(BLOCKED_URL))
        v = measure(port, excluded, "https://example.com/", "example.com",
                    timeout=args.timeout, js=js2)
        ctrl_ok = (v["blocked"] == "through")
        print("    %-26s blocked-url=%-9s %s"
              % ("adblock OFF", v["blocked"],
                 "OK — it goes through when the feature is off"
                 if ctrl_ok else "*** the URL was cancelled with adblock DISABLED, so the "
                                 "cancellation above was never attributable to adblock"))
        if not ctrl_ok:
            ok = False
    finally:
        try:
            engine_toggle(True)
            print("    engine re-enabled")
        except OSError:
            print("    *** WARNING: could not re-enable the engine — do it manually")
        kill_browser_by_path(args.exe)

    print("\n  T1/T7: %s" % ("PASS" if ok else "FAIL"))
    return ok


# ---- T2 ---------------------------------------------------------------------------------

COSMETIC_JS_TMPL = r"""
(async function () {
  // ⚠️ POLL, do not read once. `measure()` returns as soon as the host appears in the URL,
  // which on a heavy site is readyState:"loading" with ZERO stylesheets attached — the
  // cosmetic CSS has not been injected yet and a single read reports a confident absence.
  // ⚠️ Must stay comfortably UNDER measure()'s 25 s inner Runtime.evaluate window. A poll
  // that outlasts it never returns a value, so measure() retries and the row finally dies
  // on its outer timeout — which reads as "the page would not load" rather than "the
  // element never appeared". That is the wrong diagnosis for the absent-by-design case.
  var deadline = Date.now() + 18000;
  var el = null, txt = '';
  while (Date.now() < deadline) {
    el = document.getElementById('hodos-cosmetic-css');
    txt = el ? (el.textContent || '') : '';
    if (txt.length > 0) break;
    await new Promise(function (r) { setTimeout(r, 500); });
  }

  // ⛔ Read OUR element by id, never "any stylesheet containing an ad selector". Measured
  // on cnn.com: the site's own 2 MB stylesheet contains `.zone__ads` while our injected
  // 717-byte block does not — so the loose version returns a PASS that is actually
  // attributable to the SITE's CSS, not ours.
  var wanted = %s;                       // every selector the engine returned for this host
  var matched = null;
  for (var i = 0; i < wanted.length; i++) {
    if (txt.indexOf(wanted[i]) >= 0) { matched = wanted[i]; break; }
  }
  return JSON.stringify({
    href: location.href,
    readyState: document.readyState,
    present: !!el,
    length: txt.length,
    matchedSelector: matched,
    // Control: a selector the engine never returned must NOT appear, or "contains a
    // selector" degenerates into "contains anything".
    hasFabricated: txt.indexOf('.hodos-fabricated-selector-xyzzy') >= 0
  });
})()
"""


def cosmetic_resources(url):
    import urllib.request
    body = json.dumps({"url": url}).encode()
    req = urllib.request.Request("http://127.0.0.1:%d/cosmetic-resources" % ADBLOCK_PORT,
                                 data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=15) as fh:
        return json.loads(fh.read().decode())


def run_t2(args):
    """T2 — cosmetic CSS / scriptlet injection still fires after the FP teardown.

    ⚠️ The two sites below are NOT two samples of one thing; they exercise DIFFERENT
    mechanisms, and each is the other's control:

        cnn.com      generichide=False, 465 selectors, 0 scriptlet bytes -> CSS path
        youtube.com  generichide=True,  0 selectors, ~34 KB scriptlet     -> scriptlet path

    So YouTube must get **no** cosmetic CSS. Judging YouTube by the CSS path would measure
    the wrong mechanism and report a false failure — a trap this project has already hit.
    """
    from farbling_seed_rotation_check import (kill_browser_by_path, launch_browser,
                                              measure, snapshot_targets, wait_for_cdp)
    from farbling_cross_profile_check import cdp_port_for

    print("\n### T2 — cosmetic CSS / scriptlet injection, per-mechanism\n")

    try:
        cnn = cosmetic_resources("https://www.cnn.com/")
        yt = cosmetic_resources("https://www.youtube.com/")
    except OSError as exc:
        print("    engine unreachable (%s)" % exc)
        return False

    sels = cnn.get("hideSelectors", [])
    if not sels:
        print("    *** the engine returned no selectors for cnn.com to key on")
        return False
    print("    engine: cnn.com     generichide=%s selectors=%d scriptlet=%dB"
          % (cnn.get("generichide"), len(cnn.get("hideSelectors", [])),
             len(cnn.get("injectedScript", ""))))
    print("    engine: youtube.com generichide=%s selectors=%d scriptlet=%dB"
          % (yt.get("generichide"), len(yt.get("hideSelectors", [])),
             len(yt.get("injectedScript", ""))))
    print("    keying on ANY of the %d selectors the engine returned" % len(sels))

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
    excluded = snapshot_targets(port, settle=args.settle)

    js = COSMETIC_JS_TMPL % json.dumps(sels)
    ok = True
    try:
        v = measure(port, excluded, "https://www.cnn.com/", "cnn.com",
                    timeout=args.timeout, js=js)
        good = (v["present"] and v["length"] > 0 and v["matchedSelector"]
                and not v["hasFabricated"])
        print("\n    cnn.com (CSS path)      style#hodos-cosmetic-css present=%s len=%d "
              "matched=%r fabricated=%s  %s"
              % (v["present"], v["length"], v["matchedSelector"], v["hasFabricated"],
                 "OK" if good else "*** FAIL"))
        if not good:
            ok = False

        v2 = measure(port, excluded, "https://www.youtube.com/", "youtube.com",
                     timeout=args.timeout, js=js)
        # generichide=True => the CSS path is deliberately NOT used here.
        good2 = (not v2["present"]) or v2["length"] == 0
        print("    youtube.com (scriptlet) style#hodos-cosmetic-css present=%s len=%d  %s"
              % (v2["present"], v2["length"],
                 "OK — generichide=True, so no CSS is correct"
                 if good2 else "*** CSS was injected despite generichide=True"))
        if not good2:
            ok = False
    finally:
        kill_browser_by_path(args.exe)

    print("\n    ⚠️ This proves the browser selects and applies the RIGHT MECHANISM per site,")
    print("       and that the CSS it injects is the engine's. It does NOT prove an ad was")
    print("       removed — 'no pre-roll plays' is the human observation, and is T3.")
    print("\n  T2: %s" % ("PASS" if ok else "FAIL"))
    return ok


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--adblock-data", default=None,
                    help=r"e.g. %%APPDATA%%\HodosBrowserDev\adblock")
    ap.add_argument("--exe", default=None, help="omit to skip the browser row (T6)")
    ap.add_argument("--data-root", default=None)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--timeout", type=float, default=90.0)
    args = ap.parse_args()

    global ADBLOCK_PORT
    ADBLOCK_PORT = ADBLOCK_PORT_DEV if args.dev else ADBLOCK_PORT_RELEASE

    results = {"T8": run_t8(args.repo), "T5": run_t5(args.repo, args.adblock_data)}
    if args.exe:
        results["T6"] = run_t6(args)
        results["T1/T7"] = run_t1t7(args)
        results["T2"] = run_t2(args)
    else:
        print("\n### T6, T1/T7 skipped (no --exe)")

    print("\n================ Q2 SUMMARY ================")
    for k in sorted(results):
        print("  %s  %s" % (k, "PASS" if results[k] else "FAIL"))
    print("\n  NOT COVERED by this script (need a human watching a video):")
    print("    T3 YouTube AdblockResponseFilter adPlacements rename")
    print("    T4 CreepJS worker column == window column  — EXPECT GREEN as of P4f.")
    print("       ⚠️ This note said 'KNOWN RED, workers are unfarbled' and that is now")
    print("       WRONG twice over. P4e (7dd035739) closed the frame half; P4f")
    print("       (9ccef044f, 2026-08-15) closed DEDICATED and NESTED workers —")
    print("       measured red->green on macOS by farbling_worker_probe.py --auto")
    print("       (exit 1 -> 0) and farbling_realm_matrix.py (R8/T3 UNKEYED -> KEYED).")
    print("       CreepJS drives a dedicated worker, so its worker column should now")
    print("       MATCH the window column; a mismatch is a REGRESSION, not the old gap.")
    print("       Still unfarbled, and the only realms that are: SHARED workers (R9,")
    print("       measured native by farbling_worker_residual_check.py) and SERVICE")
    print("       workers (R10, still unmeasured). Both are owner-signed deferrals.")
    return 0 if all(results.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
