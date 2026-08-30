# ⚠️ The frontend e2e specs test the wrong subject, and nothing runs them

**Found:** 2026-08-29, by the research (c) testing-practice pass during beta.4 scoping.
**Status:** ⬜ UNASSIGNED · **Sprint:** unassigned · **Filed by:** beta.4 telescope session

> ⚠️ **Method note.** This is a **second-hand report**, not a first-hand reading. It comes from
> research task (c) (`../research/RESEARCH_c_testing_practice.md`), which read the repo directly.
> ⛔ **This session did not independently verify either claim.** Verify both before acting — the whole
> point of the ticket is that a check believed to be running may not be.

---

## What happens

Two claims, both about `frontend/e2e/`:

1. **Wrong subject.** The six `frontend/e2e/*.spec.ts` specs run **stock Playwright Chromium against
   a mocked bridge on the dev server**. They therefore do not exercise the Hodos shell, the real
   bridge, or the wallet — which is what they read as covering.
2. **Never invoked.** They are run by **neither `scripts/preflight.ps1` nor `.github/workflows/test.yml`.**

## Why it matters

Claim 1 is a direct violation of `../../0.4.0-beta.3/HARNESS.md` §2's **SUBJECT** rule — *"which
process, which browser, which binary, which build type. This column is where three farbling harnesses
died."* A spec suite that drives stock Chromium against a mock is the same instrument, in a different
lane.

Claim 2 makes it worse in one direction and better in another: nothing has been trusting these results
because nothing has been producing them. ⭐ **But a suite that exists and is not run is the shape that
gets switched on one day by someone who assumes it means something.**

⚠️ Related, from the same source and with the same caveat: **`scripts/test-all.ps1` uses
`cargo tarpaulin` while `test.yml:209` says "NOT tarpaulin".** Two tools disagreeing about which is
authoritative is how a coverage number ends up meaning nothing. Verify and reconcile.

## How exposed are we — answer this first

**Unverified.** Both claims are second-hand.

| If | Then |
|---|---|
| Both true | No exposure *today* — nothing runs them, so nothing is trusting them. The risk is entirely future |
| Spec suite is wired in somewhere not checked | ⛔ Then something **is** trusting a mocked-bridge result, and the exposure is live |

## What already protects us, and how that shapes the fix

`HARNESS.md`'s tier table (T0–T4) and the SUBJECT column are exactly the instruments that name this
defect. **The standard already exists; these specs predate its application to the frontend.** So the
fix is applying an existing rule, not inventing one.

Research (c)'s **T3** recommendation is the real answer: drive the **actual shell** over CDP
(`connectOverCDP` against `--remote-debugging-port`) instead of stock Chromium. That would also make
`R-GOLD`, `R-COUNT` and `R-INTEXT`'s UI half runnable without a human — three checks recorded as
**"owed, not waived"** at every beta.3 boundary.

⚠️ It depends on the debug port, which is itself an open beta.3 ticket
(`TICKET_cdp_port_open_in_release.md`, decision D2 approved 2026-08-04 and never built). **These two
tickets should be read together** — one wants the port closed in release, the other wants it driven
in test.

## Proposed fix

1. **Verify both claims first.** Read the specs, `preflight.ps1`, and `test.yml`.
2. **Decide, and say which:** delete the specs, or fix their subject. ⛔ Do not leave them present and
   unrun — that is the state that misleads.
3. If keeping them: retarget at the real shell over CDP, and wire them into `preflight.ps1` with a
   tier and an ID.
4. Reconcile the `tarpaulin` disagreement, and record which tool is authoritative.

**Deliberately out of scope:** the CDP security decision (its own ticket); building the full CDP
harness (research (c)'s T3 — a phase, not a ticket).

## Test and negative control

| | |
|---|---|
| **GREEN** | Every spec that remains runs against the **Hodos shell**, is invoked by `preflight.ps1`, and carries an evidence-row ID |
| **RED** | ⛔ Break the feature a spec covers **in the shell** and see that spec go red. If it stays green, it is still measuring the mock — which is the defect, not a symptom of it |
| **SUBJECT** | The **binary under test**: Hodos shell vs stock Chromium, and which build. This is the column the defect is in, so it is the column the fix must prove |
| **Tier** | T2 |

**Standing invariant?** Not directly — but if the CDP route lands, three existing `REGRESSION_SET.md`
rows become automatable, which is a change to the set's **bucketing** rather than a new row. See
`../../SCOPING_PROCESS.md` §7b-ii (R-EVERY / R-RELEASE split).

## Links

- `../research/RESEARCH_c_testing_practice.md` — the source, with its own detail
- `../../0.4.0-beta.3/HARNESS.md` §2 (SUBJECT), §3 (tiers) — ⛔ read-only
- `../../0.4.0-beta.3/REGRESSION_SET.md` — the three rows this could unblock — ⛔ read-only
- `../../0.4.0-beta.3/TICKET_cdp_port_open_in_release.md` — the other half of the port question — ⛔ read-only
- `../../SCOPING_PROCESS.md` §7b-ii — the adoption list this came from
