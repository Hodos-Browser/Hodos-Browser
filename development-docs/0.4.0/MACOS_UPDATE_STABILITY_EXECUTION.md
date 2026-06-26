# macOS Update-Stability Execution Brief (branch `0.4.0`)

> **You are a fresh Claude agent on a Mac.** This is a self-contained brief for the **update-stability** Mac work: fix the macOS minimum-version regression (the "requires macOS 26" auto-update break) and stand up the **real-update verification gate**. It is the Mac half of `development-docs/0.4.0/POST_BETA16_PLAN.md` (read that for full context; this brief is the executable distillation). Companion: `development-docs/MACOS_CATCHUP_PLAYBOOK.md` (parity work + how to build on Mac — §3.3 is the CEF-framework prerequisite you must satisfy before anything here).

## Trust posture (read first)
- **Line numbers drift. GREP for the anchor string/symbol, then read around the hit — never trust a bare `file:line`.** Every anchor below is given as a string to grep.
- **This touches the load-bearing release pipeline (`release.yml`).** Per the owner principle, changes here must be **validated by an actual update before they ship** (Task 4). Do not promote a release that hasn't passed the real-update gate.
- **Land on `0.4.0` directly; commit only when the owner asks; `git pull --rebase origin 0.4.0` before any commit.** `release.yml` is shared with the Windows side — if both machines edit it, coordinate (the owner sequences this).

## The principle this serves
> Auto-update must NEVER break such that a user has to uninstall + reinstall. We keep regressing it because we verify *proxies* (asset present, signature non-empty) instead of the *real* update, and because floating runners drift under us. The fix is two pillars: **(A) verify the real N−1→N update+relaunch before every promote**, and **(B) kill silent drift** (pin runners, explicit deployment target + minos guard). See `feedback_update_stability_principle.md` and `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` → "macOS Minimum Deployment Version."

---

## Task 1 — Measure the real macOS floor (PREREQUISITE; do this first)

The published minimum must be `max(11.0, the CEF framework's actual minos)`. CEF 136 dropped macOS 10.15 (Catalina) → floor is **at least 11.0**, but the prebuilt framework may need higher. Measure it:

```bash
FW="../cef-binaries/Release/Chromium Embedded Framework.framework/Chromium Embedded Framework"
vtool -show-build "$FW" | grep -A4 -i 'BUILD_VERSION\|minos'
# or:  otool -l "$FW" | grep -A4 LC_BUILD_VERSION
```
- Record the `minos` value. **`FLOOR = max(11.0, that value)`.** Use `FLOOR` everywhere below.
- If the framework reports e.g. 11.5 or 12.0, the floor is that — an under-stamped app passes the loader, then dyld fails to load the higher-minos framework → launch crash. Do not hard-code 11.0 if the framework needs more.
- Report `FLOOR` back to the owner before editing config (it's the one number this whole fix turns on).

---

## Task 2 — Apply the deployment target for real (Pillar B)

The root cause: `CMakeLists.txt` sets `CMAKE_OSX_DEPLOYMENT_TARGET` via `set(... CACHE ...)` **without `FORCE`, after `project()`** → silent no-op → clang stamps `minos` from the **runner's SDK** (macOS 26 on the floated `macos-latest`). Fix all of:

**2a. Pin the runner** — `release.yml`, grep `runs-on: macos-latest` (the macOS build job, ~`:313`):
```yaml
runs-on: macos-15      # was macos-latest (floated to macOS 26 Tahoe → stamped minos=26). Pinned. Never use macos-latest.
```

**2b. Pass the deployment target on the configure command line** — `release.yml`, grep `cmake -S . -B build -DCMAKE_BUILD_TYPE=Release` (the mac configure, ~`:441`). Add `-DCMAKE_OSX_DEPLOYMENT_TARGET=<FLOOR>`. A command-line `-D` populates the cache **before** `project()` — the only reliable path:
```yaml
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_DEPLOYMENT_TARGET=<FLOOR> -DAPP_VERSION="..." -DAPP_BUILD_NUMBER="..."
```

**2c. Set a job-level env** so the CEF wrapper build, cargo (`ring`/`openssl-sys`), and any sub-cmake inherit ONE floor. In the macOS build job's `env:` block:
```yaml
env:
  MACOSX_DEPLOYMENT_TARGET: "<FLOOR>"
```

**2d. Make `CMakeLists.txt` self-defending** — grep `CMAKE_OSX_DEPLOYMENT_TARGET` (~`:72`). Add `FORCE` and a non-10.15 default so a local build can't silently regress:
```cmake
set(CMAKE_OSX_DEPLOYMENT_TARGET "11.0" CACHE STRING "Minimum macOS deployment version" FORCE)
```
(The command-line `-D` in 2b overrides this in CI; the `FORCE`+11.0 default protects local `mac_build_run.sh` builds.)

**2e. Fix the dev launcher** — grep `DEPLOYMENT` / the cmake configure in `cef-native/mac_build_run.sh` (~`:19`). Add `-DCMAKE_OSX_DEPLOYMENT_TARGET=<FLOOR>` to its configure line so dev builds match CI.

**2f. Sync the plists** — grep `LSMinimumSystemVersion` in `cef-native/Info.plist` (~`:23-24`) and `cef-native/mac/helper-Info.plist.in` (~`:21-22`). Set the value to `<FLOOR>`. (These are loader metadata; the binary `minos` is authoritative, but keep them honest and identical.)
> `Info.plist`/`helper-Info.plist.in` are DevOps-owned and carry the `SU*` auto-update keys — change ONLY the `LSMinimumSystemVersion` value; preserve `SUFeedURL`/`SUPublicEDKey`/no-silent-update posture exactly.

**Do NOT** add `sparkle:minimumSystemVersion` to `generate-appcast.py` — it currently emits none (verified), and the binary `minos` is the real gate. Leave the appcast as-is unless the owner asks.

---

## Task 3 — The minos CI guard (Pillar B; fail-closed)

After the macOS build, BEFORE signing/notarizing/promoting, assert every shipped Mach-O's `minos` is **≥ the CEF framework's minos** (an inequality — the framework is the hard floor). Add a step in the macOS build job after the build, before "Sign & Notarize":

```bash
set -euo pipefail
read_minos() { vtool -show-build "$1" | awk '/minos/{print $2; exit}'; }
APP="build/.../HodosBrowser.app"   # grep the real bundle path in mac_build_run.sh / CMake POST_BUILD
FW="$APP/Contents/Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"
FW_MINOS=$(read_minos "$FW")
# Main exe + all 5 helper apps + both Rust binaries:
TARGETS=(
  "$APP/Contents/MacOS/HodosBrowser"
  "$APP"/Contents/Frameworks/*\ Helper*.app/Contents/MacOS/*
  "$APP/Contents/MacOS/hodos-wallet"     # NOTE: MacOS/, NOT Resources/ — the assemble step copies them here
  "$APP/Contents/MacOS/hodos-adblock"
)
fail=0
for t in "${TARGETS[@]}"; do
  m=$(read_minos "$t") || { echo "::error::no minos for $t"; fail=1; continue; }
  awk -v a="$m" -v b="$FW_MINOS" 'BEGIN{split(a,x,".");split(b,y,"."); if (x[1]<y[1]||(x[1]==y[1]&&x[2]<y[2])) exit 1}' \
    || { echo "::error::$t minos=$m < framework $FW_MINOS"; fail=1; }
done
[ "$fail" = 0 ] || { echo "::error::minos guard FAILED — would brick auto-update on sub-$FW_MINOS macOS"; exit 1; }
echo "minos guard OK (all >= framework $FW_MINOS)"
```
- **grep the real bundle/helper paths** in `mac_build_run.sh` and the CMake `POST_BUILD` block — do not trust the placeholders above.
- This guard, alone, would have caught the beta.16 regression.

> **🐞 KNOWN DEFECT in the SHIPPED guard (beta.17, commit `397b237` → `release.yml`): the two Rust-binary paths are wrong.** The deployed guard looks for `hodos-wallet`/`hodos-adblock` under `Contents/Resources/`, but the **Assemble app bundle** step copies them to `Contents/MacOS/` (`cp ... "$APP/MacOS/"`) — and the **Sign step correctly signs them at `Contents/MacOS/`**. The deployed guard also uses `[ -f "$t" ] || continue`, so the wrong paths **silently skip both Rust binaries with no error**. In CI run `28253107086` the guard printed `OK:` for the main exe + all 5 helpers but **nothing for the two Rust binaries**. Practical risk is low (job env `MACOSX_DEPLOYMENT_TARGET=11.0` → cargo/rustc stamp them 11.0), but it is **unproven by CI**. **FIX in beta.18:** change the two Rust paths in `release.yml` `Contents/Resources/` → `Contents/MacOS/`. Until then, **Task 4 Part C is the manual backstop** that verifies them directly.

---

## Task 4 — The real-update gate (Pillar A) ⭐ THE KEYSTONE — **RUN THIS NOW**

> **Status (2026-06-26):** The fixed build **`v0.3.0-beta.17`** is already **built, signed, and LIVE** (public + marked Latest; Tasks 2–3 + notarization all green in CI run `28253107086`). The pipeline auto-promoted it. **This gate is now a retroactive must-pass: it gates cutting public `0.4.0`, NOT beta.17.** Until this is GREEN on real sub-26 hardware, **do not cut `0.4.0`** — beta.17 is the canary.

**This is a runbook for the Mac Claude. Run it on the macOS 15.7.5 machine, then report results in the format at the bottom.** CI runs on macOS 26 and **structurally cannot** reproduce a sub-floor loader rejection, so this must be exercised on a real sub-floor machine.

### ⚠️ Source version: use **beta.15**, NOT beta.16 (this is a correction)

The earlier draft of this task said "install beta.16 → update to the fix." **That is impossible.** beta.16 is the *broken* build — its binaries are stamped `minos = macOS 26`, so **beta.16 will not launch on macOS 15** (dyld rejects it). Sparkle runs *inside* the running app, so a non-launching app cannot auto-update to anything. Therefore the update **source must be beta.15** — the last good build that launches on macOS 15 and can run the updater. This is also the *realistic* path: every macOS < 26 user is currently stranded on beta.15 (they could never successfully move to beta.16), so beta.15 → beta.17 is the exact rescue path beta.17 must deliver.

DMG asset name pattern: `HodosBrowser-0.3.0-beta.N.dmg`. Installed app: `/Applications/HodosBrowser.app`. User data lives in `~/Library/Application Support/HodosBrowser` and is preserved across installs.

### Part A — Auto-update gate: beta.15 → beta.17 (the real test)

1. Quit Hodos Browser if running. If `/Applications/HodosBrowser.app` exists, move it to Trash. **Do NOT delete `~/Library/Application Support/HodosBrowser`** (keep your data; auto-update preserves it).
2. Download beta.15:
   ```bash
   curl -L -o ~/Downloads/HodosBrowser-beta15.dmg \
     https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.3.0-beta.15/HodosBrowser-0.3.0-beta.15.dmg
   ```
3. Open the DMG, drag **HodosBrowser** to Applications, eject the DMG.
4. Launch `/Applications/HodosBrowser.app`. **It must open.** (If beta.15 itself won't launch on 15.7.5, STOP and report — that means even beta.15's minos is too high and the floor analysis is wrong.)
5. In the app, open **Settings → About**. Confirm it reads **Hodos Browser 0.3.0-beta.15**.
6. On that same screen, click **Check for updates** (don't wait for the 24h auto-poll).
7. Sparkle should show **"Update available: 0.3.0-beta.17."** Click **Install / Update**.
8. It downloads → verifies EdDSA → installs → relaunches. **First auto-update only:** macOS may show a one-time **System Settings → Privacy & Security → App Management** prompt — approve it (then re-trigger if it aborted the first attempt).
9. **PASS/FAIL ASSERTION:** the app **relaunches into beta.17** with **no "requires macOS X" error and no crash**. Confirm **Settings → About now reads 0.3.0-beta.17**.
   - Relaunches into beta.17 → **Part A GREEN.**
   - "Requires macOS 26" / fails to relaunch / crash on launch → **Part A RED — stop, do not cut 0.4.0, report immediately.**

### Part B — Fresh-install sanity (covers bricked-beta.16 users who must manually reinstall)

1. Quit the app; move `/Applications/HodosBrowser.app` to Trash (keep data).
2. Download + install beta.17 directly:
   ```bash
   curl -L -o ~/Downloads/HodosBrowser-beta17.dmg \
     https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.3.0-beta.17/HodosBrowser-0.3.0-beta.17.dmg
   ```
3. Open DMG → drag to Applications → launch on macOS 15.7.5. It must open cleanly; **About** reads 0.3.0-beta.17.

### Part C — Directly verify the binaries' minos (closes a CI guard hole)

The CI minos guard **silently skipped both Rust binaries** (it checked `Contents/Resources/`, but they live in `Contents/MacOS/`), so their `minos` was never machine-verified in CI. Verify them here on real hardware (CI can't; a Windows session can't). With **beta.17** installed:

```bash
for b in HodosBrowser hodos-wallet hodos-adblock; do
  printf '%s: ' "$b"
  vtool -show-build "/Applications/HodosBrowser.app/Contents/MacOS/$b" | awk '/minos/{print $2}'
done
```
Expected: each prints `11.0` (anything ≤ 15 is fine for a 15.7.5 machine; **`26` is the failure** — report it). Then **open the wallet panel in beta.17 and confirm the balance loads** — that's functional proof `hodos-wallet` actually launched (a 26-stamped wallet backend would be dead on macOS 15).

### Report back to the owner (paste this filled in)
```
Real-update gate — beta.17 canary
  Machine OS:            macOS __.__.__  (confirm < 26)
  Part A (beta.15→17):   PASS / FAIL   — relaunched into: 0.3.0-beta.__  ; error (if any): ____
  Part B (fresh beta.17 launch): PASS / FAIL
  Part C minos:          HodosBrowser=__  hodos-wallet=__  hodos-adblock=__   (expect 11.0; 26=fail)
  Part C wallet balance loaded: YES / NO
  notarytool/stapler (Task 5, from CI run 28253107086): PASS  (already confirmed green by Windows)
  VERDICT: GREEN (safe to cut 0.4.0)  /  RED (hold)
```

> This manual gate is mandatory until automated. Future work: a scripted N−1→N update check for Windows and, where possible, a VM-based mac check — but the sub-floor loader case needs a real old-OS machine.

---

## Task 5 — Confirm notarization survives the runner pin

Pinning `macos-15` changes the signing toolchain, not just the compiler. On the first pinned release, confirm `xcrun notarytool submit ... --wait` succeeds and `xcrun stapler staple` passes on both the `.app` and the `.dmg` (grep `notarytool` / `stapler` in `release.yml` and `scripts/`). If notarization regresses, that's a runner-SDK interaction — surface to the owner before promoting.

---

## Task 6 — (Separate, lower-priority phase) Durable dev rename, macOS portion

This is the **#4 decision** — its own focused phase, NOT part of the min-version fix. Only do it when the owner schedules it.
- Goal: dev build emits `HodosBrowserDev.app` (HODOS_DEV-gated) so launching dev can never collide with the installed app. The root cause of the "dev launch closes installed browser" bug is the launcher `pkill -9 HodosBrowser` (grep in `mac_build_run.sh`) matching BOTH apps.
- macOS scope (the heavy part): the rename ripples through the app bundle name, `CFBundleExecutable`, the 5 CEF helper bundles, `CFBundleIdentifier` (already flagged in the catchup playbook §C14 — give the dev `.app` `com.hodosbrowser.app.dev` + "Hodos Browser (Dev)"), and ad-hoc signing.
- **Hard guardrail:** the RELEASE `.app` must stay **exactly** `HodosBrowser.app` (notarization, appcast, auto-update key on it). The rename must be strictly HODOS_DEV/build-config-gated, with a verification that a release-config build still produces `HodosBrowser.app`.
- Interim before the phase: scope the `pkill` to the dev bundle path (`pgrep -f` against the dev `.app`) so dev runs don't kill the installed app.

---

## Validation checklist (report back to owner)
- [ ] `FLOOR` measured from the CEF framework (`vtool`), value reported.
- [ ] Runner pinned `macos-15`; `-DCMAKE_OSX_DEPLOYMENT_TARGET=<FLOOR>` on configure + job env; CMake `FORCE`; `mac_build_run.sh` + both plists updated.
- [ ] Built binary `minos` == `<FLOOR>` (spot-check `vtool` on `HodosBrowser` exe).
- [ ] minos guard step added and PASSES (and FAILS on a deliberately wrong floor — prove it bites).
- [ ] notarytool/stapler pass on macos-15.
- [ ] **Real-update gate (Task 4): beta.15 → beta.17 auto-updates + relaunches on real macOS 15.7.5 (Part A); fresh beta.17 launches (Part B); `vtool` minos == FLOOR on main exe + both Rust binaries + wallet balance loads (Part C). Must be GREEN before cutting public 0.4.0.** (Source is beta.15, NOT beta.16 — beta.16 won't launch on sub-26.)
- [ ] Nothing in `Info.plist` changed except `LSMinimumSystemVersion` (SU* keys intact).
- [ ] (Task 6 only if scheduled) release-config build still emits `HodosBrowser.app`.

## What this does NOT touch
Browser-core, the gold-pill payment IPC chain, wallet signing/DB, or the HODOS_DEV dev/prod data split. If a step seems to require touching any of those, STOP and surface it.
