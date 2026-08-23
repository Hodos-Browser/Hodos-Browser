# Session prompt — beta.3 Phase 0.8, Test 2 + UI follow-ups

Paste the block below to open the session. Written 2026-08-23 after live testing rounds 1 and 2
(`414fe73`, `5e82ed6`, `8f3982c` — all pushed to `origin/0.4.0`).

---

Continue beta.3 Phase 0.8. The parsers, the modal and migration V24 are **done, live-tested and
pushed**. What is left is **one uncovered evidence row (`A12`, the pre-fill toggle)**, a small set of
**owner-reported UI follow-ups**, and **one open decision** about what a manifest connect writes to a
permission row.

⭐ **Walk me through every test step by step, like I am five.** I am doing the clicking. Tell me
exactly what to type, where to click, and what I should see — and tell me what it means if I see
something different. Do not batch five instructions into one paragraph.

## 0. Read first, before any code

1. **Auto-loaded MEMORY.md**, especially `project_p08_live_testing_round2_2026_08_23` (the live
   results and every trap) and `project_beta3_windows_mac_deconfliction_protocol` (git workflow:
   rebase before push; ⛔ **never commit `X402_INTEGRATION.md` or
   `development-docs/Onchain-Backup-and-Sync/*`** — those are the owner's parallel work and were
   unstaged the whole of the last session).
2. `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/UI_FOLLOWUPS.md` — the four follow-ups,
   including the question that must be settled before building and the open write decision.
3. `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` — §6a consent
   provenance, §8 evidence table (what is green, what is still owed).
4. `test-fixtures/manifest-dapp/README.md` — the demo dApp, the tunnel recipes, and the **three
   gates** that make a plain `localhost` demo impossible.

## 1. Settle this before writing any UI code

⚠️ The owner wrote *"can we move it to the bottom as well"* and it has two readings — the
**counterparty hex** (out of the inline permission lines into a footnote) or the **pre-fill toggle**
(within *Default Limits for New Sites*). `UI_FOLLOWUPS.md` §1 has both. **Ask; do not guess.**

## 2. The open decision — counterparty is displayed but not written

`UI_FOLLOWUPS.md` §4. The modal says *"with 0279887cdd…"*; the grant row lands with
`counterparty = NULL` = **any counterparty**. Phase 0.8 introduced the mismatch by adding the display
without the write.

⛔ Changing it changes what is written to a permission row — **get an explicit yes** (CLAUDE.md
invariants #2/#3). The recommended shape is: send the counterparty only for a concrete 66-hex pubkey,
leave `'self'`/`'anyone'` NULL, **and verify first** what the request-time counterparty actually is
for a `self` protocol before widening it. Guessing there could wrongly **deny**.

## 3. Test 2 — the pre-fill toggle (`P0.8-A12`), the last uncovered evidence row

⛔ **This test is VACUOUS against bitgenius.net.** It declares no `spendingAuthorization`, so there
is nothing to pre-fill: flip the toggle, connect, and the limits look identical whether the feature
works or not. That is the exact shape of green this project keeps getting bitten by.

It needs an origin that suggests limits **in our unit**, which today means our **legacy** manifest
shape. Use `test-fixtures/manifest-dapp/`:

- serve it, **rename `manifest.json` out of the way** so `.well-known/wallet-manifest.json` is the
  one that gets read (BRC-73 wins when both are present, and BRC-73 has no field in our unit);
- that legacy manifest suggests **$1/tx, $5/session** — the only thing that can exercise the
  `suggested by site` marking, **Use this site's suggested limits**, and the toggle.

⛔ **Three gates block `http://localhost`**, two of them P0.5 security gates — do not loosen them to
make a demo work: no CWI shim on a loopback page; the shim is https-only; a bare authority resolves
to `https://`. **You need a real HTTPS origin** — tunnel recipes are in the fixture README
(`npx --yes localtunnel --port 8788`, `npx --yes cloudflared tunnel --url http://localhost:8788`).

**What A12 has to show, both ways:**

| Toggle | Expect |
|---|---|
| **OFF** (default) | limit fields carry the **user's** defaults, unmarked; the site's $1/$5 shown **beside** them as a suggestion, not applied |
| **ON** | fields carry the **site's** $1/$5, each **still marked** `suggested by site`, and **Use my defaults** still reverts |

⛔ **Revoke the demo origin between the two runs** — an approved domain short-circuits in
`domain_trust_gate` and never fetches, so the second run would measure nothing. Right-click →
**Manage Site Permissions**.

Also still owed visually: **`A9`** — the marking read by someone who did not write it.

## 4. UI follow-ups

`UI_FOLLOWUPS.md` §2 and §3: an info affordance explaining the counterparty (it is an identifier,
**not** a decision — the user does nothing with it; a named counterparty is *narrower* than
`anyone`); the two toggles on one horizontal line with pre-fill to the right; padding before the
Save/Reset row. ⚠️ Two checkbox+label pairs side by side is a known DPI-clipping shape — let the row
wrap, and check cells #4/#6/#9.

## 5. State of the machine

- Dev stack was left running: wallet **31401** (schema V24), Vite **5137**, dev browser from
  `cef-native/build/bin/Release`. Re-check; it may have been closed.
- ⛔ The owner's **installed** browser (`AppData\Local\HodosBrowser`, ~60 processes) and its wallet on
  **31301** must never be touched. `win_build_run.ps1` matches by exe path under the build dir, which
  is what keeps them safe — verify that before running it.
- **bitgenius.net has been revoked** by the owner, so it is usable again for a fresh manifest test.
- ⚠️ The notification overlay is **keep-alive**: after a frontend change, restart the dev browser or
  it keeps serving the old JS bundle.

## 6. Standards that still apply

- ⛔ **Negative control on every acceptance test** — and note the lesson from last session: a guard
  sitting in front of the property fakes a green (`T1f`'s first control did not trip because the
  fixture's 5,000,000 was rejected by a magnitude cap, not the unit rule).
- `T1f` is `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/probes/manifest_consent_t1f.mjs`,
  wired into `scripts/preflight.ps1` (normal + `-NegativeControl`). Run `preflight.ps1 -Full`.
- ⚠️ `T1f` guards the consent **rules**, not the JSX binding. The "boxes are not typeable" bug would
  have passed it. There is no React DOM harness — do not claim coverage there is broader than it is.

## 7. Close-out

Commit + push per the deconfliction protocol, update the contract's evidence table with whatever
Test 2 measures, add any macOS notes to `MAC_RELAY_BETA3.md`, and save memory.
