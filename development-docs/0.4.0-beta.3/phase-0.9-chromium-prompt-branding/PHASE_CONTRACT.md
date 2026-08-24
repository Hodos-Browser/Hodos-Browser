# Phase 0.9 — Hodos branding on Chromium's own prompts

**Opened 2026-08-21**, owner-requested after the Chromium loopback prompt appeared during the
bitgenius.net connect test looking like stock Chrome.

## 1. Goal

Prompts the user sees while browsing in Hodos look like **Hodos**, and say plainly what is being
asked. Start with the loopback/local-network prompt, because it is the one standing in front of a
wallet on `127.0.0.1`.

⛔ **Use the HodosBrowser logo, not the HodosWallet logo.** These are browser-level prompts about
sites and the device, not wallet operations. `frontend/public/` carries both
(`Hodos_Gold_Browser_Icon.svg` vs `Hodos_Gold_Wallet_Icon.svg`) — confirm the exact filenames before
wiring, and use the browser one everywhere in this phase. Getting this backwards teaches users that
the wallet is asking, which is precisely the confusion an attacker wants.

## 2. It is the existing pattern, not new machinery

`SimpleHandler` already implements `CefPermissionHandler`, and `PendingPermissionManager`
(`include/core/PendingPermissionRequest.h`) already parks a CEF callback while a Hodos-branded
overlay is shown, resolving it via the `permission_response` IPC. Camera, microphone, location,
notifications and clipboard already go through it. This phase **adds cases**, it does not add a
mechanism.

⛔ **CORRECTED 2026-08-24 — the constant list below was wrong in two ways.**

```
CEF_PERMISSION_TYPE_LOCAL_NETWORK    = 1 << 26   ✅ real, and emitted
CEF_PERMISSION_TYPE_LOOPBACK_NETWORK = 1 << 27   ✅ real, and emitted (MEASURED: mask=0x08000000)
```

`CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS` (1 << 25) **does not exist in our build and can
never arrive**:

1. We pass no `api_version`, so `CEF_API_VERSION` defaults to `CEF_API_VERSION_EXPERIMENTAL`
   (`cef_api_hash.h:69`). That makes `CEF_API_ADDED(15000)` true, so `cef_types.h:3917`
   compiles the name as `..._LOCAL_NETWORK_ACCESS_DEPRECATED`. The undecorated name is in the
   `#elif` arm and is not declared.
2. libcef never emits it regardless — `libcef/browser/permission_prompt.cc :: GetCefRequestType`
   has **no case** producing 1 << 25.

`site_permission_mapping_test.cpp :: DeprecatedLocalNetworkAccessBitIsNotMapped` pins this.

⚠️ `SitePermissionType` in `SitePermissionStore.h` is **deliberately decoupled** from CEF's bitflag
enum so a Chromium bump renumbering `cef_permission_request_types_t` cannot corrupt stored rows.
New types get **new stable integers** there and are mapped only at the callback boundary. Do not
reuse a CEF bit value as a stored value.

## 3. Scope — decide before building

Chromium prompts fall into three groups, and only the first is clearly ours:

| Group | Examples | Interceptable via `CefPermissionHandler`? |
|---|---|---|
| **A — permission prompts** | loopback / local network, and the five already done | ✅ yes — this phase |
| **B — browser-feature bubbles** | **save password**, save address, save card, translate, download warnings | ❓ **UNKNOWN.** These are Chrome-UI bubbles, not permission requests, and CEF may not surface them at all. **Answer this before promising it.** |
| **C — OS/native dialogs** | file picker, print | ❌ out of scope |

⛔ **The owner asked specifically about save-password.** It is group B. Do not scope it in until
someone has checked whether CEF 150 exposes it — the honest answer may be "only by patching
Chromium," which is possible for us (we build our own) but is a completely different size of job.
**That check is task 1 of this phase.**

## 4. Done means

- [x] Loopback / local-network prompts render as a Hodos overlay with the **browser** logo.
- [x] Wording states the real ask: *this site wants to talk to a server running on your computer*.
- [x] ⛔ **No blanket auto-allow.** ⚠️ **AMENDED 2026-08-24, owner decision — see §7.** A wallet
      connect modal may now answer a parked loopback permission, because loopback access is
      all-or-nothing industry-wide and a site that can reach the wallet can reach other local
      software by definition. This is NOT a hardcoded origin exemption and NOT a silent grant:
      it requires an explicit user approval on a modal that **discloses** the local-access
      scope, and the grant is refused if that disclosure did not render (§7.2).
- [x] Allow / Block map onto the existing `SitePermissionStore` states with new **stable**
      `SitePermissionType` integers (LocalNetwork=6, Loopback=7), pinned by unit test.
      ⚠️ "Allow once" was REMOVED for these two types — CEF cannot express it (§7.1).
- [x] Group B answered in writing: **patch-only** (§8).
- [~] macOS parity from the start — code paths written in both arms, **entirely unexercised**.
      Owed to the Mac relay round.
- [ ] Reachable and dismissable at DPI matrix cells #4/#6/#9. **NOT RUN.**

## 5. Evidence table — MEASURED 2026-08-24

All timestamps are from `%APPDATA%/HodosBrowserDev/logs/debug_output.log`. ⛔ Several REDs as
originally written were unreachable; the ones recorded here are what was actually observed.

| ID | Result | Evidence |
|---|---|---|
| `P0.9-A1` | 🟢 **GREEN** | `13:40:58.339 OnShowPermissionPrompt origin=https://bitgenius.net/ mask=0x08000000 mapped=[loopback]` → Hodos overlay. RED as specified ("pre-fix: stock Chromium bubble") was observed live on 2026-08-21 and again whenever the handler declined. |
| `P0.9-A2` | 🟢 **GREEN** | Subject re-derived (owner-approved): a throwaway loopback listener's access log, **not** the wallet log — wallet endpoints are served by our interceptor and never reach the network, so they can never raise this permission and the original subject was unmeasurable. Allow → `13:27:38.366 permission_response 'allow_always'` and `13:27:38.367 ARRIVED GET /probe Origin=https://example.com` (1 ms later). Block → `13:28:18.279` request attempted, **no ARRIVED line**, and **0** further `OnShowPermissionPrompt` calls (Chromium short-circuits on its stored setting). Attempted-but-absent is the correct pair; a page-side error alone proves neither half. |
| `P0.9-A3` | 🟢 **GREEN**, semantics changed | Allow persists: `Profile_1 loopback_network bitgenius.net ALLOW`. "Allow once" no longer exists for these types (§7.1). Prompt **denials are now temporary** by owner standard: `13:27:12 permission_response 'block'` → `13:27:16 OnShowPermissionPrompt … example.com` — re-asked 4 s later, nothing stored. |
| `P0.9-A4` | 🟢 **GREEN** | `BRC100AuthOverlayRoot.tsx :: HodosBrowserHeader` → `/Hodos_Gold_Browser_Icon.svg`; `HodosWalletHeader` is a separate component used only by money-path branches. Confirmed visually by the owner. Pre-existing and unregressed — the diff touches no header component. |
| `P0.9-A5` | 🔴 **NOT RUN** | DPI cells #4/#6/#9 never exercised. Carries to Phase 1 (WS1). |
| `P0.9-A6` *(new)* | 🟢 **GREEN** | Site controls write-through: `13:27:47.150 🛈 Mirrored loopback=block to Chromium content settings for example.com`, and the setting reads BLOCK. RED measured before the fix — the toggle wrote only our SQLite while Chromium's setting governed, so revoke did nothing. |
| `P0.9-A7` *(new)* | 🟢 **GREEN** | Connect-modal binding shows **one** modal, not two: `13:40:58.630 Connect modal … claimed a parked local-network permission` → `13:40:58.748 Deferred permission … is bound … not showing a second prompt` → `13:41:23.602 … -> ACCEPT`. Zero `No connect modal claimed` lines that run. |

### Unit tests

`cef-native/tests/site_permission_mapping_test.cpp` — 10 tests. Two negative controls demonstrated:
removing the loopback/LAN mapping arms turned exactly the three mapping tests red while the other
four stayed green; renaming a stored id 7→9 **compiled silently** (the real hazard) and turned only
the frozen-id test red. A third control proved the `static_assert` guard fires: flipping a mirrored
CEF bit failed the build with `CEF bit drift: LOOPBACK_NETWORK`.

⚠️ **Gap, owner-accepted:** there is no test that goes red if `LocalAccessNotice` stops rendering.
The fail-closed acknowledgement (§7.2) makes the resulting defect non-exploitable, but the invariant
itself is unguarded.

## 6. Out of scope

- Auto-allowing anything (see §4).
- Group C native dialogs.
- Rewording the five existing prompts — separate UX work; do not smuggle it in here.

---

## 7. Design decisions taken during implementation (owner, 2026-08-24)

These changed the phase from what §1–§4 originally described. Recorded here so the *why* survives.

### 7.1 "Allow this time" removed for the network types

CEF exposes only `ACCEPT / DENY / DISMISS / IGNORE` — **there is no "grant once"**. Answering
ACCEPT makes Chromium write a persistent content setting. MEASURED: an "Allow this time" click on
example.com produced `loopback_network → ALLOW` surviving restart. A button reading "this time" that
means "forever" is a lie on a consent surface, so these two types show **Allow / Don't allow** only.

Camera and mic keep all three buttons: they arrive via `OnRequestMediaAccessPermission`, whose
`Continue()` persists nothing, so "Allow this time" is truthful there. Confirmed by a stored mic
Allow in our DB with no matching Chromium content setting.

⚠️ Location / notifications / clipboard have the same defect and were deliberately NOT changed —
the contract forbids smuggling changes to the existing five into this phase. Ticketed.

### 7.2 One consent, not two — and the disclosure that makes it defensible

**Owner's reasoning:** loopback access is all-or-nothing everywhere. No browser offers per-app local
access, and Chromium keys the grant on the requesting origin alone (`TOP_ORIGIN_ONLY_SCOPE`, no
target in the record). A site that can reach the wallet can reach other local software *by
definition*. Two prompts for one decision is worse UX and no more protective.

So a wallet connect modal may claim a parked loopback permission and answer it. This is only
defensible because the modal **says what it grants** — `LocalAccessNotice`, styled informational
rather than as a warning, because this is industry-standard behaviour and dressing it as an alarm
would train users to dismiss it.

⛔ **Fail closed.** `LocalAccessNotice` reports that it actually mounted; that acknowledgement rides
with the approval, and C++ **refuses the grant without it**. A connect branch that carries the flag
and renders nothing degrades to DISMISS instead of granting silently. This replaced a code comment
asserting the flag and disclosure agreed — a comment that was already false for `domain_approval`
when it was written, and was caught only by adversarial review.

⚠️ A CWI call is **not** a loopback call, and neither is a direct wallet fetch — both are answered by
our interceptor before the network. So the permission only ever exists when the site made some
*other* local request, and the notice only appears when a real parked permission exists to bind.

### 7.3 Prompt denials are temporary; overt actions persist

> "Decisions based on prompts are temporary and get re-prompted; overt user actions stay until the
> user overtly changes them back." — owner, 2026-08-24

A denial in a **prompt** now resolves `DISMISS` (stores nothing, Chromium asks again). The user was
interrupted, may not have understood the ask, and should not have to find a settings panel to undo a
snap judgement. A denial in the **Site controls panel** is deliberate and stays put.

MEASURED: `13:27:12 permission_response 'block'` → `13:27:16` re-asked. Applied to the two network
types only; the other three prompt-path types are ticketed.

### 7.4 Waiting on state, not on a timer

The first implementation guessed the connect-modal gap with a fixed 750 ms window. Measured gaps on
one machine, same site: **239 ms, 282 ms, 830 ms**. The 830 ms case flashed the standalone prompt for
71 ms before the connect modal replaced it.

Replaced with `WalletActivityTracker`: while the host is actively talking to the wallet (or a modal
is queued for it), keep waiting; show the standalone prompt once it goes quiet. ⚠️ Deliberately a
**timestamp, not an in-flight counter** — a counter needs decrementing on every completion, cancel
and error path, and one miss would suppress the prompt forever for that host, failing silent and
open. 15 s backstop, well under the 60 s watchdog.

### 7.5 Accepted limitation — wallet is global, loopback is per-profile

One `wallet.db` serves every browser profile, so wallet approvals are global. Chromium stores the
loopback setting **per profile**. The two therefore disagree: a site approved for the wallet in one
profile still gets asked about loopback in another. There is no fix short of per-profile wallets.
Owner decision: accept and set the precedent; revisit only if profiles ever get their own wallets.

---

## 8. Group B verdict (task 1) — save-password

**PATCH-ONLY for interception. Suppression is available with zero patch. Neither measured live.**

- `grep` over `libcef/` and `libcef_dll/` for `password_manager|ManagePasswords|PasswordBubble|ChromePasswordManagerClient` → **zero hits**. The only password strings in CEF are `.pak` resource ids and Chrome command ids — data, not API.
- There is **no CEF client callback** for browser-feature bubbles. `CefPermissionHandler` is bounded by `cef_permission_request_types_t`, which has no password or autofill member.
- The machinery IS live: chrome-style CEF puts the WebContents into a real Chrome `Browser`'s tab strip (`chrome_browser_host_impl.cc:129 :: AddWebContents`), attaching Chrome's tab helpers including `ManagePasswordsUIController`.
- **Patch size for real interception:** a new CEF client interface = public header + libcef impl + `libcef_dll` cpptoc/ctocpp regeneration + `cef_api_versions.json` rehash, plus a Chromium hunk diverting `ManagePasswordsUIController::UpdateBubbleAndIconVisibility`. ~300–500 LOC across two repos, re-hashed and re-verified on **every Chromium bump** — and it buys a *dialog*. A Hodos password manager (encrypted store, form detection, fill) is a separate feature an order of magnitude larger.
- **Zero-patch suppression:** `CefRequestContext::SetPreference("credentials_enable_service", false)`. The pref is registered by Chrome's browser prefs and `pref_helper.cc :: CanSetPreference` has no allowlist. **UNMEASURED.**
- ⛔ Not claimed: whether the bubble appears in Hodos today. That needs a live test, not a source read.

---

## 9. Test controls

`reset_test_state.py` in this folder. Every wrong conclusion on 2026-08-24 came from testing against
unmeasured state (a four-day-old stored BLOCK; a site already wallet-approved; a jammed parked
permission; a profile that inherited a deleted one's settings).

    python reset_test_state.py show
    python reset_test_state.py clear-loopback <profile|ALL>
    python reset_test_state.py clear-wallet-domain <domain|ALL>
    python reset_test_state.py verify [--profile P] [--domain D]      # non-zero exit on mismatch

⛔ Stop the dev browser first — Chromium rewrites `Preferences` on exit.
⛔ `clear-wallet-domain` sets `PRAGMA foreign_keys=ON`; without it the child rows silently do not
cascade (the P0.8 trap).
