# Auth — Browser Authentication Investigation

> **Status:** parked outline. Adopted as a phase in the next sprint (TBD). Until then this is a scope/intent document, not a workplan.
>
> **Scope contract:** This folder is about Hodos's behavior as a **browser** with respect to standard third-party authentication flows. It is NOT a wallet-side primitive effort. No BSM/BRC-77/`signMessage` Rust handlers are scoped here. If wallet-side message-signing primitives are needed later, they live in a separate phase.

---

## 1. Trigger

Google account sign-ins have been intermittently failing in Hodos. Suspected root cause: **fingerprint farbling** that's already on the roadmap to be moved/relocated. Concrete user-facing impact today: users cannot reliably sign into Gmail, YouTube, Google Drive, or any third-party site that uses Google as an OAuth/OIDC provider.

The fix is almost certainly mechanical (adjust farbling scope or auth-domain exemptions). The investigation matters because:

- We haven't tested whether the same issue affects other providers (Microsoft, Apple, Facebook, Sigma, …).
- We don't have a recorded baseline of which providers work today, so any farbling/cookie change risks silent regressions.
- The Sigma "Phase 2" thread is collapsing into this work — Sigma is one OAuth provider out of many we need to test, not a special case.

The current Sigma-BRC121-Sprint Phase 2 is **window-CWI shim** only. Anything labelled "Sigma Auth" or "BSM/BRC-77 signing primitives" lives here in `Auth/`, not in that sprint.

---

## 2. Goal

Audit Hodos's behavior as a browser for standard third-party authentication flows. Produce:

- A baseline matrix of `provider × browser variable → works | broken | works-with-quirk | not-tested`.
- A root-cause finding for the Google sign-in regression.
- A list of CEF/C++ code paths that need changes (most likely: fingerprint farbling scope, cookie safelist coverage, third-party-cookie passthrough).
- Regression tests for the matrix so a future farbling/cookie/storage change can't silently break Google again.

Out of scope: building any new wallet-side auth code, intercepting/substituting identities in any provider's flow, or shipping a Sigma-specific feature.

---

## 3. Providers to test

| Provider | Protocol | Notes |
|----------|----------|-------|
| **Google** | OAuth 2.0 + OIDC | Suspect #1 for current breakage. Test: direct Gmail login, YouTube watch-history sign-in, Google Drive, a third-party SaaS using "Sign in with Google" (e.g. Notion, Figma) |
| Microsoft | Entra ID OAuth + OIDC | Cross-tenant signin, Outlook.com, Office 365 web |
| GitHub | OAuth 2.0 | Direct github.com login, "Sign in with GitHub" on a downstream app |
| Apple Sign-In | OIDC, popup-restricted | Apple is strict about popup vs redirect; good canary for `window.open` policies |
| Facebook | OAuth 2.0 | Third-party-cookie heavy; canary for 3PC behavior |
| X / Twitter | OAuth 2.0 + PKCE | Standard PKCE flow; recently changed redirect rules |
| **Sigma Identity** | OAuth 2.1 + OIDC + bitcoin-auth iframe signer | See companion docs in this folder. The iframe-signer model is fragile under any third-party-storage tightening; the **finding from the 2026-05 Sigma research stands: we cannot substitute Hodos's identity into Sigma's flow**. Test Sigma the way we test Google — as a passive OAuth provider. |
| HandCash | Custodial OAuth-style | Out of scope long-term but worth confirming the redirect flow loads |
| Yours / `window.CWI` | JS provider injection | Touch-point only. The actual shim work is Phase 2 of `Sigma-BRC121-Sprint`. We only need to confirm that whatever we do here doesn't break the shim's connect/disconnect prompts. |

Add new providers as we hit them in the wild.

---

## 4. Browser variables that can break auth

For each, record: the current Hodos default, what (if anything) is exempted for auth domains, what the suspected interaction with Google's flow is, and whether the variable should be touched in this investigation.

- **Fingerprint farbling** — `cef-native/include/core/FingerprintProtection.h:226-264` ships an auth-domain exemption list. Roadmap already includes moving/relocating the farbling code. **Suspect #1 for the Google regression.** Verify the exemption list still hits the URLs Google actually uses (`accounts.google.com`, `oauth2.googleapis.com`, `apis.google.com`, `*.googleusercontent.com`, the various reCAPTCHA endpoints).
- **Cookie safelist** — `cef-native/src/core/CookieBlockManager.cpp:775-779`. Auth provider cookies are preserved when cookie blocking is on. Check that Google's full cookie set (NID, SID, HSID, SSID, APISID, SAPISID, __Secure-1PSID, etc.) round-trips correctly during a sign-in.
- **Ephemeral cookies** — `cef-native/src/core/EphemeralCookieManager.cpp:284-293`. Auth provider cookies are NOT wiped in ephemeral mode. Confirm this still holds with the moved farbling and that incognito-like browsing doesn't silently 401 sign-ins.
- **Third-party cookies (3PC)** — Chrome's 3PC phase-down and Safari ITP have been breaking OAuth iframes industry-wide for years. CEF 136 inherits Chrome's current 3PC defaults; we may need explicit per-origin overrides for the `auth.sigmaidentity.com` iframe and for Google's `accounts.youtube.com` cross-domain hits.
- **Popup vs redirect (`window.open` policies)** — Apple Sign-In requires a popup with a real user gesture; Google offers both. CEF popup blocker defaults can swallow these. Verify the popup-allowance path for auth-domain origins.
- **FedCM** — Google has been pushing FedCM as a 3PC replacement. CEF 136 supports it but it may need to be enabled explicitly. Document current FedCM state and whether enabling it changes the Google flow.
- **Iframe storage isolation** — partitioned localStorage / IndexedDB. Especially relevant for Sigma's iframe signer at `auth.sigmaidentity.com/signer` and for Google's cross-domain frames.
- **CSP / Permissions-Policy passthrough** — confirm we don't strip headers that auth providers rely on.
- **CORS preflight** — token endpoints (`oauth2.googleapis.com/token`, etc.) must complete OPTIONS preflights with the right headers. Our adblock engine and request interceptor should leave these alone.
- **User-Agent string** — some providers gate signin attempts by UA pattern (Google has flagged Hodos before for "non-standard browser" in early builds). Verify the current UA string is acceptable to all listed providers.
- **Service worker / cache interference** — auth callback URLs shouldn't be cached. Confirm.

---

## 5. Test matrix

Each cell records: `works | broken | works-with-quirk | not-tested`, plus a one-line note when not `works`.

|             | Farbling | Cookies | Ephemeral | 3PC | Popup | FedCM | Iframe Storage | CSP | CORS | UA |
|-------------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Google      |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ? |
| Microsoft   |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ? |
| GitHub      |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ? |
| Apple       |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ? |
| Facebook    |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ? |
| X / Twitter |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ?  |  ? |
| Sigma       |  ?  |  ?  |  ?  |  ?  |  ?  | n/a |  ?  |  ?  |  ?  |  ? |
| HandCash    |  ?  |  ?  |  ?  |  ?  |  ?  | n/a |  ?  |  ?  |  ?  |  ? |

Reference site list per provider lives in a subordinate doc once the phase starts (e.g. `Auth/test-sites.md`).

---

## 6. Existing infra to verify is NOT regressed

The four load-bearing UX safeguards in the root `CLAUDE.md` are independent of this work but should not regress as side-effects of any farbling/cookie/storage change made here:

- Tab payment badge animation (green dot on auto-approved payments) — `HttpRequestInterceptor.cpp:1656-1681` → `simple_render_process_handler.cpp:1020` → `useTabManager.ts:141`
- Right-click "Manage Site Permissions" (`MENU_ID_MANAGE_PERMISSIONS` at `simple_handler.cpp:6696`)
- `DomainPermissionForm` "Always notify me" toggle
- Privacy perimeter prompts (identity-key reveal, key-linkage reveal, sensitive cert fields, large spends — always prompt regardless of settings)

Any farbling/cookie change has to ship with a smoke test against these.

---

## 7. Out of scope

- **No new wallet primitives.** This is browser-side work. BSM/BRC-77/`signMessage` handlers, if ever needed, get their own phase.
- **No Sigma identity substitution.** Conclusively ruled out by the Sigma-BRC121-Sprint OQ#1 finding (2026-05-05). Sigma here is just one of many OAuth providers we test.
- **No window.CWI / yours / panda shim work.** That's Sigma-BRC121-Sprint Phase 2 (`phase-2-window-cwi-shim/`).
- **No new auth provider integrations.** We're testing the browser's ability to round-trip existing providers' flows, not building new client adapters.

---

## 8. Where the Sigma docs in this folder fit

When Sigma was the original Phase 2 thread, four docs accumulated under `Sigma-BRC121-Sprint/phase-2-sigma-auth/`. They were moved into this folder on 2026-05-28 as reference material for the Sigma row of the test matrix:

| File | What it is | Status |
|------|------------|--------|
| `phase-2-sigma-auth-README.md` | The original phase README that framed Sigma as a 2A/2B implementation effort | **Historical.** Phase 2A/2B framing is dead. Preserved for archaeology. |
| `sigma-research-findings.md` | 2026-05-05 deep-dive on Sigma protocol, BAP, BMAP, bitcoin-auth | **Mostly current** on the protocol layer. The §A4 "Sigma will accept arbitrary keys" reading is **superseded** by OQ#1 — iframe signer architecture blocks key substitution. |
| `BRC103_SIGMA_AUTH_GUIDE.md` | Developer-facing OAuth integration guide | **Reusable** for the eventual Phase 4 demo's "Sign in with Sigma" button. |
| `BRC103_SIGMA_COMPARISON_AND_IMPLEMENTATION.md` | Original protocol research comparing BRC-103 / Sigma / signing schemes | Sigma-interception sections are **obsolete**; BRC-77/BSM primitives sections are **reference material** if message-signing ever becomes its own phase. |

These do not gate the investigation — they're context for the Sigma row of the matrix.

---

## 9. Next-sprint adoption

When this folder is slotted as a phase in the next sprint, the phase should produce:

1. A populated `Auth/test-matrix.md` with results per cell.
2. A root-cause finding for the Google sign-in regression (most likely a fingerprint farbling exemption fix or a 3PC override).
3. Concrete CEF/C++ patches scoped narrowly to whatever the root cause turns out to be.
4. A regression smoke test (manual checklist or scripted Chrome DevTools probe) covering at least Google + Microsoft + GitHub + Sigma sign-in, runnable before every CEF/cookie/farbling change.

Until then this folder is a parking lot.

---

## See also

- `development-docs/Sigma-BRC121-Sprint/README.md` — the wallet provider plumbing sprint; do not mix scope with Auth
- `development-docs/Sigma-BRC121-Sprint/OPEN_QUESTIONS.md` — answered Sigma questions, especially OQ#1 (Sigma interception cancelled) and Q12 (Sigma OAuth as normal provider works zero-code)
- `cef-native/include/core/FingerprintProtection.h` — current auth-domain exemption list
- `cef-native/src/core/CookieBlockManager.cpp` — auth-cookie safelist
- `cef-native/src/core/EphemeralCookieManager.cpp` — auth-cookie preservation in ephemeral mode
- Root `CLAUDE.md` — load-bearing UX safeguards that must not regress
