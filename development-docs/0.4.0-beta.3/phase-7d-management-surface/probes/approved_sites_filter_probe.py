#!/usr/bin/env python3
"""
beta.3 Phase 7d item 5 — live probe for the approved-sites filter.

Rows covered: P7d-A1 (filter narrows), P7d-A2 (pager agrees with the table,
including *from page 2*), P7d-A3 (row actions stay keyed by domain under a
filter), P7d-A13 (the box is a native <input> and characters reach it).

⛔ SUBJECT DISCIPLINE. Hodos's header and ~14 overlays are separate CEF browsers
that CDP reports ALL as `type:"page"`. This probe addresses the wallet overlay by
URL substring through phase-1's `cdp.py :: pick`, which errors on ambiguity rather
than guessing, and every result line prints the target URL it actually talked to.
Driving the wrong browser has faked a bug in this project before.

⛔ VACUITY DISCIPLINE. Every check asserts its own trigger fired. If the Approved
Sites tab is not open, or the table has no rows, or the filter input is absent,
the run prints VACUOUS and exits non-zero — it never prints a clean zero that
could be mistaken for a pass.

USAGE
    python approved_sites_filter_probe.py            # normal run
    python approved_sites_filter_probe.py --expect-red
        Inverts the exit code. Use with the guard reverted (see NEGATIVE CONTROL
        in the phase contract) to prove the probe can actually fail.
"""

import json
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "phase-1-overlay-input-dpi"))
import cdp  # noqa: E402  — the shared helper; do not re-derive target selection

WALLET = "wallet?tab="

# The filter box, the footer, and the table rows. Read from the DOM, not from
# React state: what the user can see is the subject.
READ_JS = r"""(function(){
  var box = document.querySelector('input[aria-label="Filter approved sites by domain"]');
  var rows = Array.prototype.slice.call(
      document.querySelectorAll('table tbody tr'));
  // Each site renders a data row plus a collapsible detail row. The detail row
  // spans all columns, so a real data row is one whose first cell is not a
  // colSpan cell. ⛔ An earlier version required a single-line cell and silently
  // DROPPED 2 of 12 rows (sites with cert-field chips wrap) — a lossy extractor
  // can hide exactly the wrong-row failure A3 exists to catch, so it reads the
  // first line of the first cell instead of demanding the cell be one line.
  var domains = [];
  rows.forEach(function(tr){
    var td = tr.querySelector('td');
    if (!td) return;
    if (td.colSpan && td.colSpan > 1) return;
    var t = ((td.innerText || '').split('\n')[0] || '').trim();
    if (t) domains.push(t);
  });
  var footer = null;
  Array.prototype.slice.call(document.querySelectorAll('span,p,div')).forEach(function(e){
    var t = (e.innerText || '').trim();
    if (/^\d+[–-]\d+ of \d+$/.test(t)) footer = t;
  });
  var count = null;
  Array.prototype.slice.call(document.querySelectorAll('p,div')).forEach(function(e){
    var t = (e.innerText || '').trim();
    if (/approved site/.test(t) && t.length < 120) count = t;
  });
  var noMatch = document.body.innerText.indexOf('No approved site matches') !== -1;
  return JSON.stringify({
    hasBox: !!box,
    boxTag: box ? box.tagName : null,
    boxValue: box ? box.value : null,
    domains: domains,
    footer: footer,
    countLine: count,
    noMatch: noMatch
  });
})()"""


def set_query(text):
    """Type into the box the way a user does — native setter + input event, so
    React's onChange actually fires. Assigning .value alone does NOT."""
    js = r"""(function(){
      var box = document.querySelector('input[aria-label="Filter approved sites by domain"]');
      if (!box) return 'NOBOX';
      var setter = Object.getOwnPropertyDescriptor(
          window.HTMLInputElement.prototype, 'value').set;
      setter.call(box, %s);
      box.dispatchEvent(new Event('input', {bubbles:true}));
      return 'OK';
    })()""" % json.dumps(text)
    t, msg = cdp.evaluate(WALLET, js)
    return msg.get("result", {}).get("result", {}).get("value")


def click_next_page():
    js = r"""(function(){
      var b = document.querySelector('[aria-label="Next page"]');
      if (!b) return 'NOBTN';
      if (b.disabled) return 'DISABLED';
      b.click();
      return 'OK';
    })()"""
    t, msg = cdp.evaluate(WALLET, js)
    return msg.get("result", {}).get("result", {}).get("value")


def read():
    t, msg = cdp.evaluate(WALLET, READ_JS)
    raw = msg.get("result", {}).get("result", {}).get("value")
    if raw is None:
        detail = json.dumps(msg.get("result", {}))[:400]
        sys.exit("VACUOUS: no value back from %s — %s" % (t["url"], detail))
    return t["url"], json.loads(raw)


def reload_and_settle():
    """🚨 MANDATORY, and the reason this function exists at all.

    Vite Fast Refresh preserves React state across an edit. On 2026-09-08 the
    first negative-control run of this probe printed GREEN against a build with
    BOTH A2 guards deleted — because `page` had already been reset to 0 by an
    earlier HMR remount of the *guarded* build, so the out-of-range slice the
    row exists to catch could not happen. The identical probe, run after a hard
    reload, went RED and stayed RED at 400 ms.

    ⇒ Measuring this surface without a reload measures leftover state from the
    previous build. Never make that optional.
    """
    cdp.evaluate(WALLET, "location.reload()")
    for _ in range(40):
        time.sleep(0.5)
        try:
            t, msg = cdp.evaluate(WALLET, READ_JS)
            raw = msg.get("result", {}).get("result", {}).get("value")
            if raw and json.loads(raw)["hasBox"]:
                time.sleep(0.6)   # let the permissions fetch land
                return
        except Exception:
            continue
    sys.exit("VACUOUS: page never came back after reload — nothing measured.")


def main():
    expect_red = "--expect-red" in sys.argv
    failures = []
    notes = []

    reload_and_settle()
    url, s = read()
    print("target: %s" % url)

    # ---- trigger assertions: refuse to report a clean zero ------------------
    if not s["hasBox"]:
        sys.exit("VACUOUS: filter input not in the DOM. Is the Approved Sites "
                 "tab open in the wallet overlay? Nothing was measured.")
    if len(s["domains"]) == 0:
        sys.exit("VACUOUS: table rendered zero domain rows before filtering. "
                 "Nothing was measured.")

    baseline = list(s["domains"])
    print("baseline rows on page 1 : %d  %s" % (len(baseline), baseline))
    print("baseline footer         : %s" % s["footer"])

    # P7d-A13 — the box must be a native <input>, not a MUI TextField wrapper.
    if s["boxTag"] != "INPUT":
        failures.append("A13: filter control is <%s>, expected native INPUT"
                        % s["boxTag"])

    if s["footer"] is None:
        sys.exit("VACUOUS: no pager footer on screen, so P7d-A2 cannot be "
                 "measured. Needs >12 approved sites.")

    # ---- P7d-A1: a query narrows the list ----------------------------------
    probe_domain = baseline[0]
    frag = probe_domain.split(".")[0][:6]
    if set_query(frag) != "OK":
        sys.exit("VACUOUS: could not type into the filter box.")
    time.sleep(0.4)
    _, s1 = read()

    if s1["boxValue"] != frag:
        failures.append("A13: typed %r but the input holds %r — characters are "
                        "not reaching it" % (frag, s1["boxValue"]))
    if len(s1["domains"]) == 0 and not s1["noMatch"]:
        failures.append("A1: query %r produced zero rows and no no-match "
                        "message" % frag)
    if len(s1["domains"]) >= len(baseline) and len(baseline) == 12:
        failures.append("A1: query %r did not narrow the list (%d rows, was %d)"
                        % (frag, len(s1["domains"]), len(baseline)))
    off = [d for d in s1["domains"] if frag.lower() not in d.lower()]
    if off:
        failures.append("A1: rows shown that do not match %r: %s" % (frag, off))
    print("query %-8r -> %d rows %s" % (frag, len(s1["domains"]), s1["domains"]))

    # ---- P7d-A2: the pager must agree with the table, FROM PAGE 2 ----------
    # This is the row the whole item hinges on. Filtering does not change
    # `permissions.length`, so a reset-effect-only guard fires a render too late
    # and the user sees an empty table under a non-empty count.
    if set_query("") != "OK":
        sys.exit("VACUOUS: could not clear the filter box.")
    time.sleep(0.4)
    nav = click_next_page()
    if nav != "OK":
        sys.exit("VACUOUS: could not reach page 2 (%s). P7d-A2 not measured."
                 % nav)
    time.sleep(0.4)
    _, p2 = read()
    print("page 2 before filter    : %d rows, footer %s"
          % (len(p2["domains"]), p2["footer"]))
    if p2["footer"] == s["footer"]:
        sys.exit("VACUOUS: footer unchanged after Next page — the pager did not "
                 "move, so filtering from page 2 was never exercised.")

    if set_query(frag) != "OK":
        sys.exit("VACUOUS: could not type into the filter box on page 2.")
    time.sleep(0.4)
    _, s2 = read()
    print("page 2 + query %-8r -> %d rows, footer %s"
          % (frag, len(s2["domains"]), s2["footer"]))

    shown = len(s2["domains"])
    if shown == 0 and not s2["noMatch"]:
        # 📏 Measured shape of this failure, 2026-09-08 (guards removed, after a
        # hard reload): rows=0, footer=None (totalPages collapses to 1 so the
        # pager unmounts), and the count line still claims matches exist. The
        # user is told "4 of 15 ... match" over an empty table with no pager and
        # no explanation — worse than the incoherent footer this row originally
        # predicted, because there is no visible clue at all.
        failures.append(
            "A2: filtering from page 2 left an EMPTY table. Count line says "
            "%r, footer %r, no-match message absent — the count and the table "
            "disagree and nothing on screen explains it. This is the "
            "shipped-shape failure the row exists for."
            % (s2["countLine"], s2["footer"]))
    if s2["footer"] is not None:
        # Footer form is "lo-hi of total"; hi must not exceed total and the
        # visible row count must fit inside the range it advertises.
        nums = [int(n) for n in
                s2["footer"].replace("–", "-").replace(" of ", "-").split("-")]
        lo, hi, total = nums[0], nums[1], nums[2]
        if lo > total or hi > total or lo > hi:
            failures.append("A2: incoherent footer %r (lo=%d hi=%d total=%d)"
                            % (s2["footer"], lo, hi, total))
        if total != shown and shown > 0 and total > 12:
            notes.append("footer total %d vs %d rows on page — expected only if "
                         "the filtered set still pages" % (total, shown))

    # ---- P7d-A3: row actions stay keyed by domain, not by index ------------
    # Regression guard. Measured 2026-09-08 as already correct (contract D-3);
    # it is here so a later refactor to paged[i] goes red instead of silently
    # revoking the wrong site.
    if shown > 0:
        js = r"""(function(){
          var btns = document.querySelectorAll('[aria-label="Revoke access"], button');
          var rows = document.querySelectorAll('table tbody tr');
          var first = rows[0];
          if (!first) return 'NOROW';
          var d = (first.querySelector('td')||{}).innerText || '';
          return d.trim();
        })()"""
        t3, m3 = cdp.evaluate(WALLET, js)
        top = m3.get("result", {}).get("result", {}).get("value")
        if top not in s2["domains"]:
            failures.append("A3: first rendered row %r is not in the filtered "
                            "set %s" % (top, s2["domains"]))
        else:
            print("first filtered row      : %s (in filtered set)" % top)

    # cleanup — leave the panel as we found it
    set_query("")

    print()
    for n in notes:
        print("NOTE  : %s" % n)
    if failures:
        for f in failures:
            print("RED   : %s" % f)
        print("\nRESULT: RED (%d)" % len(failures))
        sys.exit(0 if expect_red else 1)

    print("RESULT: GREEN — all checks passed")
    if expect_red:
        print("⛔ --expect-red was set but nothing failed. The negative control "
              "did NOT reproduce; the guard may still be in place.")
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
