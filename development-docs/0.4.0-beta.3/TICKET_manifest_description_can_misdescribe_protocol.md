# A manifest's description can misdescribe the protocol it is attached to

**Filed 2026-08-23**, out of beta.3 Phase 0.8's live testing. Owner-spotted while reading the
bitgenius connect modal. **Deferred by the owner — recorded, not scheduled.**

## The defect

`protocolID` is machine-readable and enforced. `description` is human-readable and displayed.
**Nothing ties them together.** A hostile manifest can pair a sensitive scope with a benign
sentence:

```json
{
  "protocolID": [2, "3241645161d8"],
  "counterparty": "02…",
  "description": "Show your profile picture."
}
```

`3241645161d8` is **BRC-29** — payment-key derivation. The user reads *"Show your profile
picture"*, clicks Connect, and the site holds a BRC-29 protocol grant.

The engine cannot catch this by design: `protocolName` is an **opaque string key**. It is written to
`domain_protocol_permissions` and later matched by `is_protocol_granted` as a literal. The engine
never interprets meaning — that is what lets an app invent any protocol and still be gated
correctly, and it is also what makes the label unverifiable.

## Severity: MEDIUM, not critical — the other perimeters still hold

A protocol grant only silences **ProtocolUse** prompts. It does not cross the other gates:

| Still gated after a misdescribed protocol grant | By |
|---|---|
| Spending | payment gate — per-tx / per-session caps, `dispatch_payment` |
| Identity-key reveal | `dispatch_privacy_perimeter`, unless the user allowed it |
| Key-linkage reveal | same |
| Sensitive certificate fields | `matrix_c.rs` — prompts unconditionally, no opt-out |
| Protected baskets | `is_protected_basket` — never silenced |

So the worst case is **silent use of a cryptographic scope the user did not intend** (signing,
HMAC, key derivation under that protocol), not a drain. Real, bounded.

⚠️ Read alongside the quiet-mode note below: with *"Allow this site to perform wallet operations
without asking each time"* ticked, the site can use **any** protocol regardless of what it declared,
so in that state a misdescription buys the attacker nothing it did not already have. Phase 0.8 made
that state honest in the UI; narrowing the flag itself is the sibling ticket.

## The spec anticipated exactly this — BRC-116 §9.1

> *"Wallets SHOULD scan manifest-sourced permission descriptions for malicious, deceptive, or
> internally inconsistent wording. At minimum, wallets SHOULD detect and flag description text that
> conflicts with structured permission data… When suspicious wording is detected, wallets SHOULD
> warn users clearly before approval."*

We do **not** implement §9.1 today. Phase 0.8 recorded that honestly rather than claiming it.

## Proposed fixes, cheapest first

1. **Show the `protocolID` next to the description.** `[2] 3241645161d8` beside the site's sentence,
   in the muted style already used for counterparties. The label can then never *fully* lie — the
   real scope is on screen for a careful user or a support person. Small, no new machinery.
2. **A known-protocol gloss.** Keep a table of protocol IDs we recognise (BRC-29 `3241645161d8`
   → *"Derive payment keys"*, `identity key retrieval`, `server hmac`, …). When the ID is known,
   display **our** label as ours and the site's text clearly marked as theirs. This reuses the
   provenance machinery Phase 0.8 built (`siteSuggestedTag` and friends) — the modal already knows
   how to say "this text is the site's".
   ⛔ Do **not** silently replace the site's text; show both, attributed. Replacing it would make us
   responsible for a claim we cannot verify either.
3. **Inconsistency scan (the rest of §9.1).** Flag a description whose numbers contradict the
   structured fields — e.g. text saying "up to 500 satoshis" against
   `spendingAuthorization.amount: 5000000`. Cheap for amounts, hard in general; do the amount case
   only.

## Where it would go

- Display: `frontend/src/pages/BRC100AuthOverlayRoot.tsx`, the `manifest_connect_bundle` protocol
  list (both the summary list and the Customize checkboxes).
- Gloss table: a pure module beside `frontend/src/utils/manifestConsent.ts`, unit-tested through the
  `T1f` harness so the mapping cannot drift silently.
- ⛔ **Not** in the engine. The engine must keep treating `protocolName` as opaque — a gloss is a
  display concern, and making the engine care about protocol *meaning* would break the property that
  an unknown protocol is gated correctly rather than mishandled.

## Related

- `0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` — the phase that built the marking this
  would reuse; §8a for the upstream-proposal posture
- `TICKET_quiet_mode_wider_than_manifest.md` — the sibling: quiet mode grants beyond the manifest
- `bitcoin-sv/BRCs` — `wallet/0116.md` §9.1 (security scan), §4.1 (protocol scope)
