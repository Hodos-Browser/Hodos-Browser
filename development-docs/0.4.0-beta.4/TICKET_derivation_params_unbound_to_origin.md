# TICKET — key-derivation parameters are taken from the app, unbound to the requesting origin

**Filed:** 2026-09-02 (found while fact-checking the identity video's isolation claim)
**Severity:** privacy — a connected site can obtain **another site's** derived key for the same user,
and any two connected sites can agree on a shared, unclearable cross-site identifier
**Status:** OPEN — beta.4 candidate. ⛔ **NOT beta.3.**
**Confidence:** ⚠️ **CODE READING ONLY. Nothing executed, nothing measured.** ⭐ **Item 1 below is a
prerequisite for acting on any of this.**
**Background:** `Marston Enterprises/Hodos/Marketing/Videos/3 - Account Creation and Authentication/FINDING-silent-derived-key-tracking.md`

---

## Statement

`get_public_key` (`rust-wallet/src/handlers.rs:259–468`) takes `protocolID`, `keyID` and
`counterparty` **entirely from the request body**, and never binds any of them to the origin that
actually sent the request.

`X-Requesting-Domain` — the header the C++ layer stamps on external requests — **is used five times
in the whole wallet, and every one of them is access control**:

| Where | Use |
|---|---|
| `main.rs:59` `domain_trust_mw` | Universal trust gate — approved / blocked / unknown |
| `handlers.rs:626` `check_domain_approved` | Is this domain approved? |
| `handlers.rs:12991` / `15368` / `17229` / `17312` | Reject external origins from backup / restore / export / import |

⛔ **It is never used to construct a derivation parameter.** So the wallet knows exactly which site is
asking, and then derives from whatever that site claims instead.

## Consequence 1 — a site can request another site's key

Any scheme that scopes a key by putting the origin in the `keyID` is defeated, because the `keyID` is
supplied by the caller. Site B sends:

```json
{ "protocolID": [2, "login"], "keyID": "https://site-a.example", "counterparty": "self" }
```

…and receives **the user's site-A login key**. It cannot sign with it — that needs the private key —
but it does not need to. **The public key alone identifies the visitor as someone who has an account
at site A.**

⚠️ **This is a live design question, not just a code issue.** It is precisely the convention proposed
in the video outline's §6c-bis, and it would have shipped with the hole in it.

## Consequence 2 — two sites can agree on a shared identifier

`derive_child_public_key(master_priv, counterparty_pub, invoice)` — with `counterparty: anyone` the
counterparty is a constant (`PrivateKey(1)`), and with `self` it is the user's own master pubkey,
also constant. Either way the result depends only on **the master key and the invoice string**.

⛔ **Two sites using the same invoice therefore receive the same key**, and can correlate the user
between them. **And it cannot be cleared** — it is derived, not stored, so clearing cookies, site
data and local storage changes nothing.

⭐ **Only `counterparty` = the requesting site's own identity key is inherently site-scoped.** The
shipped `/.well-known/auth` handshake does use that (`2-identity`, counterparty = the app's identity
key from the request) — **that path is not implicated.** The exposure is the general-purpose
`getPublicKey` beside it.

## Consequence 3 — no gate fires on this path at all

`get_public_key` contains **exactly one** gate — `dispatch_privacy_perimeter`, which fires only when
`wants_identity_key` is true. Supplying protocol params is what makes it not fire:

```rust
let wants_identity_key = identity_key || protocol_id.is_none() || key_id.is_none();
```

⛔ **And the comment at `handlers.rs:245` is wrong about this**, which is probably how it was missed:

> *"Derived child keys (BRC-42 path below) bypass this gate; their own protocol/basket gates fire via
> createHmac/createSignature endpoints."*

`create_hmac` (`:935`) and `create_signature` (`:3669`) do carry `dispatch_scoped_grant`. **But that
does not cover `getPublicKey`**, and a party that only wants a stable identifier never needs to call
either of them. **The gated endpoints are the ones it can skip.**

## Mitigation that already exists

⭐ `domain_trust_mw` means an **unapproved** origin cannot reach this at all — it gets 202 and the
connect modal. **This is not a drive-by; it requires one connect click.**

⚠️ **But connecting is not creating an account**, and no user's model of "connect" includes "you may
now derive a permanent identifier for me." That gap is the substance of the ticket.

## Proposed fix — the rule, not just the patch

⭐ **The wallet must supply the origin itself, from `X-Requesting-Domain`, and MUST NOT accept it from
the application.** Any scheme in which the application declares its own scope is not a scope.

Options, not yet compared:

| Option | Note |
|---|---|
| **Bind the origin into the invoice server-side** for external callers — wallet overwrites or prefixes the app-supplied `keyID` | Strongest. ⚠️ May break callers that legitimately choose their own keyID |
| **Reject `counterparty: anyone` / `self` from external origins**, forcing the site's own identity key | Closes the cross-site case structurally. ⚠️ BRC-174 §3 derives with `anyone` — but as a *first-party* operation, not an external request. **Needs a survey of real callers first** |
| **Add a scoped grant** to the derived path, mirroring `createHmac` | Consistent with the existing design. ⚠️ **Prompt fatigue** — this path is called constantly; a modal per derivation is unusable |

⚠️ **Interop risk is the reason this is a ticket and not a patch.** BRC-100 apps in the wild pass
their own protocol IDs and key IDs, and that is what the spec tells them to do. **A unilateral change
could break conforming apps.** Whatever we choose is also a candidate BRC comment.

## Tasks

1. ⭐ **MEASURE IT.** Scratch approved domain; call `getPublicKey` with identical protocol params from
   two different origins; confirm the returned keys are identical. Then request another origin's
   `keyID` and confirm the key comes back. ⛔ **Nothing below proceeds until this is done** — the
   house standard for a claim of this kind is MEASURED, per the `set_domain_permission` note in
   `main.rs:76–110`.
2. **Fix the misleading comment at `handlers.rs:245`** — worth doing regardless of the outcome, and
   it is a one-line change.
3. **Survey which real callers pass `counterparty: anyone` or `self`** across our own demos, the
   `@bsv/sdk` examples and any dApps we can reach, before choosing an option above.
4. **Decide the option** and whether it needs a BRC comment on BRC-43's app-declared security level.
5. **Add a regression** asserting that two origins requesting identical params receive **different**
   keys.

## Note on BRC-43 — not our defect, but relevant

BRC-43's answer to this is the security level: level 0 requires no permission, 1 asks per protocol, 2
asks per protocol per counterparty. ⛔ **But the application declares the level** — BRC-43's own
example reads *"The application is requesting to use security level 0."* A protocol designed to track
declares level 0 and is never questioned. **The control is advisory and the party it constrains is
the party that sets it.**

⚠️ This is not unique to BRC-43 — OAuth scopes and Android permission declarations have the same
shape. Worth raising with the registry; not worth presenting as a scandal.
