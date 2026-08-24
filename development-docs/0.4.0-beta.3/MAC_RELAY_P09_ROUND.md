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
