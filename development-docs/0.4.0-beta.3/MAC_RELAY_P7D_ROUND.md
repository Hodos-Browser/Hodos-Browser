# Mac relay — beta.3 Phase 7d (the management half of the consent surface)

**Commits:** `ec1c465` · `982d757` · `0821274` · `08aa761` · `8874232` · `a1712e9` · `fc931bb` ·
`6338208` · `8a716ba`
**Base:** `a052033` (the end of the 7c relay). **Branch:** `0.4.0`.
**Written 2026-09-08.** Predecessor: `MAC_RELAY_P7C_ROUND.md`.

⭐ **The short version.** 7d is **React + one C++ function**. No new overlay, no HWND/NSWindow work,
no platform-conditional code, no schema change. The C++ change is inside
`simple_handler.cpp :: MirrorSitePermissionToChromium`, which is already cross-platform — it calls
`CefRequestContext::SetContentSetting`, not a Win32 API. `git pull` and rebuild.

⚠️ **But one row genuinely needs re-measuring on Mac, and it is the one with teeth.** See `M3`.

---

## M1 — What changed, in five pieces

| # | Item | Layer |
|---|---|---|
| 5 | Approved-sites **domain filter** (native `<input>`, Clear, count, no-match panel) | React |
| R4 | **Site controls Block now blocks** — Location / Notifications / Clipboard mirrored to Chromium | C++ (1 fn) + React (1 caption) |
| 1 | **Edit Limits closes only on Save/Cancel**, on *both* management entry points | React |
| 2 | Granted-permissions list **capped at 240 px and scrolls** | React |
| 3 | Limits section **collapsible** on the management surfaces | React |

Item 4 (persist declined permissions) is **deferred to beta.4** — schema change, CLAUDE.md
invariant #2, owner decision `D-B`.

## M2 — ⛔ Three plan claims that were false. Do not re-derive them on Mac.

1. **`ApprovedSitesTab.tsx` does not render the approved-sites list.** It is the *defaults* panel —
   zero `.map()` calls in the file. The list is `DomainPermissionsTab.tsx`, which **already had sort
   and 12-row pagination**. The ticket's "no search, filter or sort — the words do not appear in the
   file" was measured on the wrong file.
2. **The right-click "Manage Wallet Permissions" path never shows that list.** It opens a
   *single-domain* editor via `CreateNotificationOverlay("edit_permissions", domain)`.
3. 🚨 **Item 1's close paths are React, not C++** — and the ticket says the opposite, explicitly
   (*"do not implement this as a React onClick handler and assume it holds"*). The Approved Sites
   list is a **browser tab** (`/wallet`), not the wallet overlay (`/wallet-panel`), so no
   `WM_ACTIVATE` path sits over its dialog. **This matters on Mac too:** the equivalent temptation is
   `InstallClickOutsideMonitor` / `windowDidResignKey`, and it is equally wrong. Nothing to port.

## M3 — 🎯 The one thing that must be re-measured on Mac: R4

`MirrorSitePermissionToChromium` now writes Chromium content settings for three more types:

| Our type | CEF type | Why it needs a Mac run |
|---|---|---|
| Notifications | `CEF_CONTENT_SETTING_TYPE_NOTIFICATIONS` | single type, low risk |
| Location | `CEF_CONTENT_SETTING_TYPE_GEOLOCATION` | ⚠️ `GEOLOCATION_WITH_OPTIONS` also exists and its header says the permission *"is migrating to use permissions with options, which won't be stored as ContentSettings"*. **Measured on Windows: plain `GEOLOCATION` IS consulted.** That is a per-build fact, not a per-platform guarantee |
| Clipboard | `CEF_CONTENT_SETTING_TYPE_CLIPBOARD_READ_WRITE` | ⚠️ `CLIPBOARD_SANITIZED_WRITE` is *"special-cased in the permissions layer to always allow"* — **sanitized write cannot be blocked on any platform.** The Site controls panel discloses this (owner decision `D-C`) |

⛔ **Seeing the content setting appear is NOT the green.** The subject is the **site's behaviour**.
Both platforms run the same engine pin (`150.0.43-7871.3576+g9ccef04`), so the expectation is that
Mac matches — but "expected to match" is a code reading, and this row's whole point is that a control
can report success and do nothing.

**Mac run, ~3 minutes.** Open any `https://` page, set Location / Notifications / Clipboard to
**Block** in Site controls, then in the page console:

```js
await Notification.requestPermission()            // expect "denied", and NO prompt
navigator.geolocation.getCurrentPosition(         // expect PERMISSION_DENIED (code 1)
  () => console.log('GOT LOCATION — RED'), e => console.log(e.code));
await navigator.clipboard.readText()              // expect it to reject: NotAllowedError
```

Then **Reset permissions for this site** → all three back to a prompt.
⛔ **Also assert the control:** camera and microphone must stay `prompt`. They ride
`OnRequestMediaAccessPermission`, whose `Continue()` persists nothing, so our store is genuinely
authoritative there and no content setting may be written. If they go `denied`, the mirror widened
onto them and that is a defect.

⛔ **Do not use CDP `Browser.setPermission` to plant a test denial.** 📏 Measured on Windows with this
build: it is a **silent no-op** — returns `{"result":{}}` with no error, origin-scoped or
context-wide, and changes nothing. Believing its success reply produced three false "the reader is
blind" verdicts. Prove reader sensitivity against an origin that already carries a real block.

## M4 — What does *not* need a Mac port

- **No new overlay**, so nothing to add to `cef_browser_shell_mac.mm` and nothing for
  `ReleaseOverlaysOwnedBy`. Windows stays at 15 overlays, macOS at 14 (the Phase 4 tab context menu
  is still the only gap — `MAC_RELAY_P35_P4_ROUND.md` M3).
- **No `#ifdef`** was added. The C++ change is one `switch` inside an existing cross-platform
  function.
- **No schema change, no migration.**
- The React work is platform-agnostic. ⚠️ One thing to *look* at rather than port: the filter box is
  a native `<input>` and the Approved Sites list renders in a **tab**, so the CEF overlay input rule
  does not strictly bite — but `DomainPermissionForm` (same component tree) *is* rendered inside the
  **notification overlay** on the right-click path, and that is a real OSR overlay on Mac too.

## M5 — Re-measure on Mac, cheaply

| Row | What to check | Expect |
|---|---|---|
| `A4`–`A8` | **M3 above** — the one that matters | site behaviour changes; camera/mic unchanged |
| `A9` | Edit Limits + right-click editor: change a value, click outside and press Escape | value kept, "Click Save or Cancel to close" hint, Cancel still closes |
| `A1`/`A2` | Filter the approved list; then go to page 2 **and then** filter | matches shown, never an empty table under a non-empty count |
| `A11` | Connect + payment modals | unchanged: no "Spending limits" header, granted list uncapped |

## M6 — Instrument traps this phase paid for

1. 🚨 **Vite Fast Refresh preserves React state across an edit.** A negative control printed **GREEN**
   against a build with the guard *deleted*, because HMR had preserved the state the guard produced.
   **Hard-reload before every React measurement** — it is baked into the probes now.
2. ⛔ **Grep for a behavioural token, never a comment.** "Confirming" a stale bundle by grepping the
   served module for a marker comment fails — Vite strips comments.
3. ⛔ **A zero can be a suppressed log, not an absence.** `R-INTEXT` read 0 lines because
   `domain_trust_mw` logs at **debug** and the wallet defaults to **info**. Run it with
   `RUST_LOG=hodos_wallet=debug`.
4. ⛔ **Assert the trigger fired.** A probe that clicked an expander present on only one of three
   modals scored the other two clean while never rendering the thing under test.

## M7 — Owed, and honest about it

- **DPI matrix cells #4/#6/#9** — not run on either platform.
- **Stubbed `R-INTEXT` REDs** — both GREEN halves are now observed (first time at any beta.3
  boundary), plus a same-path 3 ms A/B that rules out a stuck discriminator. The forced-flip stubs
  remain owed before the release boundary.
- **`R-GOLD` / `R-COUNT`** — no real payment this session.
- **7c's adversarial review** and `TICKET_wallet_quiet_detector_blind_to_long_polls.md` — still open
  from the previous boundary.

---

## ✅ ANSWERED BY MAC 2026-09-08 — `R4` is **GREEN** on macOS

M3's re-measure is done. Full evidence: `MAC_RELAY_BETA3.md` (round 2026-09-08 Mac, §C) and
`phase-7d-management-surface/PHASE_CONTRACT.md` §4b. Probe: `probes/dual_store_probe_mac.py`.

- `A4`/`A5`/`A6` all flip `prompt`→`denied`; `A7` reset returns all three to `prompt`.
- `A8` control holds at **both** layers: camera/mic stay `prompt` on the blocked origin, and
  **zero** `🛈 Mirrored …camera/microphone` lines were emitted all session.
- ⭐ `D-10`: plain `GEOLOCATION` **is** consulted on macOS too — `getCurrentPosition` → code 1.
- Origin-specificity control added (`www.wikipedia.org` stayed `prompt`), because your
  `validate_instrument()` youtube gate **cannot pass on macOS**: our SQLite row exists here
  (2026-08-10, identical) but the **Chromium** half was never planted, since notifications only
  began mirroring in the build under test.

### ⛔ One correction to M3's method

📏 `await navigator.clipboard.readText()` → `NotAllowedError` in **both** arms — it also rejects on
focus/transient-activation grounds. **That probe passes with the feature removed (HARNESS §6 Q1),
so alone it is void.** `A6` is green on the `permissions.query('clipboard-read')` flip plus the
mirror log line. Your row is right; the stated check is the thing to fix.
