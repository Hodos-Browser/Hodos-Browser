# TICKET — `/signAction` returns `rawTx` (hex) instead of BRC-100's `tx` (AtomicBEEF bytes)

**Status:** 🟢 **FIXED 2026-09-02** — implemented, unit-tested with an observed negative control, preflight `-Full` + `-NegativeControl` PASS. ⬜ **Awaiting the owner's live retest on beta.zanaadu.com.**
**Was:** DIAGNOSED — root cause confirmed against the SDK type, not inferred
**Opened:** 2026-09-02
**Origin:** live failure on **`beta.zanaadu.com`** name-token mint, driven by the owner, watched in the dev wallet log
**Severity:** 🔴 **Breaks every conforming BRC-100 client that calls `signAction` directly.** Money is spent and the transaction *is* broadcast — the caller just cannot see the result, so downstream indexing never happens
**Platform:** cross-platform (Rust wallet)

---

## 1. What the user saw

> `pf head 656f24882329e7da69813423965bafe3597ff7bd166c7b66e92f6ea7ee97f568 broadcast but the`
> `wallet returned no BEEF to submit; the overlay will not index it until it is submitted`

That message is **Zanaadu's own** (§7). Their app is correct; we are not.

## 2. Root cause — one field name and one type

`rust-wallet/src/handlers.rs :: SignActionResponse`:

```rust
pub struct SignActionResponse {
    pub txid: String,
    #[serde(rename = "rawTx")]        // ⛔ spec says "tx"
    pub raw_tx: String,               // ⛔ spec says Byte[] , not a hex string
    #[serde(rename = "unsignedInputs", skip_serializing_if = "Option::is_none")]
    pub unsigned_inputs: Option<Vec<usize>>,   // not in the spec (extra; harmless)
}
```

The authoritative shape, read from the vendored SDK at
`demos/brc121-402/node_modules/@bsv/sdk/dist/types/src/wallet/Wallet.interfaces.d.ts:343`:

```ts
export interface SignActionResult {
    txid?: TXIDHexString
    tx?: AtomicBEEF            // export type AtomicBEEF = Byte[]   (:120)
    sendWithResults?: SendWithResult[]
}
```

| | BRC-100 / `@bsv/sdk` | Hodos today | |
|---|---|---|---|
| field | `tx` | `rawTx` | ⛔ |
| type | `Byte[]` (number array) | hex `String` | ⛔ |
| `sendWithResults` | present | **absent** | ⛔ |
| `txid` | `string` | `string` | ✅ |

`HTTPWalletJSON` (the SDK substrate every dApp uses, default base
`http://localhost:3321`) returns the parsed JSON **verbatim**. So `result.tx` is
`undefined`, and a correct client concludes the wallet returned no BEEF.

## 3. ⭐ The proof it is an oversight, not a design choice

**Our own `createAction` gets it right, in the same file**
(`handlers.rs :: CreateActionResponse`):

```rust
#[serde(rename = "tx", skip_serializing_if = "Option::is_none")]
pub tx: Option<Vec<u8>>,   // Atomic BEEF (BRC-95) as byte array per BRC-100 spec
```

Two response structs, one spec, two different answers. `signAction` is the one that drifted.

## 4. Why nobody noticed — and this is the important part

📏 **Our own code never consumes the non-conforming field the way an external client does.** When
`createAction` drives signing internally it reads the hex and converts:

```rust
// handlers.rs:6242
let tx_data = if let Some(raw_tx_hex) = json_resp["rawTx"].as_str() { … }
```

So the **one-shot** path (`createAction` with `signAndProcess`) works end to end, and every internal
test exercises that path. The bug is only reachable on the **two-phase** BRC-100 flow —
`createAction` → `createSignature` → `signAction` — which is exactly what a dApp signing its own
custom inputs must use, and exactly what Zanaadu does.

⇒ **This is a "works for us, broken for everyone else" defect**, and no amount of internal testing
would have surfaced it. The acceptance test must drive the SDK, not our own handler.

## 5. What actually happened on the wire (measured 2026-09-02, dev wallet 31401)

| Step | Result |
|---|---|
| `/createAction` | ✅ 200. Outputs: `[0]` 1 sat basket `pf-name-head`, `[1]` 10 000 sats tags `platform-fee,pf`. `acceptDelayedBroadcast=false` |
| basket validation | ✅ passed — `pf-name-head` normalized unchanged |
| `/createSignature` ×3, `/verifyHmac`, `/verifySignature` | ✅ BRC-103 handshake with their backend, all 200 |
| `/signAction` | ✅ signed `656f2488…`, built **103 644-byte** standard BEEF → **Atomic BEEF (BRC-95)** |
| broadcast | ✅ `📡 Broadcasting via Services chain (ARC GP → TAAL → MAPI → WoC): 103 680 bytes` |
| HTTP response | ✅ `200`, **207 446 bytes** — the BEEF *was* returned |
| ⛔ the client | read `result.tx` → `undefined` |

🎯 **The money was spent, the transaction was broadcast, and the caller was told nothing came back.**
That is the worst shape of this bug: not a clean failure, a *silent* one after the spend.

## 6. ⛔ Refuted hypothesis — recorded so nobody re-runs it

The initial theory (owner's and mine) was **basket prefixes**: `validate_and_normalize_basket_name`
rejects every name starting with `p ` (BRC-99 permissioned baskets), and BRC-174 defines a legitimate
`p 1sat` basket — so we would reject every 1Sat Ordinals write.

📏 **Not this bug.** Zanaadu's basket is `pf-name-head` — `pf-`, a hyphen, no space — so the rule never
fired. The `p ` overreach is **still a real defect** (§8) and still worth fixing, but it is not what
the owner hit.

## 7. The truncated popup is **theirs**, not ours

📏 Checked the browser log for the same window: the only Hodos overlay shown was
`promptType=manifest_connect_bundle` (the connect prompt, approved). We never render app error text.
The wording *"the wallet returned…"* is written from the app's point of view about us. ⇒ the
truncation is Zanaadu's UI. ⚠️ Worth mentioning to their dev anyway — the message is the only
diagnostic their users get, and it is being cut off.

## 8. Proposed fix

⭐ **Emit both fields for one release, then drop the old one.** A straight rename breaks our own
internal reader at `handlers.rs:6242`/`:6206` and any integrator already coded against `rawTx`.

1. Add `tx: Option<Vec<u8>>` (Atomic BEEF bytes) to `SignActionResponse`, populated from the same
   `beef_hex` already built.
2. Keep `rawTx` for now; mark it deprecated in a comment with the removal release.
3. ~~Add `sendWithResults`.~~ ⛔ **DROPPED on measurement — see §11.** `SignActionOptions.send_with`
   is parsed and then **never read** by `sign_action`, so emitting a results array would be reporting
   on a feature that does not run. Reported instead.
4. ~~Update `handlers.rs:6242` / `:6206` to prefer `tx`.~~ **Not needed while `rawTx` stays** — those
   readers keep working unchanged, and touching them would add risk for no present benefit. The
   coupling is recorded in the deprecation comment on the field so whoever removes `rawTx` finds it.
5. Separately (**not** the same commit): `validate_and_normalize_basket_name` should refuse
   **unknown** `p <scheme>` names, not all of them — `p 1sat` is spec-defined (BRC-174). And
   `list_outputs` should validate at all: it calls `find_or_insert` with no check, so a *read* on any
   `p …` name silently creates the basket row. Both were filed unverified in
   `TICKET_loopback_host_form_wallet_routing.md` §7.3; **both are now confirmed by reading the code.**

## 9. ⛔ Acceptance test — and its negative control

The test **must be driven through `@bsv/sdk`'s `WalletClient` / `HTTPWalletJSON`**, not through curl
against our handler, or it re-creates the blind spot in §4.

| | |
|---|---|
| 🟢 **GREEN** | Two-phase flow (`createAction` with a custom input → `createSignature` → `signAction`) against the dev wallet; assert `typeof result.tx === 'object'` and `result.tx.length > 0`, and that `Transaction.fromAtomicBEEF(result.tx)` parses |
| 🔴 **RED** | Revert the `tx` field → the same script must report *"no BEEF"*. ⭐ This RED is already **observed**: it is the live 2026-09-02 Zanaadu failure |
| 🎯 **SUBJECT** | The **SDK client's** parsed `result`, not our JSON body and not our log. The wallet already logs `✅ Atomic BEEF created` today, while the client gets nothing — reading our log is exactly how this stayed invisible |

## 10. Placement

Small and self-contained (one struct + one call site + a test). ⚠️ It **blocks a live integration
partner today**, and the failure spends money before failing.

- Recommend **beta.3**, not beta.4 — it does not depend on the Ordinals/OpNS work and the partner is
  waiting.
- §8.5 (the basket-namespace half) is genuinely **beta.4**, alongside the 1Sat Ordinals sprint, since
  `p 1sat` is that sprint's basket.


---

## 11. 📖 Found while fixing — reported, NOT fixed (working rule #3)

**`signAction` accepts `sendWith` and silently ignores it.**

📏 `SignActionOptions` declares `#[serde(rename = "sendWith")] pub send_with: Option<Vec<String>>`, and
`grep send_with` across the whole of `sign_action` (handlers.rs:7391–8546) returns **nothing**. The
option is parsed and dropped.

⚠️ Consequence: a dApp that batches with `signAction({ options: { sendWith: [...] } })` gets a 200 and
believes its other transactions were broadcast. They were not. That is the same *shape* as the bug
this ticket fixes — a silent success — and arguably worse, because here the caller is told nothing
at all rather than merely being handed the wrong field name.

⛔ Deliberately not fixed here: it is a behaviour change to the broadcast path, not a response-shape
correction, and bundling it would make this fix un-revertable on its own. It is also why
`sendWithResults` was dropped from §8 — populating a result array for an input we discard would be
inventing a value, which is exactly the kind of plausible-looking output this sprint keeps catching.

**Suggested home:** beta.4, alongside the Ordinals/OpNS work that will actually batch transactions.

---

## 12. Implementation record — 2026-09-02

| | |
|---|---|
| **Change** | `SignActionResponse` gains `tx: Option<Vec<u8>>` (BRC-100 AtomicBEEF bytes). `rawTx` kept and marked deprecated with its two internal readers named |
| **Seam** | ⭐ `SignActionResponse::from_atomic_beef(txid, beef_hex, unsigned)` derives **both** fields from one hex string, so "they drifted" and "someone passed `tx: None`" are unrepresentable rather than merely discouraged |
| **Tests** | 4 in `handlers.rs :: sign_action_response_shape_tests` |
| 🔴 **Negative control** | **Observed.** Forced `tx = None` inside the constructor → `tx_is_present_as_a_byte_array` and `tx_and_raw_tx_cannot_drift` went **red** with `BRC-100 SignActionResult.tx is missing — this is the shipped defect`. Reverted; green returned; zero residue |
| **Gates** | `preflight.ps1 -Full` **PASS** (8 gates + 7 T1). `-NegativeControl` **PASS** |

### ⭐ The test caught a hole in itself first

The first version of `sample()` built `SignActionResponse` as a **struct literal**. That version passed
— and would have kept passing if the handler set `tx: None`, because it never touched the handler's
code path. It asserted only that serde renames fields.

That is the same false-green shape as the four farbling harnesses in `CLAUDE.md`, in miniature. The
constructor exists so the test and the handler share one seam; the negative control above is only
meaningful *because* of it.

⬜ **Still owed: the live retest.** A unit test proves the JSON shape. It does **not** prove that
`@bsv/sdk` accepts it end to end, which is the actual claim. That needs a real mint on
beta.zanaadu.com.

---

## 13. Live retest — 2026-09-02 10:04. ✅ The fix works. The new error is the OLD failure's aftermath.

The owner retested and got a different error:

> `pf-mint: the transaction is broadcast on-chain but the overlay did not index it after retries`
> `(Overlay /submit failed: 502 - … ARC broadcast rejected: SEEN_IN_ORPHAN_MEMPOOL). Do NOT retry`
> `the action — a rebuilt pf spend would double-spend…`

⭐ **That is progress, not a regression.** The app got past *"the wallet returned no BEEF to submit"*
to *"I submitted it and the overlay rejected it"* — which is only reachable **because `tx` is now
present**. The fix is confirmed end to end through `@bsv/sdk`, which is the claim the unit test could
not make.

### 📏 What actually happened, measured

| | Round 1 — 09:23 (pre-fix) | Round 2 — 10:04 (post-fix) |
|---|---|---|
| tx | `656f2488…` | `94bcfb42…` |
| our broadcast | ✅ `gorillapool_mapi accepted` | ⛔ **all 4 providers rejected: `"Missing inputs"`** |
| on-chain | ✅ **CONFIRMED, block 965009, 5 confs** (WhatsOnChain) | never landed |
| overlay | ⛔ never submitted — we returned no `tx` | ⛔ `502 SEEN_IN_ORPHAN_MEMPOOL` |

**The input tells the whole story.** Round 1's confirmed transaction spends `8830f832…:0` and
`3df84d90…:2`. In round 2 the dApp supplied, as a user input, `Input 0: 8830f8321116c3e4:0` — **the
same outpoint round 1 already spent and confirmed.**

⇒ Round 2 was a genuine **double-spend attempt** and the network was right to refuse it.
`SEEN_IN_ORPHAN_MEMPOOL` and `"Missing inputs"` are two miners' wording for the same fact.

### 🎯 The causal chain, and where the fault lies at each link

1. Round 1's mint was built, broadcast and **confirmed on-chain**. ✅ ours, worked.
2. We returned no `tx`, so the dApp could not submit it to the overlay. ⛔ **our bug — now fixed.**
3. The overlay therefore never indexed it, and the dApp's view of "which pf head is unspent" comes
   from the overlay. Their state is stale **because of step 2**.
4. Round 2 the dApp handed us the stale head. ⛔ **their state, downstream of our bug.**

⇒ Nothing in round 2 indicts the fix. The residue is a **data** problem on their side, seeded by our
defect, and their own error text prescribes the remedy: do not retry; let the drift auditor re-index.

### ⛔ We do NOT talk to that overlay

Worth stating because it is the owner's first question. `src/overlay/mod.rs` submits to exactly one
topic — `TOPIC_IDENTITY = "tm_identity"`, for BRC-52 certificates. There is no pf/OpNS submit path in
this wallet. **`Overlay /submit` is Zanaadu's own call**, made with the BEEF we hand back. Our job
ends at returning a valid `tx`; theirs begins there.

## 14. 🚨 NEW — found while confirming the retest: a fatal broadcast failure still returns `200`

📏 Round 2's broadcast failed **fatally** — `is_fatal_broadcast_error` fired and logged
`❌ Fatal broadcast error: … "Missing inputs"` — and `signAction` then returned
**`200` with 334 774 bytes of BEEF**. The caller cannot tell a broadcast that landed from one the
whole provider chain permanently refused.

```rust
Err(e) => {
    log::error!("   ❌ Broadcast failed: {}", e);
    // Leave status as 'sending' — TaskSendWaiting (120s) will retry
}
```

Two problems, and the second is worse:

1. **The caller is told success.** Zanaadu only discovered the failure because their *overlay*
   independently rejected it. A dApp with no overlay step would record a mint that never happened.
   ⭐ This is the **same shape as the bug this ticket fixed** — a silent success after a real failure.
2. **The retry comment is wrong for this class.** The error was classified **fatal**, yet the status
   is left `sending` so `TaskSendWaiting` re-attempts it every 120 s. `"Missing inputs"` is permanent:
   the input is spent and confirmed. That is an unbounded retry of a transaction that can never
   succeed.

⛔ **Reported, not fixed** (working rule #3): this is a behaviour change on the money path — it
changes what a dApp sees on a failed spend — and it is not what this ticket came for. It also needs a
decision the owner should make: does a fatal broadcast failure become a non-2xx, or a 200 carrying an
explicit failure field? BRC-100's `SignActionResult` has no error member, so the answer is not
obvious from the spec.

**Suggested home:** beta.3 Phase 8 (alongside `TICKET_wallet_backend_death_is_silent_and_unrecovered`
— same family: a failure the user is never told about).
