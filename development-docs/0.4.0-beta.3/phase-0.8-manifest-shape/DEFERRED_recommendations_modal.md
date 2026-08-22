# DEFERRED — the "App ABC recommends these settings" modal

**Moved here 2026-08-22** when beta.3 Phase 0.8 closed and
`development-docs/TICKET_brc73_group_permissions_manifest.md` was archived. This is the
**only** part of that ticket that was still live; everything else it carried (the BRC-73 /
BRC-116 spec citations and the adoption survey) now lives in `PHASE_CONTRACT.md` §3, which is
the single home for those facts.

**No target release.** The blocker is not risk, it is inputs — see below.

---

## 4. Deferred — the "App ABC recommends these settings" modal

**Owner's requirement (2026-08-22):** *the user definitely needs to see and approve this themselves* —
a modal saying the app recommends these settings, which the user can accept or adjust manually.

Agreed as a design. Deferred because **no site in the survey declares `spendingAuthorization`**, so
there is nothing to recommend. Building a modal against zero real inputs is how you get the wrong
modal. Phase 0.8 parses the category anyway
(`demos/manifest-shapes/brc73-all-categories.json`), so the data will be there the moment it is worth
showing.

Two mismatches to resolve whenever it is built:

- **Units and period.** BRC-73's `amount` is *monthly satoshis*. Ours are *per-transaction* and
  *per-session* **USD cents**. Neither the unit nor the period lines up, and satoshis→USD moves.
- **Direction of trust.** A site's declared figure must never pre-fill our caps.

⭐ **Recommended resolution:** show the site's declared figure as **information** beside our own
defaults, and let the user set ours. That keeps *"a site can never set its own caps"* literally true,
sidesteps the conversion entirely, and still tells the user what the app is asking for. Phase 0.8's
`P0.8-A5` asserts the invariant this rests on.

**Prerequisite before building:** at least one real site declaring `spendingAuthorization`, so the
modal is designed against a real input rather than a hypothetical one.

---

## What Phase 0.8 already built for it

The data this modal would need now exists, so building it later is additive:

- **The category parses.** `spendingAuthorization` lands in
  `ManifestSpending::monthly_satoshis` (Rust) / `ManifestSpending::monthlySatoshis` (C++), is
  carried in the modal payload, and is already **displayed** — the connect modal shows
  *"This site declares a monthly allowance of N satoshis / month. Hodos does not enforce
  monthly limits."* Fixture: `demos/manifest-shapes/brc73-all-categories.json`.
- **The approved manifest is stored.** `domain_manifest_snapshots` (V24) keeps what the site
  asked for *as approved*, so a per-site "restore the settings this app recommended" action
  has something truthful to restore from. ⛔ Snapshot rules in `PHASE_CONTRACT.md` §6a —
  informational only, as-approved not live.
- **The provenance rule is settled and enforced.** `frontend/src/utils/manifestConsent.ts`
  owns "whose number is this?", is unit-tested (`T1f`), and already marks every site-sourced
  field. A recommendations modal must reuse it rather than growing a second rule.

## The two mismatches still to resolve when it is built

- **Units and period.** BRC-73's `amount` is *monthly satoshis*; ours are *per-transaction*
  and *per-session* **USD cents**. Neither lines up, and satoshis→USD moves. Phase 0.8's
  answer was to display the site's figure in the site's own unit and never convert it — a
  recommendations modal has to decide whether it can do better, and "it cannot" is a
  legitimate answer.
- **Direction of trust.** A site's declared figure must never pre-fill our caps unless the
  user has opted in (`settings.default_prefill_from_manifest`, V24, default off), and even
  then it stays marked. `R-CAPS` + `R-PROV` bind this modal too.

## Prerequisite before building

⛔ **At least one real site declaring `spendingAuthorization`.** The 2026-08-22 survey found
**zero** (`PHASE_CONTRACT.md` §3). Designing against a hypothetical input is how you get the
wrong modal. Re-run `demos/manifest-shapes/probe_manifests.py` before starting.

## Related

- `PHASE_CONTRACT.md` — the phase that parses the category, §3 for the survey and spec citations
- `bitcoin-sv/BRCs` — `wallet/0073.md` (schema), `wallet/0116.md` (lifecycle)
- `demos/manifest-shapes/` — fixtures and the re-runnable probe
