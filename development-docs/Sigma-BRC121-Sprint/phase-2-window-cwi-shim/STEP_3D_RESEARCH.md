# Phase 2 Step 3d — `yours.getSignatures` Research + Possible Implementation

> **Status:** Planned. Not started. Scheduled after Step 3b (address derivation /
> sendBsv / exchange rate) and Step 3c (ECIES Electrum BIE1) land and smoke-verify.
> This document is a research charter, not an implementation spec — the spec gets
> written during the research session, informed by real call payloads captured
> from live ordinal sites.

## Why this exists as a separate step

`getSignatures` is the only legacy Yours method that doesn't translate mechanically
to BRC-100. The architectural mismatch is real:

- **Yours `getSignatures`**: dApp builds rawtx in JS, hands raw bytes to wallet,
  asks wallet to sign specific inputs with specific SIGHASH flags. The wallet
  signs opaque bytes — it can't reason about what it's signing.
- **BRC-100 `signAction`**: Wallet builds the transaction via `createAction`. dApp
  references the action by an internal wallet reference. Wallet signs from its own
  knowledge of UTXO derivation metadata. The dApp never sees rawtx.

There is no generic 1:1 translation. Translating requires either reverse-engineering
the dApp's transaction intent from raw bytes, OR adding a parallel low-level signing
API that bypasses BRC-100's safety invariants (basket sub-permissions, action
categorization, semantically-meaningful approval prompts).

The Step 3b + 3c shim deferral (typed `NOT_IMPL` with a clear migration message) is
correct for the foundation. Step 3d is where we decide whether to build the
translation layer — and that decision needs **real data** from live ordinal sites.

## Why we're likely going to need it

The primary use case for `getSignatures` in the Yours/RelayX/1Sat-era ecosystem is
**partial-transaction atomic swaps** — the canonical pattern for ordinal sales and
token trades:

```
User A wants to sell ordinal → User B for 50k sats:

1. A's dApp constructs partial tx:
     input #0  = A's ordinal UTXO
     output #0 = 50,000 sats → A's payment address
2. A's wallet signs ONLY input #0 with
     SIGHASH_SINGLE | SIGHASH_ANYONECANPAY | SIGHASH_FORKID
3. A posts partial signed tx to marketplace
4. B's wallet adds inputs (≥50k sats) + change output, signs with SIGHASH_ALL
5. B broadcasts. Atomic.
```

Yours-era ordinal marketplaces (3dordi.io, 1Sat-era apps, RelayX legacy) use this
pattern. Without `getSignatures` support, those sites can't list / sell / buy
ordinals through Hodos at all.

The BRC-100 path CAN express the same atomic-swap pattern, but the API shape inverts:

```js
// BRC-100 equivalent (wallet-driven, reference-based)
const { actionReference } = await window.CWI.createAction({
  inputs:  [{ outpoint: ordinalUtxo, unlockingScriptLength: 107 }],
  outputs: [{ satoshis: 50000, lockingScript: paymentScript, description: '...' }],
  options: {
    signOutputs: 'single',
    acceptDelayedBroadcast: false,
    noSend: true
  }
});
const { signedAction } = await window.CWI.signAction(actionReference);
// signedAction.tx is the partial tx for the dApp to forward
```

The capability survives; the API differs. So Step 3d is essentially:
**translate `getSignatures(rawtx, inputs)` → `createAction + signAction` flow,
preserving SIGHASH semantics and partial-tx output shape.**

## Research charter

### Phase 3d.A — Live probing (no code)

Goal: capture every `getSignatures` payload from at least three live ordinal sites
to understand actual call shapes.

**Target sites:**
- `3dordi.io` — user-named priority target
- One additional ordinal marketplace (1Sat-era; identify during research)
- One additional token-swap or RelayX-era app (identify during research)

**Probing method:**
1. Inject a DevTools shim that wraps `window.yours.getSignatures` and `window.panda.getSignatures` and logs every call (rawtx hex, sighash flags, inputs array shape) to console — no signing, just observe + reject.
2. Walk each site's primary flows (list NFT, buy NFT, sell NFT, transfer, swap).
3. Capture 5–10 distinct payload shapes per site.

**What we're looking for:**
- Are inputs always single-index with SIGHASH_SINGLE | ANYONECANPAY, or do sites
  ever ask for multi-input signing with SIGHASH_ALL?
- Do any sites use SIGHASH_NONE or non-FORKID variants? (Treat anything missing
  FORKID as a red flag — BSV requires it post-2017.)
- What's the rawtx structure? Always one input + one output (the canonical
  ordinal-sale shape)? Or more complex?
- Do inputs reference UTXOs that live in known wallet baskets (we can look them up
  via `listOutputs`), or are they cross-wallet references (orderbook-style)?
- Are there any non-ordinal use cases mixed in (plain payment that just chose the
  rawtx route)?

### Phase 3d.B — Pattern classification (whiteboard)

Decide which translation strategies cover which observed patterns:

| Strategy | Covers | Cost |
|---|---|---|
| **Pattern-specific JS translators in the shim** | Each known pattern (canonical ordinal sale, multi-input swap, etc.) gets a hand-written `_translateSighashSingleAnyonecanpay()` function in CWIShimScript.h | Low per pattern; many patterns = many handlers |
| **Generic Rust-side rawtx parser** | Any `getSignatures` call: parse rawtx, identify which inputs match wallet UTXOs via `listOutputs` basket lookup, synthesize a `createAction` reference, route through `signAction`, reconstruct the partial tx | High one-time cost; covers everything once built |
| **Hybrid (recommended starting point)** | Pattern-specific handlers for the dominant cases (probably 80% of calls), generic fallback for the long tail | Medium |

The decision depends on Phase 3d.A findings: if all observed payloads cluster
around 1–2 patterns, pattern-specific wins. If they're scattered, generic wins.

### Phase 3d.C — Implementation (separate plan doc)

After 3d.A and 3d.B, write `STEP_3D_PLAN.md` (the actual implementation spec)
parallel to `STEP_3B_PLAN.md`'s style. Define:

- Rust-side helpers needed (rawtx parser, basket-aware UTXO lookup, partial-tx
  reconstructor)
- Shim-side translation layer (which patterns handled inline, which delegate to
  Rust)
- Test plan with captured payloads as integration test vectors
- Atomic commit-level plan
- Forward-think for Phase 3 (which pieces of 3d become the foundation for full
  ordinal-aware UTXO classification)

## Architectural overlap with Phase 3 (ordinals)

Step 3d's "identify which inputs reference wallet UTXOs in known baskets" step
overlaps with Phase 3's ordinal-aware UTXO classification. Two reasonable
sequencings:

- **(A) 3d first, Phase 3 inherits it:** 3d builds basket-aware UTXO lookup as a
  side-effect; Phase 3 extends with full ordinal classification. Lower upfront cost,
  may require 3d refactor later.
- **(B) Phase 3 first, 3d trivially inherits:** Phase 3 ships ordinal-aware UTXO
  model; 3d's `getSignatures` translator drops into the finished model. Cleaner but
  blocks 3d on Phase 3 completion.

Decide based on demand: if ordinal-site interop is the immediate gating need, go
(A) and accept the refactor. If Phase 3 (full ordinal UX) is the primary roadmap
item, go (B) and let 3d wait.

## Out of scope for Step 3d (explicit)

- Full ordinal UX in the Hodos wallet (1Sat envelope construction, ordinal basket
  management, ordinal-aware send flow). Those are Phase 3.
- BSV20 / BSV21 token-specific handling. Phase 3 / Phase 4.
- Modifications to `signAction` itself. Step 3d uses `signAction` as a primitive
  unchanged.
- Multi-party orchestration (escrow brokers, marketplace order matching). The
  shim handles the local-wallet signing leg only; off-chain coordination is the
  dApp's job.

## Risk assessment

| Risk | Mitigation |
|---|---|
| Adding `getSignatures` lets dApps bypass BRC-100's basket-sub-permission gates | Route the translated `createAction` through the same PermissionEngine path as direct `createAction` calls. No bypass. |
| Generic rawtx parser becomes a parallel signing API the wallet can't audit | Lock the translator to a closed set of validated SIGHASH/script shapes; reject anything else with a typed error. |
| Translation layer ships with subtle bugs that mis-sign atomic-swap inputs (user loses ordinal AND gets no payment) | Comprehensive unit tests with captured real-world payloads from 3d.A. Integration tests with mocked counterparty. Live smoke on at least three sites before declaring 3d done. |
| BIP69 / canonical input ordering surprises (Yours-era dApps may not have sorted inputs canonically; signing order matters for SIGHASH preimage) | Capture this during 3d.A. Document the assumption explicitly in the translator. |

## Status

**Not started.** Scheduled after Step 3b + Step 3c land and smoke-verify.

Capturing as a separate step (vs folding into Step 3b) keeps the immediate session
focused on mechanical translations (3b) + self-contained crypto (3c). 3d's risk
profile and overlap with Phase 3 deserve their own decision window.

## Resume instructions

When picking up 3d:

1. Re-read this doc.
2. Read `STEP_3B_PLAN.md` and the Phase 2 README to confirm 3b + 3c landed clean.
3. Read the relevant memory entries: `[[phase2-step3-getsignatures-deferred]]`,
   `[[phase2-step3b-plan-ready]]`, `[[phase2-smoke-verified]]`.
4. Start with **Phase 3d.A** — DevTools probing on 3dordi.io. No code in Hodos
   until we have payloads in hand.
5. After 3d.A, write `STEP_3D_PLAN.md` (the implementation spec) with the captured
   payloads as input. Don't write the spec from speculation.
