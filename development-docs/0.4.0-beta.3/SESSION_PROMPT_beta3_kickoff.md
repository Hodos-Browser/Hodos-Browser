# Session kickoff prompt — open the beta.3 sprint

> Paste everything between the rules into a fresh session.
> **Current as of 2026-08-18.** Supersedes the 2026-08-17 draft.

---

Kick off the **beta.3 sprint**. This session is **planning and verification, not implementation** —
per CLAUDE.md's mandatory phase-kickoff workflow, hand back a tight summary and wait for confirmation
before the first commit of feature code.

## Read first

1. `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` — workstreams, pre-flight findings, the agreed
   order in **§4**. **This is the spine.**
2. `development-docs/0.4.0-beta.3/TICKET_stray_log_in_install_root.md` — **Phase 0.** Read it in full,
   including **A1, A2, A3**.
3. The other 7 tickets in that folder. `README.md` is a **catalogue, not the running order**.
4. `development-docs/0.4.0-beta.3/MAC_RELAY_BETA3.md` — what Mac owes; **check for a reply round**.
5. `CLAUDE.md` → Guidelines, the phase-kickoff workflow, Testing Standards (**negative control**),
   and the load-bearing UX safeguards list.

## Already established — do NOT redo, do NOT re-derive

- `v0.4.0-beta.2` is **built, signed, notarized, byte-verified, and deliberately NOT promoted.**
  It is a draft soak build. **beta.3 is the release users get.**
- Both release gates (AV seeding + farbling rotation) **passed their first-ever CI execution**
  (`promote.yml` dry run `32050154040`). Nothing was published; verified three ways.
- Engine is CEF `150.0.43-7871.3576+g9ccef04` (P4f), asserted out of both CI artifacts.
- Node 22, `build.target: 'chrome150'`, Sparkle 2.9.6, WinSparkle-tool 0.9.4 are already bumped.
- **The signed-manifest question is ANSWERED.** `VerifyTreeAgainstManifest` iterates only its own
  entries, so it does **NOT** reject unknown files in `{app}`. The exposure is the **backup walk** —
  full chain with file:line in the Phase 0 ticket.
- **A3 is investigated.** `{app}\debug.log` is Chromium's default sink; **"set the log path earlier"
  is NOT feasible** (no public CEF API configures browser-process logging before `CefInitialize`).
  Setting the process CWD was **considered and rejected** — reasons in the ticket.

## Start here — Phase 0

**`TICKET_stray_log_in_install_root.md`. A shipping defect, in beta.1 and beta.2 alike, that can
SILENTLY disable auto-update.**

44 relative-path `ofstream("debug_output.log")` writes land a log inside `{app}`. At update time
`BuildManifestForTree` hashes every file there, and `Sha256FileW` opens without `FILE_SHARE_WRITE`,
so a file held open for writing yields an empty hash, producing
`"Silent apply: cannot manifest {app} — abort"` — **the update does not happen, and nothing tells
the user.**

Phase 0 scope, all four parts:

- **the fix** — delete the 44 writes, route anything worth keeping through `Logger`, ship a cleanup
  for existing installs, add the build guard;
- **A1** — the backup's scope. **Staged plan already accepted:** exclusion now, manifest-driven
  backup next, never `FILE_SHARE_WRITE`;
- **A2** — the abort is a `LOG_WARNING` nobody sees. Raise it, record the reason in update state,
  make repeated aborts visible;
- **A3** — remove/re-route `TabManager`'s raw `LOG(INFO)`, and keep `debug.log` excluded.

⚠️ **A3 verification is ordering-dependent — one clean launch proves NOTHING.** Check across several
launches that `{app}\debug.log` stops being recreated.

## ⛔ Hazards — handle each explicitly

**H-A — the second logging defect is Phase 2, not Phase 0.**
`TICKET_production_debug_logging_unbounded.md` (no level gate, no rotation, 1.58 GB of plaintext
browsing history) is serious but **not self-blocking**, so it sits after the money-path work.
⚠️ Its fix has a specific trap: a gate that silences logging *entirely* looks identical to a working
one. This project has already had renderer logging be a silent no-op for the whole life of a feature.

**H-B — the stall incident is UNSOLVED. Do not adopt a cause.**
beta.1 stalled with balances, wallet, local DB **and web pages** failing together. Established: the
BSV price was healthy throughout (`bsvPrice: 15.145`) — **the exchange-rate theory is ruled out by
evidence** — and the failure was `/wallet/balance` returning no balance from 15:18:02.
**Unexplained: why web pages stalled too.** One experiment settles it: stub `/wallet/balance` to hang
and observe whether page loads stall with it. Run that before theorising.

**H-C — the appcast cannot be fixed late.** It is **signed at build time**
(`BUILD_AND_RELEASE.md` §2.5.6). `minimumSystemVersion` must land in the build that produces the
promoted feed, or a macOS 11 user's auto-update **bricks their install**. This is a beta.3
**prerequisite**, not a queued ticket.

**H-D — CI minutes are exhausted until ~2026-09-01.** Nothing has been tested in CI since
2026-08-14, *including everything in beta.2*. `test.yml`'s push trigger is suspended. Plan
local-first; anything needing a tag build queues behind the reset. Do not re-enable the trigger to
"just check something".

**H-E — WS1 touches the money path.** The wallet-overlay cursor offset on a second monitor means the
user hovers one control and activates another during a send. Treat it as correctness, not polish.

## Decisions still owed by the owner — do not settle these yourself

1. **WS4 Chrome-import scope**, against the encryption wall in SPRINT_PLAN §2: DPAPI + Chrome 127
   App-Bound Encryption make cross-machine cookie/password import effectively impossible. Is this
   "bookmarks + history + passwords-via-CSV, same machine, one button", or a full research pass?
   Also: is importing another browser's **live logged-in sessions** into a wallet browser acceptable?
2. **Big Sur users** — nothing, a pinned final 0.3.x, or an in-app message? *(Mac has been asked for
   a view; check the relay.)*
3. **The cut line** — all workstreams in beta.3, or does WS4 slip?

*(Ordering and A1 are already settled — confirm you agree rather than re-opening them, or say why not.)*

## Your job this session

1. **Verify the plan against current code.** Every file:line cited in SPRINT_PLAN and the tickets —
   confirm it still exists with the documented shape, and update the doc inline where it moved.
2. **Reuse-first audit.** Prove the equivalent does not already exist before proposing anything new.
   Known anchors: `Logger` (the 44 writes bypass it), `AppPaths::GetLogDir()`, `IsExcludedTopLevel`
   (exact first-path-component match — **no pattern support**), `ProfileImporter.h` (already imports
   Chrome/Brave/Edge bookmarks+history), and the AUMID per-profile suffix logic whose **gate** is the
   only wrong part.
3. **Blast radius.** Audit against the load-bearing safeguards — the **gold pill** payment indicator,
   `g_wallet_overlay_prevent_close`, `g_file_dialog_active`, the four privacy-perimeter gates,
   per-session counters. WS1 touches overlay input directly, so this is not a formality.
4. **Sequence the phases** with dependencies, and say which need Mac, which need CI (i.e. queue
   behind the reset), and which are local-only.
5. **Per-phase test plan**, each with its **negative control** — what must be seen to FAIL, and how.
6. **Hand back a tight summary**: proposed sequencing, open questions, assumptions, and the three
   remaining owner decisions. Then stop.

## Hard rules

- ⛔ **Every acceptance test must be demonstrated to FAIL when its feature is off.** This project has
  shipped four harnesses that would each have passed with the feature absent. Also assert the test
  measures the intended **subject** — right process, right browser, right binary.
- ⛔ **Read `CEF_VERSION`, never the Chromium version**, when identifying an engine. P4e and P4f are
  both `chromium-150.0.7871.187`.
- ⛔ **Never push a `v*` tag to `origin`** — code lands on `origin` first; tags go to `release` only.
- **On a failing test, decide which side is wrong from independent evidence.** Test-only fixes may
  proceed; if the evidence points at production code, STOP and ASK (CLAUDE.md invariant #13).
- **Correct stale rationale in place rather than deleting it** — a plausible-but-wrong reason is how
  this project has previously talked itself into the wrong conclusion.
- **Do not report a cause you have not reproduced.** Two open items (H-B, and how often A1 bites) are
  explicitly hypotheses with named experiments.

## Then state plainly

Whether beta.3's scope is achievable before the CI reset, what must wait for Mac, and what you would
cut first if it is not.
