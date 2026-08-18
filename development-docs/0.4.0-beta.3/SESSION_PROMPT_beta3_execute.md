# Session kickoff prompt — EXECUTE the beta.3 sprint

> Paste everything between the rules into a fresh session.
> **Current as of 2026-08-18.** Replaces `SESSION_PROMPT_beta3_kickoff.md`, which opened the sprint.
> That earlier prompt is now **archaeology** — several of its premises were refuted by the review it
> asked for. Do not work from it.

---

Execute the **beta.3 sprint**, starting at **Phase 0**. Planning and verification are **done** — a
prior session verified every cited file:line against the tree, and the plan already reflects the
corrections. **Do not re-run the kickoff review.** Read, confirm the first phase's contract still
matches the code, and start.

## Read first, in this order

1. `development-docs/0.4.0-beta.3/HARNESS.md` — **the standard you will be held to.** Especially §2
   (the four-column evidence table), §4 (ratcheted gates), §6 (adversarial review), §8 (reporting).
2. `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` §4 — the running order.
3. `development-docs/0.4.0-beta.3/phase-0-stray-logs/PHASE_CONTRACT.md` — **your first unit of work.**
4. `development-docs/0.4.0-beta.3/REGRESSION_SET.md` — run in full at every phase boundary.
5. `CLAUDE.md` — Guidelines, Testing Standards, the load-bearing UX safeguards, invariants #2/#3/#13.

Read the other phase contracts (`phase-0.5-money-path/`, `phase-0.6-qr-bsv-uri/`) when you reach them,
not before.

## The order

`0 → 0.5 → 0.6 → 1 → 2 → 3 → 4 → 5 → 6`

| Phase | Work |
|---|---|
| **0** | Delete the 52 stray `{app}` writes + A1/A2/A3 |
| **0.5** | `send_transaction` request context · `block_on_origin_mismatch` · the three `:5137` gates |
| **0.6** | QR: accept `bsv:` at all four sites |
| **1** | Overlay input & DPI (sizing contract + the Windows offset) |
| **2** | Logger level gate, rotation, retention |
| 3–6 | WS2 → WS3 → WS5(b) → WS4 |

**Also a beta.3 prerequisite, not in the phase order:** the appcast `minimumSystemVersion` fix
(`TICKET_appcast_missing_minimum_system_version.md`). It is signed at build time and cannot be
patched at promote time. Land it before the tag.

## ⛔ Settled — do NOT re-derive, do NOT re-litigate

- **Phase 0's original headline is REFUTED.** "The stray log silently aborts auto-update" does not
  survive verification: `MaybeApplyStagedUpdate` (`cef_browser_shell.cpp:4787`) runs before
  `CefInitialize` (`:5277`) and only past `selfCount == 1` (`:4155`) + wallet-dead (`:4172`). The real
  justification is that **the BIP39 recovery phrase reaches `{app}` in plaintext**. The struck text
  stays in the ticket deliberately — it is the obvious-but-wrong read.
- **`:5137` is NOT dev-only.** `IsFrontendAvailable()` finds `{app}\frontend\` in production, so those
  gates are live in **release** builds.
- **The internal/external discriminator is the caller's frame origin, not the port.** Both address
  `127.0.0.1`. Internal ⇒ no `X-Requesting-Domain` ⇒ ungated by design. This is correct; preserve it.
- **The CI outage is the dev fork's test lane only.** `release.yml`/`promote.yml` run on the org repo
  with free minutes. beta.3 *can* ship before ~Sept 1; what cannot run is `cargo test`, clippy, F8,
  `cargo audit`, `npm audit`. Plan local-first. **Do not re-enable the push trigger to "just check".**
- **Windows overlays are `SetAsPopup` (windowed); macOS are `SetAsWindowless` (OSR).** Neither
  platform's fix transfers by default.
- Gate baselines were measured **by `preflight.ps1` itself**. Do not re-count by hand.

## 🚨 Phase 0 — start here, and start with the experiment

`P0-S1` is a **reproduction, and it comes before any edit**: create a wallet in a dev build, then grep
the log for a phrase word. The claim is that `WalletService.cpp:222-227` and `:311-313` log full
response bodies and `POST /wallet/create` returns the mnemonic inside the size cap.

⛔ **That has never been reproduced.** If it does not reproduce, say so plainly and amend the contract
— a refuted premise is a valid, complete outcome. Do not quietly proceed as though it held.

Then the rest of the contract: 52 writes across three files (`debug_output.log` ×44,
`startup_log.txt` ×8), A1 (exclusion policy — option 3 agreed), A2 (`UpdateState` already has
`lastFailureBuild`/`lastFailureReason`; reuse them), A3 (`TabManager.cpp:33`).

⭐ **Already implemented — verify, do not rebuild:** `installer/hodos-browser.iss` has **both**
`[InstallDelete]` and `[UninstallDelete]`, and both already list all three files.

## How to work — this is the part that differs from previous sessions

1. **The contract is the unit of work.** Confirm its cited code is still current, then execute it. If
   scope changes, **amend the contract in the same commit** that changes the scope.
2. **Every acceptance row needs its RED observed.** Not "this would fail if broken" — the run you
   actually did, and its result. A green result is reported with its red half **or not at all**.
3. **Fill the SUBJECT column honestly.** Which process, which browser, which binary, which build type.
   Read `CEF_VERSION`, never the Chromium version.
4. **A fix re-runs the whole evidence table**, not the failing row.
5. **Run the regression set in full at every phase boundary** and record it. `R-INTEXT` needs **both**
   halves — internal must not prompt, external must gate.
6. **Cite row IDs in commit messages** (`P0-A3`, `R-INTEXT`).
7. `pwsh scripts/preflight.ps1` before each phase closes. Exit `2` = INCOMPLETE — **record it as
   INCOMPLETE**, do not round it up.
8. **Adversarial review before sign-off**, four questions answered in writing (`HARNESS.md` §6). A
   workflow panel is warranted for **Phase 0.5** and **Phase 5** only.

## Hard rules

- ⛔ **On a failing test, decide which side is wrong from independent evidence.** Test-only fixes may
  proceed. If the evidence points at production code, **STOP and ASK** (CLAUDE.md #13).
- ⛔ **Never push a `v*` tag to `origin`.** Code lands on `origin` first; tags go to `release` only.
- ⛔ **Do not report a cause you have not reproduced.**
- **Correct stale rationale in place, struck rather than deleted.**
- Do not change wallet DB schema or crypto/derivation without asking.
- ⚠️ `cmake --build … | tail` **discards the exit code** — capture `$?` separately.

## Mac is working in parallel

`MAC_RELAY_BETA3.md` — pull before reading, push after writing, newest round first. They owe: the
`com.apple.security.device.audio-input` entitlement fix (cause found, one line), the interactive
Sparkle relaunch check, and a decision on who owns the macOS QR one-liner. They are **not** blocking
Phase 0, 0.5 or 0.6.

## Owner decisions still open — do not settle these yourself

1. **WS4 scope** and whether it is the cut.
2. **Big Sur**: nothing / pinned final 0.3.x (Mac's recommendation) / in-app message.
3. **Disclosure posture** for the two shipping security defects.
4. Whether Mac owns the macOS QR fix.

## First reply

Confirm Phase 0's contract still matches the tree, state what you will do first, and flag anything in
it you believe is wrong **before** you start editing. Then run `P0-S1` and report the result either
way.
