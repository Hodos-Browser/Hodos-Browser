# Scope: Read-Only OpNS Name Resolution

**Created:** 2026-08-10
**Status:** scoped, not started. Sprint-deferred.
**Related:** `OPNS_REVIEW.md` (protocol teardown), `README.md` (naming posture),
`Marston Enterprises/Standards/BRCs/drafts/consensus-unique-name-tokens.md` (the BRC this validates)

> **Why this exists now:** the BRC draft's weakest point under adversarial review is that we
> specify a resolution algorithm we have not run. Phase 0 below removes that objection in days, not
> weeks, and does not touch the product. Phases 1–3 are the actual feature and can wait for a
> sprint slot.

---

## What we already have (verified 2026-08-10)

Nothing here needs to be built.

| Capability | Where | Note |
|---|---|---|
| SHIP/SLAP host discovery | `rust-wallet/src/overlay/mod.rs` (1,122 lines) | `DEFAULT_SLAP_TRACKERS`, per-topic discovery |
| SWR host cache | `rust-wallet/src/overlay/ship_cache.rs` | `FRESH_TTL` 5 min, `STALE_TTL` 30 min, bg refresh |
| Overlay lookup POST | `rust-wallet/src/identity_resolver.rs` | `{service, query}` shape, multi-endpoint fan-out |
| Name→identity UI plumbing | send form + omnibox | today backed by `ls_identity` |
| Identity cache | `identity_resolver.rs` | 10 min TTL, caches negatives |
| BEEF / AtomicBEEF parse | `rust-wallet/src/beef.rs` | needed to fetch + verify the name tx |
| Script parsing | `rust-wallet/src/transaction/` | PushDrop decode is the one gap |

**The gap is small and specific:** PushDrop decode + the BRC-42/43 public-key re-derivation check.
Everything else is a new topic string and a new query shape against machinery that already runs in
production.

## What is deliberately NOT in scope

- **Minting / registering names.** Requires `mine.shruggr.cloud`, closed source, ~0.25 BSV flat.
  Separate decision, separate budget, separate risk. Read-only means read-only.
- **Writing or transferring bindings.** Publish-side comes after resolve-side proves out.
- **Replacing `ls_identity`.** OpNS resolution is *additive*. The certifier-filtered path keeps
  working unchanged; we add a second resolution source, we do not swap one for the other.
- **`.hodos` TLD.** The contract supports a `tld` constructor prop, but standing up our own
  namespace is exactly the "run our own ecosystem" posture we rejected.

---

## Phase 0 — Verification spike (BRC-blocking, ~1–3 days)

**Goal:** one verified end-to-end resolution against mainnet, reproducible, outside the product.
This is what lets the BRC say "implemented" instead of "proposed."

Standalone Rust test or CLI binary — no UI, no wallet integration, no persistence.

1. `GET https://api.1sat.app/1sat/opns/origin/<name>` → current outpoint.
   Known-good fixtures: `bitcoin` → `3ae7afae…914a90.2`, `shruggr` → `d10057af…46b84b.2`,
   `satoshi` → `033558de…f26e3.2`.
2. Fetch the tx, extract the locking script at that vout.
3. Decode the PushDrop; read `fields[0]` as the 33-byte identity pubkey.
4. Re-derive the expected locking pubkey:
   `KeyDeriver('anyone').derivePublicKey([0,'p 1sat'], "opns:{txid}_{vout}", idKey)`
   where the outpoint in `keyID` is the **input spent to create this output**, not the output
   itself. Verify it matches the script, and verify the field signature.
5. **Negative controls — required, not optional.** A resolution that only ever succeeds proves
   nothing:
   - tamper one byte of `fields[0]` → step 4 MUST fail
   - substitute a valid binding from a different name → MUST fail (this is what `keyID` prevents)
   - point at a spent outpoint → MUST be rejected
   - an unbound name token → MUST return "no binding," never fall through to another source

**Exit criteria:** all five pass, with the byte-level derivation written down well enough that a
third party could reproduce it from our notes alone. That artifact goes straight into the BRC's
Implementations section and into the message to shruggr.

**Risk:** the `keyID` outpoint semantics in step 4 are from one line of SDK documentation
(`docs/protocols/opns-paymail-bind.md`). If our reading is wrong, step 4 fails against real data
and we need to read `@1sat/actions` source or ask. **That discovery is itself worth the spike** —
better to find it here than in a published spec.

## Phase 1 — Resolver module (~3–5 days)

- New `rust-wallet/src/opns_resolver.rs`, modeled on `identity_resolver.rs`.
- SHIP discovery for the OpNS topic — reuse `ship_cache`, add the topic constant.
  **Confirm the real topic name first** (`GET /listTopicManagers` on the four SLAP trackers);
  `tm_opns`/`ls_opns` is our proposal, not a verified fact.
- Static fallback endpoint `https://api.1sat.app` for cold start, same pattern as
  `STATIC_OVERLAY_ENDPOINTS`.
- Multi-endpoint fan-out; on disagreement, verify each and surface rather than pick.
- Cache resolved bindings with the same 10-minute TTL, **and cache negatives** — an unregistered
  name is the common case and should not re-query.
- Freshness rule: re-check spend status within 60s before any send. Match the BRC's § 6.

## Phase 2 — Wire into resolution UX (~2–3 days)

- Send form and omnibox try OpNS **in parallel** with `ls_identity`, not in sequence — do not
  regress send-form latency.
- Distinguish the two visually. A consensus-unique name and a certifier-attested handle are
  different objects and should not render identically; conflating them is the whole argument.
- Key-change warning per BRC § 7.4 — reuse whatever the certificate path already does.
- Confusability check before send for names not in the address book. Reuse BRC-169 § 2.3's
  skeleton algorithm rather than inventing one, so we agree with other clients on what is
  suspicious.

## Phase 3 — Dogfood (~1 day, gated)

- Claim `hodos` and `marston` (~0.25 BSV each, ~$3, still unregistered as of 2026-08-10).
- Bind them to our identity keys, resolve them in our own browser.
- **Gate:** do not do this until Phase 0 proves we can verify a binding. Buying names we cannot
  yet verify is backwards.

---

## Open questions

1. **Topic name.** Unverified. Blocks Phase 1 and BRC § 5.2. Cheapest thing on this list — do it first.
2. **`keyID` outpoint semantics.** Input-spent vs. output-created. Phase 0 settles it empirically.
3. **Resolver policy on an unbound name.** Spec says return "no binding" and stop. Confirm the UI
   does not silently fall through to `ls_identity`, which would defeat the point.
4. **Do we need our own overlay?** `b-open-io/opns-overlay` is MIT, so self-hosting is legally
   clear — unlike the rest of the 1Sat stack. Not needed for read-only, but it is the answer if we
   ever need a second source we control.

## Effort

| Phase | Estimate | Blocking? |
|---|---|---|
| 0 — verification spike | 1–3 days | **Blocks credible BRC submission** |
| 1 — resolver module | 3–5 days | no |
| 2 — UX integration | 2–3 days | no |
| 3 — dogfood | 1 day | gated on Phase 0 |

Phase 0 alone is the high-leverage item. It is small, it is off the critical path of the current
sprint, and it converts the BRC from a proposal into a description.
