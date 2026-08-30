# beta.4 sprint — scope and running order

**Opened:** 2026-08-29, from `SESSION_PROMPT_beta4_kickoff.md` (the telescope pass).
**Status:** 🔭 TELESCOPED. Sprint-level scope is set; **no phase-level design exists yet** and none
should be written here. The microscope pass produces that — see `TELESCOPE.md`.

> **Naming.** This folder produces **`v0.4.0-beta.4`**. It sits beside `0.4.0-beta.3/`, which is
> still live and owned by another session. ⛔ **Do not edit anything under `0.4.0-beta.3/`.**

---

## What beta.4 is

beta.3 is browser-shell work — overlays, DPI, window identity, logging, the trust boundary.
**beta.4 is the wallet's asset layer.** Four sprints, in a fixed order, that take the wallet from
"cannot tell a token from a coin" to "holds, spends and recovers tokens deliberately".

| # | Sprint | Folder | One-line goal |
|---|---|---|---|
| 1 | **UTXO safety guard** | `sprint-1-utxo-safety-guard/` | No path — automatic or manual — can spend an output the wallet has not classified as spendable |
| 2 | **1Sat Ordinals** | `sprint-2-1sat-ordinals/` | Hold, display, receive and deliberately transfer 1Sat ordinals to BRC-147 + BRC-150 |
| 3 | **OpNS unique names** | `sprint-3-opns-naming/` | Resolve and register OpNS names against BRC-174, with a live overlay proof-of-concept |
| 4 | **On-chain backup & sync** | `sprint-4-onchain-backup-sync/` | Delta-chain backup measured against real token workloads, multi-device sync |
| — | Tickets | `tickets/` | Reviewed and assigned into sprints by the owner, not worked ad hoc |

## Why this order — settled, do not relitigate

**Guard → 1Sat → OpNS → Backup.** Each one is a prerequisite for the next, not a preference:

1. **Guard is first because the hazard is already live.** Users can receive a 1-sat output at a Hodos
   address today, and three code paths treat it as spendable value — one of them
   (`monitor/task_consolidate_dust.rs`) runs automatically every 24 hours with no user action. Every
   later sprint *creates* the assets those paths destroy. Shipping sprint 2 before sprint 1 means
   shipping a feature and its own destroyer in the same release. **This sprint is needed even if we
   never ship ordinals.**
2. **1Sat before OpNS** because an OpNS name is carried by a 1-sat output. Names inherit ordinal
   handling; building names first means building ordinal handling badly, twice.
3. **OpNS before Backup** because names are the second real token workload, and the backup sprint's
   central open question is measured against real rows.
4. **Backup last** because BRC-150 provenance rows (`beefB64`) *are* the token-heavy workload that
   deltas exist to solve. Sequencing it last means measuring a workload that exists rather than
   designing for one that does not.

## Decisions taken at kickoff (owner, 2026-08-29)

| # | Question | Decision |
|---|---|---|
| 1 | Minimal defensive floor in beta.3? | ✅ **Yes — ship now.** `satoshis > 1` floor in the dust consolidator, recovery sweep and the coin-selection dust pass, landing in beta.3 under its existing ticket. Do **not** wait on the exposure question; it changes urgency, not correctness. The full classification guard stays sprint 1. |
| 2 | Harness: inherit or fork? | ✅ **Reference beta.3, extend by delta.** `../0.4.0-beta.3/HARNESS.md` and `REGRESSION_SET.md` are the standard. beta.4's additions live in `HARNESS_DELTA.md` and `REGRESSION_ADDITIONS.md` only. Fold both into a version-neutral `development-docs/HARNESS.md` **after** beta.3 closes. |
| 3 | Guard reach in sprint 1? | ✅ **General seam, one classifier.** Build the classification point with a general shape (`Spendable` / `Token` / `Unknown`, fail closed on `Unknown`); implement only the 1-sat/inscription classifier. BSV-20/21 slots in later as a classifier, not as a rewrite of the call sites. |

## The harness — inherited, not copied

⛔ **The standard is `../0.4.0-beta.3/HARNESS.md`.** Read it before writing a phase contract. It is
not restated here and it has not been forked. The reason for referencing rather than copying: its §9
gate baseline registry is *measured state*, and beta.3 is still lowering baselines. Two copies of a
measurement diverge silently.

| File | Where it lives |
|---|---|
| The standard — phase contracts, four-column evidence table, tiers, ratchets, adversarial posture | `../0.4.0-beta.3/HARNESS.md` |
| The standing regression set — R-INTEXT, R-GOLD, R-CLOSE, R-PERIM, R-COUNT, R-UPDATE | `../0.4.0-beta.3/REGRESSION_SET.md` |
| Phase contract template | `../0.4.0-beta.3/PHASE_CONTRACT_TEMPLATE.md` |
| **What beta.4 adds** — new tiers, new gates, the token-specific evidence rules | **`HARNESS_DELTA.md`** |
| **What beta.4 adds to the standing set** — new invariants that must survive every later phase | **`REGRESSION_ADDITIONS.md`** |

## The one rule carried forward from beta.3

> ⛔ **The claim follows the matrix, never the other way round.** A green result is reported with its
> red half or not at all. A row with an empty RED or SUBJECT cell is not done.

And its beta.4 restatement, because this release handles assets that cannot be recovered:

> ⛔ **Fail closed.** An output the wallet cannot classify is not spendable. "We could not tell, so we
> spent it" is the failure mode that destroys a user's asset permanently, and it has no undo.

## Folder mechanics — done 2026-08-29

Both prior standalone sprint folders were **moved into this folder**, not copied — originals no
longer exist, and BSV-21 is out of the ordinals folder name per §2 of the kickoff:

| Was | Now |
|---|---|
| `development-docs/1SatOrdinals-BSV21/` | `0.4.0-beta.4/sprint-2-1sat-ordinals/` |
| `development-docs/Onchain-Backup-and-Sync/` | `0.4.0-beta.4/sprint-4-onchain-backup-sync/` |

Cross-references rewritten in `development-docs/README.md`, the moved
`SPRINT_KICKOFF_PROMPT.md`, and four `research/*.md` files. Verified: **zero stale references remain
outside `0.4.0-beta.3/`.**

⚠️ **Six files under `0.4.0-beta.3/` still carry the old paths** — five `SESSION_PROMPT_beta3_*.md`
and `TICKET_token_outputs_destroyed_by_dust_paths.md`. They were **deliberately not edited**, because
another session owns that folder. Five of the six are historical session prompts and are archaeology.
The ticket is live and its "Links" section points at the old ordinals path — **that one is owed**, and
belongs to whoever next touches beta.3.

## What each sprint folder contains right now

| Folder | State |
|---|---|
| `sprint-1-utxo-safety-guard/` | ⬜ **New. Scope only** (`README.md`). No prior research existed; sprint 1 was created at this kickoff. |
| `sprint-2-1sat-ordinals/` | 🟡 Carried in: `README.md` (scope + the 2026-08-05 BRC-147/150 decision) and `RESEARCH_FINDINGS.md` (protocol mechanics, provider APIs, indexer infrastructure). Both predate the guard work — read against `TELESCOPE.md` before trusting scope claims. |
| `sprint-3-opns-naming/` | ⬜ **New. Scope only.** ⛔ A new naming sprint doc is **required** — see below. |
| `sprint-4-onchain-backup-sync/` | 🟢 Carried in and **authoritative**: `IMPLEMENTATION_PLAN.md` (8 phases, D1–D15 decisions, two adversarial reviews, owner sign-offs), `README.md`, `ADVERSARIAL_REVIEW.md`, `research/`. ⛔ **Not to be redesigned.** |

⛔ **`Future-Features/Decentralized-Naming/`** (README, `OPNS_REVIEW.md`, `OPNS_RESOLVER_SCOPE.md`,
Xanaverse review) **predates BRC-174 and will be archived.** It stays where it is, is read **once**
during the sprint-3 outline pass, and is **not referenced after that**. Sprint 3 gets a new doc built
on the merged BRC, not on the old research.

## Ecosystem position — as of 2026-08-29

| | |
|---|---|
| **BRC-174** (ours, OpNS) | **MERGED** 2026-08-28 as `tokens/0174.md`, zero review comments. The development base for sprint 3. ⚠️ Merging is publication, **not endorsement** — §4 and §10.1 are unimplemented by anyone. |
| **Collectables** — BRC-147, 150, 159, 160, 165 | All merged and mutually coherent. **Safe to build on.** This is sprint 2's foundation. |
| **Fungibles** — BRC-163 vs BRC-175 | 🔴 **Contested. Do not build on.** See `WATCH_fungibles.md` for the state and the explicit re-check gate. |
| **BSV-21 encodings** | Two exist: BRC-161 (JSON) and BRC-162 (binary/CBOR). Relevant only if the fungibles gate ever opens. |

## Verified code findings carried in — do not re-derive

Read by direct inspection of `rust-wallet` on 2026-08-29. Full detail in
`0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md` (read-only) and in
`sprint-1-utxo-safety-guard/README.md`.

**Nothing files a 1-sat output into a protective basket, and three paths treat it as spendable:**

| # | Path | Trigger | Effect |
|---|---|---|---|
| 1 | `monitor/task_consolidate_dust.rs` | **Automatic, every 86,400 s** (`monitor/mod.rs:79`) | Selects `satoshis <= 1000` from the default basket, fires at 20 accumulated, packs into one output. **Destroys ordinals with no user action and no prompt.** |
| 2 | `recovery.rs` | User-triggered — but it is the **restore** flow | External scan sums every UTXO with no value filter; `build_sweep_transactions` batches into one P2PKH output. Only guard is a dust check on the *output*. |
| 3 | `handlers.rs:7257` `select_utxos_with_preference` | Ordinary coin selection | `dust_threshold_sats: 5000`, and line 7330 *deliberately includes* UTXOs at or under it. |

⭐ **The exclusion logic already exists and works.** `database/output_repo.rs:98,148` filter payment
selection to `basket_id IS NULL OR b.name = 'default'`, so a correctly-basketed output is already
safe. **The gap is classification on ingest, not exclusion.** That is why decision 3 above builds a
classification seam rather than adding value checks at five sites.

⚠️ **Unverified and owed:** whether an ordinary incoming 1-sat payment becomes a tracked
default-basket row without a recovery scan. If yes, path 1 is live for any user who has ever received
one. This changes **urgency, not the fix** — which is why the beta.3 floor ships without waiting.

### Basket and permission mechanics — verified against BRC-46, 99, 147, 165

- `p ` is **request-time routing only**. BRC-165 normalization rewrites the storage basket to `1sat`;
  the DB never stores a `p ` name.
- `database/basket_repo.rs:62-65` already rejects `p ` names per BRC-99 — the correct fail-closed
  default. It becomes route-if-supported when we implement a scheme.
- Our `domain_basket_permissions` (V18, `domain_permission_repo.rs:407`) grants **one domain, one
  basket, binary**. BRC-99/165 scopes let a grant name an axis (`all` / `collection` / `app` /
  `creator` / `id`) with the value carried in tags. Ours is narrower than the spec.
- ⛔ **Spend is a `createAction` label** (`p 1sat input id <key>`), not a basket. **BRC-147 says pay
  and auto-pay grants MUST NOT authorize ordinal spends.** So the auto-approve engine needs a
  separate token-spend permission class, and the approval modals must show which baskets and
  sub-categories a grant covers. This lands in sprint 2, and it touches the privacy-perimeter
  surface — treat it with the seriousness of `R-PERIM`.

## What "done" looks like for beta.4

To be filled in at the end of the microscope pass, not now. Two structural rules that hold regardless:

1. **Every sprint doc sets specific goals with well-defined outcomes** for functions and tests,
   against the testing methods identified by research (c) — see `research/`.
2. **A sprint that ships a token capability without the guard in front of it is not done**, however
   green its own evidence table is.

## Decisions owed before sprint 1 implementation starts

1. **Exposure question** (above) — answer by experiment, not by reading. Sizes the guard's urgency
   and tells us whether existing users are already affected.
2. **Where the classification seam sits** — ingest-only, or ingest plus a verification pass on
   reconcile. Microscope decision; do not settle it here.
3. **The BSV-20/21 review phase's own gate question** (owner's): *do wallets need BSV20/21 code at
   all, or is it only the apps that talk to wallets?* Carried into sprint 2 as a **review** phase,
   not a build phase. Record the testing problem with it: most 1Sat/BSV21 apps ship their own
   wallets, so we may have nothing to test against.
4. **Whether the OpNS overlay PoC is in scope for the release** or is a parallel public artifact.
   Sprint 3 treats it as a live PoC, not a production feature — confirm that reading.

## Reading order for a fresh session

```
1. TELESCOPE.md            ← how the four sprints interact, and how to run the microscope pass
2. SPRINT_PLAN.md          ← the sprint-level breakdown
3. ../0.4.0-beta.3/HARNESS.md + REGRESSION_SET.md   ← the standard
4. HARNESS_DELTA.md + REGRESSION_ADDITIONS.md       ← what beta.4 adds to it
5. the one sprint folder you are working in — and no others
```

⛔ **Do not read all four sprint folders in one context.** That is the specific mistake
`TELESCOPE.md` §"Context strategy" exists to prevent.
