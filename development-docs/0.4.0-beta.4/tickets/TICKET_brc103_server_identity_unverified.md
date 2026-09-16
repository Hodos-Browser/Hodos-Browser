# 🔴 AuthFetch accepts the server's identity key without verifying it

**Found:** 2026-09-16, reading `rust-wallet/src/authfetch.rs` while assessing the upstream
`@bsv/sdk` 2.7.1 authentication fix. **Nobody reported this to us** — it is not an upstream advisory,
not a CVE, and not a GHSA.
**Status:** ⬜ UNASSIGNED · **Sprint:** proposed beta.4 · **Filed by:** Claude, confirmed with the owner

> ⚠️ **Method note.** Everything below is **code reading**, on both sides: our `authfetch.rs` and the
> reference `Peer.ts` in `reference/ts-stack`. ⛔ **Nothing was executed and nothing was measured.** No
> request was sent, no server was impersonated, and the exploitability claims in *How exposed are we*
> are reasoning, not results. ⭐ **The first task in the fix is to measure it** — stand up a server that
> asserts an identity key it does not hold, and confirm we accept it.

---

## What happens

`AuthFetchClient::handshake()` POSTs `initialRequest` to `/.well-known/auth` and reads the response:

| `authfetch.rs` | What it does |
|---|---|
| `:143` | Reads `identityKey` from the response body |
| `:147` | Reads `initialNonce` |
| `:151` | Hex-decodes the identity key |
| `:159-163` | Stores both in `AuthSession` and returns |

⛔ **The `signature` field of the `initialResponse` is never read and never verified.** The server's
identity key is taken as asserted. From that point on, `session.server_identity_key` is treated as
the peer's true identity: it is the BRC-42 counterparty for every request signature
(`sign_with_derived_key`, `:199-203`).

`authenticated_request()` then checks nothing on the way back either — `:240-246` inspects the HTTP
status, returns `Rejected` on 401/403, and passes every other response through. **No response
signature is verified.**

⇒ **BRC-103 is mutual authentication. Our implementation performs one direction of it.** We prove
ourselves to the server. The server proves nothing to us.

## ⭐ The reference implementation does verify — this is a conformance defect, not a spec gap

`reference/ts-stack/packages/sdk/src/auth/Peer.ts :: authenticateInitialResponse` (line 610), read
2026-09-16:

- verifies `message.yourNonce` via `verifyNonce`
- verifies the signature over `base64ToBytes(sessionNonce + initialNonce)`, with
  `protocolID: [2, 'auth message signature']`, `keyID: "${sessionNonce} ${initialNonce}"`, and
  `counterparty: message.identityKey`
- **only then** sets `peerSession.peerIdentityKey = message.identityKey` and `isAuthenticated = true`

⭐ **Verifying against the claimed key is correct here, and it is the whole design.** This is the one
moment where a claimed key becomes a verified key: the peer signs both nonces with the key it claims,
so the signature is what promotes the claim. Every later message must then match that verified key —
which is exactly what `@bsv/sdk` 2.7.1 added (`requireMatchingSessionIdentity`).

**The two halves are one mechanism:**

| Half | What it establishes | ts-stack | Hodos |
|---|---|---|---|
| 1. Verify the `initialResponse` signature | The peer holds the key it claims | ✅ since before 2.7.1 | ⛔ **absent** |
| 2. Require every later message to carry that same key | The session cannot be re-attributed mid-stream | ✅ added in 2.7.1 | n/a — we never take the verifier role |

We implement neither. Half 2 does not apply to us today because `authfetch.rs` is a one-directional
client with no inbound message handler. **Half 1 applies and is missing.**

## Why it matters

**No user-visible consequence today that I can demonstrate**, and the ticket should say so rather than
inflate it. What is lost is the ability to detect a server that is not who it says it is:

- **A hijacked or misconfigured endpoint goes unnoticed.** An expired domain re-registered, a DNS
  takeover, or a staging host pointed at the wrong place would present a different identity key. The
  reference SDK rejects; we continue and re-derive keys against the new key without comment.
- **Any trust decision keyed on "we are talking to server X" rests on an unchecked claim.** Today that
  is MessageBox and PeerPay (`messagebox.rs`, `monitor/task_check_peerpay.rs`) — the only consumers of
  `AuthFetchClient`.
- **It weakens our own standards position.** We are drafting
  `Standards/BRCs/drafts/originator-scoped-authentication-keys/`, which argues about *which* key goes
  in BRC-103's `IdentityKey` field. ⚠️ **Arguing about which key to send is awkward while our client
  does not check the one it receives.**

⚠️ **What it is not:** TLS still authenticates the host and protects the channel, so this is not
network-level impersonation. The gap is that we accept a host's self-asserted *BSV* identity on top of
a TLS connection that says nothing about BSV identity.

## How exposed are we — answer this first

| If | Then |
|---|---|
| A measurement shows we accept a server asserting a key it does not hold | Confirmed. Fix in beta.4 as scoped below |
| Something in the wallet **displays, stores or pays** based on `server_identity_key` | ⬆️ Raise priority — an unchecked claim reaching a user-facing trust statement is worse than an internal one. **Not yet traced** |
| MessageBox/PeerPay becomes a headline feature before beta.4 ships | Pull the fix forward |

⛔ **Not a beta.3 interrupt.** Phase 10 is mid-flight on the critical advisories, there is no
compatibility break forcing a date, and no path to spending money has been found through this. Owner
decision 2026-09-16: **talk first, do not add to the current sprint.**

## What already protects us, and how that shapes the fix

- **TLS** — the transport is authenticated even though the BSV identity is not.
- **`certificate_handlers.rs:1532`** already verifies a server signature with `secp.verify_ecdsa` in
  the certificate-acquisition flow, so the pattern exists in our codebase and can be followed rather
  than invented.
- **BRC-42 derivation exists both ways** in `crypto/brc42.rs`. We already derive a child *private* key
  to sign; verification needs the child *public* key from the server's key plus our private key and
  the same invoice.

⇒ **This is closing a gap, not building a system.** The inputs are already in `AuthSession`.

## Proposed fix

**The floor — verify the handshake.** In `handshake()`, read the `signature` field, derive the
server's child public key for invoice `2-auth message signature-{client_nonce} {server_nonce}`, and
verify the ECDSA signature over `base64_decode(client_initial_nonce + server_initial_nonce)`. Fail the
handshake if it does not verify or the field is absent. ⚠️ **Confirm the exact preimage and invoice
against `Peer.ts :: authenticateInitialResponse` before writing it** — the reference orders the nonces
`sessionNonce + initialNonce`, and getting the order backwards produces a check that always fails.

**The system — verify responses.** Verify the per-response `x-bsv-auth-signature` on authenticated
replies. Larger, and it needs a decision about what to do when a server omits the header.

**Deliberately out of scope:** rewriting `certificate_handlers.rs`'s hand-rolled header path to share
`AuthFetchClient`; adding an inbound BRC-103 verifier role; the certifier-mismatch enforcement
question at `certificate_handlers.rs:1678` (logged as CRITICAL, continues anyway — **its own ticket**).

## Test and negative control

⛔ Per `../../0.4.0-beta.3/HARNESS.md`: **a fix is not done until the check has been *seen* to fail.**

| | |
|---|---|
| **GREEN** | A handshake against a server that signs correctly succeeds, and MessageBox/PeerPay still work end to end |
| **RED** | Stand up a local `/.well-known/auth` that returns a well-formed `initialResponse` asserting an identity key it does not hold, or a valid key with a corrupted signature. The handshake must fail. ⭐ **Run this against today's build first** — it is the measurement that confirms the ticket, and it must be seen to *pass* before the fix and *fail* after |
| **SUBJECT** | The Rust wallet process on `:31301`, `AuthFetchClient::handshake`, against the scratch server — not the certificate-acquisition path, which has its own verification |
| **Tier** | Proposed T2 |

**Standing invariant?** ⭐ **Yes.** Propose for `../REGRESSION_ADDITIONS.md`: *"the wallet refuses a
BRC-103 handshake whose `initialResponse` signature does not verify against the asserted identity
key."* This is the kind of check that silently regresses when someone refactors the handshake.

## Links

- `rust-wallet/src/authfetch.rs` — `handshake()` `:110-164`, `authenticated_request()` `:167-247`
- `reference/ts-stack/packages/sdk/src/auth/Peer.ts :: authenticateInitialResponse` — the reference behaviour
- `@bsv/sdk` 2.7.1, `bsv-blockchain/ts-stack` commit `134e4ec` — the other half of the mechanism
- `Marston Enterprises/Standards/BRCs/drafts/originator-scoped-authentication-keys/` — our draft on
  *which* key belongs in the `IdentityKey` field
- `Marston Enterprises/Morning Report/2026-09-16-Wed.md` — where the 2.7.1 fix surfaced
- `TICKET_brc140_key_shares_vs_bip39.md` — the other key-handling ticket filed the same day
