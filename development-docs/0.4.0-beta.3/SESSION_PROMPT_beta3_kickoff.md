# Session kickoff prompt — open the beta.3 sprint

> Paste everything between the rules into a fresh session.

---

Kick off the **beta.3 sprint**. This session is **planning and verification, not implementation** —
per CLAUDE.md's mandatory phase-kickoff workflow, hand back a tight summary and wait for confirmation
before the first commit of feature code.

## Read first

1. `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` — the workstreams, the pre-flight findings, the
   decisions owed. **This is the spine.**
2. `development-docs/0.4.0-beta.3/README.md` — candidate list and where beta.2 left things.
3. The **8 tickets** in that folder. Two are shipping defects; one is a beta.3 prerequisite.
4. `development-docs/0.4.0-beta.3/MAC_RELAY_BETA3.md` — what Mac owes, and check for a reply round.
5. `CLAUDE.md` → Guidelines, the phase-kickoff workflow, Testing Standards (**negative control**),
   and the load-bearing UX safeguards list.

## Already established — do NOT redo

- `v0.4.0-beta.2` is **built, signed, notarized, byte-verified, and deliberately NOT promoted.**
  It is a draft soak build. **beta.3 is the release users get.**
- Both release gates (AV seeding + farbling rotation) **passed their first-ever CI execution**
  (`promote.yml` dry run `32050154040`). Nothing was published; verified three ways.
- The farbling rotation token was produced and negative-controlled on all four vectors.
- Node 22, `build.target: 'chrome150'`, Sparkle 2.9.6 and WinSparkle-tool 0.9.4 are already bumped.
- Engine is CEF `150.0.43-7871.3576+g9ccef04` (P4f), asserted out of both CI artifacts.

## ⛔ Hazards — handle each explicitly

**H-A — two defects are SHIPPING, in beta.1 and beta.2 alike.**
`TICKET_stray_log_in_install_root.md` (44 relative-path `ofstream` writes landing a log inside
`{app}` — the condition that already broke silent-update hashing once) and
`TICKET_production_debug_logging_unbounded.md` (no level gate, no rotation, 1.58 GB of plaintext
browsing history). ⇒ Decide where these sit in the order. They are cheap and user-affecting.
**Answer the open question in the first ticket before scoping it:** does the signed
`expected-new-manifest.json` check *reject* unknown files in `{app}`, or only assert listed ones?
That single answer moves it between "tidy-up" and "every silent update is at risk".

**H-B — the stall incident is UNSOLVED. Do not adopt a cause.**
beta.1 stalled with balances, wallet, local DB **and web pages** failing together. Established:
the BSV price was healthy throughout (`bsvPrice: 15.145`) — **the exchange-rate theory is ruled out
by evidence** — and the failure was `/wallet/balance` returning no balance from 15:18:02.
**Unexplained: why web pages stalled too.** One experiment settles it: stub `/wallet/balance` to
hang and observe whether page loads stall with it. Run that before theorising further.

**H-C — the appcast cannot be fixed late.** It is **signed at build time**
(`BUILD_AND_RELEASE.md` §2.5.6). `minimumSystemVersion` must land in the build that produces the
promoted feed, or a macOS 11 user's auto-update **bricks their install**. Treat as a beta.3
prerequisite, not a queued ticket.

**H-D — CI minutes are exhausted until ~2026-09-01.** Nothing has been tested in CI since
2026-08-14, *including everything in beta.2*. `test.yml`'s push trigger is suspended. Plan
local-first; anything needing a tag build queues behind the reset. Do not re-enable the trigger to
"just check something".

## Decisions to put to the owner — do not settle these yourself

1. **Where WS1b (logging/sync-IO) sits in the order** — it was promoted from an incident after the
   original WS1→WS4 ordering was agreed, and it is a live shipping defect.
2. **WS4 Chrome-import scope**, against the encryption wall in SPRINT_PLAN §2: DPAPI + Chrome 127
   App-Bound Encryption make cross-machine cookie/password import effectively impossible. Is this
   "bookmarks + history + passwords-via-CSV, same machine, one button", or a full research pass?
   Also: is importing another browser's **live logged-in sessions** into a wallet browser acceptable?
3. **Big Sur users** — nothing, a pinned final 0.3.x, or an in-app message?
4. **The cut line** — all workstreams in beta.3, or does WS4 slip?

## Your job this session

1. **Verify the plan against current code.** Every file:line cited in SPRINT_PLAN and the tickets —
   confirm it still exists with the documented shape, and update the doc inline where it moved.
   ⚠️ Several findings there are hours old but the tree moves.
2. **Reuse-first audit.** Before proposing anything new, prove the equivalent does not exist.
   Known anchors: `ProfileImporter.h` already imports Chrome/Brave/Edge bookmarks+history;
   `Logger` already exists (these 44 writes bypass it); `AppPaths::GetLogDir()` already resolves the
   correct directory; the AUMID per-profile suffix logic is already written and only its **gate** is
   wrong.
3. **Blast radius.** Audit against the load-bearing safeguards — the **gold pill** payment
   indicator, `g_wallet_overlay_prevent_close`, `g_file_dialog_active`, the four privacy-perimeter
   gates, per-session counters. WS1 touches overlay input directly, so this is not a formality.
4. **Sequence the phases** with dependencies, and say which need Mac, which need CI (i.e. which
   queue behind the reset), and which are local-only.
5. **Per-phase test plan**, each with its **negative control** — what must be seen to FAIL, and how.
6. **Hand back a tight summary**: proposed order, open questions, assumptions, and the four owner
   decisions. Then stop.

## Hard rules

- ⛔ **Every acceptance test must be demonstrated to FAIL when its feature is off.** This project has
  shipped four harnesses that would have passed with the feature absent. Also assert the test
  measures the intended **subject** — right process, right browser, right binary.
- ⛔ **Read `CEF_VERSION`, never the Chromium version**, when identifying an engine. P4e and P4f are
  both `chromium-150.0.7871.187`.
- ⛔ **Never push a `v*` tag to `origin`** — code lands on `origin` first, tags go to `release` only.
- **On a failing test, decide which side is wrong from independent evidence.** Test-only fixes may
  proceed; if the evidence points at production code, STOP and ASK (CLAUDE.md invariant #13).
- **Correct stale rationale in place rather than deleting it** — a plausible-but-wrong reason is how
  this project has previously talked itself into the wrong conclusion.

## Then state plainly

Whether beta.3's scope is achievable before the CI reset, what must wait for Mac, and what you would
cut first if it is not.
