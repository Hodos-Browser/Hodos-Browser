# SESSION PROMPT — v0.4.0-beta.4 kickoff (telescope pass)

**Written:** 2026-08-29, at the end of a long session, for a **fresh context**.
**Status of everything below:** owner-decided unless marked OPEN. Do not relitigate settled items.
**Read first:** `development-docs/0.4.0-beta.3/README.md`, `HARNESS.md`, `REGRESSION_SET.md`,
`PHASE_CONTRACT_TEMPLATE.md` — beta.4 copies that convention and then extends it.

---

## 0. What this session is, and what it is NOT

This is the **telescope** pass: the long-distance look at how four sprints fit together. It produces
a compressed output file that a later **microscope** session reads.

**It is not** detailed design. Do not specify APIs, schemas, or phase-level implementation.

⚠️ **The owner's own named risk, in his words:** he over-plans, and wants to avoid analysis paralysis
and "spend a lot of tokens looping through architecture and design that really just need a decision
instead of going back and forth." **Human-in-the-loop is a required control, not a courtesy.** When
two options are close, present both in one message with a recommendation and ask — do not iterate
silently. Development sometimes just requires building, testing and adapting.

**Hard exit condition for this session:** the deliverables in §7 exist and the owner has answered the
OPEN questions in §8. Then stop.

---

## 1. Two tracks in this work

Track A is the **beta.4 release plan** (four sprints).
Track B is a **reusable scoping process** the owner is designing, which uses beta.4 as its feedback
loop. Track B is the bigger long-term item. Keep them separate in the outputs.

---

## 2. Track A — the release, and the decisions already made

Order is **settled**: **Guard → 1Sat → OpNS → Backup**, then a misc/tickets bucket.

### Sprint 1 — UTXO safety guard (new, up front)

Distinguish token outputs from spendable value **everywhere it matters**. First job is to enumerate
where the distinction is needed, at minimum: balance, coin selection, the dust consolidator, the
recovery sweep, reconcile. Fail closed — if an output cannot be classified, do not spend it.

This is needed **even if we never ship ordinals**, because users can receive one to a Hodos address
today. See §5 for the verified code paths.

### Sprint 2 — 1Sat Ordinals

Implement to **BRC-147 + BRC-150** (owner-approved 2026-08-05).

**BSV-20/21 is split out as a REVIEW phase, not a build phase.** Its gate question is the owner's:
*do wallets need BSV20/21 code at all, or is it only the apps that talk to wallets?* Note the testing
problem he raised — most 1Sat/BSV21 apps ship their own wallets, so we may have nothing to test
against. Record that in the phase doc.

Fungibles are deferred **with a watch note**, not dropped. Re-check if the BRC-163 vs BRC-175 dispute
resolves (§6).

### Sprint 3 — OpNS unique-name system

Built on **BRC-174** — our merged BRC is the development base, not the old research.

Resolve through **shruggr's overlay first**. Possibly stand up **our own overlay on Cloudflare** as a
live proof-of-concept, potentially proving the per-query micropayment economics from BRC-174 §10.1.
This is a PoC feature that is live, more than a production feature. The owner may pair the public
demo with a strategic/tactical post on what users should demand of devs.

`Future-Features/Decentralized-Naming/` (README, OPNS_REVIEW, OPNS_RESOLVER_SCOPE, Xanaverse review)
stays where it is and is **read once in the outline pass, then NOT referenced** — it predates BRC-174
and will be archived. A new naming sprint doc is required.

### Sprint 4 — On-chain Backup & Sync

Last. Reason: deltas exist to solve token-heavy wallets, and BRC-150 provenance rows (`beefB64`) are
that workload — measure against real ordinal rows rather than designing for a workload that does not
exist yet. Existing `Onchain-Backup-and-Sync/IMPLEMENTATION_PLAN.md` (8 phases, two adversarial
reviews, owner sign-offs recorded) stands and is not to be redesigned here.

**Open owner question to resolve during this sprint's scoping:** what actually needs backing up vs
what can be re-fetched and rebuilt on recovery. Inscription content is already on chain at the
outpoint, so store outpoints and re-query. The size problem is likely `beefB64` ancestry depth, not
images. Confirm with measurement, do not assume.

### Misc / tickets

The owner wants a **tickets folder** he reviews and assigns into sprints — not beta.3's flat
`TICKET_*.md` at folder root. He has items on paper already; do not chase them now.

### Folder mechanics (decided)

Copy `1SatOrdinals-BSV21/` and `Onchain-Backup-and-Sync/` into the beta.4 folder, fix
cross-references, then delete the originals. Rename the ordinals folder so BSV-21 is no longer in its
name.

---

## 3. Track B — the scoping process to design

The owner wants a **reusable process for every sprint/sub-sprint**, with its own goals and defined
outcomes:

1. **Scope** — outline the sprint/sub-sprints (what this session is doing).
2. **Telescope** — long-distance look at how all sub-sprints work together. **Output must include
   recommendations on how contexts and workflows should be used in the microscoping stage.**
3. **Microscope** — dig into each sprint and each phase. Runs in a **fresh context** that reads the
   telescope output. Each microscope may need its own context; where they genuinely cross over, use a
   designed workflow with sub-loops.
4. **Telescope again** — zoom out, assess microscope findings against the broad plan, change it if it
   broke, and only loop back in if warranted.

**The process must scope itself** — defined exit conditions, a loop limit, and explicit
human-decision points, because checks happen again before implementation anyway.

**Deliverables for Track B:**

- `development-docs/SCOPING_PROCESS.md`
- a reference to it from the project top-level `CLAUDE.md`
- a top-level agent file that owns the process

**Research to farm out to agents (the owner asked for these explicitly):**

- **(a)** Existing skill `.md` files written for **product managers writing good PRDs** — what to adopt.
- **(b)** The **Karpathy method** — what to lift into our harness/guidelines, and whether it belongs in
  `SCOPING_PROCESS.md` or top-level `CLAUDE.md`.
- **(c)** Current **best practice for test harnesses, regression sets, and testing** (including unit
  tests that run in CI/CD), to feed our harness/regression/testing docs.

Goal for all three: **prevent drift**, and force agents to state exactly what each feature requires
and how it will be tested before building.

---

## 4. What beta.4 docs must achieve

Copy beta.3's structure, then **expand it** (the owner intends to, but not yet — do not over-build
now). Every sprint doc sets **specific goals with well-defined outcomes** for all aspects, functions
and tests, built against the best testing methods identified by research (c).

---

## 5. VERIFIED code findings — carry these in, do not re-derive

Read by direct code inspection 2026-08-29 in `rust-wallet`. **Nothing files a 1-sat output into a
protective basket, and three paths treat it as spendable value:**

1. **`src/monitor/task_consolidate_dust.rs` — automatic, every 24h** (`src/monitor/mod.rs:79`).
   Selects `satoshis <= 1000` (line 25) from the **default basket**, fires at 20+ accumulated
   (`MIN_DUST_COUNT`), packs them into one output. **This destroys ordinals with no user action.**
2. **`src/recovery.rs`** — external scan sums every UTXO with no value filter;
   `build_sweep_transactions` batches them into one P2PKH output. The only guard is a dust check on
   the *output*. Zero `basket` references in `recovery.rs`, `reconcile.rs`, `utxo_fetcher.rs`.
3. **`src/handlers.rs:7257`** `select_utxos_with_preference` — `dust_threshold_sats: 5000`, and line
   7330 *deliberately includes* UTXOs at or under it.

**Existing partial protection:** `src/database/output_repo.rs:98,148` filter payment selection to
`basket_id IS NULL OR b.name = 'default'`. Correctly-basketed outputs are already safe — **the gap is
classification on ingest, not exclusion logic.** That shapes the guard.

**Basket / permission mechanics** (verified against BRC-46, 99, 147, 165):

- `p ` is **request-time routing only**. BRC-165 normalization rewrites the storage basket to `1sat`;
  the DB never stores a `p ` name.
- `src/database/basket_repo.rs:62-65` already rejects `p ` names per BRC-99 — correct fail-closed
  default; becomes route-if-supported when we implement a scheme.
- Our `domain_basket_permissions` (V18, `domain_permission_repo.rs:407`) grants **one domain one
  basket, binary**. BRC-99/165 scopes let a grant name an axis
  (`all`/`collection`/`app`/`creator`/`id`) with the value carried in tags.
- **Spend is a `createAction` label** (`p 1sat input id <key>`), not a basket. BRC-147 says pay /
  auto-pay grants **MUST NOT** authorize ordinal spends, so the auto-approve engine needs a separate
  token-spend permission class, and the approval/permission modals need to show which baskets and
  sub-categories are being granted.

---

## 6. Ecosystem state as of 2026-08-29

- **BRC-174 (ours) MERGED** 2026-08-28, `tokens/0174.md`, zero review comments. It is the OpNS
  development base. Merging is publication, not endorsement — §4 and §10.1 are unimplemented.
- **Collectables are stable:** BRC-147, 150, 159, 160, 165 all merged and coherent. Safe to build on.
- **Fungibles are contested:** BRC-163 (basket `bsv21`) merged 08-28; BRC-175 (basket `1sat-ft`,
  competing model) opened 08-27, still open, same author, criticised by shruggr and by a reviewer who
  called it not merge-ready. The unresolved question is how a fungible amount is committed —
  re-inscription costs a hop but proves it, remittance is cheap and unverifiable. Do not build on it.
- BSV-21 itself has two encodings: BRC-161 (JSON) and BRC-162 (binary/CBOR).

---

## 7. Deliverables for this session

1. `0.4.0-beta.4/README.md` — release scope, the four sprints, the settled order and the reasoning.
2. `0.4.0-beta.4/SPRINT_PLAN.md` — sprint-level breakdown, no phase detail.
3. Folder moves per §2, cross-references fixed, originals deleted.
4. `WATCH_fungibles.md` — the 163/175 state and an explicit re-check gate.
5. `tickets/` folder with a ticket template.
6. A telescope output file that **recommends the context/workflow structure for microscoping**.
7. Track B: a first cut of `SCOPING_PROCESS.md` plus the three research tasks dispatched.

---

## 8. OPEN — ask the owner early, do not assume

1. **Does the minimal defensive floor go in beta.3?** Recommendation from the prior session: yes. The
   dust consolidator destroys assets automatically on a 24h timer, and a floor (`satoshis > 1` plus a
   basket check) is a small fix. The full classification guard stays beta.4 sprint 1.
2. Does beta.4 **inherit** beta.3's `HARNESS.md` / `REGRESSION_SET.md`, or get its own copies to
   extend? (The owner said he wants to expand the harness, "not right now.")
3. How far does the guard reach in sprint 1 — 1-sat outputs only, or a general "classified token"
   concept that BSV-20/21 and future protocols slot into later?
