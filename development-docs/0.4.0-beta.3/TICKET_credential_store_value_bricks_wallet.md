# 🚨 A bad OS-credential-store value silently bricks the wallet — and how to repair one in the field

**Found:** 2026-09-08, on the owner's **installed production** macOS wallet (0.4.0-beta.2).
**Severity:** high — the whole wallet surface is dead, with no recovery path from the UI.
**Platforms:** **cross-platform.** Windows masks it exactly as it masked
`TICKET_locked_wallet_is_unrecoverable.md`: DPAPI normally returns the right value, so the
branch never runs there.
**Status:** code FIXED (`7fe8a7e`, `c078423`, runtime-verified). Owner's wallet repaired in
place 2026-09-08. ⛔ **Root cause of the bad value itself is still UNKNOWN — see §4.**

---

## 1. Symptom

The macOS Keychain slot (`service=HodosBrowser`, `account=wallet-mnemonic`) held a **2-word**
value. Auto-unlock read it, cached it, and reported success. Everything after that was silent:

```
ERROR Failed to get master public key:
      Invalid mnemonic: mnemonic has an invalid word count: 2. Word count must be 12, 15, 18, 21, or 24
```

61+ occurrences. No identity key, no backups (`TaskBackup` failing every 30 s), no site could
connect. The wallet panel meanwhile showed a normal balance, because `is_unlocked()` was true.

**The wallet was unrecoverable while holding everything needed to recover itself** — the correct
PIN-encrypted phrase and salt were in the database the whole time.

## 2. Why it could not recover

Three gates, each individually reasonable:

| # | Gate | Effect |
|---|---|---|
| 1 | `try_dpapi_unlock` cached the credential-store value **without validating** it | wallet believes it is unlocked |
| 2 | `WalletPanelPage` early-returned on `cachedExists`, so it never re-read `/wallet/status` | panel shows the balance view, never the PIN screen |
| 3 | the unlock-time repair was gated on `mnemonic_dpapi.is_none()` — a column that only ever holds the sentinel `"KEYCHAIN"` | even a successful PIN unlock would **not** rewrite the bad value |

Gate 2 was fixed 2026-08-26. Gates 1 and 3 are fixed here. ⭐ **Gate 3 was found by the live
repair test, not by reading the code** — with it in place the wallet unlocked but the Keychain
entry's `mdat` never moved, which would have meant a PIN prompt on *every* launch, forever.
That is worse than the bug being fixed, and it looked correct on the page.

## 3. The fix (in code, from `c078423`)

- `crypto/mnemonic_guard.rs` — `is_valid_mnemonic()` parses with the **same**
  `Mnemonic::parse_in(Language::English, ..)` call the consumer makes, so passing validation
  guarantees the downstream parse cannot fail. A word-count check is **not** sufficient (there
  is a test for a right-length wrong-checksum phrase). `describe_shape()` lets a rejection be
  logged by shape — ⛔ never by content, since a bad value may still be a real secret.
- All **five** `cached_mnemonic = Some(..)` sites route through `Self::validated_mnemonic`.
- Rejection ⇒ `Ok(false)` ⇒ wallet stays **locked** ⇒ PIN screen ⇒ self-heals.
- The unlock-time credential-store write is now **unconditional**.

## 4. ⛔ What is NOT established

**What wrote a non-mnemonic value into that slot is unknown.** Do not assert a cause.

Keychain forensics facts, each measured, that constrain any future theory:

- **A plain `security find-generic-password` READ bumps `mdat`.** So `mdat` proves *access*,
  not modification. This invalidated the first plausible-looking inference drawn in this
  investigation; do not repeat it.
- `dpapi_encrypt` (delete + set) ⇒ **`cdat == mdat`** (a brand-new item). Proven by the
  `HodosBrowserDev` entry written 2026-08-26, whose `cdat` and `mdat` are identical and match
  its log line to the second.
- `security add-generic-password -U` (in-place update) ⇒ **`cdat` preserved, `mdat` bumped**.
- The production item had `cdat = 20260714151601Z` — the day of `deff765`, the dev/prod service
  name split — with a later `mdat`. So it was touched in place, **not** rewritten by our normal
  write path.
- ⛔ `security find-generic-password -w` **blocks on an auth prompt**. Never run it unattended.

## 5. Field repair — an already-broken wallet on a build WITHOUT the fix

Works on 0.4.0-beta.2. It makes the shipped binary repair itself using its own code; no
patched build required. **Verified end to end on the owner's production wallet 2026-09-08.**

> Prerequisite: the user knows their PIN. The database must still hold `mnemonic` and
> `pin_salt` — check first; if either is missing this procedure does not apply.

1. **Back up** `wallet.db`, `wallet.db-wal`, `wallet.db-shm`.
2. **Quit the app completely.** Not optional: while running it has the bad value cached, so
   `is_unlocked()` is true and `/wallet/unlock` refuses with *"Wallet is already unlocked"*.
   ⚠️ AppleScript `quit` was ignored in practice — quit from the UI and confirm with
   `pgrep -f '/Applications/HodosBrowser.app'` returning nothing.
3. **Clear the stale sentinel** so the shipped `is_none()` gate opens:
   `UPDATE wallets SET mnemonic_dpapi = NULL WHERE id = 1;`
   Then `PRAGMA integrity_check;`. Reverse with `SET mnemonic_dpapi = 'KEYCHAIN'`.
4. **Relaunch.** Confirm by API, not by the panel — the panel lies on pre-fix builds:
   `curl -s http://127.0.0.1:<port>/wallet/status` ⇒ `{"exists":true,"locked":true}`, and
   `getPublicKey` should now say *"Wallet is locked"* rather than *"Invalid mnemonic"*.
5. **The user unlocks** — have them type their own PIN so it never passes through a third party:
   `curl -s -X POST http://127.0.0.1:<port>/wallet/unlock -H 'Content-Type: application/json' -d '{"pin":"####"}'`
   (No lockout exists, so a wrong attempt is safe to retry.)
6. **Verify the repair, do not assume it:** the Keychain entry's `cdat` **and** `mdat` must
   BOTH be new — that proves a delete + re-add rather than a skipped write.
7. **Restart the app** and confirm auto-unlock with no PIN, plus a real `getPublicKey`.

### Measured result on the owner's wallet

```
before   dpapi=KEYCHAIN  keychain cdat=20260714151601Z  "Invalid mnemonic: word count: 2"
step 3   dpapi=NULL      integrity_check: ok            addresses=20 outputs=76 txs=32 (unchanged)
step 4   {"exists":true,"locked":true}                  "Wallet is locked. Enter PIN to unlock."
step 5   {"success":true}
step 6   keychain cdat=mdat=20260908215930Z             ← deleted + recreated
after    {"exists":true,"locked":false}                 getPublicKey -> 037b557e…5e075
RESTART  {"exists":true,"locked":false}, same key, NO PIN ENTERED
```

⛔ Ports: **31301 production / 31401 dev**. This procedure deliberately touches production and
is the one sanctioned exception to the standing "do not touch 31301" rule — run it only as a
deliberate, backed-up repair with the owner present.
