# Wallet Hardening — living backlog

> **Scope:** robustness, correctness, and structural tech-debt in the Rust wallet
> backend (`rust-wallet/`). This is a *living* document — we add to it as we
> research, chase bugs, and learn. It is NOT a committed sprint; it is the
> register we consult before deciding what (if anything) to schedule.
>
> **Not in scope here:** new wallet *features* (new capabilities/UX). Those get a
> separate track so this backlog stays about quality/robustness, not scope.

**Last updated:** 2026-07-07 (initial capture)
**Owner:** Matt
**Status:** register only — nothing scheduled yet.

---

## How to use this doc

Each item is scored on three axes so we can sequence by value-per-risk rather
than by enthusiasm:

- **Severity** — how much it threatens *product quality*. We split this
  deliberately into **correctness** (a user could hit a defect) vs.
  **maintainability/velocity** (it changes how fast/safely we fix *future*
  bugs). Conflating the two leads to over-investment.
- **Effort** — XS (<1h) · S (half-day) · M (1–2 sessions) · L (multi-session) · XL (sprint).
- **Risk of the fix** — the chance the *change itself* introduces a regression.
  Money-handling code makes this non-trivial even for "pure" refactors.

**Guiding principle:** surgical and evidence-driven, not a rewrite. Do the
zero-risk and cheap-high-leverage items early; time the reviewability refactors
for when branches are quiet; hold big architectural changes until evidence
demands them. See [[feedback_research_first_do_it_once]] and the CLAUDE.md
invariants (don't change crypto/schema silently; prefer minimal reversible
changes).

---

## Severity framing (read this before scheduling anything)

Only **one** of the currently-identified items is about *user-facing
correctness* (the concurrency footgun). The rest are about *maintainability and
velocity* — real, because they compound into quality over time (they change the
odds the next bug is caught and how fast it's fixed), but **not defects a user
would ever hit.**

That distinction is the whole reason this is a backlog and not a sprint: a full
multi-week hardening sprint right now would compete with the higher-priority
0.4.0 / auto-update release work and would churn money code before a public
build. "Highest quality" here = ship the release *and* pick off the
high-leverage cheap wins *and* not destabilize working code.

---

## Issue register

### H-1 · Schema-version documentation drift
**Severity:** Trivial (doc hygiene — zero user impact) · **Effort:** XS · **Risk:** None

`rust-wallet/CLAUDE.md` headlines the schema as **V19** and includes a V20–V24
table that partially describes migrations that don't exist (it even self-flags a
fictional "V24" from a parallel branch). The **authoritative** source is the
code:

- **Actual current version: V23.** The runner in
  `rust-wallet/src/database/connection.rs` is a sequential
  `if current_version < N { migrate…; INSERT INTO schema_version (version) … }`
  ladder that tops out at **23** (Phase 2.6-H: drop `engine_shadow_log`).
- Migrations live in `rust-wallet/src/database/migrations.rs`; real tokens run
  V1 (consolidated) through V23.
- Note: `connection.rs` *also* carries inline "startup repair" that
  unconditionally re-adds missing columns (e.g. `max_tx_per_session`) after the
  ladder — same accretion pattern as H-2, worth documenting.

**Fix:** rewrite the schema section of `rust-wallet/CLAUDE.md` to match the code
(headline V23; correct/remove the V24 fiction; fix "dropped in V24" table rows);
add a one-line "authoritative source = `migrations.rs` + the `connection.rs`
ladder" pointer so it can't silently re-drift.

**Recommendation:** do now — it's isolated and de-risks all future DB work
(including AI sessions that would otherwise trust the wrong number).

**Status:** ☐ not started

---

### H-2 · `main.rs` inline startup-repair logic
**Severity:** Low (maintainability — the code already works) · **Effort:** S · **Risk:** Low

`main()` inlines ~250 lines of startup reconciliation: stale-pending-tx cleanup,
ghost-output deletion, placeholder-reservation restore, externally-spent-cert
reset, master-key derivation backfill, and a spawned phantom-UTXO sweep. It's
correct and battle-tested defensive crash-recovery — but it's accreting inside
`main()` and isn't independently testable.

**Fix:** extract into a `startup_recovery` module with named functions
(`cleanup_stale_pending_txs`, `restore_placeholder_reservations`,
`reset_externally_spent_certs`, `backfill_master_derivation`,
`sweep_phantom_utxos`). `main()` becomes a readable call sequence.

**Trap to avoid:** do **not** move most of this into `migrations.rs`. Migrations
run *once*; these blocks run *every boot* on purpose (they catch new
crash-induced ghosts, not just historical ones). Only genuinely one-time
backfills are migration-appropriate. Wrong home = subtle behavior change.

**Payoff:** readability + the extracted functions become unit-testable against a
fixture DB (they aren't today).

**Recommendation:** contained win; do in a dedicated small session with a dev-DB
smoke boot. Lower value than H-1/H-3A — working money-recovery code has downside
to moving and modest upside.

**Status:** ☐ not started

---

### H-3 · Concurrency model — blocking DB mutex + "drop before await" discipline
**Severity:** MEDIUM — **the only correctness item** (latent; not an observed defect today) · **Risk/Effort:** tiered

The DB is a blocking `std::sync::Mutex<WalletDatabase>` shared via
`web::Data<AppState>`. Correctness depends on a *documented human discipline*:
"drop the DB lock before any `.await`." A guard accidentally held across an
`.await` causes deadlocks or serializes the entire wallet behind one lock. Two
additional tokio async mutexes (`utxo_selection_lock`, `create_action_lock`) sit
on top. It works today because the discipline is being followed — the risk is
that it's enforced by convention, not by the compiler.

Three tiers of response, from cheap to architectural:

**H-3A — enforce the invariant with a lint (do first).** Effort S · Risk Low.
Clippy's `clippy::await_holding_lock` fires on exactly this pattern. Add it as
`warn` first to *measure* how many existing sites trip it, fix those, then bump
to `deny`. Converts "fragile documented discipline" → "the build fails if you
violate it." ~80% of the practical safety for a fraction of the effort. The
warn-mode measurement pass may directly surface latent stall bugs.
*Risk note:* it may reveal existing violations — that's a feature, but scope the
fix work from the measured count.

**H-3B — structural guardrail (optional middle).** Effort M · Risk Low-Med. A
scoped `with_db(|conn| …)` closure API that borrows the guard, runs the closure,
and returns before any await — making a leak structurally awkward, not just
discouraged. Migrate call sites incrementally.

**H-3C — connection pool (the "correct" architecture; defer).** Effort XL ·
Risk **High**. Replace the single global mutex with
`r2d2::Pool<SqliteConnectionManager>` + run blocking DB work in
`web::block`/`spawn_blocking`, so each request checks out its own connection.
Eliminates the footgun class and improves throughput — but touches **every**
handler's DB access, changes SQLite WAL/locking semantics, and produces a huge
diff in money code requiring a full re-test pass (create/sign/broadcast/backup).
**Only justified with evidence of real contention/deadlock in practice.**

**Recommendation:** H-3A now (best safety-per-effort in the whole register).
H-3C stays on the shelf until evidence demands it. If a hang/freeze bug shows
up, start here.

**Status:** ☐ not started

---

### H-4 · `handlers.rs` 18.8k-line monolith
**Severity:** Medium (reviewability/velocity — no direct user impact) · **Effort:** L · **Risk:** Med (mostly coordination)

`handlers.rs` is 18,813 lines; `certificate_handlers.rs` is another 6,211. The
`src/CLAUDE.md` navigates the monolith by line-number ranges ("Transactions
3234–9000"), which drift constantly and are hard to trust. It's the most likely
place for a bug to hide and the hardest place to review a fix — an *indirect*
quality lever (raises the odds future bugs slip through; slows every fix).

**Fix:** split by the protocol groups that already exist conceptually (identity,
crypto, transactions, wallet-mgmt, addresses, outputs, blockchain, domain-perms,
messages, peerpay, paymail, settings) into `handlers/<group>.rs`, re-exported
from `handlers/mod.rs`. The pattern already exists — `certificate_handlers.rs`
is exactly this. Route registration in `main.rs` is unchanged.

**Risks:**
- **Shared private helpers** (fee-calc, key-derivation, structs) must become
  `pub(crate)` or move to `handlers/common.rs`. That reshuffling is where a
  mechanical move can subtly break.
- **Most security-critical file** — a "pure reorg" of money code still risks a
  subtle behavior change. Discipline: *moves only, zero logic edits per commit*,
  compile + smoke after each.
- **Merge-conflict blast radius** — many live branches + active auto-update
  sprint. A big split conflicts with essentially every in-flight branch touching
  `handlers.rs`. Timing (branches merged/quiet) matters as much as the code.

**Recommendation:** incremental — one group extracted per commit, not big-bang.
Low *conceptual* risk (compiler is the safety net), high *coordination* cost.
**Keep this out of any active bug-hunt session** — splitting renumbers
everything and muddies the diff you want clean while chasing a defect.

**Status:** ☐ not started

---

## Suggested sequencing (current thinking, not committed)

| When | Item | Why |
|------|------|-----|
| Now | **H-1** schema doc | Zero risk, ~30 min, facts already gathered |
| Now / this week | **H-3A** clippy lint (warn→measure→deny) | Best safety-per-effort; may surface latent bugs |
| Next small session | **H-2** startup_recovery extraction | Contained, adds testability |
| Incremental, well-timed | **H-4** handlers split | One group/commit, when branches are quiet |
| Deferred | **H-3C** connection pool | Only if contention/deadlock evidence appears |

---

## Parking lot / candidates (unsorted — add freely)

Items surfaced but not yet worked into the register. Bug hunts and reviews feed
here first.

- _(bug under discussion 2026-07-07 — to be characterized; may become an H-item
  or a standalone fix)_
- Inline "startup repair" in `connection.rs` (re-adds missing columns after the
  migration ladder) — same accretion smell as H-2; document and consider folding
  into the recovery/migration story.
- `certificate_handlers.rs` (6.2k lines) — smaller sibling of the H-4 monolith;
  same split rationale if/when H-4 proves the pattern.

---

## Related

- `rust-wallet/CLAUDE.md` — layer overview, invariants, schema table (the one
  H-1 corrects)
- `rust-wallet/src/CLAUDE.md` — module map + handler groups (line ranges that
  H-4 would stabilize)
- Root `CLAUDE.md` — safety invariants (#2 schema, #3 crypto, #4 AppState,
  #13 test-failure triage)
