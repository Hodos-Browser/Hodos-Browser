# 🔑 A connected site can fetch a derived public key with no prompt, and two sites can be handed the same one

**Found:** 2026-09-02 by code reading (`FINDING-silent-derived-key-tracking.md` in the Video 3 folder); re-read and compared against `wallet-toolbox` on 2026-09-21.
**Status:** ⬜ UNASSIGNED · **Track:** unassigned · **Filed by:** Claude, at the owner's request

> ⚠️ **Method note.** **Code reading**, both sides: `rust-wallet/src/handlers.rs :: get_public_key` and
> `cef-native/src/core/HttpRequestInterceptor.cpp :: extractProtocolScope`. ⛔ **Nothing was executed.**
> Not verified: that two live origins really receive the same key. That is the RED below.

---

## What happens

`getPublicKey` with `protocolID` + `keyID` derives a BRC-42 child public key and returns it.

- **No prompt of its own.** The only gate inside `get_public_key` is `dispatch_privacy_perimeter`, and
  it fires only for the identity-key path. The scoped-grant prompt covers `/createSignature`,
  `/createHmac`, `/encrypt` and `/decrypt` — in Rust and in `extractProtocolScope` — and not
  `/getPublicKey`. The site does have to be connected (`domain_trust_mw`).
- **The app picks every derivation input.** With `counterparty: 'self'` or `'anyone'`, two sites that
  send the same `protocolID` and `keyID` receive the **same** key. A site can also send another
  site's values.
- **It cannot be cleared.** The key is recomputed from the master key, not stored.

## Why it matters

A connected site gets a permanent identifier without the user clicking anything, and two cooperating
sites can link one user. ⚠️ Nobody is known to do this today. It is the gap between what Video 3 says
about private accounts and what the wallet enforces.

## What already protects us

- The site must be connected before any call is answered.
- The identity key always prompts, and auto-approve cannot silence it.
- `counterparty: <the site's own key>` is site-scoped by the math. An honest site is fine today.
- `derived_key_cache` already records `forSelf` derivations (`derived_pubkey`, `invoice`,
  `counterparty_pubkey`) — but **not which site asked.**

## Prior art

`wallet-toolbox :: WalletPermissionsManager.getPublicKey` runs
`ensureProtocolPermission({ usageType: 'publicKey' })` for level 1 and level 2 protocols and skips it
at level 0 (config flag `seekPermissionsForPublicKeyRevelation`). ⇒ the reference wallet prompts here.

## Proposed fix — three parts, smallest first

1. **Give derived `getPublicKey` the same scoped grant `createSignature` has** — same check, same
   saved grant, keyed on site + protocol. 👤 Owner agreed 2026-09-21. The login flow gains no extra
   prompt: today the prompt fires at `createSignature`; with this it fires one call earlier, at
   `getPublicKey`, and the signature call is then silent because the grant exists. A site that asks
   for a key and never signs loses its free pass.
   - ⚠️ **Level 0 stays open under this part alone** — the engine returns `SilentProtocolLevelZero`,
     matching `wallet-toolbox`. A tracker can simply declare level 0.
   - 👤 **Owner's idea for that, 2026-09-21:** silence a level-0 request **only if the site already
     holds a grant at a higher level**; otherwise prompt. ⚠️ Weigh it against
     `TICKET_level_0_protocol_prompts_after_connect.md` (beta.3), which removed the level-0 prompt
     because it was annoying. Part 3 closes the cross-site case with no prompt at all, so decide this
     after part 3, not before.
2. 👤 **Owner's idea, 2026-09-21: log every derived-key request with the requesting site, and check each
   new one against what other sites have already been given.** Same `(invoice, counterparty)` from two
   different sites ⇒ warn or refuse. Needs a `requesting_domain` column. ⛔ **Schema change — owner
   approval.**
3. **The wallet adds the requesting site's address to the keyID itself**, from `X-Requesting-Domain`,
   never from the page. Closes the hole structurally. ⚠️ Changes derived keys, so it belongs with the
   BRC draft: `Standards/BRCs/drafts/originator-scoped-authentication-keys/`.

**Deliberately out of scope:** changing the identity-key prompt; anything in `well_known_auth` (own ticket).

## Test and negative control

**RED first:** approve two scratch origins. From each, call `getPublicKey({protocolID:[2,'login'],
keyID:'1', counterparty:'self', forSelf:true})`. Expect the **same** key with **no prompt**. If the keys
differ or a prompt fires, this ticket is wrong and closes.
**GREEN after part 1:** the first call from a site with no grant prompts. **Control:** a site holding the
grant gets no prompt, and the identity-key prompt still fires on its own path.

## Links

- `Marston Enterprises/Hodos/marketing/Videos/3 - Account Creation and Authentication/HOW_THE_KEYS_WORK.md`
- `…/FINDING-silent-derived-key-tracking.md`
