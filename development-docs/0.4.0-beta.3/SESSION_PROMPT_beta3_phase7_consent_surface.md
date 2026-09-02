Start beta.3 Phase 7 — the consent surface (ticket bundle). ⛔ **Kickoff has NOT been run. Run it
before any code.** This is the first of the four consolidation phases (7–10); the owner has assigned
it — per SPRINT_PLAN §4.1 these were a *holding pattern, not a queue*, so "assigned" is what makes
this real work.

# 0. Read first, in this order

1. Auto-loaded `MEMORY.md`, then `project_p6_chrome_import_cut_2026_09_02.md` (Phase 6 just closed —
   CUT, deferred to beta.5) and `project_p08_consent_surface_round3_2026_08_23.md` (Phase 0.8 already
   worked this surface — quiet mode, the limit-field contrast fix, gates T1g + V25).
2. `SPRINT_PLAN.md` §4.1 — the **Phase 7 bundle row** and the two ❔-marked tickets in it. Then §6
   (decisions owed) — none are open *for this phase*, but confirm that.
3. `HARNESS.md` (tiers, ratchets, §4, §8, §9) and `REGRESSION_SET.md` — ⭐ **this phase is squarely
   inside `R-PERIM` and `R-INTEXT`**, unlike Phase 6. The consent surface *is* the trust boundary's
   display. Gates **T1f** (`manifestConsent.ts` connect-modal rule) and **T1g** (limit-field contrast)
   are consent-surface gates — keep them green, and extend them where a row adds a rule.
4. `development-docs/0.4.0-beta.3/PROMPT_BRANDING_INVENTORY.md` — the branding ticket is "21 remaining";
   the inventory says **7 of 28 branded**, and names which bubbles have **no CEF hook** (the
   save-password bubble among them — the beta.5 password work depends on it too).

# 1. State

✅ Sprint order `0 · 0.5 · 0.6 · 1 · 2 · 3 · 3.5 · 4 · 5` complete; **Phase 6 CUT** (`bac8355`).
⛔ **`git fetch` and read `git log HEAD..origin/0.4.0` yourself — the Mac side pushes to this branch.**
As of Phase 6 close, `HEAD == origin/0.4.0 == bac8355`.

# 2. The six tickets — and TWO of them may not be real work

The bundle is *one surface*: **what the user is shown when deciding to trust a site.** Rows:

| Ticket | Note |
|---|---|
| `TICKET_consent_surface_fetches_third_party_favicon.md` | a consent screen that reaches out to a third party leaks the visit it is asking you to authorise |
| `TICKET_quiet_mode_wider_than_manifest.md` | quiet mode silences more than the manifest declared — built on Phase 0.8's quiet-mode work |
| `TICKET_brand_remaining_permission_prompts.md` | 21 remaining. ⛔ Not all are brandable — the inventory marks some as **no CEF hook exists**; those need a fork patch or feature-suppression, not a React change |
| `TICKET_site_permission_dual_store.md` | one permission fact stored in two places that can disagree — a correctness bug wearing a UI hat |
| ❔ `TICKET_connect_modal_two_views_drift.md` | **triage first** — is the drift still real on the current tree? Two connect-modal views claimed to diverge |
| ❔ `TICKET_manifest_description_can_misdescribe_protocol.md` | **triage first** — relates to the two-layer manifest parser (`ManifestFetcher.cpp` + `manifest.rs`, "change both or neither") |

⛔ **The two ❔ rows are not yet work.** Measure whether each is still real on the current tree
*before* treating it as a task — the same "inventory against the tree" that has caught drift in **five
kickoffs in a row** (P3.5, K7, P4×2, P5, and Phase 6, where the import UI turned out orphaned and two
of my own claims were wrong). Write the delta into the contract `§0`.

# 3. ⭐ The load-bearing discipline for THIS phase

⭐⭐ **`feedback_consent_surface_needs_human_eyes`** — the gates guard the *rule*, not the *screen*.
Phase 0.8 shipped **six** consent-surface defects with every gate green throughout: a spending cap
rendered at 1.07:1 contrast (invisible), quiet mode making an unticked box inert, a vacuous test.
⇒ ⛔ **Every row in this bundle needs a human to look at the actual pixels.** A green T1f/T1g proves
the rule held; it does not prove the user could *read* what they were consenting to. Budget owner
eyes, and per §11 check your own screenshots before sending them.

🚨 **The favicon row is a privacy leak, not cosmetics.** A consent surface that fetches a third-party
resource tells that third party "this user is about to authorise us" — the exact visit the screen
exists to gate. Treat its RED as *observed network silence*, not reasoned.

# 4. ⛔ Scope fence

**IN:** the six rows above (two pending triage), all on the consent/trust-decision surface —
connect modal, permission prompts, quiet mode, the favicon fetch, the dual permission store.
**OUT:** the routing predicate (beta.4 W4/W6/W7/W8) · the money-path (Phase 8) · anything needing a
real install (→ `INSTALL_TEST_BATCH.md`, batched) · macOS (relay only).

# 5. Evidence — where this phase is exposed

- `R-INTEXT`, `R-PERIM` — **directly at risk**; run both, both halves, correct SUBJECT. This is the
  phase they were written for. State it, don't skip it (HARNESS §8: a skip is SKIPPED, never PASS).
- `R-GOLD` / `R-COUNT` / `R-CLOSE` — still owed from every prior boundary; one real payment closes
  R-GOLD + R-COUNT. If a payment happens for any reason this session, take it.
- The **dual-store** row is the dangerous one: changing where a permission is read from can silently
  re-open a gate. Prove nothing that *was* gated stopped being gated (the Phase 5 lesson: a narrowing
  is a privilege change).

# 6. Machine state

⛔ The owner's INSTALLED browser + wallet are running (wallet on **31301**).
⛔⛔ **NEVER stop a Hodos process by image name** — all three share the installed build's name.
⭐ Use `& '.\scripts\stop-dev.ps1'` (the `powershell -File` form is broken — `$PSScriptRoot` empty at
bind time; ticket that if you touch it). `-WhatIf` previews.
**Bring dev up:** `.\dev-wallet.ps1` (→ 31401) · `cd frontend && npm run dev` (→ 5137) · `Start-Process`
the dev exe (`cef-native/build/bin/Release/HodosBrowser.exe`) with `$env:HODOS_DEV='1'`
`--profile=Default` `--remote-debugging-port=9322`. ⛔ A detached bash `&` launch comes up minimized —
use `Start-Process`.

# 7. Rig

`phase-3.5-layout-window-scoping/p35drive.py` (CDP: `list` · `key` · `send` · `eval`; `eval` does not
`awaitPromise` — assign to a global and poll). ⛔ CDP reaches React's handlers but **not** the OSR
mouse path — `SendInput` clicks are dropped in the agent environment, so anything needing a real
click, or a real *look* at the screen, goes to the owner.

# 8. Deliverable

1. A **contract** (`phase-7-consent-surface/PHASE_CONTRACT.md` from the template — seven sections)
   whose `§0` records the plan-vs-tree delta, incl. the two ❔ triage verdicts, **before** code.
2. The agreed rows, each with an evidence table whose REDs were **observed** — and each with a
   **human-eyes** confirmation of the rendered screen, not just a green gate.
3. preflight + `-NegativeControl`; the 6 → 7 regression boundary (`R-PERIM` / `R-INTEXT` are live here).
4. Mac relay entry. What stays manual, said plainly.

# 9. Carried, not this phase

- 🎫 `TICKET_wallet_backend_death_is_silent_and_unrecovered` · `token_outputs_destroyed_by_dust_paths`
  🚨 (must close before beta.4 Ordinals) · `signaction_response_not_brc100_shape` §11/§14 — **all
  Phase 8.**
- 🎫 `appcast_missing_minimum_system_version` 🚦 — Phase 9, **must close before promotion.**
- 🔴 Phase 4 owner items open: **O2** (click-outside dismiss), **O3** (focus half of P4-A4), **O5**
  (macOS), **O6** (hear a muted tab).
- ⛔ `G11` stays at **60**; `G12` stays at **4** until beta.4's W8 retires `LegacyWalletGateMatch`.

# 10. Working style

⭐ Walk the owner through anything they must click *or look at*: what to do, what they should see, and
what a wrong result would mean. ⭐⭐ **Check your own work — screenshots included — before sending
them.** Every late-found defect this sprint was a two-minute owner test of something reported as done.
⛔ **Do not write documents nobody asked for.** ⛔ **Say "I don't know" plainly** rather than asserting
from a static read (Phase 6: I claimed the import UI was "shipping" off a code read; it was orphaned).
