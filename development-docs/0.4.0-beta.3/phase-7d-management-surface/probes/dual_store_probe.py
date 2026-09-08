#!/usr/bin/env python3
"""
beta.3 Phase 7d R4 — the dual-store probe (`TICKET_site_permission_dual_store.md`).

Rows: P7d-A4 (notifications), A5 (location), A6 (clipboard), A7 (reset), A8
(camera/mic must NOT be mirrored).

⛔ THE SUBJECT IS THE SITE, NOT THE PANEL. The panel rendering "Block" is the
lie under test. Every assertion here reads the *page's* view of the permission
(`Notification.permission`, `navigator.permissions.query`) and never the panel.

⛔ INSTRUMENT SENSITIVITY IS ASSERTED, NOT ASSUMED. A probe that cannot observe
a block prints the same "prompt" on the broken build and the fixed one, which
would make them indistinguishable. `--validate-instrument` reads a control
origin that already carries a real Chromium block (youtube.com / notifications)
and requires it to report "denied" before any result below is trusted.

⛔ It does NOT use CDP `Browser.setPermission` to plant one. 📏 Measured
2026-09-08 on this CEF build: that command is a SILENT NO-OP — it returns
success and changes nothing. Types with no control origin are reported
UNPROVEN, never assumed sensitive and never called blind.

USAGE
    python dual_store_probe.py --validate-instrument
    python dual_store_probe.py --capture-red      # before the fix
    python dual_store_probe.py                    # after the fix
"""

import json
import os
import sys

# Windows consoles default to cp1252, which raises UnicodeEncodeError on the
# marks this project's output uses. Reconfigure rather than strip them: a probe
# that dies while PRINTING a result has thrown the result away.
try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except AttributeError:  # pragma: no cover - py<3.7
    pass
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "phase-1-overlay-input-dpi"))
import cdp  # noqa: E402

PORT = cdp.DEV_PORT
SITE = "=https://example.com/"
SITE_HOST = "example.com"
PANEL = "site-info"

# Our capability code -> the name navigator.permissions.query understands.
# ⚠️ clipboard-read maps to CEF's CLIPBOARD_READ_WRITE. CLIPBOARD_SANITIZED_WRITE
# is documented "special-cased in the permissions layer to always allow" and has
# no prefs data, so it is NOT blockable and is deliberately not asserted here.
QUERY_NAME = {
    "notifications": "notifications",
    "location": "geolocation",
    "clipboard": "clipboard-read",
}

READ_JS = """(async function(){
  async function q(n){ try{ var r = await navigator.permissions.query({name:n});
                            return r.state; } catch(e){ return 'ERR:'+e.name; } }
  return JSON.stringify({
    origin: location.origin,
    notification_permission: (typeof Notification!=='undefined') ? Notification.permission : 'NO_API',
    notifications:  await q('notifications'),
    location:       await q('geolocation'),
    clipboard:      await q('clipboard-read'),
    camera:         await q('camera'),
    microphone:     await q('microphone')
  });
})()"""


def read_site():
    t, msg = cdp.evaluate(SITE, READ_JS, PORT)
    r = msg.get("result", {})
    if "exceptionDetails" in r:
        sys.exit("VACUOUS: page eval threw — %s"
                 % json.dumps(r["exceptionDetails"])[:300])
    raw = r.get("result", {}).get("value")
    if raw is None:
        sys.exit("VACUOUS: no value from the page. Nothing measured.")
    return t["url"], json.loads(raw)


def set_panel(states):
    """Drive the Site controls panel's own IPC — the surface the ticket is about.
    `states` maps our capability code -> 'allow' | 'block' | 'ask'."""
    sets = "".join(
        "window.cefMessage.send('site_permissions_set', ['%s','%s','%s']);"
        % (SITE_HOST, code, state) for code, state in states.items())
    js = """(function(){ return new Promise(function(resolve){
      var got=null; window.onSitePermissionsResponse=function(d){got=d;};
      %s
      setTimeout(function(){
        window.cefMessage.send('site_permissions_get', ['%s']);
        setTimeout(function(){ resolve(JSON.stringify(got)); }, 700);
      }, 700);
    });})()""" % (sets, SITE_HOST)
    t, msg = cdp.evaluate(PANEL, js, PORT)
    raw = msg.get("result", {}).get("result", {}).get("value")
    if not raw or raw == "null":
        sys.exit("VACUOUS: the panel never answered site_permissions_get, so "
                 "the store was not confirmed. Nothing measured.")
    return json.loads(raw)


def store_states(resp):
    return {p["code"]: p["state"] for p in resp["permissions"]}


# A control origin that already carries a Chromium content-setting BLOCK,
# created outside this probe and outside our store. It is the evidence table in
# TICKET_site_permission_dual_store.md ("notifications https://www.youtube.com
# BLOCK 2026-08-10"). Confirmed still present 2026-09-08.
KNOWN_BLOCKED = ("=https://www.youtube.com/", "notifications")

# ⛔ DO NOT reach for CDP `Browser.setPermission` to plant a denial here.
# 📏 Measured 2026-09-08 against this CEF build (150.0.43-7871.3576): it is a
# SILENT NO-OP. It returns `{"result":{}}` — no error — for both the
# origin-scoped and the context-wide form, and `navigator.permissions.query`
# is unchanged afterwards. Believing its success reply produced three false
# "⛔ BLIND" verdicts and nearly condemned a reader that demonstrably works.


def validate_instrument():
    """⛔ Run this before believing any row.

    A reader that cannot observe a block prints "prompt" on the broken build
    AND on the fixed one, so it would make them indistinguishable. This proves
    sensitivity the only way that works in this build: against a real
    content-setting block that something else created.

    ⚠️ It can only prove the type that control origin carries. Types it cannot
    prove are reported UNPROVEN — never assumed sensitive, and never reported
    as blind either.
    """
    print("=== instrument sensitivity ===")
    target, code = KNOWN_BLOCKED
    try:
        t, msg = cdp.evaluate(target, READ_JS, PORT)
        raw = msg.get("result", {}).get("result", {}).get("value")
        seen = json.loads(raw)[code] if raw else None
    except SystemExit:
        print("  control origin %s not open — cannot prove sensitivity." % target)
        return False
    if seen != "denied":
        print("  %-13s control %s reports %r, expected 'denied'. Either the "
              "control's block is gone or the reader is blind — resolve before "
              "trusting anything below." % (code, t["url"], seen))
        return False
    print("  %-13s control %s reports 'denied'  SENSITIVE" % (code, t["url"]))
    for other in QUERY_NAME:
        if other != code:
            print("  %-13s UNPROVEN — no control origin carries a block for this "
                  "type. A post-fix flip to 'denied' proves reader AND fix "
                  "together; no flip is UNRESOLVED, not a failure (D-D)." % other)
    return True


def main():
    capture_red = "--capture-red" in sys.argv

    if "--validate-instrument" in sys.argv:
        sys.exit(0 if validate_instrument() else 1)

    if not validate_instrument():
        sys.exit("\n⛔ VACUOUS: the page-side reader could not observe a planted "
                 "denial for every type. A 'prompt' result below would be "
                 "meaningless. Nothing measured.")

    print("\n=== baseline (store reset to ask) ===")
    before_store = store_states(set_panel(
        {c: "ask" for c in QUERY_NAME}))
    url, before = read_site()
    print("subject: %s" % url)
    print("  store: %s" % {c: before_store[c] for c in QUERY_NAME})
    print("  site : %s" % {c: before[c] for c in QUERY_NAME})

    print("\n=== panel set to BLOCK ===")
    after_store = store_states(set_panel({c: "block" for c in QUERY_NAME}))
    _, after = read_site()
    print("  store: %s" % {c: after_store[c] for c in QUERY_NAME})
    print("  site : %s" % {c: after[c] for c in QUERY_NAME})

    failures = []
    for code in QUERY_NAME:
        if after_store[code] != "block":
            sys.exit("VACUOUS: our own store did not record block for %s — the "
                     "panel half never happened, so the mirror was not tested."
                     % code)
        if after[code] != "denied":
            proven = (code == KNOWN_BLOCKED[1])
            failures.append(
                "%s: panel says block, site says %r%s"
                % (code, after[code],
                   "" if proven else "  [reader UNPROVEN for this type — see D-D: "
                                     "report with the measurement, do not call it "
                                     "a silent failure]"))

    # A8 control — camera/mic must stay OUR store's business, unmirrored.
    if after["camera"] == "denied" or after["microphone"] == "denied":
        failures.append("A8: camera/mic acquired a Chromium denial; they must "
                        "stay governed by our store with no content setting")

    print("\n=== reset (A7) ===")
    reset_store = store_states(set_panel({c: "ask" for c in QUERY_NAME}))
    _, after_reset = read_site()
    print("  store: %s" % {c: reset_store[c] for c in QUERY_NAME})
    print("  site : %s" % {c: after_reset[c] for c in QUERY_NAME})
    for code in QUERY_NAME:
        if after_reset[code] == "denied":
            failures.append("A7: reset left %s still denied on the site" % code)

    print()
    if failures:
        for f in failures:
            print("RED   : %s" % f)
        print("\nRESULT: RED (%d)" % len(failures))
        # Before the fix this IS the expected result and the exit code says so.
        sys.exit(0 if capture_red else 1)

    print("RESULT: GREEN — the panel's decision governs the site")
    if capture_red:
        print("⛔ --capture-red was set but everything passed. The shipped "
              "defect did not reproduce; check you are on the pre-fix build.")
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
