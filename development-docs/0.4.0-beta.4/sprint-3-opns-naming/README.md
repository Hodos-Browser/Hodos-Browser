# Sprint 3 — OpNS unique-name system

**Opened:** 2026-08-29 (created at the beta.4 telescope pass).
**Status:** 🔭 SCOPE ONLY. ⛔ **The naming sprint doc itself does not exist yet** — writing it is
sub-sprint 3.1.
**Standard:** `../../0.4.0-beta.3/HARNESS.md` + `../HARNESS_DELTA.md`.

---

## Goal

> **Resolve and register OpNS names against BRC-174, with a live overlay proof-of-concept.**

## ⛔ The development base is BRC-174, not the old research

**BRC-174 merged 2026-08-28** as `tokens/0174.md` in `bsv-blockchain/BRCs`, with **zero review
comments**. It is ours. It is the development base for this sprint.

> ⚠️ **Merging is publication, not endorsement.** Zero review comments means nobody objected — not
> that anybody checked. **§4 and §10.1 are unimplemented by anyone.** Building on BRC-174 means being
> its first implementation, and finding the parts that do not survive contact with code.
>
> When that happens: **feed it back to the spec.** Do not quietly diverge, and do not treat the
> merged text as correct because it is merged.

### The old naming research is superseded

`development-docs/Future-Features/Decentralized-Naming/` — `README.md`, `OPNS_REVIEW.md`,
`OPNS_RESOLVER_SCOPE.md`, `BOOTSTRAP_PROBLEM.md`, `NARRATIVES.md`, `GOODWILL_BRIDGE_PREMINT.md`,
`Domain-Names/`, `Paymail/`, `Xanaverse-Contracts-Review/`.

| | |
|---|---|
| **Where it lives** | Stays where it is. **Will be archived.** Not moved into this folder. |
| **How to use it** | Read **once**, during the 3.1 outline pass. Extract what survives BRC-174. |
| **After that** | ⛔ **Do not reference it.** It predates BRC-174 and disagrees with it in places. |

**A new naming sprint doc is required.** Built on the merged BRC. That is 3.1's output.

## Prerequisite

**Sprint 2.** An OpNS name is carried by a **1-sat output** — names inherit ordinal handling
end to end. Building names before ordinals means building ordinal handling badly, twice, and it
means sprint 1's guard has nothing tested in front of it.

## Sub-sprints

| # | Sub-sprint | Produces |
|---|---|---|
| **3.1** | **Outline against BRC-174** | ⛔ **The new sprint doc.** Reads the old naming folder once, states what BRC-174 changed, and records what from the old research survives — and what does not, with reasons. |
| **3.2** | **Resolve through shruggr's overlay** | Names resolve in Hodos. First, because it is the path that works today. |
| **3.3** | **Registration** | A user can claim a name from the wallet. |
| **3.4** | 🧪 **Our own overlay on Cloudflare — live PoC** | See below. |
| **3.5** | **Public artifact** *(owner, optional)* | The owner may pair the public demo with a strategic/tactical post on what users should demand of developers. **Not an engineering deliverable** — recorded so it is not a surprise. |

### ⚠️ 3.4 is a proof-of-concept that is live, more than a production feature

Two purposes, in order:

1. **Demonstrate independence from a single overlay.** Resolution that only works through one
   operator's infrastructure is a dependency, and naming it out loud is worth more than pretending
   otherwise.
2. **Potentially prove the per-query micropayment economics of BRC-174 §10.1** — the section nobody,
   including us, has implemented. If 3.4 gets that far, it produces **numbers**, and numbers are the
   deliverable.

⛔ **Do not let "live" become "production".** No SLA, no uptime commitment, no paid service. If it
stays up, good; that is not a promise. State this on the artifact itself, not only here.

## In scope / out of scope

| ✅ In | ❌ Out |
|---|---|
| Resolution and registration against BRC-174 | Anything from the pre-BRC-174 research that BRC-174 supersedes |
| A live overlay PoC, explicitly labelled as one | Production SLAs, uptime commitments, a paid service |
| Measuring §10.1 economics, if 3.4 reaches it | Paymail, domain-name bridging, goodwill/premint schemes (`Future-Features/`, archiving) |
| Feeding spec defects back to BRC-174 | Silent divergence from the merged text |

## ⭐ Verification policy and the ancestry cache — decide in the microscope pass (added 2026-09-16)

**Where this came from:** the public back-to-genesis argument on 2026-09-16 (shruggr, Siggi, Calhoun).
Full context, positions and the decision not to involve BOLT:
`Marston Enterprises/Standards/Ecosystem/PROVENANCE_BACK_TO_GENESIS.md`.

**The thing to design here is not the spec, it's the wallet's behaviour.**

**1. What triggers a full ancestry verification?** Resolving a name to display it and paying to a name
are not the same risk. Being wrong about the first shows the wrong avatar; being wrong about the second
sends money to a stranger. So verification should scale with value at risk, the way the auto-approve
engine already scales prompting:

| Action | Proposed bar *(to decide, not settled)* |
|---|---|
| Display a name while browsing | Overlay's answer is enough; no walk |
| Show a name as the counterparty **before** a payment | Walk, or a cached result for that name |
| Receive a name transfer into the wallet | Full walk, always |
| Register a name | N/A — our own mint |

⚠️ **The owner raised the case that decides this:** a user browsing Web3 / Metanet apps or playing games
could resolve many names quickly. Per-name cost measured at **4–21 transactions, 0.06–0.38 MB**
(`Marston Enterprises/Standards/BRCs/drafts/consensus-unique-name-tokens/MEASUREMENT_2026-09-04_ancestry-walk.md`).
A thousand names verified naively is hundreds of megabytes, which a browser wallet should not carry.

**2. Cache segments, not names.** ⭐ The same measurement found **walk length tracks prefix popularity,
not name length** — names sharing a prefix share the claims along that prefix. So the ancestry is a tree
with shared branches, and the cache should be keyed on **prefix segments**, not on whole names
*(inferred from the measurement; not separately measured)*.

Consequences worth testing:
- **Shallow prefixes are shared by the most names**, so a small cache of them should serve a large share
  of lookups — the same shape as any popularity-weighted cache.
- **Claims are immutable once mined**, so a cached segment never needs revalidating. It is a permanent
  fact, unlike a UTXO's spend status.
- **The per-name tail is small**; the shared prefix chain is the expensive part, and it is exactly the
  part that amortises.

**The measurement that settles it, and it is cheap:** resolve a sample of real names, record the
segments each walk touches, then plot cache size against hit rate. If a few megabytes of popular
prefixes covers most lookups, the policy above is comfortable and the wallet can verify aggressively. If
it doesn't, verification has to be rarer and the trigger table gets stricter. **Do this before 3.2
ships**, since it decides whether resolution can verify inline or must defer.

## Owed to the microscope pass

| Question | Why it matters |
|---|---|
| **Is 3.4 in the release, or a parallel public artifact on its own timeline?** | Changes the sprint's size materially. Also listed in `../README.md`'s decisions-owed. |
| What happens when shruggr's overlay is unavailable — degrade, fail, or fall back to ours? | This is the dependency-risk question, and it should be answered **before 3.2 ships**, not after. |
| Which parts of BRC-174 are unimplementable as written | **Expect some.** §4 and §10.1 are the likeliest. |
| Does name registration reuse sprint 2.3's token-spend permission class, or need its own? | A name is a 1-sat output, so the default answer is "reuse" — verify rather than assume. |
| **What triggers a full ancestry walk, and what does the cache keep?** | See the section above. It decides whether 3.2 can verify inline or must defer, and the measurement that answers it is cheap. |

## Verified context carried in

- **BRC-174 merged** 2026-08-28, `tokens/0174.md`, zero review comments.
- **Collectables are stable** — BRC-147, 150, 159, 160, 165 all merged and coherent. A name output is
  handled as a collectable.
- ⛔ **Fungibles are contested and out of scope** — `../WATCH_fungibles.md`.

## Links

- `../README.md` · `../SPRINT_PLAN.md` · `../TELESCOPE.md`
- `../sprint-2-1sat-ordinals/README.md` — the prerequisite, and the existing "Naming & OpNS — adjacent
  check" section in it (⚠️ written 2026-07-15, **predates BRC-174** — read with that in mind)
- `tokens/0174.md` in `bsv-blockchain/BRCs` — the spec
- `development-docs/Future-Features/Decentralized-Naming/` — ⛔ read **once** in 3.1, then not again
