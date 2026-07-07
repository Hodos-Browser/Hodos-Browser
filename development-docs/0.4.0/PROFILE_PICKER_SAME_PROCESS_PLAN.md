# Profile Picker → Same-Process (Chrome model) — Scoping Plan

**Status:** SCOPING ONLY — do NOT implement in the current bug-fix pass. The owner
explicitly wants this done in a **dedicated fresh session** with deep-dives, danger
analysis, and adversarial reviews at each step, because it touches the **fragile CEF/Rust
startup handoff** (CLAUDE.md invariant #8: do not change the message loop, browser-creation
timing, or render-process handlers without asking).

This document is the tight starting point for that session. It captures the decision, the
research basis, the current-code anchors, the real danger areas, and the open questions to
resolve BEFORE any code.

---

## Decision (owner, 2026-07-07)

Adopt **Option 1 — same-process, picker-first startup** (the Chrome model). When the picker
is shown and the user selects a profile, open that profile **in the already-running process**
instead of spawning a fresh `HodosBrowser.exe --profile=X` and cold-booting. This eliminates
the multi-second D1 handoff gap (picker closes → blank → new process cold-boots).

Rejected: Option 2 (pre-warm hidden profile) and Option 3 (transition animation only) — see
the research report. Option 1 is the architecturally-correct, no-cold-boot fix.

> Note: **D2 (small modal picker) is dropped** — the perf win comes from same-process, not
> from the window style. Keep the current full-window picker UI; only change the process model.

## Why (research basis)

Chrome/Edge/Brave are single-process-per-instance with **`Profile` as an in-process object**;
the picker runs in a minimal "system profile" and, on selection, constructs the chosen
`Profile` + `BrowserWindow` in the same process, then closes the picker window. No exe spawn →
no cold-boot. Firefox (separate process per profile) has the same gap we have today. Full
report: the picker-UX research deliverable (browser comparison + Chromium source refs) from
the 2026-07-07 session.

---

## Current two-process flow (verified code anchors — RE-VERIFY at session start)

| Step | Location |
|------|----------|
| Startup resolves profile + picker mode | `cef_browser_shell.cpp` ~4383–4401 (`ParseProfileArgument` → `ResolveStartup` → `g_picker_mode`) |
| Picker URL loaded in header browser | `simple_app.cpp` ~204–206 (`/profile-picker?mode=window` when `g_picker_mode`) |
| Selection → spawn new exe | `simple_handler.cpp` ~3229–3260 (`profiles_switch` → `ProfileManager::LaunchWithProfile` → `PostMessage(g_hwnd, WM_CLOSE)`) |
| New process launch | `ProfileManager::LaunchWithProfile` — `CreateProcessW("HodosBrowser.exe --profile=\"ID\"")` |
| Picker-decision inputs (now logged) | `cef_browser_shell.cpp` startup log — `profileCount / pickerSettingOn / defaultId / showPicker` (added 2026-07-07 for C3) |
| Wallet launch (per-profile) | `LaunchWalletProcess()` at startup |

---

## ⚠️ Danger areas (the deep-dive must resolve each BEFORE coding)

1. **CEF cache path is process-global and set BEFORE `CefInitialize`.** This is the linchpin.
   The current model gives each profile its own on-disk cache/storage by launching a **new
   process** with `--profile=X` (so `CefSettings.root_cache_path`/`cache_path` differ per
   process). In a same-process model you **cannot** re-point the global cache after
   `CefInitialize`. Chrome's answer is **`CefRequestContext` per profile** with its own
   `CefRequestContextSettings.cache_path`. **Open question:** can we create a per-profile
   `CefRequestContext` after `CefInitialize` and give every browser/overlay for that profile
   that context? What breaks (cookies, storage, service workers, our adblock/cookie
   interception, wallet-endpoint routing) if the profile's storage is a request-context cache
   path rather than the global one? This is likely the largest single work item and the
   biggest risk — scope it first.

2. **Render-process `--profile` propagation.** History isolation currently rides on the render
   subprocess receiving `--profile` (see the profile-system landing notes: render `--profile`
   propagation + per-profile history). In same-process, all render subprocesses share one
   command line — profile identity must travel another way (request-context, or an IPC after
   context creation). Audit every place that reads the profile from the command line.

3. **Rust wallet handoff (separate process).** The wallet (Actix on 31301) is launched per
   profile and reads its DB from the profile dir. In same-process the profile is chosen
   *after* startup, so wallet launch must be **deferred** until selection, OR the wallet must
   accept a "set active profile / DB path" call after start. Decide: defer `LaunchWalletProcess`
   to post-selection (simplest, but adds latency after pick) vs. a wallet profile-switch
   endpoint (more moving parts, keeps the wallet warm). The adblock engine (31302) is
   profile-agnostic — confirm.

4. **CEF lifecycle / browser-creation timing (inv #8).** The picker currently IS the header
   browser at a special URL. Same-process means: show a minimal picker window first, then on
   selection create the real header + tab browsers for the chosen profile — a different
   creation sequence than today. This is exactly the fragile zone the invariant guards. Every
   step needs a design pass + adversarial review.

5. **Silent-update apply gate interaction.** `MaybeApplyStagedUpdate`'s sole-instance gate
   already special-cases the transient picker (`SU_CountSelfBrowsers` + the bounded picker
   wait, `AUTOUPDATE_PICKER_GATE_DESIGN.md`). A same-process picker (no separate picker exe /
   no self-spawn) changes that accounting — re-derive the gate. Also the `update.lock`
   honor-at-launch and the instance-presence mutex assume the current process model.

6. **Multi-window with different profiles.** Chrome allows window A = profile 1, window B =
   profile 2 in one process (per-window request context). Decide whether we support that now
   or keep one-profile-per-process-lifetime (simpler, but then "open another profile" still
   needs a new window with a new request context). `WindowManager`/`TabManager`/`ProfileManager`
   singletons assume a single current profile today.

7. **C3 is SEPARATE from this refactor.** The "picker shows once then default forever" bug on
   the Win10 machine is NOT the cold-boot gap — the code says the picker should keep showing
   (`SetShowPickerOnStartup(false)` is never called). Diagnose C3 first via the startup
   `pickerDecision` log now emitted (profileCount / pickerSettingOn / defaultId). Do not
   conflate it with this refactor.

---

## Open questions to answer before writing code

- Can per-profile `CefRequestContext` (distinct `cache_path`) fully isolate storage the way
  separate processes do today? Prototype it in isolation first.
- Wallet: defer-launch vs. profile-switch endpoint? (Owner decision.)
- Do we need multi-profile-per-process (different windows, different profiles), or is
  one-active-profile-per-run enough for 0.4.0?
- Fallback: if same-process proves too risky for the CEF cache-path reason, is Option 2
  (pre-warm the last-used profile hidden) the acceptable middle ground?

## Suggested phased approach (each phase = its own design + adversarial review)

1. **Spike:** per-profile `CefRequestContext` with a separate cache path — prove storage
   isolation (cookies, localStorage, history, our interception) works without a new process.
   Go/no-go on the whole refactor hinges here.
2. **Wallet handoff:** defer/relaunch design + smoke on a funded wallet.
3. **Picker-first startup:** minimal picker window → on selection, build header/tabs for the
   chosen profile in-process; retire the `--profile` self-spawn path (keep it as a fallback).
4. **Update-gate + mutex + lock** re-derivation for the new process model.
5. **Full smoke** across the Testing Standards basket + the funded-wallet update path.

## Non-goals for this refactor

- No picker UI redesign (D2 dropped).
- No change to the wallet DB schema or crypto (invariants #2/#3).
- Keep the existing profile persistence (`profiles.json`, v2 migration) as-is unless the
  spike forces a change.
