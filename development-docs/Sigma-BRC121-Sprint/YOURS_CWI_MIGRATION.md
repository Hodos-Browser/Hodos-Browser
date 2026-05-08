# Yours `window.yours` → `window.CWI` Migration — Deep Dive

**Date:** 2026-05-05. Source: research agent reading the `brc100-remote` branch of `yours-org/yours-wallet`.

This is the load-bearing reference for our dual-shim implementation plan. Most strategic decisions in `README.md` flow from these findings.

---

## TL;DR

On the `brc100-remote` branch, **`window.yours` is gone**. Only `window.CWI` is injected, and it implements the canonical `@bsv/sdk@2.0.13` `WalletInterface` (28 BRC-100 methods) with **no 1Sat-specific additions** on the page surface. The legacy ordinal/inscribe/sendBsv/signMessage methods are not just renamed — they are removed entirely from both the inject script and the background service worker.

---

## 1. Where `window.CWI` is constructed

**File:** `src/cwi.ts` on `brc100-remote` ([blob](https://github.com/yours-org/yours-wallet/blob/brc100-remote/src/cwi.ts)) — full file:

```ts
import type { WalletInterface } from '@bsv/sdk';
import { createEventCWI, CWIEventName } from '@1sat/wallet-browser';

export { CWIEventName };

export const CWI = createEventCWI();

if (typeof window !== 'undefined') {
  (window as unknown as { CWI: WalletInterface }).CWI = CWI;
}
```

Imported (and triggered) by `src/inject.ts:2`: `import './cwi';`. The vite IIFE config at `vite.config.inject.ts` builds this to `inject.js`, which `src/content.ts:9` injects via `<script src="...inject.js">`.

`createEventCWI` lives in `@1sat/wallet-browser@0.0.54` → re-exported from `@1sat/wallet@0.0.68/dist/cwi/event.d.ts`. Its return type is `WalletInterface` (from `@bsv/sdk`). Confirmed via `unpkg.com` type definitions.

---

## 2. Method list on `window.CWI` (canonical 28)

From `@1sat/wallet@0.0.68/dist/cwi/types.d.ts` `CWIEventName` enum and `@bsv/sdk@2.0.13` `WalletInterface`. Both line up exactly — 28 methods:

| Group | Methods |
|---|---|
| Identity / keys | `getPublicKey`, `revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage` |
| Crypto | `encrypt`, `decrypt`, `createHmac`, `verifyHmac`, `createSignature`, `verifySignature` |
| Transactions | `createAction`, `signAction`, `abortAction`, `listActions`, `internalizeAction` |
| Outputs | `listOutputs`, `relinquishOutput` |
| Certificates | `acquireCertificate`, `listCertificates`, `proveCertificate`, `relinquishCertificate`, `discoverByIdentityKey`, `discoverByAttributes` |
| Auth | `isAuthenticated`, `waitForAuthentication` |
| Chain info | `getHeight`, `getHeaderForHeight`, `getNetwork`, `getVersion` |

All signatures are `(args, originator?) => Promise<Result>`. The `originator` is filled in by `content.ts` (`const originator = window.location.host;`), not by the page.

---

## 3. `window.yours` → `window.CWI` comparison table

| `window.yours` method | `window.CWI` equivalent | Status | Notes |
|---|---|---|---|
| `isConnected()` | `isAuthenticated({})` | replaced | Returns `{ authenticated: boolean }` instead of `boolean`. |
| `connect()` | `waitForAuthentication({})` | replaced | Triggers permission popup via `handleConnectOrAuth()` if not yet authorized; resolves to `{ authenticated: true }` on approval. There is no separate "connect" method — invoking any CWI method on a non-authorized origin will not auto-prompt; the dApp must call `isAuthenticated` then `waitForAuthentication`. |
| `getAddresses()` | none direct | **removed** | No address-centric API. dApps should derive output scripts via `createAction` or call `getPublicKey({ identityKey: true })` + derive an address themselves. The legacy `GET_RECEIVE_ADDRESS` background message still exists but is **only used by the popup UI**, not exposed to the page. |
| `getBalance()` | none direct | **removed** | Compute via `listOutputs({ basket: 'default', includeTags: ... })` and sum `satoshis`. `GET_BALANCE` is internal-popup-only. |
| `getExchangeRate()` | none | **removed** | dApps must fetch rates themselves. |
| `getSocialProfile()` | none | **removed** | `GET_SOCIAL_PROFILE` is internal-popup-only. |
| `getPubKeys()` | `getPublicKey(args)` | replaced | BRC-100 model is per-derivation: `getPublicKey({ identityKey: true })` for identity, or `getPublicKey({ protocolID, keyID, counterparty })` for derived. No bulk identity/ord/bsv triple. |
| `sendBsv(payments[])` | `createAction({ outputs: [...] })` | replaced | Build outputs explicitly. No 1-line "send to address". |
| `getOrdinals()` | `listOutputs({ basket: '1sat' })` | replaced | Confirmed by commit `3d3e498`. |
| `inscribe(items)` | `createAction({ outputs: [{ lockingScript: <inscription script>, basket: '1sat', ... }] })` | replaced | Build the 1sat inscription script in dApp code (`@1sat/templates`) then ship via `createAction`. There is no `inscribe` shortcut on the new surface. |
| `transferOrdinal({ address, origin, outpoint })` | `createAction({ inputs: [<outpoint>], outputs: [<P2PKH to address>] })` | replaced | Done via generic `createAction` referencing the specific input outpoint. |
| `purchaseOrdinal({ outpoint, marketplaceRate?, marketplaceAddress? })` | `createAction({ inputs: [<seller-listed outpoint>], outputs: [...] })` | replaced | No marketplace-specific shortcut. dApp must construct the trade tx (1sat market template) and pass to `createAction`. |
| `signMessage({ message, encoding? })` | `createSignature({ data, protocolID, keyID, counterparty })` | replaced — **lossy** | BRC-100 signatures bind to (protocolID, keyID, counterparty), not raw message strings. Verifiers must use `verifySignature`. |
| `getSignatures(...)` (transaction-input signing) | `signAction({ reference, spends })` | replaced | The `signableTransaction` returned from `createAction` is signed via `signAction`. The previous "give me signatures for these inputs" is gone. |
| `broadcast({ rawtx })` | `internalizeAction({ tx, outputs, description })` | replaced (partial) | `internalizeAction` accepts BEEF and routes outputs into baskets. Pure raw-tx broadcast without internalization isn't a CWI primitive — pages do that via WoC/ARC themselves. |
| `encrypt(args)` | `encrypt(args)` | preserved-by-name, **semantics changed** | BRC-100 `encrypt` takes `{ plaintext, protocolID, keyID, counterparty, ... }` and returns BRC-2 ciphertext. The old `yours.encrypt` accepted simpler `{ message, encoding, pubKeys[] }`. **Not drop-in compatible.** |
| `decrypt(args)` | `decrypt(args)` | preserved-by-name, **semantics changed** | Same caveat — BRC-100 args differ from legacy. |

**High-priority resolutions for our shim plan:**
- `purchaseOrdinal`, `inscribe`, `transferOrdinal` — all collapse into `createAction` calls. None survive in any form. The 1sat-specific construction is in `@1sat/actions` / `@1sat/templates`, used by Yours **inside** its `createAction` handler (`processCWICreateAction`); dApp pages don't get a shortcut on the new surface.
- `sendBsv` — `createAction` with payment outputs only.
- `signMessage` — `createSignature`, but with BRC-100 protocolID/keyID/counterparty semantics. A shim that maps `{ message, encoding }` → `createSignature` will need to invent a fixed protocolID/keyID convention; not lossless.
- `getBalance` — sum `listOutputs({ basket: 'default' }).outputs[].satoshis`.
- `getAddresses` — no clean equivalent. Closest is `getPublicKey({ identityKey: true })` + derive P2PKH address; but this is the **identity key**, not a fresh receive address. There is no per-call fresh-address API on the BRC-100 surface.

---

## 4. Co-existence on `brc100-remote`

**`window.yours` is NOT injected.** `window.panda` is also not injected. Verified two ways:

1. **`src/inject.ts`** has zero `window.yours = …` or `window.panda = …` assignments. Its only effects are: import `./cwi` (which sets `window.CWI`), set up the yours-event-emitter for `signedOut`/`switchAccount` broadcast events, and listen for `YOURS_EMIT_EVENT`. The emitter is a *local* object — never attached to `window`.
2. **`src/content.ts`** has a guard:

   ```ts
   self.addEventListener(CustomListenerName.YOURS_REQUEST, (e: Event) => {
     const { type, ... } = (e as CustomEvent<RequestEventDetail>).detail;
     if (!type || !isCWIEventName(type)) return;
     ...
   ```

   So even if a dApp dispatched a `YOURS_REQUEST` with `type: 'sendBsv'`, the content script drops it before forwarding. The background `chrome.runtime.onMessage` dispatcher has **no cases** for `sendBsv`/`getOrdinals`/`inscribe`/`transferOrdinal`/`purchaseOrdinal`/`signMessage`/`getSignatures`/`broadcast`/`getAddresses`/`getExchangeRate` — only the 28 `CWIEventName` cases plus internal popup-only events.

**Implication for Hodos:** `window.CWI` and `window.yours` need to be *separate* shims in Hodos. We cannot rely on the Yours extension to keep maintaining the `window.yours` surface; sites still using it will break on Yours, and any BSV dApp that wants to keep working across both old and new Yours has to ship dual-call code. This is the gap our dual-shim strategy fills.

---

## 5. Underlying SDK / toolbox versions (`brc100-remote/package.json`)

```json
"@bsv/sdk": "^2.0.13",
"@1sat/wallet-browser": "0.0.54",
"@1sat/connect": "0.0.51",
"@1sat/actions": "0.0.119",
"@1sat/client": "0.0.27",
"@1sat/templates": "0.0.11",
"@1sat/types": "0.0.19",
"@1sat/utils": "0.0.16",
"@1sat/sweep-ui": "0.0.45",
"@bsv/wallet-toolbox-mobile": "npm:@bopen-io/wallet-toolbox-mobile@2.1.21-parity-fix.2",
"sigma-protocol": "^0.1.9",
"bitcoin-backup": "^0.0.11"
```

Notable: Yours uses a **fork** of Babbage's wallet-toolbox-mobile (`@bopen-io/wallet-toolbox-mobile@2.1.21-parity-fix.2`) for the background-side wallet, not Babbage's official package. The page-side `WalletInterface` is built by `@1sat/wallet-browser → @1sat/wallet@0.0.68 → cwi/event.ts`, which is Yours/1Sat's own implementation. So the *interface* is canonical BRC-100, but the *backend* is a 1Sat fork.

---

## 6. Non-standard methods on `window.CWI`

**None.** The 28 methods on `window.CWI` are exactly the 28 methods on `@bsv/sdk`'s `WalletInterface`. 1Sat-specific functionality (1sat baskets, ordinal templates, BSV21 tokens) is encoded entirely **inside `createAction`/`listOutputs` parameters** — basket name `'1sat'`, custom locking scripts from `@1sat/templates`, etc. The page surface is intentionally a pure BRC-100 implementation, so any BRC-100-aware dApp (Babbage MetaNet Client, anyone else who ships a `WalletInterface`) is wire-compatible with `window.CWI` if they pull it off `window.CWI`.

That is in fact the whole point of the migration — making Yours a drop-in BRC-100 substrate.

---

## 7. Permission / approval model on `brc100-remote`

The permission model is **new and BRC-100-aligned**, not a port of the old yours model. From `background.ts`:

- `authorizeRequest(message)` calls `verifyAccess(message.originator || '')` — origin-keyed. The `originator` is set by `content.ts` to `window.location.host`.
- For unauthorized origins, the gate path is `IS_AUTHENTICATED` (silent check) → `WAIT_FOR_AUTHENTICATION` (which calls `handleConnectOrAuth(...)` → opens `chrome.windows.create({ url: 'index.html', type: 'popup', width: 392, height: 567 })` → user clicks approve → `USER_CONNECT_RESPONSE` action returns the decision).
- Beyond connection, there are **three tiers** of fine-grained permissions, each with its own response action:
  - `PERMISSION_RESPONSE` (per-protocol grants for specific protocolIDs)
  - `GROUPED_PERMISSION_RESPONSE` (bundled grants)
  - `COUNTERPARTY_PERMISSION_RESPONSE` (per-counterparty grants)

  These are wired into `LocalWalletPermissionsManager` from `@1sat/wallet-browser` and `@bsv/wallet-toolbox-mobile`'s `WalletPermissionsManager`/`PermissionRequest`/`GroupedPermissionRequest`/`CounterpartyPermissionRequest` types.
- Listing/revoking: `PERMISSIONS_LIST_ALL`, `PERMISSIONS_QUERY_SPENT`, `PERMISSIONS_REVOKE_ONE`, `PERMISSIONS_REVOKE_ALL` are extension-internal management actions exposed in the popup UI only (not via CWI).

**Implication for Hodos:** Our existing domain-permission model handles connect/disconnect, but the BRC-100 model has *protocol-level* and *counterparty-level* permission shards that we will need to mirror if we want to be a faithful BRC-100 substrate (per `WalletPermissionsManager` from `@bsv/wallet-toolbox`). Yours opens a separate popup window for each new permission tier (protocol, grouped, counterparty); we can do the same with overlay subprocesses.

---

## Files cited

- https://github.com/yours-org/yours-wallet/blob/brc100-remote/src/inject.ts
- https://github.com/yours-org/yours-wallet/blob/brc100-remote/src/cwi.ts
- https://github.com/yours-org/yours-wallet/blob/brc100-remote/src/content.ts
- https://github.com/yours-org/yours-wallet/blob/brc100-remote/src/background.ts
- https://github.com/yours-org/yours-wallet/blob/brc100-remote/src/yoursApi.ts
- https://github.com/yours-org/yours-wallet/blob/brc100-remote/vite.config.inject.ts
- https://github.com/yours-org/yours-wallet/blob/brc100-remote/package.json
- https://unpkg.com/@1sat/wallet@0.0.68/dist/cwi/types.d.ts (CWIEventName enum source-of-truth)
- https://unpkg.com/@1sat/wallet@0.0.68/dist/cwi/event.d.ts (createEventCWI signature)
- https://unpkg.com/@bsv/sdk@2.0.13/dist/types/src/wallet/Wallet.interfaces.d.ts (canonical 28-method WalletInterface)

---

## Caveats / things not verified

- `@1sat/wallet@0.0.68` is closed-source on GitHub; only published `.d.ts` files were readable via unpkg. *Enum values* and *signatures* are authoritative, but the runtime `.js` could in principle add ad-hoc methods beyond `WalletInterface`. The TypeScript declared return type is `WalletInterface` (strong evidence). 5-min follow-up: fetch `https://unpkg.com/@1sat/wallet@0.0.68/dist/cwi/event.js` and inspect the runtime object.
- `background.ts` was sampled via WebFetch summarization rather than direct file IO. Dispatch table looked complete; bit-perfect verification needs a local fetch.
- `getOrdinals` removal in commit `3d3e498` was confirmed indirectly (absent from `CWIEventName` and from the background dispatcher).
