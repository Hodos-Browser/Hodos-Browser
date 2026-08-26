# 🚨 A locked wallet cannot be unlocked — the panel stops asking, so the PIN screen is unreachable

**Found:** 2026-08-26, macOS, during the beta.3 Mac verification round.
**Severity:** high — the entire wallet surface is dead for any user in this state, with no recovery.
**Platforms:** **cross-platform.** Windows masks it (DPAPI auto-unlocks before the panel ever sees
a locked wallet), which is why it has never been reported.
**Status:** FIXED this round. Owner-directed, verified end-to-end on a dev build.

---

## What the user sees

A site asks to connect. Hodos shows the consent modal. The user approves. The site then silently
fails to continue — no error, no prompt, nothing. Meanwhile the wallet panel shows a normal
**$0.00 balance**, as if everything were fine.

Underneath, every wallet call is returning `Wallet is locked. Enter PIN to unlock.` On this machine
that had happened **61 consecutive times** across 11 days of logs, and no site had ever connected.

## The chain, in the order it actually breaks

### 1. The credential-store write can fail silently at wallet creation

`wallet_repo.rs` called `dpapi_encrypt` once, and on failure logged at **info** level and wrote
`NULL` to `wallets.mnemonic_dpapi` — creating the wallet anyway:

```rust
Err(e) => {
    info!("   ⚠️  DPAPI encryption unavailable: {} — wallet will require PIN on startup");
    None            // <- wallet created regardless
}
```

### 2. That state can never repair itself

Auto-unlock needs the stored key. The backfill that would *create* the key
(`main.rs`) only runs `if db.is_unlocked()`. A wallet with no key is never unlocked at startup, so
the repair only runs in the one case where it is not needed. Deadlock.

### 3. ⭐ And the PIN screen — which does exist, and does work — is unreachable

This is the actual reason recovery was impossible. `WalletPanelPage.tsx`:

```js
// If localStorage says wallet exists, trust it and skip the fetch
if (cachedExists) return;
```

Once the panel had ever seen a wallet exist, it **stopped requesting `/wallet/status` entirely**,
so it never learned `locked: true` and rendered the balance view. `renderLocked()` and the whole
unlock flow were fully built, correct, and dead code.

**MEASURED:** `/wallet/status` returned `{"exists":true,"locked":true}` three times (HTTP 200, no
gating) while the panel displayed a $0.00 balance.

## Why macOS surfaced it and Windows did not

Nothing here is macOS-specific. On Windows DPAPI auto-unlock succeeds, so the wallet is never locked
and the branch never executes. This machine's **dev** wallet (created 2026-06-25) has a NULL key
column; the **production** wallet on the same machine (created 2026-04-15, written by the
Developer-ID-signed app) has `mnemonic_dpapi = KEYCHAIN`, a live Keychain entry, and works normally.

Best explanation for the original write failure — **well-supported, not proven**: before `deff765`
(2026-07-14) dev and production shared the Keychain service name `"HodosBrowser"`. The production
entry already existed, created by the signed app, and the ad-hoc-signed dev binary was refused access
to an item owned by a different application. That name collision is already fixed. Confirming it
would require writing to the production service, which was not done.

⛔ **Ad-hoc signing was REFUTED as the cause.** A probe binary signed identically to the dev wallet
(`Signature=adhoc`, `TeamIdentifier=not set`) wrote, read back and deleted a Keychain entry with no
error. Dev builds can use the Keychain fine — which is also what makes retry a sensible fix rather
than an infinite loop.

## The fix

| Where | Change |
|---|---|
| `crypto/dpapi.rs` | New `dpapi_encrypt_retrying()` — 5 attempts, 50/100/200/400 ms backoff. Reports failure; never swallows it. |
| `database/wallet_repo.rs` (both paths) | The credential-store write is **mandatory**. On failure the wallet is **not created** and the error is returned, instead of creating one that is broken forever. |
| `database/connection.rs :: store_dpapi_blob` | Was `Ok(())` on failure — so a user could enter their PIN, believe it fixed, and find it locked again next launch. Now returns `Err`. |
| `main.rs`, `handlers.rs` | The two repair call sites logged nothing (`let _ =`). They now log the failure. |
| `pages/WalletPanelPage.tsx` | ⭐ Always fetch status. `cachedExists` still seeds the first paint, so there is no loading flash — it just no longer suppresses the truth. |

## Evidence — GREEN with its RED half

**RED (before), measured:** `mnemonic_dpapi` = NULL, no `HodosBrowserDev` Keychain entry,
`{"exists":true,"locked":true}`, panel showing a $0.00 balance, `getPublicKey` failing, 61 times.

**GREEN (after), measured on the same wallet:**

```
panel:  🔒 Wallet Locked — Auto-unlock was unavailable. Enter your PIN to unlock.   (4 PIN inputs)
log:    🔓 /wallet/unlock called
        ✅ Wallet unlocked successfully
        ✅ Wallet key stored in OS credential store (wallet 1)
db:     mnemonic_dpapi NULL -> KEYCHAIN
keychain: HodosBrowserDev  no entry -> ENTRY EXISTS
status: locked:true -> locked:false
```

**And after a full wallet restart** — the test that matters:

```
🔓 DPAPI auto-unlock succeeded          (no PIN)
{"exists":true,"locked":false}
POST /getPublicKey -> {"publicKey":"0302cabd012b3dd3f277851aca311e055ccc4a6657e0461208e52a691eb6c9aa49"}
```

**Owner-verified:** bitgenius.net and teragun.com both connected successfully afterwards, having
both failed before.

**Production wallet untouched throughout** — `HodosBrowser` Keychain entry and `dpapi = KEYCHAIN`
verified intact at every step.

## Retroactive repair for existing users

Self-healing, one prompt, no data loss and no recovery phrase. An affected user updates, opens the
wallet panel, sees the lock screen for the first time, enters their PIN once — the key is saved and
it auto-unlocks from then on. Users whose wallet is healthy auto-unlock at startup and never reach
that branch, so **they see nothing**.

## ⛔ Also fixed: a test that destroyed the production wallet's auto-unlock

`crypto::dpapi::tests::test_keychain_round_trip` called `dpapi_encrypt` with `HODOS_DEV` unset, so
`keychain_service()` resolved to **`"HodosBrowser"`** — the production service. It deleted the user's
real Keychain entry and replaced it with the hardcoded test mnemonic
`"abandon abandon … about"`, with no cleanup. A developer running `cargo test` on macOS would have
left their production wallet auto-unlocking with a **foreign mnemonic**.

Now: opt-in via `HODOS_KEYCHAIN_TEST=1`, asserts the dev service name before touching anything, and
deletes its entry afterwards. Verified: `cargo test dpapi` leaves the production entry intact.
