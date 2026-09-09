#!/usr/bin/env python3
"""
beta.3 Phase 7d R4 — the dual-store probe, macOS arm.
Rows: P7d-A4 (notifications), A5 (location), A6 (clipboard), A7 (reset),
A8 (camera/mic must NOT be mirrored).

WHY A SECOND FILE RATHER THAN A FLAG ON `dual_store_probe.py`
------------------------------------------------------------
The Windows probe gates every result on `validate_instrument()`, which reads a
CONTROL ORIGIN that already carries a real Chromium notifications BLOCK
(`https://www.youtube.com`, planted 2026-08-10) and refuses to report anything
until that origin reads "denied".

📏 MEASURED on this Mac 2026-09-08: our SQLite store carries the identical row
(`www.youtube.com | type 4 Notifications | state 2 Block | 2026-08-10 14:10:23`),
but the CHROMIUM side of that control was never planted here — notifications
only began mirroring in Phase 7d, i.e. in the build under test. So on macOS the
Windows gate cannot pass for a reason that has nothing to do with the subject,
and running it would print VACUOUS and measure nothing.

⛔ That is NOT a licence to drop the sensitivity question. It is answered a
different and strictly stronger way here:

  1. **The flip is self-validating (decision D-D).** A blind reader returns
     "prompt" on the broken build AND the fixed one — what it cannot do is
     FABRICATE "denied". So an observed prompt→denied transition proves the
     reader is sensitive and the mirror works, in one step. Only a NON-flip is
     ambiguous, and that case is reported UNRESOLVED, never as a pass.
  2. **Origin specificity control.** A second, untouched origin is read in the
     same breath and must stay "prompt". This rules out the failure the youtube
     gate cannot even see: a reader (or a mirror) that returns "denied" for
     everything. The Windows gate proves the reader can say "denied"; this also
     proves it can still say "prompt" while another origin is blocked.
  3. **A8 control.** camera/microphone on the SAME blocked origin must stay
     "prompt" — if the mirror widened onto the media path that is a defect, and
     it is observable on exactly the origin we just blocked.

⛔ SUBJECT DISCIPLINE. The subject is the SITE'S BEHAVIOUR, never the panel.
Every assertion reads the page's own view (`navigator.permissions.query`,
`Notification.permission`) from the external https target.

⛔ NOT USED: CDP `Browser.setPermission`. 📏 Measured a silent no-op on this CEF
build; believing its success reply produced three false verdicts on Windows.

⚠️ HOW THE BLOCK IS APPLIED. `site_permissions_set` is sent from the HEADER
browser, an internal origin. Sending it from the page is correctly DENIED by the
IPC allowlist (P4 M9), which would silently measure "Ask" on both arms. This is
the same C++ arm the Site controls panel's own click invokes —
`simple_handler.cpp :: site_permissions_set` → `SetState` +
`MirrorSitePermissionToChromium` — so the code under test is identical; what is
not exercised is the React widget, which is not what R4 is about.

USAGE
    python dual_store_probe_mac.py                 # after the fix
    python dual_store_probe_mac.py --expect-red    # invert exit code
"""

import json
import os
import sys
import time

try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
except AttributeError:
    pass

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "phase-1-overlay-input-dpi"))
import cdp  # noqa: E402

PORT = cdp.DEV_PORT
SUBJECT_URL = "https://example.com/"
SUBJECT_HOST = "example.com"
CONTROL_URL = "https://www.wikipedia.org/"   # origin-specificity control
CONTROL_HOST = "www.wikipedia.org"
HEADER = "=http://127.0.0.1:5137/"           # internal origin, sends the IPC
TYPES = ["notifications", "location", "clipboard"]

READ_JS = """(async function(){
  async function q(n){ try{ var r = await navigator.permissions.query({name:n});
                            return r.state; } catch(e){ return 'ERR:'+e.name; } }
  return JSON.stringify({
    origin: location.origin,
    notification_permission: (typeof Notification!=='undefined') ? Notification.permission : 'NO_API',
    notifications: await q('notifications'),
    location:      await q('geolocation'),
    clipboard:     await q('clipboard-read'),
    camera:        await q('camera'),
    microphone:    await q('microphone')
  });
})()"""

# Behavioural probes. Each is individually timed out: at baseline these would
# open a real prompt and hang forever, and a hung probe reports nothing at all.
BEHAVIOUR_JS = """(async function(){
  function withTimeout(p, ms, label){
    return Promise.race([p, new Promise(function(r){
      setTimeout(function(){ r('TIMEOUT('+label+') — no answer in '+ms+'ms, a prompt is probably open'); }, ms);
    })]);
  }
  var out = {};
  out.notification_requestPermission = await withTimeout(
    (async function(){ try { return await Notification.requestPermission(); }
                       catch(e){ return 'ERR:'+e.name; } })(), 6000, 'notif');
  out.geolocation = await withTimeout(new Promise(function(res){
      navigator.geolocation.getCurrentPosition(
        function(){ res('GOT LOCATION — RED'); },
        function(e){ res('error code ' + e.code); },
        {timeout: 5000});
    }), 8000, 'geo');
  out.clipboard_readText = await withTimeout(
    (async function(){ try { var t = await navigator.clipboard.readText();
                             return 'RESOLVED len=' + t.length; }
                       catch(e){ return 'rejected: ' + e.name; } })(), 6000, 'clip');
  return JSON.stringify(out);
})()"""


def read(url_substr):
    t, msg = cdp.evaluate(url_substr, READ_JS, PORT)
    r = msg.get("result", {})
    if "exceptionDetails" in r:
        sys.exit("VACUOUS: page eval threw — %s"
                 % json.dumps(r["exceptionDetails"])[:300])
    raw = r.get("result", {}).get("value")
    if raw is None:
        sys.exit("VACUOUS: no value from %s. Nothing measured." % url_substr)
    return t["url"], json.loads(raw)


def behaviour(url_substr):
    t, msg = cdp.evaluate(url_substr, BEHAVIOUR_JS, PORT)
    raw = msg.get("result", {}).get("result", {}).get("value")
    if raw is None:
        return {"ERROR": "no value — %s"
                % json.dumps(msg.get("result", {}))[:200]}
    return json.loads(raw)


def set_perms(host, states):
    """Drive the same IPC arm the Site controls panel's click invokes."""
    sets = "".join(
        "window.cefMessage.send('site_permissions_set', ['%s','%s','%s']);"
        % (host, code, state) for code, state in states.items())
    js = """(function(){ return new Promise(function(resolve){
      var got=null; window.onSitePermissionsResponse=function(d){got=d;};
      %s
      setTimeout(function(){
        window.cefMessage.send('site_permissions_get', ['%s']);
        setTimeout(function(){ resolve(JSON.stringify(got)); }, 800);
      }, 800);
    });})()""" % (sets, host)
    t, msg = cdp.evaluate(HEADER, js, PORT)
    raw = msg.get("result", {}).get("result", {}).get("value")
    if not raw or raw == "null":
        sys.exit("VACUOUS: the header browser never answered "
                 "site_permissions_get — the store half never happened, so the "
                 "mirror was not tested.")
    return {p["code"]: p["state"] for p in json.loads(raw)["permissions"]}


def reset_perms(host):
    js = """(function(){ return new Promise(function(resolve){
      var got=null; window.onSitePermissionsResponse=function(d){got=d;};
      window.cefMessage.send('site_permissions_reset', ['%s']);
      setTimeout(function(){
        window.cefMessage.send('site_permissions_get', ['%s']);
        setTimeout(function(){ resolve(JSON.stringify(got)); }, 800);
      }, 800);
    });})()""" % (host, host)
    t, msg = cdp.evaluate(HEADER, js, PORT)
    raw = msg.get("result", {}).get("result", {}).get("value")
    if not raw or raw == "null":
        sys.exit("VACUOUS: reset produced no store readback.")
    return {p["code"]: p["state"] for p in json.loads(raw)["permissions"]}


def show(label, d):
    print("  %-8s origin=%s  %s" % (
        label, d["origin"],
        {k: d[k] for k in ["notifications", "location", "clipboard",
                           "camera", "microphone"]}))


def main():
    expect_red = "--expect-red" in sys.argv
    failures, unresolved = [], []

    print("=== A. baseline — subject and specificity control, nothing blocked ===")
    su, before = read(SUBJECT_URL)
    cu, ctl_before = read(CONTROL_URL)
    print("subject target: %s" % su)
    print("control target: %s" % cu)
    show("subject", before)
    show("control", ctl_before)

    for c in TYPES:
        if before[c] == "denied":
            sys.exit("VACUOUS: %s already reads 'denied' on the subject BEFORE "
                     "anything was blocked. There is no flip left to observe; "
                     "reset the profile's content settings first." % c)

    print("\n=== B. block the three types on %s (IPC from internal origin) ===" % SUBJECT_HOST)
    store = set_perms(SUBJECT_HOST, {c: "block" for c in TYPES})
    print("  our store now: %s" % {c: store[c] for c in TYPES})
    for c in TYPES:
        if store[c] != "block":
            sys.exit("VACUOUS: our own store did not record block for %s — the "
                     "panel half never happened, so the mirror was not tested." % c)
    time.sleep(1.0)

    _, after = read(SUBJECT_URL)
    _, ctl_after = read(CONTROL_URL)
    show("subject", after)
    show("control", ctl_after)

    print("\n  behavioural probes on the blocked subject:")
    for k, v in behaviour(SUBJECT_URL).items():
        print("    %-32s %s" % (k, v))

    for c in TYPES:
        if after[c] == "denied":
            print("  FLIP  : %s prompt→denied (reader proven sensitive AND mirror proven, D-D)" % c)
        else:
            unresolved.append(
                "%s: store says block, site says %r — either the mirror did not "
                "reach this type on macOS or the reader cannot see it. "
                "UNRESOLVED, not a pass." % (c, after[c]))

    # Origin specificity — the control that the youtube gate cannot provide.
    for c in TYPES:
        if ctl_after[c] == "denied":
            failures.append(
                "SPECIFICITY: %s reads 'denied' on %s, which was never blocked. "
                "The reader or the mirror is not origin-scoped, so every "
                "'denied' above is worthless." % (c, CONTROL_HOST))

    # A8 — camera/mic must NOT have been mirrored.
    for c in ["camera", "microphone"]:
        if after[c] == "denied":
            failures.append(
                "A8: %s acquired a Chromium denial on the blocked origin; it must "
                "stay governed by our store with no content setting written." % c)
        else:
            print("  A8    : %s stays %r on the blocked origin — mirror did not widen" % (c, after[c]))

    print("\n=== C. reset permissions for the site (A7) ===")
    rstore = reset_perms(SUBJECT_HOST)
    print("  our store now: %s" % {c: rstore[c] for c in TYPES})
    time.sleep(1.0)
    _, after_reset = read(SUBJECT_URL)
    show("subject", after_reset)
    for c in TYPES:
        if after_reset[c] == "denied":
            failures.append("A7: reset left %s still denied on the site" % c)

    print()
    for u in unresolved:
        print("UNRESOLVED: %s" % u)
    for f in failures:
        print("RED       : %s" % f)

    if failures:
        print("\nRESULT: RED (%d)" % len(failures))
        sys.exit(0 if expect_red else 1)
    if unresolved:
        print("\nRESULT: INCOMPLETE — %d type(s) never flipped. Not a pass." % len(unresolved))
        sys.exit(0 if expect_red else 2)
    print("\nRESULT: GREEN — the panel's decision governs the site, scoped to "
          "the origin, without touching camera/mic")
    if expect_red:
        print("⛔ --expect-red was set but everything passed.")
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
