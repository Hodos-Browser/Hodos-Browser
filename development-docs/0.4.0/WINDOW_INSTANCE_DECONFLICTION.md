# Window + Single-Instance Deconfliction (Task 1, second half)

> **Status:** PLAN — awaiting adversarial review, then implement. Windows-only code; macOS is compile-verify + one Info.plist nicety (flagged).
> **Driver:** Port deconfliction lets the dev build (`HODOS_DEV`, ports 31401/31402) and the installed build (31301/31302) run at once — but they then collide at the **Windows window-management layer**. Clicking the wallet button in one instance focuses the *other* instance's overlay. Ports were necessary but not sufficient.
> **Branch:** `0.4.0`.

---

## 0. Why this exists (verified root causes)

The app was built assuming **one instance ever runs**. Three Windows-layer assumptions break with two instances (dev+installed, OR — in production — two concurrent profiles, since `LaunchWithProfile` spawns a process per profile):

1. **`FindWindow` anti-pattern** (`simple_handler.cpp`). Four overlays are located by a **system-wide** `FindWindow(class, title)` with hardcoded names, so the call returns *whichever instance's window Windows finds first* → `SetForegroundWindow`/`DestroyWindow` hits the wrong instance. Sites (all inside `#ifdef _WIN32`):
   - `overlay_close`: `settings` (~4060), `backup` (~4072) — **(wallet ~4064 and brc100auth ~4076 ALREADY use the global — these two are the inconsistent leftovers)**
   - `overlay_hide` (brc100auth, ~4426)
   - `overlay_input`: `settings`/`wallet`/`backup` (~5177/5181/5185)
   - dead `if(false && …)` block (~5145) — convert for hygiene
   `FindWindow` is also a documented anti-pattern beyond multi-instance: it sends `WM_GETTEXT` to every top-level window (**hangs** if another UI thread is busy) and only searches the current desktop.
2. **Single-instance pipe keyed on profile only** (`SingleInstance.cpp:43`): `\\.\pipe\hodos-browser-{profileId}`. Dev and prod both use profile `Default` → **same pipe** → cross-instance forward/activation (the listener even calls `AllowSetForegroundWindow(ASFW_ANY)`).
3. **AUMID not dev-gated** (`cef_browser_shell.cpp:3835`): `HodosBrowser[.{profile}]`, only set when >1 profile. Dev and prod share it → taskbar buttons merge/confuse.

**The fix-pattern already exists in-tree:** `ProfileManager`'s `RegistryLock` is **already** scoped per data-root (`Local\HodosProfilesLock_<appDataPath>`) so dev/prod don't cross-lock. The window layer + pipe + AUMID just never got the same treatment.

**Best-practice basis (researched):** this is the **Chrome Canary side-by-side model** — separate user-data-dir singleton, separate AppUserModelID, distinct window identity. And "don't `FindWindow` your own windows; use the handle you already hold" is established Win32 guidance.

---

## 1. Verified facts

- All four globals exist: `cef_browser_shell.cpp:78-81` (`g_settings_overlay_hwnd`, `g_wallet_overlay_hwnd`, `g_backup_overlay_hwnd`, `g_brc100_auth_overlay_hwnd`).
- `simple_handler.cpp` already `extern`s and uses these globals directly elsewhere (e.g. `g_settings_overlay_hwnd` at ~2893 for destroy; `g_wallet_overlay_hwnd` at ~4064; `g_brc100_auth_overlay_hwnd` at ~4076). So the swap matches the **majority** pattern (8 of 12 overlays already use globals; only settings/wallet/backup/brc100auth ever `FindWindow`, and wallet/brc100auth only in some handlers).
- Globals are maintained in sync with window lifecycle (set on create, cleared on destroy) — same lifecycle the working `overlay_close` wallet path already relies on.
- `IsDevEnv()` (cef-native/include/core/PortConfig.h) is the dev/prod discriminator (== `HODOS_DEV=1` == data-root `HodosBrowserDev`).
- **macOS:** no `FindWindow` analog anywhere (`cef_browser_shell_mac.mm` + handler `.mm` greps clean). Mac overlay handlers already use `extern NSWindow* g_*_overlay_window` in-process. `SingleInstance.cpp` pipe is Windows-only (mac branch is stubs). AUMID is Windows-only. Mac single-instance leans on `flock` (per data-root profile dir — already separated) + LaunchServices.

---

## 2. The fix (Windows-only)

**Commit A — replace the `FindWindow` overlay lookups with the in-process HWND globals.** (~6 call sites in `simple_handler.cpp`.) `settings`→`g_settings_overlay_hwnd`, `wallet`→`g_wallet_overlay_hwnd`, `backup`→`g_backup_overlay_hwnd`, brc100auth→`g_brc100_auth_overlay_hwnd`. `extern HWND …;` at each site (matching the existing inline-extern style). Behavior is equivalent (global ⟺ the window this process created) but **process-local** — kills the cross-focus, the hang risk, and fixes production concurrent multi-profile. No mac change (mac arms already use globals).

**Commit B — scope single-instance + AUMID to dev/prod.**
- `SingleInstance.cpp::GetPipeName`: prefix with a dev marker so ALL pipe names (profile pipes + the `.picker` pipe) separate dev from prod: `"\\.\pipe\hodos-browser-" + (hodos::IsDevEnv() ? "dev-" : "") + profileId`.
- AUMID (`cef_browser_shell.cpp:3835`): when `IsDevEnv()`, append `.Dev` and set it **unconditionally** for dev (so the dev taskbar button is always distinct, even with one profile). Prod path unchanged.

> Window **class names** are left shared. Once no `FindWindow` remains, `RegisterClass` per-process with a shared name is harmless (class registration is per-process; the collision was only the system-wide `FindWindow`). Per-SKU class-name gating is an optional Canary-style nicety, deferred unless review wants it.

---

## 3. macOS considerations

| Concern | macOS status / action |
|---|---|
| `FindWindow` cross-focus | **N/A on mac** — overlay handlers already use `extern NSWindow* g_*_overlay_window` in-process. Commit A is `#ifdef _WIN32`. **Action: compile-verify only.** |
| Single-instance pipe | Windows-only (`SingleInstance.cpp` mac branch = stubs). No pipe collision on mac. Commit B is `#ifdef _WIN32`. |
| AUMID | Windows-only (taskbar). No mac equivalent in code. |
| Dev/prod coexistence | Handled on mac by **data-root separation** (`HodosBrowserDev`) + `flock` on a file under that root (already per-data-root). |
| Dock / LaunchServices identity | **REQUIRED for packaged `.app`s (review: SEV-MEDIUM, not a "nicety")** — macOS groups by **`CFBundleIdentifier`** (hardcoded `com.hodosbrowser.app` in Info.plist), and the mac single-instance is a **stub** (`SingleInstance.cpp` non-Windows branch) relying only on `flock`. With both builds packaged as `.app`s sharing the bundle ID, LaunchServices treats them as the **same app**: `applicationShouldHandleReopen:` → `[NSApp activateIgnoringOtherApps:YES]` (`cef_browser_shell_mac.mm` ~164/205) can raise the **wrong** instance — the mac equivalent of the Windows cross-focus. **Data-root + `flock` separation covers DB corruption, NOT activation identity.** The dev `.app` must carry a distinct `CFBundleIdentifier` (e.g. `com.hodosbrowser.app.dev`) + display name ("Hodos Browser (Dev)") — Info.plist is DevOps-owned (§4-D). Only moot when the mac dev build is run **directly** (`mac_build_run.sh`), not as a packaged `.app`. |

**Mac playbook:** add a brief item so the mac Claude (a) compile-verifies Commits A/B are inert/clean on mac, (b) confirms two mac instances (dev + installed) don't cross-focus or single-instance-collide, (c) surfaces the dev `CFBundleIdentifier` nicety to DevOps.

---

## 4. Test plan

- **Cross-focus (the reported bug):** run two CURRENT builds at once (or dev + a fresh build). In each, click wallet / open settings / open backup → the action stays in **that** instance's window; no switch to the other.
- **Production multi-profile:** open two profiles concurrently (installed build) → same — no cross-focus.
- **Single-instance:** launching a 2nd dev instance forwards to the 1st **dev** instance (not prod), and vice-versa; the `.picker` gate likewise separates.
- **Taskbar:** dev and prod show **distinct** taskbar buttons (AUMID).
- **No regression single-instance/within build:** a 2nd same-build same-profile launch still forwards to the running one.
- **macOS:** compile-verify; (when a Mac is available) two mac instances don't cross-focus.

---

## 6. Adversarial review outcome (2026-06-25) — GO-WITH-EDITS, LANDED

Three-agent review of this plan + code; direction sound, all edits folded into the implementation:
- **Commit A (`1aeaedd`):** FindWindow→global safe at the `IsWindow`-guarded `overlay_close` sites; review caught that globals are nulled by *IPC handlers, not WM_DESTROY*, so `overlay_hide`/`overlay_input` needed `IsWindow()` guards added + `g_brc100_auth_overlay_hwnd` nulled after `DestroyWindow`. Multi-window concern **refuted** (one overlay, parented to the primary window). FindWindow inventory confirmed **complete** (no `EnumWindows`/`GetClassName`/`RegisterWindowMessage`/`GlobalAddAtom`/`HWND_BROADCAST` anywhere).
- **Commit B (`f9408fd`):** caught (a) the missing `PortConfig.h` include in `SingleInstance.cpp`; (b) the AUMID `>1 profile` gate → single-profile dev/prod never separated → now set the dev AUMID **unconditionally**; (c) a **missed shared resource** — DevTools `remote_debugging_port` 9222 was ungated → now `+100` for dev. Pipe discriminator uses the `.`-invalid-char trick (not the brittle `"dev-"` concat). Confirmed already-separated: `RegistryLock`, CEF `SingletonLock`, anonymous job objects, per-process `WNDCLASS`.
- **macOS:** the FindWindow claim holds (mac uses in-process `NSWindow*` globals), BUT the mac `CFBundleIdentifier`/LaunchServices cross-activation is **required for packaged `.app`s, not optional** (§3 updated). Commits A/B are `#ifdef _WIN32` → inert on mac (compile-verify only).

## 5. Risk

Fragile window-lifecycle/startup code (CLAUDE.md invariant #8). Mitigations: Commit A is a like-for-like swap to a pattern already used by the majority of overlays (low risk); Commit B mirrors the existing `RegistryLock` data-root scoping. Small commits, build per step, adversarial review before coding.
