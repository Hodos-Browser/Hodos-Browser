# TICKET — `/signAction` returns `rawTx` (hex) instead of BRC-100's `tx` (AtomicBEEF bytes)

**Status:** 🟢 **DIAGNOSED — root cause confirmed against the SDK type, not inferred**
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
3. Add `sendWithResults` — `signAction` already knows the broadcast outcome (`handlers.rs:8490`), so
   the data exists and is simply not surfaced.
4. Update `handlers.rs:6242` / `:6206` to prefer `tx` and fall back to `rawTx`.
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
