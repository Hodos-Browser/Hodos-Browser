# Session prompt — beta.3 Phase 8 (first ticket): the 1-sat destruction guard

Paste the block below into a fresh session. Everything above the line is context for whoever is
assembling it; the prompt itself starts at **PROMPT**.

**Written 2026-09-08**, at the close of Phase 7d. Base commit: `f0c9282` — pushed and verified
against `origin/0.4.0` with `git ls-remote`, not a stale ref.

---

## PROMPT

Start beta.3 **Phase 8 — money-path correctness**, and do **only the first ticket** to begin with:
`development-docs/0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md`.

It is being pulled ahead of the full Phase 8 kickoff at the owner's decision. Reason, in one line:
**path 1 is an automatic daily task that destroys a 1-satoshi asset with no user action and no
prompt**, and the sprint plan scheduled it *"before or alongside Phase 4"* — Phases 4, 5 and 7 have
all shipped since.

⛔ Run the kickoff before any code. Read, in this order:

1. `development-docs/0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md` — the whole thing
2. `development-docs/0.4.0-beta.4/sprint-2-1sat-ordinals/README.md` — "Two rules from BRC-147 that are load-bearing
   for us", rule 2. The rule this violates was already written down before the defect was found
3. `development-docs/0.4.0-beta.3/HARNESS.md` §6 (adversarial posture) and the negative-control rule
4. `development-docs/0.4.0-beta.4/` `README.md` + `TELESCOPE.md` — sprint 1 is the *real*
   classification guard. ⛔ This ticket is the **defensive floor only**; do not build beta.4's work

### ⛔ Answer this BEFORE sizing the fix — the ticket says so itself

The ticket is explicit that **everything in it is a code reading, not an execution**, and it names
the one unverified thing that decides severity:

> *Whether an ordinary incoming 1-sat payment becomes a tracked default-basket row without a
> recovery scan.*

- If **yes** → path 1 is live for anyone who has ever received a 1-sat output. Urgent.
- If **no** → exposure is the recovery/sweep path plus anything that internalizes such an output.
  Still real, less urgent.

Measure it. Do not inherit the ticket's own uncertainty into the fix.

### The three paths, and the proposed floor

| # | Path | File | Trigger |
|---|---|---|---|
| 1 | Daily dust consolidator | `rust-wallet/src/monitor/task_consolidate_dust.rs` | **automatic, 86,400 s**, fires once 20 dust UTXOs accumulate |
| 2 | Recovery sweep | `rust-wallet/src/recovery.rs` | user restores from seed — when they are least able to notice a loss |
| 3 | Coin selection prefers dust | `rust-wallet/src/handlers.rs :: select_utxos_with_preference` | any spend |

Proposed minimal fix (ticket §"Proposed minimal fix"): exclude `satoshis <= 1` in all three.
⛔ **Out of scope, deliberately:** inscription detection, basket assignment, BRC-147/150 semantics,
UI. All beta.4.

⭐ **The shape of the real fix, per the ticket:** `output_repo.rs` already excludes correctly-basketed
outputs from spending (`AND (o.basket_id IS NULL OR b.name = 'default')`). The exclusion logic
*works* — nothing ever **files** a token into a basket. So this ticket is a floor, and beta.4's
classification-on-ingest is the actual answer. Say so in the contract rather than implying it is
solved.

### ⛔ Inventory against the tree before believing any line numbers

Every kickoff this sprint has found plan claims that did not survive measurement — Phase 7c found
**four** in one document, Phase 7d found **eight**, including a ticket whose central measurement was
taken on a file that does not render the thing it measured. The dust ticket cites specific line
numbers (`task_consolidate_dust.rs:25,28,84,135`; `handlers.rs:7257,7263,7330`;
`recovery.rs:~516,~602`). **Verify each before quoting it.** Prefer `file.rs :: symbol` in what you
write.

### Test plan is already written — and it is the hard part

Ticket §"Test / negative control" is specific and non-negotiable:

- Stage a wallet with **20+ dust UTXOs including one 1-sat output**, run the consolidator, assert the
  1-sat output is still unspent and the others consolidated.
- ⛔ **Negative control:** revert the floor, re-run, show the 1-sat output **is** consumed. If it is
  not consumed, the test is not exercising the path — that is a VACUOUS result, not a pass.
- Same pair for the recovery sweep.
- Propose for `REGRESSION_SET.md`: **no automatic path may ever spend a 1-satoshi output** — an
  invariant that should outlive this ticket and hold through the beta.4 guard work.

⚠️ Staging 20 dust UTXOs is the real cost of this ticket. Decide early whether that is a unit test
over a seeded SQLite fixture or a live dev-wallet setup, and say which — a live run that cannot be
repeated is worth less than a fixture that can.

### 🚨 Invariants — this is the money path

- **CLAUDE.md #2 / #3:** do not change wallet DB schema or crypto/derivation without asking. The
  proposed fix needs neither; if you find yourself wanting one, **stop and ask**.
- **CLAUDE.md #13:** if a test fails, decide from independent evidence whether the test or the
  production code is wrong. Test-only fixes may proceed; if the evidence points at production code,
  **stop and ask**.
- ⚠️ `task_consolidate_dust` also pays the **1000-sat Hodos service fee** on a schedule the user did
  not initiate (root `CLAUDE.md` §Wallet Service Fee). Do not "improve" that here — it is not what
  you came for — but note it if the code makes it relevant.

### Instrument discipline — every one of these cost real time this sprint

- `npm run build` or `preflight.ps1 -Full`. ⛔ **Never `npx tsc --noEmit`** — it passes on code the
  build rejects. ⛔ **A skip is never a pass**: bare `preflight.ps1` skips `T1d` and says so.
- 🚨 **A zero can be a suppressed log, not an absence.** `R-INTEXT` read 0 lines because
  `domain_trust_mw` logs at **debug** while the wallet defaults to **info**. For any wallet-log
  assertion, start the wallet with `RUST_LOG=hodos_wallet=debug`.
- 🚨 **`cargo build --release` passed clean over 8 broken `cfg(test)` call sites in Phase 7c.**
  A release build does not compile test code — run `cargo test` before believing the tests are fine.
- ⛔ **Assert your probe's trigger fired.** A Phase 7d probe clicked an expander that existed on only
  one of three screens and scored the other two clean while never rendering the thing under test.
- ⛔ Exit **143** is SIGTERM (usually your own timeout), not a failure. Do not pipe a long-running
  launcher through `head` — it closes the pipe and kills the server.

### Machine state

⛔ **Read `NOTES_parallel_work.md` in the repo root first.** It is untracked **on purpose** — do not
commit it. It documents a second git worktree at `C:\Users\archb\Hodos-Browser-msgbox` on
`experiment/messagebox-privacy`, and two standing rules: **never switch branches in either
directory**, and **run `git branch --show-current` before any writing git command** — in this
directory the answer must be `0.4.0`.

The owner's **installed** browser and wallet are normally running. `.\scripts\stop-dev.ps1` is
path-matched and spares them; `-WhatIf` shows what it would stop. ⛔ Never stop a Hodos process by
image name — all three share the installed build's exe name.

Dev stack: `.\dev-wallet.ps1` (31401) · `cd frontend && npm run dev` (5137) · then Start-Process the
dev exe with `HODOS_DEV=1 --profile=Default --remote-debugging-port=9322`.
⚠️ A stray vite may already hold 5137. And **Vite Fast Refresh preserves React state across an
edit** — hard-reload before any React measurement, or a negative control can print GREEN against a
build with the guard deleted (it did, on 2026-09-08).

### Carried from the Phase 7d boundary — not blockers for this ticket

- DPI matrix cells #4/#6/#9; the **stubbed** `R-INTEXT` REDs; `R-GOLD` / `R-COUNT` (no real payment
  yet); `R-PERIM` T2 end-to-end.
- macOS: `MAC_RELAY_P7D_ROUND.md` is written; the Mac `R4` run is owed.
- 7c's own adversarial review, and `TICKET_wallet_quiet_detector_blind_to_long_polls.md`.
- ⭐ **A finding for `REGRESSION_SET.md`:** `R-CLOSE`'s SUBJECT names three C++ paths and **no React
  one** — which is exactly why a React backdrop that discarded unsaved edits was missed on the first
  pass of Phase 7d item 1. That set should grow a React layer.

Hand me the contract with its §0 delta and the severity answer to the "how exposed are we actually"
question, and wait for my confirmation before the first commit.
