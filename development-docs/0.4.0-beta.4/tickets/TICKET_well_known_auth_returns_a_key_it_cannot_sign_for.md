# 🔐 The wallet's own `/.well-known/auth` answers with an identity key that does not match its signature

**Found:** 2026-09-21, reading `handlers.rs :: well_known_auth` against BRC-103 and the TS, Go and Python SDKs.
**Status:** ⬜ UNASSIGNED · **Track:** unassigned · **Filed by:** Claude, at the owner's request

> ⚠️ **Method note.** The handler is **code reading**. The mismatch is **measured on the math**: a Node
> script using the real `@bsv/sdk` reproduced the handler's two derivations with random keys.
> ⛔ **The wallet itself was not run**, and whether anything ever calls this handler is **unknown**.

---

## What happens

The handler (added 2026-01-05, commit `0be3ee2`) builds a BRC-103 `initialResponse`:

- `identityKey` = `derive_child_public_key(master_priv, app_key, "2-identity")`
- signature by `derive_child_private_key(master_priv, app_key, "2-auth message signature-<nonces>")`

Every SDK verifies by deriving the signer's child key **from the `identityKey` in the message**.

```
signer's child key                          0394e8be…
what a verifier derives from the key SENT   030f383a…   NO MATCH
what it derives from the master public key  0394e8be…   MATCH
the "app-scoped" key == the APP's own child key:  true
```

⇒ A standard verifier rejects the response. And the "app-scoped identity key" is a key the **app**
holds the private half of — the wallet cannot sign for it.

## Why it matters — probably very little today

A normal site login never reaches this handler. The browser sends `https://site/.well-known/auth` to
the site's own server (`HttpRequestInterceptor.cpp`, BRC-104 arm) and only re-points **loopback**
requests at the wallet. So it fires only when a page runs the handshake against the wallet itself.

## How exposed are we — answer this first

| If | Then |
|---|---|
| `Babbage auth request received` never appears in a week of `debug_output` logs | Dead code. Delete it or fix it at leisure |
| It appears, and the caller carries on | The caller is not verifying. Find out who |
| It appears, and the caller fails | A live bug someone has been working around |

## Proposed fix

Decide what this endpoint is for, then either delete it, send the master public key (what every SDK
does), or derive a scoped **private** key and sign with a child of *that*, so key and signature agree.
⚠️ `rust-wallet/src/CLAUDE.md` §"App-Scoped Identity Keys" describes this handler as a privacy feature
and needs correcting with it.

## Test and negative control

Drive the handshake from the TS SDK's `Peer` against the running wallet. **RED:** `Unable to verify
initial response signature`. **GREEN** after the fix. **Control:** corrupt one signature byte ⇒ RED again.
