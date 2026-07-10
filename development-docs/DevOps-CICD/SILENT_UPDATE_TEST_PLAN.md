# Windows Silent Auto-Update — Test Plan (path to the flag flip)

> ## ✅ COMPLETE — silent update SHIPPED + LIVE (2026-07-09)
>
> All four stages are DONE. The flag is **ON** and silent auto-update is proven live on real
> hardware: **Windows beta.25 → beta.26** applied silently through the two-process profile
> picker; **macOS beta.21 → beta.22** applied silently via Sparkle. Latest public release =
> **v0.3.0-beta.26 (LATEST)**.
>
> **The final blocker was the profile picker.** Stage 2 fixed the sole-instance gate to WAIT
> for the picker to die, but a second root cause surfaced on real hardware: the picker-spawned
> `--profile=` child still tore down the ~8-process CEF tree while `MaybeApplyStagedUpdate`
> ran, so the sole-instance count never reached 1 and the apply deferred forever. Fixed by the
> **exact picker-exit wait** (commit `ae5beb6`, shipped in beta.26): the picker-spawned child
> waits for the picker process to fully exit before the sole-instance check, so the apply sees
> count = 1 and proceeds. Proven by real hardware beta.25 → beta.26.
>
> The rest of the silent saga also landed: signer-continuity CN gate (beta.23), external
> rollback-supervisor, promote.yml redirect-verify retry hardening, and BUILD_AND_RELEASE
> tag-derived version + draft → manual-promote gate.
>
> **What's still living below:** the reusable rig table (Stage 1) and the rollback/safety
> checklist are the **standing regression procedure** — run them before every future
> update-touching change (e.g. the Chromium/CEF rebuild sprint). The Stage 2–4 narrative is
> kept as the historical de-risking record.

**Status:** ✅ COMPLETE — mechanism shipped, flag ON, silent live on Windows + macOS. This
doc is retained as the staged de-risking record + living regression procedure. Design +
supervisor detail: [`AUTOUPDATE_6B_SUPERVISOR_DESIGN.md`](./AUTOUPDATE_6B_SUPERVISOR_DESIGN.md).

## The principle (why staged, not "test locally then flip")

Every prior "this should just work" in update-land has bitten this project
([[feedback_update_stability_principle]]). A funded user bricked by a bad update is the
one unrecoverable case. So we de-risked in stages, each cheaper-to-catch than the next
(**all four DONE — see the COMPLETE banner above; silent is live**):

```
  1. Local unit + fault-injection rigs   (DONE, green)   — logic + rollback correctness
  2. Local REAL-BUILD test  (dev wallet)  (DONE, green)  — real bootstrap/Inno/CEF/wallet, test-signed
  3. Production-signed test (trivial prod wallet) (DONE) — real Marston Authenticode + prod EdDSA
  4. Soak, then flip the default for users (DONE — ON)   — flipped; live beta.25→26 (Win) / beta.21→22 (mac)
```

A green stage-1 rig proves the *logic*. It does NOT prove a real signed build works on a
real machine — that's stages 2–3. **Do not skip to a funded wallet.**

## Stage 2 RESULTS — DONE + GREEN (2026-07-03)

Ran the local real-build test end-to-end on a 3-profile dev wallet (real bootstrap + real
CEF health-probe + real dev wallet; fake copy-installer standing in for Inno; test-signed).
**Both legs passed:** happy path COMMITs to 0.4.1 with the wallet intact; a health-failing
build ROLLS BACK to 0.4.0 (updates paused, high-water preserved, wallet byte-intact), and
the deferred-then-`--resume` recovery completed the rollback correctly.

It also found **3 real production bugs the isolated rigs could not** — all now fixed +
re-verified on real builds:
1. **Health-probe used `--profile ` (space)**, but the browser only parses `--profile=`
   (equals) → probe fell into the picker → never healthy → **false rollback of a GOOD update**
   for multi-profile users. Fixed (`transaction.cpp` uses `--profile=`).
2. **Browser wrote `debug_output.log` into `{app}`** (relative path + shortcut's working dir
   is `{app}`); the open log made the `{app}` backup fail (`cannot manifest {app}`) → **the
   update could NEVER apply for installed users**. Fixed (`AppPaths::GetLogDir()` → absolute
   `%APPDATA%\<ns>\logs`, out of `{app}`).
3. **Sole-instance gate counted the transient profile picker** (+ its CEF subprocess tree)
   → every picker-shown launch deferred forever. Fixed with a bounded WAIT for the picker to
   die (a lock-probe alternative was adversarially REJECTED as unsafe — it would kill the
   shared wallet under a concurrently-booting browser). See
   [`AUTOUPDATE_PICKER_GATE_DESIGN.md`](./AUTOUPDATE_PICKER_GATE_DESIGN.md).

Deferred (non-blocking) follow-ups: **Fix D** (chronic-deferral → degrade-to-notify, for the
always-open-browser / slow-picker tail — must ship WITH its notify-surface wiring, not a half
counter); an **"Updating…" progress splash** (helper-owned native window during the apply, to
remove the confusing several-second pause on an apply-boot — Windows-only, macOS Sparkle shows
its own) + a post-update "updated to vX" toast; and CEF's own `debug.log` still lands in
`{app}` (harmless — closed at backup time — but move it for the "install dir immutable"
principle).

---

## Stage 1 — local rigs (DONE, green)

All run in an isolated temp sandbox / dev namespace; none touch a real wallet.

| Rig | What it proves | Run |
|-----|----------------|-----|
| `scripts/test-update-feed.ps1` | staging: appcast + installer + **signed manifest** fetch/verify | `pwsh -File scripts/test-update-feed.ps1` |
| `scripts/test-apply-rollback.ps1` | **rollback restore**: old build + old money DB restored, stale new `-wal`/`-shm` deleted, wallet intact | `pwsh -File scripts/test-apply-rollback.ps1` |
| `scripts/test-apply-forward.ps1` | **forward apply**: install→integrity→health→commit, + OS-block & integrity rollback triggers | needs a rig build (below); `pwsh -File scripts/test-apply-forward.ps1` |

The apply rigs need a **rig build** of the helper (test seams compiled in — off in production):
```
cd cef-native
cmake -B build -DHODOS_SILENT_AUTOUPDATE=ON -DHODOS_UPDATE_TEST_SEAM=ON -A x64 \
  -DCMAKE_TOOLCHAIN_FILE=<vcpkg>/scripts/buildsystems/vcpkg.cmake
cmake --build build --config Release --target hodos-update-helper HodosBrowserShell
```
> The rig scripts ABORT if a real Hodos wallet is listening on 31301/31302 — close Hodos first.

---

## Stage 2 — local REAL-BUILD test (dev wallet, no prod keys, no push)

This is the important next step: it exercises everything the rigs stub out — the **real
bootstrap** (`MaybeApplyStagedUpdate`), the **real CEF browser** + health probe (build-number
+ live wallet/adblock `/health`), the inherited-handle spawn, and a **real (dev) wallet +
migration** — using the test seams to stand in for the production signing keys (which are CI
secrets). It runs in the `HodosBrowserDev` namespace, so your real wallet is untouched. Use a
throwaway dev wallet (empty or a few cents). (Inno itself is deferred to Stage 3's signed
build; here a tiny fake copy-installer stands in, so the real Inno step is the only thing not
covered.)

> **Automated by `scripts/setup-real-apply-test.ps1`** — it builds N (0.4.0) + N+1 (0.4.1),
> installs N into the dev app dir, pre-stages a signed N+1, and writes you a launch + verify
> script. Happy: `pwsh -File scripts/setup-real-apply-test.ps1`. Rollback:
> `pwsh -File scripts/setup-real-apply-test.ps1 -Break`. The manual steps below are the
> underlying mechanics (kept for reference / debugging).

**Prep once:** a throwaway dev wallet, its recovery phrase written down. A rig build (above).
A throwaway Ed25519 key (`openssl genpkey -algorithm ed25519 -out rig.pem`) + its raw-32
base64 pubkey (see `test-update-feed.ps1` steps 1 for the extraction).

**Build "N" (the installed build) and "N+1" (the update):**
```
# N+1 must have a HIGHER build number than N (anti-rollback). APP_BUILD_NUMBER scheme:
#   MAJOR*1e6 + MINOR*1e4 + PATCH*100 + (beta N | 99 final). 0.4.0 -> 40099 ; 0.4.1 -> 40199.
cmake -B build -DHODOS_SILENT_AUTOUPDATE=ON -DHODOS_UPDATE_TEST_SEAM=ON \
      -DAPP_VERSION=0.4.0 -DAPP_BUILD_NUMBER=40099 ...   # build N, install it (Inno) to the dev {app}
cmake -B build ... -DAPP_VERSION=0.4.1 -DAPP_BUILD_NUMBER=40199 ...  # build N+1
# Make a real Inno installer for N+1 (scripts/build-release.ps1 -Version 0.4.1 + ISCC),
# then SIGN it + the appcast + the manifest with your throwaway key:
python scripts/generate-appcast.py --version 0.4.1 --build-number 40199 \
    --windows-url http://127.0.0.1:8099/HodosBrowser-0.4.1-setup.exe --windows-size <bytes> \
    --windows-signature DUMMYDSA --windows-ed-signature <ed-over-installer-bytes> --output appcast.xml
python scripts/sign-appcast.py --in appcast.xml --key rig.pem --out appcast.xml.ed
python scripts/generate-tree-manifest.py --staging staging/HodosBrowser \
    --out expected-new-manifest.json --key rig.pem --build-number 40199
# Serve the folder (installer + appcast + appcast.xml.ed + expected-new-manifest.json + .ed):
python -m http.server 8099 --bind 127.0.0.1
```

**Run the test** (dev namespace + point the real shell at the local feed via the seam):
```
$env:HODOS_DEV = '1'
$env:HODOS_UPDATE_TEST = '1'
$env:HODOS_UPDATE_TEST_PUBKEY = '<raw-32 base64 of rig.pem>'
$env:HODOS_UPDATE_RIG_URL = 'http://127.0.0.1:8099/appcast.xml'   # HODOS_UPDATE_TEST_SEAM shell URL override
# In the dev Settings, set autoUpdateMode = "silent".
# 1) Launch the installed N build. Within ~1s the silent thread stages N+1 into
#    %LOCALAPPDATA%\HodosBrowserDev\update\pending\ (watch update\pending\ + the log).
# 2) Fully close it. Relaunch. At cold boot MaybeApplyStagedUpdate applies: spawns the
#    supervisor, runs the Inno installer, integrity-checks, launches the health probe.
# 3) EXPECT: it comes up as 0.4.1, and your dev wallet still opens with its balance.
```

**Rollback leg (the important one):** rebuild N+1 with a deliberately broken
`HodosBrowser.exe` (e.g. truncate it after staging, or ship a stub that exits non-zero) so
the health probe never confirms. Repeat the run. EXPECT: it rolls back to 0.4.0, sets
`update-state.json paused=true`, and the dev wallet opens intact. Verify
`%APPDATA%\HodosBrowserDev\wallet\wallet.db` is unchanged.

**Pass criteria:** happy update commits + wallet intact; broken update rolls back + wallet
intact; no "no browser" state in either case.

---

## Stage 3 — production-signed test (trivial-balance prod wallet)

The final gate before flip. Exercises the **production** trust chain the seams bypass in
stage 2: real Marston Authenticode on the installer, the real EdDSA appcast/manifest key,
SmartScreen/Smart App Control reputation, and the prod `%APPDATA%\HodosBrowser\wallet` path.

- Requires a **CI-built, signed** release with `HODOS_SILENT_AUTOUPDATE=ON` (a deliberate
  "silent test build"), published to a **PRIVATE** appcast (never the public feed). The
  `0.4.0` public release stays notify-only.
- Install the signed N build; point it at the private feed serving the signed N+1.
- Use a **fresh or trivial-balance** prod wallet with its **recovery phrase backed up**.
- Verify the CI self-checks are green first: the appcast-doc + expected-new-manifest key
  self-checks ("… key self-check PASSED") on the real release, and website byte-stability
  (served appcast/manifest stay LF/un-minified — the CRLF trap silently breaks signatures).
- Do BOTH legs (commit + rollback) as in stage 2, on the real signed builds.

---

## Stage 4 — soak, then flip — DONE (silent is ON + live)

- Soaked across real updates; the **default is now silent** for users. Proven live: Windows
  beta.25 → beta.26 (silent through the picker), macOS beta.21 → beta.22 (Sparkle). The
  picker-gate exact-exit-wait (`ae5beb6`) was the last fix required to make the Windows apply
  fire on multi-profile machines.
- `HODOS_UPDATE_TEST_SEAM` must remain OFF in every shipped build (the CMake config warns
  loudly if on).
- The four fleet-safety mitigations that gate enabling silent are all in place: post-
  apply health gate (6d), watchdog auto-revert (6b RunOnce + 6e in-browser), signer-
  continuity degrade (6c.2), and the kill-switch client (6e.2) — the latter needs its
  server-side `kill-list.json` publishing deployed before it's load-bearing.

## Rollback / safety checklist (every real-wallet test)

- [ ] Recovery phrase written down BEFORE the test.
- [ ] Trivial or throwaway wallet, not your main funded one.
- [ ] Confirm `%APPDATA%\HodosBrowser[Dev]\wallet\wallet.db` opens + balance is correct after each leg.
- [ ] Rollback leg tested, not just the happy path.
- [ ] No `update.lock` / RunOnce / stray `pending\` left behind after a clean run.
