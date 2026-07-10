# Follow-up Ticket — Unify the "next derivation index" source of truth

> **Type:** Wallet-hardening cleanup (tech-debt / privacy, NOT a money bug).
> **Priority:** Low–Medium. Not urgent; **explicitly out of scope** for the
> spent-input reconcile sprint (see [`RECONCILE_SPENT_INPUTS_PLAN.md`](./RECONCILE_SPENT_INPUTS_PLAN.md)).
> **Status:** identified 2026-07-10 during reconcile Phase-1 research; not started.

---

## What we found

Hodos derives the "next" self-derivation index (BRC-42 invoice
`"2-receive address-{index}"`) from **two different sources** depending on the
code path — and they can drift apart:

| Source | Used by | Location |
|---|---|---|
| `wallet.current_index + 1` | **New receive address** (user taps "generate address"); certificate change | `handlers.rs:9299-9326`; `certificate_handlers.rs:2859-2900` |
| `MAX(addresses.index) + 1` (via `AddressRepository::get_max_index`, which excludes special indices −1/−2/−3) | **Change output** on normal sends; backup-tx change | `handlers.rs:5452-5462`, `:13225-13229` |

The change path deliberately distrusts `wallet.current_index` — its own comment
says *"Use MAX(index) from database instead of wallet.current_index … more
reliable — current_index can get out of sync"* (`handlers.rs:5452-5453`). There
is even ad-hoc self-heal code that patches `wallet.current_index` forward when it
detects drift (`handlers.rs:5531-5581`). `wallet.current_index` is *also* the
value backed up and restored (`handlers.rs:16603-16606`) and advanced by recovery
(`handlers.rs:14901-14904`, `:15124-15127`).

## Why it exists

Historical drift-patching: a `current_index` desync bug was hit, the **change**
path was defensively switched to `MAX()`, but the two counters were never
unified. Classic accreted tech debt.

## Severity — deliberately proportionate

- **No fund-loss / wrong-key risk.** Same index → same invoice → **same key**.
  If the two sources disagree, the worst outcomes are:
  - **Address reuse** (an index derived twice → same address twice) → a
    **privacy** regression, not a spend failure.
  - A **gap** in the used-index sequence → harmless for spending; only matters to
    a naive gap-scan (which is why the reconcile primitive anchors on
    `MAX(addresses.index)` with a *symmetric* window — it already tolerates this).
- So this is a **cleanliness + privacy** cleanup, not a money bug. It does **not**
  gate the reconcile sprint.

## Proposed fix (design later)

Make **`MAX(addresses.index)` over `index >= 0`** the single source of truth for
"next self-derivation index," and either (a) derive `wallet.current_index` from it
on read, or (b) retire the column, migrating backup/restore/recovery to the MAX
source. Because this touches **receive-address generation + backup/restore +
recovery** simultaneously, it needs its own small design + adversarial review
pass — do **not** fold it into the reconcile sprint's blast radius.

### Open questions for that design
- Keep `wallet.current_index` as a cache-of-MAX (cheap, backward-compatible) vs
  drop it (cleaner, but a schema/migration + backup-payload change → invariant #2
  "don't change schema without asking").
- Confirm no consumer depends on `current_index` being *ahead* of `MAX` (e.g. a
  reserved-but-unwritten index). Grep every `current_index` read.
- Reconcile with recovery's `update_current_index(max_index)` calls so a restore
  can't regress the counter.

## Related (separate, even smaller) UX check — advanced-wallet address display
Owner observed the advanced wallet panel may **display the most-recently-generated
address rather than a fresh one**. `generate_address` produces a modern BRC-42
receive address (not legacy BIP32; legacy `m/{index}` is recovery-only). Whether
the panel shows-most-recent vs generates-new is a **frontend/UX** decision
(fresh-each-time is better for privacy; neither is a safety issue). Track
separately from this ticket; requires locating the advanced-wallet component + the
endpoint it calls.
