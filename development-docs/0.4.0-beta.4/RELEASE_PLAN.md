# beta.4 release plan — track level only

**Written:** 2026-08-29, telescope pass. **Companion:** `TELESCOPE.md` (how these interact),
`README.md` (scope and settled decisions).

⛔ **This document stops at candidate phase granularity by design.** No APIs, no schemas, no phase
contracts, no file-level design. That is the microscope pass's output, and writing it here would
mean writing it twice — once uninformed. If you are tempted to specify an interface, put the
*question* in the track's "Owed to microscope" table instead.

**Order — settled:** **0 `reqwest` TLS bump (👤 added 2026-09-15, first)** → **0.5 UTXO reservation ownership** (👤 placed 2026-09-15, owner to confirm at kickoff; `../TICKET_reservation_ownership_converge_on_spent_by.md`) → 1 Guard → 2 1Sat → 3 OpNS → 4 Backup → tickets. Reasoning in `README.md`; track 0's outline and research notes in `track-0-reqwest-tls-bump/README.md` — it needs its own scoping pass before any code.

---

## Track 1 — UTXO safety guard

> **Goal.** No path in the wallet — automatic, user-triggered or recovery — can spend an output the
> wallet has not positively classified as spendable.

**Why first:** the hazard is live today and one of its three paths needs no user action. See
`README.md` § verified findings. This ships **even if we never ship ordinals**.

**Shape — decided at kickoff:** a **general classification seam with one classifier implemented**.
The classification point returns `Spendable` / `Token` / `Unknown`; `Unknown` is **not spendable**.
Track 1 implements the 1-sat/inscription classifier only. BSV-20/21 and future protocols arrive
later as classifiers, without reopening the call sites.

⭐ The design constraint that makes this cheap: **the exclusion logic already works**
(`output_repo.rs:98,148`). This track fills the classification gap on ingest; it does not build a
second exclusion system.

### Candidate phases

| # | Candidate phase | What it produces |
|---|---|---|
| 1.1 | **Enumerate the surface** | The definitive list of places the token/value distinction is needed. Minimum, from the kickoff: **balance, coin selection, dust consolidator, recovery sweep, reconcile**. The deliverable is the *complete* list, proven by call-site sweep — not a re-statement of the five. |
| 1.2 | **Classification on ingest** | Every output entering the wallet gets classified once, at ingest, and the classification is persisted. This is where the general seam lives. |
| 1.3 | **Fail-closed enforcement at every site in 1.1** | `Unknown` is refused, not defaulted. Each site in 1.1 either consults the classification or is proven not to need to. |
| 1.4 | **Recovery and reconcile parity** | `recovery.rs`, `reconcile.rs`, `utxo_fetcher.rs` today contain **zero** `basket` references. Recovery is the most dangerous path because it fires when the user is least able to notice a loss. |
| 1.5 | **The standing invariant** | The new regression rows in `REGRESSION_ADDITIONS.md`, live and run at every later boundary. |

### In scope / out of scope

| ✅ In | ❌ Out |
|---|---|
| Classification on ingest, and its persistence | Inscription *rendering* or any UI beyond what proves the classification |
| Fail-closed refusal at every enumerated site | BSV-20/21 classifiers (contested — see `WATCH_fungibles.md`) |
| Recovery/sweep/reconcile parity | Ordinal transfer, listing, or purchase — that is track 2 |
| The standing "no automatic path spends an unclassified output" invariant | Permission classes for token spends — track 2 |

### Exit condition

- Every site in 1.1 either consults the classification or carries a written reason it does not.
- The regression rows in `REGRESSION_ADDITIONS.md` are **green with their reds observed**.
- ⛔ The negative control is the load-bearing part: **revert the guard, re-run, and see the 1-sat
  output consumed.** If it survives without the guard, the test is not exercising the path.

### Owed to microscope

| Question | Why it is not answered here |
|---|---|
| Where exactly the seam sits — ingest only, or ingest + a reconcile verification pass | Depends on whether reconcile can introduce rows that bypass ingest. Needs a code answer, not a preference. |
| What "classified" persists as, and whether it reuses baskets or sits beside them | Baskets carry BRC-99/165 semantics we do not fully implement; overloading them may be wrong. Real design decision. |
| How an existing wallet's already-ingested outputs get classified | Migration question. May be "on next reconcile", may need a one-time pass. |
| Whether the beta.3 floor is removed once the guard lands, or kept as defence in depth | Cheap either way; decide with the code in front of you. |

---

## Track 2 — 1Sat Ordinals

> **Goal.** Hold, display, receive and *deliberately* transfer 1Sat ordinals, to **BRC-147 + BRC-150**
> (owner-approved 2026-08-05).

**Prerequisite:** track 1. Ordinals are the assets track 1's guard protects; shipping them first
means shipping a feature and its destroyer together.

**Carried in:** `track-2-1sat-ordinals/README.md` and `RESEARCH_FINDINGS.md`. ⚠️ Both predate the
guard work and the 2026-08-29 code findings — read them against `README.md`'s verified-findings
section, and correct them in place where they disagree.

### Candidate phases

| # | Candidate phase | What it produces |
|---|---|---|
| 2.1 | **BRC-147 basket + BRC-165 normalization** | Ordinals land in the `1sat` basket by the normalization rules, not by ad-hoc naming. Closes track 1's classifier against the real spec. |
| 2.2 | **Provenance — BRC-150** | Provenance rows carried and verified. ⚠️ These rows (`beefB64`) are the workload track 4 measures against; **record their real sizes as they land.** |
| 2.3 | **Token-spend permission class** | ⛔ BRC-147: pay / auto-pay grants **MUST NOT** authorize ordinal spends. A separate permission class, plus approval modals that show which baskets and sub-categories a grant covers. |
| 2.4 | **Display and transfer** | The user can see what they hold and move it on purpose. Scope of "display" is a microscope decision. |
| 2.5 | **Indexer dependency posture** | The existing README already flags this as a deliberate decision, not a default. Carry it forward and *decide* it. |
| 2.6 | 🔍 **BSV-20/21 — REVIEW phase, not a build phase** | See below. |

### ⚠️ 2.6 is a review, and its gate question is the owner's

> *Do wallets need BSV20/21 code at all, or is it only the apps that talk to wallets?*

The phase doc must record, as findings rather than as a plan:

- the answer to that question, with the evidence for it;
- **the testing problem** — most 1Sat/BSV21 apps ship their own wallets, so we may have **nothing to
  test against**. A capability we cannot test is not a capability we can claim;
- the state of the BRC-163 / BRC-175 dispute at the time of review (`WATCH_fungibles.md`).

⛔ **Fungibles are deferred with a watch note, not dropped.** No fungible classifier, no fungible
basket semantics, in this release. Re-check only if the gate in `WATCH_fungibles.md` opens.

### In scope / out of scope

| ✅ In | ❌ Out |
|---|---|
| BRC-147 + 150 + 165 collectables | Any BSV-20/21 *implementation* |
| Token-spend permission class and its modals | A marketplace, listings, or buy/sell flows |
| Receive, hold, display, deliberate transfer | Minting — unless the microscope pass argues it in |
| Recording real provenance-row sizes for track 4 | Naming — that is track 3, built on the same outputs |

### Owed to microscope

- Does 2.3 extend `domain_basket_permissions` (one domain, one basket, binary) or replace it? Ours is
  **narrower than BRC-99/165 scopes**, which name an axis with the value in tags.
- What the approval modal must show for a grant to be honestly described. This is a
  privacy-perimeter-class surface; treat it with `R-PERIM` seriousness.
- Whether display requires inscription content fetch, and what that means for the indexer posture.

---

## Track 3 — OpNS unique-name system

> **Goal.** Resolve and register OpNS names against **BRC-174** — our merged BRC — with a live
> overlay proof-of-concept.

**Prerequisite:** track 2. A name is carried by a 1-sat output; names inherit ordinal handling.

⛔ **BRC-174 is the development base, not the old research.** `Future-Features/Decentralized-Naming/`
(README, `OPNS_REVIEW.md`, `OPNS_RESOLVER_SCOPE.md`, Xanaverse review) **predates BRC-174, stays where
it is, and will be archived.** Read it **once** in the outline pass, then do not reference it.
**A new naming track doc is required** — `track-3-opns-naming/` currently holds scope only.

⚠️ **Merging is publication, not endorsement.** BRC-174 merged with **zero review comments**, and its
§4 and §10.1 are unimplemented by anyone. Building on it means being the first implementation, and
finding the parts that do not survive contact.

### Candidate phases

| # | Candidate phase | What it produces |
|---|---|---|
| 3.1 | **Outline against BRC-174** | The new track doc. Reads the old naming folder once, extracts what survives, and states what BRC-174 changed. |
| 3.2 | **Resolve through shruggr's overlay** | Names resolve. This is the dependency path, and it is first because it is the one that works today. |
| 3.3 | **Registration** | A user can claim a name from the wallet. |
| 3.4 | 🧪 **Our own overlay on Cloudflare — live PoC** | ⚠️ **A live proof-of-concept, more than a production feature.** Its purpose is to demonstrate independence from a single overlay and, potentially, to **prove the per-query micropayment economics of BRC-174 §10.1** — the section nobody has implemented. |
| 3.5 | **Public artifact** *(optional, owner)* | The owner may pair the public demo with a strategic/tactical post on what users should demand of developers. Not an engineering deliverable; recorded so it is not a surprise. |

### In scope / out of scope

| ✅ In | ❌ Out |
|---|---|
| Resolution and registration against BRC-174 | Anything from the pre-BRC-174 naming research that BRC-174 supersedes |
| A live overlay PoC, explicitly labelled as such | Production SLAs, uptime commitments, or a paid service |
| Measuring §10.1 economics if 3.4 gets that far | Paymail or domain-name bridging (`Future-Features/`, archived) |

### Owed to microscope

- Is 3.4 in the **release**, or a parallel public artifact on its own timeline? (Also in
  `README.md`'s decisions-owed list — it changes the track's size materially.)
- What happens to resolution when shruggr's overlay is unavailable — degrade, fail, or fall back to
  our own? This is the dependency-risk question and it should be answered before 3.2 ships.
- Which parts of BRC-174 turn out to be unimplementable as written. **Expect some.** Feed them back
  to the spec rather than quietly diverging.

---

## Track 4 — On-chain backup & sync

> **Goal.** Delta-chain backup and multi-device sync, sized and proven against **real token
> workloads**.

**Prerequisite:** tracks 2 and 3 — for the workload, not for the code. Deltas exist to solve
token-heavy wallets, and BRC-150 provenance rows are that workload.

⛔ **`track-4-onchain-backup-sync/IMPLEMENTATION_PLAN.md` stands and is NOT to be redesigned.**
8 phases, decisions D1–D15, two adversarial reviews, owner sign-offs recorded. The microscope pass
for this track **reconciles** that plan against what tracks 1–3 actually built. It does not rewrite
it. If a track 1–3 outcome breaks a D-decision, that is a finding to surface, not a licence to
re-plan.

### The track's own open question — the owner's, answered by measurement

> **What actually needs backing up, versus what can be re-fetched and rebuilt on recovery?**

The stated hypothesis, to be confirmed or refuted rather than assumed:

- Inscription **content is already on chain at the outpoint** ⇒ store outpoints and re-query, do not
  back up images.
- The size problem is therefore likely **`beefB64` ancestry depth**, not media.

⛔ **Confirm with measurement.** This is exactly the class of plausible-but-unverified reasoning the
harness §8 exists to catch. Track 2's provenance rows are the measurement subject — which is why
2.2 is told to record real sizes as they land.

### In scope / out of scope

| ✅ In | ❌ Out |
|---|---|
| Executing `IMPLEMENTATION_PLAN.md`'s phases | Redesigning the plan, the delta format, or D1–D15 |
| Measuring real token rows and sizing against them | The BRC draft — it follows the code, and it waits on the provisional-patent decision |
| Reconciling the plan against tracks 1–3 outcomes | Anything the plan already marked out of scope |

### Owed to microscope

- Which of D1–D15 the token work touches, and whether any is invalidated by tracks 1–3.
- The measured answer to the backup-vs-refetch question, with the experiment named.
- How a classified-but-unrecoverable output behaves on restore — the intersection of track 1's
  fail-closed rule and track 4's recovery path. **This is the sharpest cross-track edge in the
  release**; see `TELESCOPE.md`.

---

## Misc / tickets

`tickets/` — a folder the **owner reviews and assigns into tracks**. Not beta.3's flat
`TICKET_*.md` at folder root, and not a queue anyone works from unprompted.

- Template: `tickets/TICKET_TEMPLATE.md`. Conventions: `tickets/README.md`.
- The owner has items on paper. ⛔ **Do not chase them now.**
- A ticket is not work until it carries a track assignment.

---

## Cross-track dependencies at a glance

```
       ┌──────────────────────────────────────────────────────────┐
       │ beta.3: minimal defensive floor (satoshis > 1)           │  ← ships now, separate release
       └───────────────────────────┬──────────────────────────────┘
                                   │ disarms the automatic destroyer
                                   ▼
  T1 Guard ──── classification seam ────► T2 1Sat ──── 1-sat outputs ────► T3 OpNS
     │                                       │                                │
     │ fail-closed rule                      │ beefB64 provenance rows        │ name outputs
     └───────────────────┬───────────────────┴────────────────────────────────┘
                         ▼
                    T4 Backup  (measures the workload T2/T3 created;
                                inherits T1's fail-closed rule on restore)
```

**The three edges that matter**, in the order they will bite:

1. **T1 → T2.** The classifier track 1 writes must be the one BRC-147/165 normalization expects. If
   track 1 invents its own basket naming, track 2 rewrites it.
2. **T2 → T4.** Track 4's central question is answered with track 2's data. If track 2 does not
   record provenance-row sizes as it goes, track 4 measures retrospectively or guesses.
3. **T1 → T4.** Fail-closed on restore. An output that cannot be classified during recovery must not
   silently become spendable — and must not silently vanish either. Neither track owns this edge
   alone, which is exactly why it needs naming here.
