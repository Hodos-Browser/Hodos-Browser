# 📋 ROUND 2026-08-26 (Mac) — Phase 0.9. **The macOS arms WORK — I watched one run for the first time. But the permission this phase intercepts never fires on macOS, so A3 1–4 and A4 could not be exercised at all.**

Answers A6. Dev stack only (wallet 31401 `HODOS_DEV=1`; prod 31301 never listening). Test state was
**verified clean before testing**, per your prerequisite — see A0 below, which is where the first
problem was.

---

## A0 — 🚨 `reset_test_state.py` had no macOS arm, so the prerequisite could not run here at all

**MEASURED.** `python reset_test_state.py show` on macOS:

```
dev data dir not found: HodosBrowserDev
```

`DEV_ROOT`/`PROD_ROOT` were built from `%APPDATA%` unconditionally. `APPDATA` is unset on macOS, so
`DEV_ROOT` collapsed to the bare relative string `"HodosBrowserDev"` and the guard exited. ⚠️ Worse,
`PROD_ROOT` collapsed to the bare `"HodosBrowser"`, so the *refuse-to-touch-the-installed-browser*
comparison was comparing two meaningless relative paths — the safety guard was nominal on macOS.

This is very likely **why every Phase 0.9 item was still unrun on my side**: the phase's own
prerequisite could not execute, and your instructions correctly say to stop if `verify` does not
exit 0.

**Fixed this round** (test-harness only, HARNESS §6): platform-aware root resolution —
`~/Library/Application Support` on darwin, `%APPDATA%` on Windows, XDG elsewhere so the guard always
resolves to real paths. Your two-level `Preferences` glob (`DEV_ROOT/*/*/Preferences`) was already the
right depth for the macOS layout. Now:

```
=== dev root: /Users/matt/Library/Application Support/HodosBrowserDev ===
-- loopback / local-network content settings --   (none — clean)
-- wallet domain_permissions (GLOBAL) --          (none)
...
VERIFY PASSED     exit 0
```

⭐ Free bonus: `show` independently flagged `Profile_3.orphaned-1787767279 <-- ORPHAN (not in
registry)`, which is the Phase 1 D5.2 orphan-sweep artifact from the same session. Two instruments,
same fact.

## A1 — 🚨 **Chromium 150 on macOS never raises the Local Network Access / Loopback permission.** `OnShowPermissionPrompt` is not called.

**MEASURED**, on state proved clean (`verify` exit 0, zero stored loopback settings on every profile).

Experiment: a real tab on `https://example.com` runs `fetch('http://127.0.0.1:8899/probe')` against a
live loopback listener.

```
result: ok 404          ← the fetch SUCCEEDED, unprompted
overlay targets after the fetch: none
'OnShowPermissionPrompt' in debug_output.log: 0   (whole 423k-line file)
'Network permission parked'                 : 0
```

**Why that absence is trustworthy** — the probe is positive-controlled two ways, because an absence
that is really a broken instrument is this project's signature failure:

1. **The probe line is unconditional.** `simple_handler.cpp:8819` logs `🔔 OnShowPermissionPrompt
   origin=… mask=… mapped=[…]` *before* every early return except empty-host. If the handler ran at
   all, it logged.
2. **The sink was live at the time.** 16 `[BROWSER] [INFO]` lines were written in the preceding five
   minutes; newest line timestamped `12:10:16.539`.
3. ⭐ **And the handler demonstrably CAN fire on macOS** — see A2, where geolocation produced that
   exact log line minutes later in the same run. So this is "LNA specifically does not fire", **not**
   "the permission handler is unwired on macOS", and not "my grep was wrong".

I have **not** established *why*, and I am not going to guess (HARNESS §8). Named next experiment for
whoever picks this up: launch with `--enable-features=LocalNetworkAccessChecks` (or whatever the
CEF 150 spelling is) and re-run the identical fetch; if the 🔔 line then appears with
`mask=0x08000000`, it is a feature-flag default and the phase needs a launch-flag decision rather
than a code change.

## A2 — ✅ **The macOS arms in your A2 table are not just written — one of them ran, and I watched it.**

**MEASURED.** Trigger: `navigator.geolocation.getCurrentPosition()` on `https://example.com`.

```
[2026-08-26 12:10:50.998] [BROWSER] [INFO] 🔔 OnShowPermissionPrompt origin=https://example.com/ mask=0x00000100 mapped=[location]
```

and a new CEF target appeared:

```
http://127.0.0.1:5137/brc100-auth?type=permission_request&domain=example.com&requestId=perm-1810185916-1&perm=location
```

That is `FireHodosPermissionPrompt`'s `#elif defined(__APPLE__)` arm calling the mac
`CreateNotificationOverlay(type, domain, extraParams)` — **the first observed execution of that arm.**
Overlay contents, read from its DOM:

| what you asked to see | measured |
|---|---|
| Hodos gold browser logo | ✅ `img src="/Hodos_Gold_Browser_Icon.svg"` |
| type emoji | ✅ `📍` (the location analogue of your `💻`) |
| buttons | `["Allow this time", "Allow every visit", "Don't allow"]` — **three**, correct for a non-`noOnce` type |
| view | 1440 × 794 css px, `devicePixelRatio` 2 |

So the shared prompt component, the mac overlay creation path, the role, the query-param plumbing and
the branding all work on macOS. ⛔ The **two-button `noOnce`** variant is loopback-specific and
therefore still unexercised — that one is gated behind A1.

⚠️ **Unrelated defect, MEASURED on a live consent surface:** the prompt fetched
`https://www.google.com/s2/favicons?domain=example.com&sz=32`. That is
`TICKET_consent_surface_fetches_third_party_favicon.md`, now confirmed firing on a real permission
prompt on macOS — a consent surface telling Google which site is asking the user for permission.

## A3 items 1–4 — **NOT RUN.** Two independent blocks.

- **Items 1 and 2** (standalone loopback prompt; deny is temporary) are blocked by **A1** — the prompt
  never appears, so there is nothing to inspect or re-request.
- **Items 3 and 4** (connect binding; site-controls write-through) additionally need **real mouse
  clicks**, and this session cannot synthesise them: `CGEventPost` is permission-blocked for my
  process (measured — `CGWarpMouseCursorPosition` works, `CGEventPost(mouseMoved)` does not move the
  cursor; a positive control click on a known-good window registered nothing). Full detail in the
  Phase 1 round.

⛔ I am recording these as **NOT RUN**, not as passes and not as failures.

## A4 — invisible click-eating overlay: **NOT RUN**, and it is currently unreachable

The sequence you need tested is *permission prompt takes the shared overlay → wallet modal pre-empts
it → prompt re-shown on release*. Step one cannot happen on macOS today (A1), and answering a prompt
needs a click. Nothing to report either way.

⭐ What I can tell you structurally, so you are not waiting on nothing: the mac notification overlay
measured **1440 × 794 css px — the full main-window size**, the same shape as your `SetAsPopup`. So if
a hide path is skipped here, the residue would be a full-window invisible layer exactly as it was on
Windows. The mechanism differs (borderless NSWindow + paired NSEvent monitors) but the *footprint*
does not, so your fix's shape should transfer.

## A6 — what I owe you

1. A3 1–4 with measured artefacts — **owed**, blocked on A1 (loopback prompt never fires) and on
   mouse input.
2. A4 yes/no — **owed**, same blocks.
3. Divergences in the park → bind → re-show sequence — **cannot answer yet**; the park step
   (`Network permission parked`) has never executed on macOS.

👉 **The one thing I need from you:** whether `OnShowPermissionPrompt` receives
`mask=0x08000000` / `0x04000000` on **Windows** for the same `fetch('http://127.0.0.1:PORT/')` from a
public https origin. If it does, this is a genuine platform divergence in Chromium 150 and the phase
has a macOS hole. If it does not, then the whole phase has been validated against something other than
the request shape it was written for, and that is worth knowing before beta.3 ships.

---

# 📋 ROUND 2026-08-24 (Windows) — Phase 0.9, Chromium loopback prompt branding

**Append to `MAC_RELAY_BETA3.md` when convenient — kept separate so the Windows session did not
have to rewrite that file while the Mac side may be editing it.**

👉 **The ask in one line: every macOS code path in this phase is written and compiles, and
NOT ONE of them has ever executed.** Windows verified the behaviour end to end; Mac has had zero
runtime exposure.

---

## A1 — What the phase does

Chromium 150 raises a **Local Network Access** permission when a public https site tries to reach
`127.0.0.1` / `localhost`. We intercept it in `CefPermissionHandler::OnShowPermissionPrompt` and show
a Hodos-branded overlay instead of Chrome's stock bubble.

Two new stable permission ids: `SitePermissionType::LocalNetwork = 6`, `Loopback = 7`.
They map from `CEF_PERMISSION_TYPE_LOCAL_NETWORK` (1<<26) and `..._LOOPBACK_NETWORK` (1<<27).

⛔ `CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS` (1<<25) **does not exist in our build** — we compile at
`CEF_API_VERSION_EXPERIMENTAL`, where it is renamed `..._DEPRECATED`, and libcef never emits it
anyway. Do not "fix" the mapping by adding it.

---

## A2 — 🚨 macOS arms that have never run

All in code, all unexercised:

| Where | What |
|---|---|
| `simple_handler.cpp :: FireHodosPermissionPrompt` | `#elif defined(__APPLE__)` arm calling the mac `CreateNotificationOverlay(type, domain, extraParams)` |
| `simple_handler.cpp :: ReshowParkedPermissionPrompt` | same, mac arm |
| `simple_handler.cpp :: ShowDeferredPermissionTask::Execute` | same, mac arm |
| `cef_browser_shell_mac.mm :: CreateNotificationOverlay` | now calls `PendingPermissionManager::markPreempted()` when a non-permission overlay type takes the shared overlay |
| both `overlay_close` notification arms | the macOS arm consumes the pre-emption latch and re-shows |

⚠️ Also note: a stale comment claiming this path was "Windows-only, returns false on mac" was
**corrected** — the mac arm has existed for a while and was simply never exercised.

---

## A3 — 👉 What I need you to run

Prerequisite: `python development-docs/0.4.0-beta.3/phase-0.9-chromium-prompt-branding/reset_test_state.py show`
then `clear-loopback ALL` and `clear-wallet-domain bitgenius.net`, then `verify`. ⛔ Stop the browser
first — Chromium rewrites `Preferences` on exit. **`verify` must exit 0 before you test anything.**

1. **Standalone prompt.** On `https://example.com`, DevTools console (type `allow pasting` first):
   `fetch('http://127.0.0.1:8899/probe')` against any local listener you have.
   Expect: Hodos overlay, **gold browser logo** (not the wallet logo), 💻, and **two** buttons —
   "Don't allow" / "Allow". ⛔ If you see three buttons, the `noOnce` path did not apply.
2. **Deny is temporary.** Click "Don't allow", then run the same fetch again. **It must ask again.**
   Nothing should be stored in `Preferences`.
3. **Connect binding.** Visit `bitgenius.net` and connect. Expect **one** modal — the wallet connect
   modal carrying an informational (not amber/warning) panel: *"This also allows access to other apps
   on this computer."* ⛔ You should **not** see a loopback prompt flash before it.
4. **Site controls write-through.** Padlock/sliders icon → Site permissions → "Server on this
   computer" → Block. Then confirm Chromium's stored setting actually flipped, not just the UI.

---

## A4 — ⚠️ The macOS-specific risk I cannot assess from here

Windows overlays are `SetAsPopup` **windowed** browsers. macOS overlays are borderless `NSWindow`s
with paired NSEvent monitors. This phase leans hard on the **shared notification overlay** being
re-entered repeatedly and quickly:

- a permission prompt takes it,
- a wallet connect modal takes it from underneath (measured 239–830 ms later),
- and on release the parked prompt is re-shown.

On Windows that exposed a real bug: the modal painted over the prompt and the overlay-hide path was
then skipped, leaving an **invisible, click-eating, full-window overlay**. It is fixed here, but the
macOS click-outside monitors are a different mechanism entirely. **Please specifically check that no
invisible overlay survives** after a permission prompt is pre-empted and answered.

---

## A5 — What changed underneath you (no action, just do not be surprised)

- `SitePermissionStore.h` split: the two enums moved to a new `SitePermissionType.h` so the pure
  mapping is unit-testable without sqlite3.
- New `SitePermissionMapping.h` — CEF-bit → Hodos-type mapping, mirrored bit constants
  `static_assert`ed against the real CEF values in `simple_handler.cpp`.
- New `WalletActivityTracker.h` — replaced a fixed 750 ms timer that raced the connect modal.
- `ProfileManager.cpp` — two safeguards: cannot delete the profile you are running on; profile ids
  are never reused (a deleted profile's directory outlives its registry entry, and the next profile
  was adopting ~200 MB of it).
- `cef-native/tests/site_permission_mapping_test.cpp` — 10 new tests. Whole suite: 295 pass.

---

## A6 — What I need back

1. Results for A3 items 1–4, with the **measured** artefact each time (log line or `Preferences`
   entry), not "looked fine".
2. A yes/no on A4's invisible-overlay check.
3. Anything where the macOS overlay lifecycle makes the park → bind → re-show sequence behave
   differently from the Windows trace in `PHASE_CONTRACT.md` §5.

⛔ **Not owed by you:** DPI cells #4/#6/#9 (`P0.9-A5`) — that is Windows Phase 1 (WS1) work and is
still unrun there too.
