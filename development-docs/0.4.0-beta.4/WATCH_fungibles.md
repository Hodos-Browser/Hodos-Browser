# WATCH — fungible tokens (BRC-163 vs BRC-175)

**Opened:** 2026-08-29. **Status:** 🔴 **DO NOT BUILD ON.** Deferred with a watch note, not dropped.

> This file exists so that "we deferred fungibles" is a **decision with a re-check gate**, not a fact
> that quietly rots. Someone will eventually ask "why don't we support BSV-21?" — the answer is here,
> with the conditions under which it changes.

---

## The state, as of 2026-08-29

| Spec | Basket | State | Notes |
|---|---|---|---|
| **BRC-163** | `bsv21` | **Merged 2026-08-28** | |
| **BRC-175** | `1sat-ft` | **Open since 2026-08-27** | Competing model, **same author**. Criticised by shruggr and by a reviewer who called it not merge-ready. |

Two specs from the same author, one merged and one open, proposing **different baskets for the same
class of asset**, one day apart. That is not a settled protocol — it is an argument in progress.

### The unresolved technical question

**How is a fungible amount committed?**

| Approach | Cost | Property |
|---|---|---|
| **Re-inscription** | Costs a hop | **Proves** the amount |
| **Remittance** | Cheap | **Unverifiable** |

This is a real trade-off, not a detail, and it determines what a wallet must store and check. A
classifier written today encodes an answer to it — and there is a meaningful chance of encoding the
side that loses.

### Also unresolved: encoding

BSV-21 has **two** encodings — **BRC-161** (JSON) and **BRC-162** (binary/CBOR). Relevant only once
the model question above is settled; noted so it is not discovered late.

## Contrast — why collectables are safe and fungibles are not

**BRC-147, 150, 159, 160, 165 are all merged and mutually coherent.** There is no competing proposal
for the collectable model and no live dispute about how a 1-sat output carries an inscription. That
is why sprint 2 builds on collectables with confidence and stops at the fungible boundary.

⚠️ **Do not generalise "1Sat is stable" from the collectable half to the fungible half.** They are in
different states, and the ecosystem writing about "1Sat tokens" often does not distinguish them.

## What is deferred, concretely

⛔ In beta.4, **none** of the following are built:

- a fungible classifier in sprint 1's classification seam;
- `bsv21` or `1sat-ft` basket semantics;
- BSV-21 balance, transfer, or display;
- either encoding (BRC-161 or BRC-162).

✅ What **is** done: sprint 2 phase **2.6** reviews the question *do wallets need BSV20/21 code at
all, or is it only the apps that talk to wallets?* — as a **review phase producing findings**, not a
build phase. Its output includes the state of this dispute at review time.

⭐ Sprint 1's classification seam is deliberately **general** (`Spendable` / `Token` / `Unknown`,
fail closed on `Unknown`) so that a fungible classifier can be added later **without reopening the
call sites**. That is the whole reason the seam is general rather than a 1-sat value check. Deferring
fungibles costs us a classifier, not an architecture.

## ⏱ Re-check gate — the conditions that reopen this

Re-check when **any one** of these becomes true. Not on a calendar; on an event.

| # | Condition | Where to look |
|---|---|---|
| 1 | **BRC-175 is merged, closed, or withdrawn** | `bsv-blockchain/BRCs` PRs |
| 2 | **BRC-163 is amended, superseded, or deprecated** | `tokens/0163.md` history |
| 3 | The **amount-commitment question is answered** in either spec — re-inscription or remittance, decided rather than proposed | Either spec's text |
| 4 | The author **reconciles the two proposals** into one, or a third model displaces both | PR discussion |
| 5 | A user-facing need forces the question — someone must hold a BSV-21 token in Hodos to do something we have committed to | Our own roadmap, not the ecosystem |

**On any trigger, the re-check answers three questions, in order:**

1. Is there now **one** model, or still two?
2. Does the winning model change what sprint 1's classification seam must produce? (If yes, that is a
   sprint-1 amendment, not a new sprint.)
3. Is there **anything to test against**? ⚠️ The testing problem from sprint 2.6 does not go away
   when the spec settles: most 1Sat/BSV21 apps ship their own wallets, so we may have no
   counterparty. **A capability we cannot test is not a capability we can claim.**

⛔ **A merged BRC is not, by itself, a trigger to build.** BRC-174 — ours — merged with zero review
comments, and merging is publication, not endorsement. Condition 1 above opens a *review*, not a
sprint.

## Log

| Date | Event | Effect |
|---|---|---|
| 2026-08-27 | BRC-175 opened (`1sat-ft`), competing with BRC-163 | — |
| 2026-08-28 | BRC-163 merged (`bsv21`) | Dispute now merged-vs-open, one day apart |
| 2026-08-29 | Fungibles deferred for beta.4; this watch opened | Sprint 1 seam kept general so the deferral is reversible |
