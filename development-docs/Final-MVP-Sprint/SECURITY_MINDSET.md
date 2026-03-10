# Security Mindset — Final MVP Sprint

**Purpose**: Security priorities, known posture, and what everyone should watch for. We are not security auditors — we'll hire a professional firm for certification later. But we build with a security-first philosophy: everyone should be thinking about security and raising concerns.

---

## Our Security Philosophy

1. **Private keys never in JavaScript.** All signing, derivation, and key operations happen in Rust. The frontend only sees public keys and transaction metadata.
2. **Defense in depth.** Multiple layers protect the wallet: process isolation, DPAPI/Keychain encryption, PIN encryption, and in-memory-only mnemonic caching.
3. **Assume breach.** If someone gets the DB file, what can they access? The mnemonic is double-encrypted (PIN + DPAPI). Everything else in the wallet DB is low-sensitivity (public keys, transaction records, domain permissions).
4. **Trust Chromium for browser security.** CEF inherits Chromium's security model — sandboxing, process isolation, DPAPI credential storage. We don't reinvent these.
5. **Question everything.** If something feels wrong, raise it. A 5-minute conversation about a potential issue is worth more than a week fixing a breach.

---

## Current Security Posture

### What's Strong

| Area | Status | Details |
|------|--------|---------|
| **Wallet mnemonic** | Encrypted at rest | Dual encryption: AES-256-GCM (PIN, 600K PBKDF2 iterations) + DPAPI (Windows) / Keychain (macOS) |
| **Private keys** | Never stored | Derived on-demand from cached mnemonic via BRC-42/BIP32, held in memory only |
| **Process isolation** | Strong | Wallet runs as separate Rust process on localhost:3301. Crash or compromise doesn't take down the browser. |
| **Saved passwords** | Chromium DPAPI | CEF uses Chromium's built-in password manager. Passwords encrypted with DPAPI on Windows. Per-profile isolation. Same mechanism as Chrome/Edge. |
| **Cookie isolation** | Per-profile | Each browser profile has its own cookie store, history, and credentials |
| **BRC-100 auth** | Secure | ECDSA challenge-response, per-request nonces, no replay |
| **Domain permissions** | DB-backed | Spending limits, rate limits, per-domain approval stored in SQLite |
| **Overlay isolation** | V8 separation | Each overlay (wallet, settings, auth) runs in an isolated V8 context |
| **Input sanitization** | JS injection fixed | CR-1.1 fixed domain/body string escaping in `ExecuteJavaScript()` |

### What's Acceptable (Known Limitations)

| Area | Status | Risk | Notes |
|------|--------|------|-------|
| **Autofill form data** | Unencrypted | Low | Non-password form fields stored in plaintext `Web Data` SQLite. This is Chromium's default — Chrome does the same. |
| **Browser history** | Unencrypted | Low | Custom `HodosHistory` SQLite DB. Standard for all browsers. |
| **Bookmarks** | Unencrypted | Low | `Bookmarks.db` SQLite. Standard for all browsers. |
| **Wallet DB (non-mnemonic)** | Unencrypted | Low | Public keys, transaction records, domain permissions. No secrets beyond the mnemonic. |
| **localhost communication** | HTTP (not HTTPS) | Low | Wallet (3301) and adblock (3302) listen on localhost only. Not exposed to network. An attacker with local access could sniff traffic, but they'd already have file access to the DB. |
| **4-digit PIN** | Short | Medium | 10,000 possibilities, but protected by 600K PBKDF2 iterations (~1s per attempt). Brute-forceable in ~2.8 hours on modern hardware. DPAPI is the primary protection; PIN is the fallback. |

### What Needs Attention

| Area | Priority | Details |
|------|----------|---------|
| **debug.log in release builds** | Medium | `debug.log` contains CEF/Chromium debug output. Should be excluded or minimized in release builds. Currently logs at `LOGSEVERITY_INFO`. |
| **identity.json** | Low | Unencrypted user identity data. Should eventually migrate to Rust-managed identity in the wallet DB. |
| **Session cookies disabled** | Verify | `persist_session_cookies` is commented out in `cef_browser_shell.cpp`. This means cookies don't survive browser restart by default. Good for privacy, but verify this doesn't break auth persistence (Mission 1.3 in the testing guide will catch this). |
| **No password management UI** | Low | Users can't view, export, or clear saved passwords through our UI. They'd need to delete `Login Data` manually. Consider adding "Clear saved passwords" to the Clear Browsing Data feature. |

---

## Security Watch List for All Devs

Things to look out for during development and testing:

### Code-Level

- [ ] **Never log secrets.** No mnemonic, private key, PIN, or DPAPI blob in logs. Search for these before committing.
- [ ] **Sanitize user input in JS injection.** Any string passed to `ExecuteJavaScript()` must be escaped. Use `escapeJsonForJs()` helper.
- [ ] **Drop DB locks before network calls.** Holding a `MutexGuard` across a network request creates deadlock risk AND timing side-channels.
- [ ] **Consume POST bodies in Actix handlers.** All POST handlers need `_body: web::Bytes` parameter even if unused. Without it, HTTP keep-alive corrupts responses (discovered the hard way).
- [ ] **No `--ignore-certificate-errors` in production.** This flag exists for dev only. Never ship it.
- [ ] **Check `target_os` conditionals.** macOS Keychain code must be behind `#[cfg(target_os = "macos")]`. Windows DPAPI behind `#[cfg(windows)]`. Don't accidentally expose platform-specific security APIs.

### UX-Level

- [ ] **Wallet overlay shouldn't close during sensitive operations.** Mnemonic display, PIN entry, and seed phrase backup must guard against `WM_ACTIVATE` close events. The `g_wallet_overlay_prevent_close` pattern exists for this.
- [ ] **Confirm before destructive actions.** Delete wallet requires 2-step confirmation. Any new destructive action should follow the same pattern.
- [ ] **No clickjacking on auth modals.** BRC-100 auth and domain permission prompts must be clearly visible and not overlappable by page content.

### Network-Level

- [ ] **Localhost binding only.** Wallet and adblock servers must bind to `127.0.0.1`, never `0.0.0.0`. Verify this after any changes to server startup.
- [ ] **CORS headers.** The Rust wallet sets CORS headers for localhost origins. Verify these aren't accidentally broadened.
- [ ] **DNT/GPC headers.** These are injected by the C++ layer. Verify they're present on outgoing requests (test at `httpbin.org/headers`).

---

## Future: Professional Security Audit Scope

When we hire a security firm, these are the areas they should focus on:

1. **Wallet cryptography** — mnemonic encryption, key derivation, ECDSA signing, BRC-42/BRC-2 implementations
2. **Process isolation** — can a malicious webpage reach the wallet process? Can it extract data from other overlays?
3. **IPC attack surface** — can a malicious page send crafted `cefMessage.send()` calls to trigger wallet operations?
4. **Credential storage** — DPAPI/Keychain implementation review, PIN brute-force resistance
5. **Network exposure** — localhost service hardening, CORS policy, header injection
6. **CEF configuration** — sandbox settings, V8 flags, JavaScript permissions
7. **Supply chain** — Rust crate audit (especially crypto dependencies), npm dependency audit
8. **macOS-specific** — Keychain ACLs, code signing, notarization, Gatekeeper bypass resistance

---

## Reference: Encryption Summary

| Data | Location | Encryption | Key Management |
|------|----------|------------|----------------|
| Mnemonic (PIN) | `wallet.db → wallets.mnemonic` | AES-256-GCM | PBKDF2 from user PIN (600K iterations) |
| Mnemonic (auto-unlock) | `wallet.db → wallets.mnemonic_dpapi` | Windows DPAPI / macOS Keychain | OS-managed, tied to user account |
| Saved passwords | `Login Data` (Chromium) | DPAPI (Windows) | OS-managed (Chromium standard) |
| Credit card numbers | `Web Data` (Chromium) | DPAPI (Windows) | OS-managed (Chromium standard) |
| Autofill data (names, addresses, card name/expiry) | `Web Data` (Chromium) | None | N/A — Chromium's default, same as Chrome |
| Cookies | `Network/Cookies` (Chromium) | Varies | CEF-managed |
| Backup file | `.hodos-wallet` | AES-256-GCM | PBKDF2 from backup password |
| BRC-2 messages | In transit | AES-256-GCM | BRC-42 derived shared secret |
| History | `HodosHistory` | None | N/A |
| Bookmarks | `Bookmarks.db` | None | N/A |
| Domain permissions | `wallet.db → domain_permissions` | None | N/A |

---

*Last updated: 2026-03-09*
