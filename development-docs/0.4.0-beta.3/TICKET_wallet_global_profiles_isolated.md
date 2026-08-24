# Wallet approvals are global; browser profiles are isolated. The two conflict.

**Opened 2026-08-24** from Phase 0.9 testing.
**Status:** 🔵 ACCEPTED FOR NOW — precedent recorded, no fix planned.

## The conflict

One `wallet.db` serves **every** browser profile
(`%APPDATA%/HodosBrowser{Dev}/wallet/wallet.db`). So `domain_permissions` — which sites may talk to
the wallet — is **global**.

Browser profiles isolate: Chromium content settings (including the loopback permission), history,
cookies, and our `site_permissions.db`.

Consequences, all observed on 2026-08-24:

- A site approved for the wallet in Profile A is approved in Profile B, with no prompt.
- Revoking wallet access in one profile revokes it everywhere.
- The **loopback** grant is per-profile, so the same site can hold wallet access in both profiles
  while being asked about loopback separately in each.

MEASURED: `bitgenius.net` held `loopback_network → ALLOW` in Profile_1 and **no row** in Profile_2,
while the wallet approved it for both.

## Why it is not being fixed

There is no reconciliation short of **giving each profile its own wallet** — a large design change
with key-management, backup, recovery and UX consequences well beyond any permissions work.

**Owner decision 2026-08-24:** accept it and set the precedent. The wallet is the higher-order
permission; loopback is subordinate and per-profile. Revisit only if per-profile wallets are ever
built.

## Practical consequence for testers

⛔ **A fresh browser profile is NOT a fresh test.** A site already approved in the wallet connects
instantly in a brand-new profile with no modal. This invalidated several Phase 0.9 test runs before
it was understood — a "clean profile" test showed no connect modal, and the wrong conclusion was
drawn about the code.

Use the reset tool and then assert:

    python phase-0.9-chromium-prompt-branding/reset_test_state.py clear-wallet-domain <domain>
    python phase-0.9-chromium-prompt-branding/reset_test_state.py verify --domain <domain>

`verify` exits non-zero on mismatch. Trust that, not the freshness of a profile.
