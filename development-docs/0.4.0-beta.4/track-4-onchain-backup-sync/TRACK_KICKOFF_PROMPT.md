# Kickoff prompt — backup/recovery research track + patent research

**Written 2026-08-22.** Paste the prompt below into a fresh session started in
`C:\Users\archb\Marston Enterprises` (so project memory auto-loads).

---

I want to run a deep research track on our on-chain wallet backup system, using multi-agent
workflows. This is the part of Hodos that has given us more trouble than any other part of
development, it handles people's real money, and I want a rigorous plan before we write more code.
Use the Workflow tool to orchestrate this — fan out agents in parallel where the work is
independent. Two workflows, run in sequence.

## Context to load first

- `C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.4\track-4-onchain-backup-sync\README.md` — the current
  work plan (items 0–7, including item 0 = BRC-38 compat gate and item 0b = delta-format prior
  art, both defined but NOT yet run). Its core rule stands: **the BRC draft follows the code, not
  the other way around.**
- `C:\Users\archb\Marston Enterprises\Standards\BRCs\drafts\wallet-backup-and-sync-onchain\` —
  `wallet-backup-and-sync-onchain.md` (the BRC draft) and `DELTA_ANALYSIS.md` (delta analysis,
  verified against our code 2026-08-19).
- `C:\Users\archb\Hodos-Browser\development-docs\ONCHAIN_BACKUP_SYSTEM.md` — what ships today
  (five strips, triggers, dirty-flag, recovery flow).
- Memory file `project_onchain_backup_delta_design` — full state including the 2026-08-22
  analysis of deggen's go-private-backup-cache and the decisions made with me.

## Workflow 1 — research track → implementation plan

**Phase A — our own code, honestly.** Agents dig into `C:\Users\archb\Hodos-Browser`:
- `rust-wallet/src/backup.rs` (~2,284 lines), the recovery/restore path, and the database
  migrations (table schemas). Map what actually exists vs what the docs claim.
- **Export/import code — I believe it is mostly disabled.** Find it, confirm what's disabled,
  document why (git log/blame), and what it would take to revive it.
- **A retrospective agent**: walk the git history of backup.rs and related files, catalog every
  backup-related bug, revert, and painful episode, and extract root causes. I want a written
  answer to "why has this component hurt us more than any other, and which past design choices
  caused it." No blame, just mechanisms — this informs what the new design must not repeat.

**Phase B — the specs.** Read BRC-38 (User Wallet Data Format), BRC-39 (encryption extension),
BRC-40 (synchronization) in full from `bsv-blockchain/BRCs` (`outpoints/0038.md`–`0040.md`), plus
BRC-42/43 as they bear on our seed-derived backup address. Then **execute item 0 from the README**:
the table-by-table compat assessment (our ~18 tables vs BRC-38's 13, classified by the tier system
already defined there: core / assets-and-attestations / wallet-local).

**Phase C — prior art in running code.** Execute item 0b from the README:
- **wallet-toolbox** — both the TypeScript (`bsv-blockchain/wallet-toolbox`) and Go
  (`go-wallet-toolbox`) implementations: how sync chunks work on the wire, how they relate to
  BRC-40, the storage schema, and any import/export/backup machinery. Ty Everett publicly stated
  toolbox remote-storage sync IS BRC-38/39 — verify what that means in code.
- **`bsv-blockchain/go-private-backup-cache`** (deggen's encrypted delta-chunk cache, our
  off-chain sibling): read `client/client.go` and `ts-client/` to determine whether blobs are
  toolbox-native (BRC-40-shaped?) or opaque under the client contract; note the log semantics we
  might share (contiguous seq, parent hash, generations-as-snapshots). ⚠️ The repo has NO license
  — read for format and semantics only, never vendor code.

**Phase D — synthesis.** Produce, in
`C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.4\track-4-onchain-backup-sync\`, an
`IMPLEMENTATION_PLAN.md` that the existing README's items 0–7 get reconciled against (update the
README where the plan supersedes it; don't leave the two contradicting). The plan must contain:

1. **Goals and requirements** — numbered, each one testable. Non-negotiables: multi-device sync
   works; the backup is always current enough that a lost device never loses spendable money;
   recovery = seed only; costs stay low; this is real money, so correctness beats cost beats
   speed.
2. **Adopt/adapt/diverge decisions on record** for the payload (vs BRC-38/39) and the delta
   format (vs toolbox chunks / the cache's log semantics), with reasons. If our chunks can align
   with toolbox's, one wallet feeds both rails (his HTTP cache, our chain) — that is the preferred
   outcome unless Phase C surfaces a real blocker.
3. **Trigger + cadence reassessment** informed by the delta analysis: how many deltas before a
   new full snapshot token (evaluate DELTA_ANALYSIS.md's 0.5×/20-deltas/16KB proposal against
   real measured payloads), what the polling rules are (poll before any spend stays), what the
   dirty-flag becomes under deltas, and the measured cost table per wallet profile (payments-only
   / mixed / token-heavy collector) — the BRC draft's current numbers are estimates and must be
   replaced with data.
4. **Phases for implementation AND testing** — each phase independently shippable, each with
   explicit acceptance criteria.
5. **The test harness spec — this is the deliverable that matters most.** Very specific
   end-to-end testing goals for the entire flow, at minimum: fresh install → seed only → full
   restore → **successfully spend**; delta replay correctness (property: snapshot+deltas ≡ full
   state, byte-exact after normalization); multi-device concurrent writes with fork detection and
   recovery; crash mid-backup; corrupted/missing chunk handling; restore from every generation
   boundary; cost accounting assertions; and a soak test simulating months of realistic activity.
   Strict pass/fail criteria, no "seems to work."

## Workflow 2 — after Workflow 1 completes: patent research

First create the folder `C:\Users\archb\Marston Enterprises\Patents\`. Then run a research
workflow on the potentially novel method in our on-chain backup design, specifically:
**deriving deterministic addresses from the wallet seed that point to UTXOs carrying the
stripped, compressed, encrypted wallet-database state (as a token chain), and the recovery
process that rebuilds full database state by walking that chain from the seed alone.**

- **Prior art sweep**, multiple angles in parallel: patent databases (Google Patents, USPTO
  full-text, WIPO/espacenet — search seed-derived backup, deterministic address data storage,
  on-chain encrypted state backup, wallet recovery); **nChain's portfolio specifically** (they
  patent heavily in BSV's neighborhood); academic literature; and existing products/protocols
  (Ledger Recover, wallet-toolbox, go-private-backup-cache, BIP/SLIP backup schemes, Namecoin-era
  data-in-chain designs, our own BRC corpus).
- **Deliverable in `Patents\`**: `PRIOR_ART.md` (what exists, closest matches, with citations) and
  `RECOMMENDATION.md` (is any element plausibly novel; what a provisional would need to claim;
  whether to consult a patent attorney; the defensive-publication alternative).
- ⚠️ **Sequencing constraint the recommendation MUST address:** we plan to publish this design as
  an open BRC, and our shipping beta already writes these tokens on mainnet. Public disclosure
  starts patent clocks (US: 12-month inventor grace; most other jurisdictions: absolute novelty —
  possibly already compromised by our own shipping/on-chain usage). The recommendation must
  assess what we have already publicly disclosed and state clearly: if a filing is worth pursuing,
  it must happen BEFORE the backup BRC is submitted publicly — the BRC submission timing now
  depends on this answer. Flag clearly that this is research, not legal advice, and that a real
  patentability opinion needs an attorney.

Throughout: verify claims against code and primary sources, not summaries; record what was checked
and what wasn't; where agents disagree, surface the disagreement instead of averaging it. When
both workflows are done, give me the short version: the three biggest findings, the
implementation plan's phase list, and the patent recommendation in two sentences.
