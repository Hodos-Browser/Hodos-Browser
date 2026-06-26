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
  "$APP/Contents/Resources/hodos-wallet"
  "$APP/Contents/Resources/hodos-adblock"
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

---

## Task 4 — The real-update gate (Pillar A) ⭐ THE KEYSTONE

CI runs on macOS 26 and **structurally cannot** reproduce a sub-floor loader rejection. So the real update must be exercised on a real machine below the runner OS **before `promote --latest`**:

1. Install the **currently-live** build (beta.16) on the owner's **macOS 15.7.5** machine.
2. Build the fixed version through the pinned-runner pipeline (Tasks 2–3 green).
3. Stage it as a draft release + appcast (the pipeline is draft-first — nothing public yet).
4. On the 15.7.5 machine, trigger Sparkle update (Settings → About → Check for updates, or wait for the poll). Confirm: **download → EdDSA verify → install → RELAUNCH into the new version**, with no "requires macOS X" rejection.
5. Confirm `notarytool`/`stapler` passed on the macos-15 SDK (Task 5).
6. **Only then** promote the draft to `--latest`.

> This manual step is mandatory until automated. Document the result (OS version, from→to versions, relaunch confirmed) in the release notes / PR. Future work: a scripted N−1→N update check for the Windows side and, where possible, a VM-based mac check — but the sub-floor loader case needs a real old-OS machine.

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
- [ ] **Real-update gate: beta.16 → fixed build auto-updates + relaunches on real macOS 15.7.5, before promote.**
- [ ] Nothing in `Info.plist` changed except `LSMinimumSystemVersion` (SU* keys intact).
- [ ] (Task 6 only if scheduled) release-config build still emits `HodosBrowser.app`.

## What this does NOT touch
Browser-core, the gold-pill payment IPC chain, wallet signing/DB, or the HODOS_DEV dev/prod data split. If a step seems to require touching any of those, STOP and surface it.
