#!/usr/bin/env python3
"""
beta.3 Phase 7d item 1 — the Edit Limits modal must not discard work.

Rows: P7d-A9 (an unsaved edit survives a stray click / Escape),
      P7d-A10 (Save and Cancel still close it — the guard cannot strand anyone).

⛔ SUBJECT. The claim is "the edit is still there", so the subject is the VALUE
IN THE FIELD after the stray click — not merely whether the dialog is still on
screen. A dialog that survives but resets its inputs has failed this row just as
badly, and a dialog-visibility-only assertion would score that as a pass.

⛔ RELOAD FIRST. Vite Fast Refresh preserves React state across an edit; on
2026-09-08 that turned a negative control GREEN against a build with the guard
deleted. Never measure this surface without a hard reload.

📏 This surface is a TAB (`/wallet?tab=4`), not the wallet overlay — the ticket's
C++ prevent-close mechanism does not apply here. See PHASE_CONTRACT D-7/D-13.

USAGE
    python edit_limits_close_probe.py
    python edit_limits_close_probe.py --expect-red   # with the guard reverted
"""

import json
import os
import sys

try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')
except AttributeError:  # pragma: no cover
    pass
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "phase-1-overlay-input-dpi"))
import cdp  # noqa: E402

PORT = cdp.DEV_PORT

WALLET_URL = "http://127.0.0.1:5137/wallet?tab=4"


def wallet_tab():
    """Resolve the Approved Sites tab by PATH, and refuse to guess.

    ⚠️ Two traps, both hit on 2026-09-08:
      • `/wallet` is a PREFIX of `/wallet-panel` (the wallet overlay), so a
        substring match is ambiguous — and the overlay has no list on it, so
        driving it would measure the wrong browser.
      • the `?tab=4` query does NOT reliably survive, so a substring containing
        it stops resolving at all.
    Match on the parsed path instead, and error if it is not exactly one.
    """
    try:
        from urllib.parse import urlparse
    except ImportError:  # pragma: no cover
        from urlparse import urlparse  # type: ignore
    hits = [t for t in cdp.targets(PORT)
            if urlparse(t.get("url", "")).path == "/wallet"]
    if not hits:
        sys.exit("VACUOUS: the wallet page is not open. Open it with the wallet "
                 "panel's 'Manage approved sites'. Nothing measured.")
    if len(hits) > 1:
        sys.exit("VACUOUS: %d targets have path /wallet — refusing to guess: %s"
                 % (len(hits), [t["url"] for t in hits]))
    return "=" + hits[0]["url"]
SENTINEL = "7.77"          # distinctive: must not collide with a stored value


def ev(js):
    t, msg = cdp.evaluate(wallet_tab(), js)
    r = msg.get("result", {})
    if "exceptionDetails" in r:
        sys.exit("VACUOUS: eval threw — %s" % json.dumps(r["exceptionDetails"])[:300])
    return r.get("result", {}).get("value")


def reload_and_settle():
    # Navigate rather than reload: guarantees the Approved Sites sub-tab is the
    # one selected, instead of depending on a query string that may be gone.
    cdp.evaluate(wallet_tab(),
                 "location.replace(%r + '&_r=' + Math.random()); 'go'" % WALLET_URL)
    for _ in range(40):
        time.sleep(0.5)
        try:
            if ev("!!document.querySelector('table tbody tr')"):
                time.sleep(0.8)
                return
        except SystemExit:
            continue
    sys.exit("VACUOUS: page never came back after reload.")


STATE_JS = r"""(function(){
  var dlg = document.querySelector('[role="dialog"]');
  var open = !!dlg;
  var title = open ? (dlg.innerText||'').split('\n')[0].trim() : null;
  var inputs = open ? Array.prototype.slice.call(dlg.querySelectorAll('input'))
                          .map(function(i){ return i.value; }) : [];
  var hint = document.body.innerText.indexOf('Click Save or Cancel to close') !== -1;
  return JSON.stringify({open:open, title:title, inputs:inputs, hint:hint});
})()"""


def state():
    return json.loads(ev(STATE_JS))


def open_editor():
    r = ev(r"""(function(){
      var b = document.querySelector('[aria-label="Edit limits"], [title="Edit limits"]');
      if (!b) {
        var btns = document.querySelectorAll('table tbody tr button');
        if (!btns.length) return 'NOBTN';
        b = btns[0];
      }
      b.click(); return 'OK';
    })()""")
    time.sleep(1.0)
    return r


def type_sentinel():
    """Put a distinctive value in the first limit field, the way a user would."""
    return ev(r"""(function(){
      var dlg = document.querySelector('[role="dialog"]');
      if (!dlg) return 'NODLG';
      var inp = dlg.querySelector('input[type="text"]');
      if (!inp) return 'NOINPUT';
      var setter = Object.getOwnPropertyDescriptor(
          window.HTMLInputElement.prototype, 'value').set;
      setter.call(inp, %s);
      inp.dispatchEvent(new Event('input', {bubbles:true}));
      return 'OK';
    })()""" % json.dumps(SENTINEL))


def click_backdrop():
    return ev(r"""(function(){
      var bd = document.querySelector('.MuiBackdrop-root');
      if (!bd) return 'NOBACKDROP';
      bd.click(); return 'OK';
    })()""")


def press_escape():
    return ev(r"""(function(){
      var dlg = document.querySelector('[role="dialog"]');
      if (!dlg) return 'NODLG';
      dlg.dispatchEvent(new KeyboardEvent('keydown',
          {key:'Escape', code:'Escape', keyCode:27, which:27, bubbles:true}));
      return 'OK';
    })()""")


def click_labelled(text):
    return ev(r"""(function(){
      var dlg = document.querySelector('[role="dialog"]');
      if (!dlg) return 'NODLG';
      var hit=null;
      dlg.querySelectorAll('button').forEach(function(b){
        if ((b.innerText||'').trim() === %s) hit = b;
      });
      if (!hit) return 'NOBTN';
      hit.click(); return 'OK';
    })()""" % json.dumps(text))


def main():
    expect_red = "--expect-red" in sys.argv
    failures = []

    reload_and_settle()
    if open_editor() != "OK":
        sys.exit("VACUOUS: could not open the Edit Limits dialog.")
    s = state()
    if not s["open"]:
        sys.exit("VACUOUS: no dialog on screen after clicking Edit. Nothing measured.")
    print("dialog opened : %r, inputs %s" % (s["title"], s["inputs"]))

    if type_sentinel() != "OK":
        sys.exit("VACUOUS: could not type into the dialog.")
    time.sleep(0.3)
    typed = state()
    if SENTINEL not in typed["inputs"]:
        sys.exit("VACUOUS: sentinel %r never reached a field (%s). The edit under "
                 "test was never made." % (SENTINEL, typed["inputs"]))
    print("unsaved edit  : %s" % typed["inputs"])

    # ---- P7d-A9a — stray click outside -------------------------------------
    if click_backdrop() != "OK":
        sys.exit("VACUOUS: no MUI backdrop to click.")
    time.sleep(0.5)
    a = state()
    print("after backdrop: open=%s hint=%s inputs=%s" % (a["open"], a["hint"], a["inputs"]))
    if not a["open"]:
        failures.append("A9: a click outside CLOSED the dialog and discarded the edit")
    elif SENTINEL not in a["inputs"]:
        failures.append("A9: dialog survived the click but the edit %r was reset "
                        "to %s — surviving is not enough" % (SENTINEL, a["inputs"]))
    if a["open"] and not a["hint"]:
        failures.append("A9: nothing told the user why the click did nothing "
                        "(no 'Click Save or Cancel to close')")

    # ---- P7d-A9b — Escape ---------------------------------------------------
    if a["open"]:
        press_escape()
        time.sleep(0.5)
        e = state()
        print("after Escape  : open=%s hint=%s inputs=%s" % (e["open"], e["hint"], e["inputs"]))
        if not e["open"]:
            failures.append("A9: Escape CLOSED the dialog and discarded the edit")
        elif SENTINEL not in e["inputs"]:
            failures.append("A9: Escape reset the edit to %s" % e["inputs"])

    # ---- P7d-A10 — Cancel must still work ----------------------------------
    # ⛔ The guard's own risk: a modal you cannot leave is worse than one that
    # closes too eagerly. This is the two-sided half of A9.
    if state()["open"]:
        if click_labelled("Cancel") != "OK":
            failures.append("A10: no Cancel button found in the dialog")
        else:
            time.sleep(0.6)
            c = state()
            print("after Cancel  : open=%s" % c["open"])
            if c["open"]:
                failures.append("A10: Cancel did NOT close the dialog — the guard "
                                "has stranded the user inside the modal")

    print()
    if failures:
        for f in failures:
            print("RED   : %s" % f)
        print("\nRESULT: RED (%d)" % len(failures))
        sys.exit(0 if expect_red else 1)

    print("RESULT: GREEN — stray click and Escape are refused with a reason; "
          "Cancel still closes")
    if expect_red:
        print("⛔ --expect-red was set but nothing failed; check the guard is "
              "actually reverted in the build being served.")
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
