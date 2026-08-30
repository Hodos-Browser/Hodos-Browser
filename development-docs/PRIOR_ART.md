# Prior art — the sources, and the ledger of what we actually learned

**Opened:** 2026-08-30. **The rule lives in** the root `CLAUDE.md`, working rule 5. This file carries
the **stack-by-stack source list** and a **running log of lookups**.

> **Why the log exists** *(owner, 2026-08-30)*: to learn **which projects are worth the trip, and for
> what.** A source that has never taught us anything is costing reading time; a source that keeps
> paying out should be consulted earlier. ⛔ **Keep it to one row per lookup.** The moment this
> becomes a chore it stops being filled in, and a half-filled ledger is worse than none.

---

## 1. Sources by stack

⛔ **Do not read the whole list for one question.** Find your layer, read the one or two that fit.

### `rust-wallet/` — BSV protocol and wallet behaviour

| Source | Good for | Trust |
|---|---|---|
| **BRC documentation** | What the spec requires vs leaves open | 🟢 Always first |
| **BSV Association SDKs + `wallet-toolbox`** (TypeScript, Go) | How a conforming wallet behaves | 🟢 **Authoritative.** Outranks everything below |
| **Bitcoin BIPs** | Lineage only — BRC-42/43 descend from BIP32 | 🟡 Read for argument, not for behaviour |
| **BDK / `rust-bitcoin`** | ⛔ One narrow thing only — see §2 | 🔴 **BTC, not BSV** |

### `cef-native/` — engine, privacy, security

| Source | Good for | Trust |
|---|---|---|
| **Chromium upstream** | What we diverge *from*. Every engine bump re-litigates it | 🟢 |
| **Brave** | ⭐ Closest to us in intent, and the origin of our approach | 🟢 |
| **Tor Browser** | ⭐ The threat model — and the **opposing** strategy | 🟢 |
| **Mullvad Browser** | Tor's hardening without the Tor network | 🟢 |
| **Firefox / Gecko** | Independent engine, different answers; conformance reference | 🟢 |
| **Safari / WebKit** | ITP; most aggressive tracking prevention at scale; macOS platform reference | 🟢 |
| **LibreWolf** | Which Firefox defaults a privacy project changes | 🟡 Config-level, not architectural |
| **ungoogled-chromium** | De-Googling patch sets; fork-maintenance burden | 🟡 Patch discipline, not design |

### `frontend/` — browser UI and interaction

| Source | Good for |
|---|---|
| **Vivaldi** | Chrome-level UI over Chromium — closest to what we do |
| **Brave / Firefox** | Permission and consent surfaces — security UI as much as UI |

### BRC drafting (lives in the Marston repo)

Sources are listed in `Marston Enterprises/Standards/BRCs/README.md` § "Prior art before drafting" —
BRCs in `reference/`, the SDKs, BIPs for structure and process, **RFC 2119** for normative language.

---

## 2. ⛔ Two corrections on record — do not repeat them

### 2.1 Farbling is **Brave's**, not Tor's

⚠️ **Corrected 2026-08-30 after the owner queried it. He was right.**

**Brave coined the term "farbling" and shipped fingerprint randomisation first.** Prior academic work
existed (**PriVaricator**, **FPRandom**) but Brave was the first mainstream browser to deploy it.
Firefox and Safari adopted the approach later.

⭐ **Tor Browser is still prior art — for a better reason than the one first given.** Its design
document is the canonical statement of the *threat model*, and its own answer is the **opposite
strategy**:

| Strategy | Approach | Who |
|---|---|---|
| **Randomisation** | Everyone looks different, and different again each session | **Brave** (and us) |
| **Uniformity** | Everyone looks *identical*, so there is nothing to distinguish | **Tor Browser** |

**Why this matters for us specifically:** our open farbling residuals — unfarbled workers, fenced
frames, the 37-host `IsAuthDomain` allowlist — are all questions of *"does this gap actually let
someone re-identify a user?"* Under uniformity a gap is a straightforward break. Under randomisation
it depends on whether the unfarbled surface is stable and high-entropy. ⛔ **We have been arguing the
residuals without a written threat model.** That is what Tor's is for.

### 2.2 BDK / `rust-bitcoin` is a **BTC** library

⚠️ **The owner flagged this and was right to.** It was over-recommended on 2026-08-30 as if it were a
general Rust prior-art source. It is not.

**BSV divergences that would produce confident, wrong assumptions:**

- **No SegWit, no Taproot/Schnorr.** BDK's entire model is descriptor-based (`wpkh`, `wsh`, `tr`) — none of it applies.
- **Key derivation differs in kind.** BSV uses BRC-42/43 invoice-number derivation, **not** output descriptors.
- **No RBF** (first-seen rule), different dust and standardness rules, no practical transaction-size cap, restored opcodes.
- **Different SPV model** — BSV uses merkle proofs / BEEF (BRC-62/74).

⭐ **Use it for exactly one thing:** the *data-model* pattern of **"a UTXO that exists but is
deliberately not selectable"** — BDK's separation of an unspendable set from coin selection. That is
wallet architecture, not chain semantics, and it is the closest Rust prior art for sprint 1's
classification seam.

⛔ **Everything else: assume it does not transfer until proven.** Where BDK and `wallet-toolbox`
disagree, **BSV-native wins, every time.**

---

## 3. The ledger

One row per lookup. Fill it when you look, not later.

**Verdict values:** 🟢 **paid off** — changed what we built · 🟡 **context only** — useful background,
no decision changed · 🔴 **dead end** — say so plainly, it is the most useful row in the table.

| Date | Question | Source(s) read | What we learned | Verdict | Landed in |
|---|---|---|---|---|---|
| 2026-08-30 | Who originated farbling, and is Tor the right reference for it? | Brave privacy-updates 3 & 4; Brave fingerprinting wiki | **Brave coined it and shipped it first** (prior work: PriVaricator, FPRandom); Firefox and Safari followed. **Tor uses uniformity, the opposite strategy.** Tor remains the reference for the *threat model* | 🟢 | §2.1; `CLAUDE.md` rule 5 |
| 2026-08-29 | Is there a canonical "Karpathy method" to adopt? | `multica-ai/andrej-karpathy-skills`; Karpathy's repos, blog, `autoresearch` | The viral file is **not his**. 3 of 4 principles adopted; the 4th declined as our false-green mechanism | 🟢 | `SCOPING_PROCESS.md` §6–7; `CLAUDE.md` working rules |
| 2026-08-29 | What do PM skill files offer sprint scoping? | `pm-*` upstream sources | ~6 adoptable items out of ~26 skills; **do not install** | 🟡 | `SCOPING_PROCESS.md` §7a, §8 |
| | ⏳ **RQ-1** — what does "classified" persist as? | BRCs 46/99/147/150/165 → `wallet-toolbox` (TS + Go) → other SDKs → BDK *(narrow, per §2.2)* | | | beta.4 M0 |
| | ⏳ **RQ-2** — restore behaviour for unidentifiable outputs | `wallet-toolbox` recovery path; other BSV wallets; recovery-related BRCs | | | beta.4 M0 |
| | ⏳ Farbling residuals — workers, fenced frames, `IsAuthDomain` allowlist | **Tor Browser design document**; Brave's fingerprinting wiki | | | beta.3 §H backlog |

⭐ **The three ⏳ rows are the open questions this file already knows about.** Filling the last one is
cheap and would settle arguments that have been running since the 0.4.0 farbling work.

---

## 4. What the ledger is for — read this before deciding it is overhead

After ~10 rows, it should be able to answer:

1. **Which sources actually change decisions**, and which we cite out of habit.
2. **Which questions we keep re-asking** — a repeated question is a missing document.
3. ⭐ **Whether "look at prior art" is paying for itself.** If a year of rows is all 🟡, the rule is
   ceremony and should be cut. ⛔ **Recording that honestly is the point.** Same standard as the rest
   of the project: a check that cannot come back negative is not a check.
