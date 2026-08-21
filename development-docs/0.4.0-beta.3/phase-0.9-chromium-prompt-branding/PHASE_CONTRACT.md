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

Verified in `cef-binaries/include/internal/cef_types.h`:

```
CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS = 1 << 25
CEF_PERMISSION_TYPE_LOCAL_NETWORK        = 1 << 26
CEF_PERMISSION_TYPE_LOOPBACK_NETWORK     = 1 << 27
```

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

- [ ] Loopback / local-network prompts render as a Hodos overlay with the **browser** logo.
- [ ] Wording states the real ask: *this site wants to talk to a server running on your computer*.
- [ ] ⛔ **No auto-allow.** Owner-agreed 2026-08-21. Not for loopback, not for the wallet's own
      origin, not "just for `127.0.0.1:31301`". A correct "this is the wallet UI" predicate is
      Phase 5's `IsWalletOrigin()` work, and hardcoding one here would be a **fourth derivation of
      a security value** — the exact mistake `extractDomain` made.
- [ ] Allow / Block / Allow-once map onto the existing `SitePermissionStore` states, with new
      **stable** `SitePermissionType` integers.
- [ ] Group B answered in writing: interceptable, patch-only, or not at all.
- [ ] macOS parity from the start (CLAUDE.md #9) — not deferred to a relay round.
- [ ] The prompt is reachable and dismissable at DPI matrix cells #4/#6/#9. See
      `TICKET_modal_buttons_unclickable_small_screen.md` — an approval overlay whose buttons cannot
      be clicked at 150% is worse than no overlay.

## 5. Evidence table

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| `P0.9-A1` | A page fetching `127.0.0.1` raises the **Hodos** overlay | ⛔ Pre-fix: stock Chromium bubble — observed 2026-08-21 | The rendered overlay | T2 |
| `P0.9-A2` | Block actually blocks: the fetch fails | ⛔ Stub the deny path → the fetch succeeds. **A blocked read is not a blocked write** — assert the server-side effect | The wallet log: the request must not arrive | T1 |
| `P0.9-A3` | Allow persists per-site; Allow-once does **not** survive a restart | ⛔ Swap the two → the ephemeral grant persists | `SitePermissionStore` rows | T1 |
| `P0.9-A4` | The logo shown is the **browser** logo | ⛔ Point it at the wallet asset → the test must go red | The rendered image, not the source path | T0 |
| `P0.9-A5` | Prompt is clickable at 125% / 1366 and 150% / 1366 | ⛔ Same prompt at 100% must be seen to work, or the test cannot distinguish DPI from broken | Instrumented hit test, not eyeballing | T2 |

## 6. Out of scope

- Auto-allowing anything (see §4).
- Group C native dialogs.
- Rewording the five existing prompts — separate UX work; do not smuggle it in here.
