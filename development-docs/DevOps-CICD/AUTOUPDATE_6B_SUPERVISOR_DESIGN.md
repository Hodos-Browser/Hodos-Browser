# 6b — External Rollback-Supervisor (`hodos-update-helper.exe`) — DESIGN (pre-code)

**Status:** DESIGN for adversarial review — NO code yet. / **Created:** 2026-06-29 / Branch `0.4.0` local.
**Parent:** `WINDOWS_AUTOUPDATE_PLAN.md` §"APPLY-PHASE DESIGN REVISION" (OD-A..E) + §D/§E. This doc is the
detailed design the revision block defers to. **6a foundations already landed** (`9c516b2`): instance
mutex `Local\HodosBrowser_AnyInstance`, `update.lock` honor-at-launch (dormant), Inno AppMutex/SetupMutex.

> **Reviewers:** attack this to *brick a funded-wallet fleet*. The whole point of the external supervisor
> is to make the worst case (OS-blocked or power-loss-truncated `HodosBrowser.exe`) RECOVERABLE. Find the
> case where it isn't, or where it corrupts the wallet DB, or where it wedges updates forever.

---

## ⛔ ADVERSARIAL REVIEW VERDICT (2026-06-29) — NOT READY FOR CODE; structural revision required

Four parallel skeptics (3× code-auditor + 1× security-ops) attacked this design against live code. They found
**multiple independent funded-wallet brick paths** + the design re-introducing the exact "in-process recovery
can't recover a dead browser" flaw the external supervisor exists to eliminate. **No supervisor code is written
until these are resolved.** Consolidated, deduped:

### STRUCTURAL (change the design shape)
- **B1 — Money DB is OUTSIDE the rollback unit (the deepest find; needs OWNER decision, touches wallet = CLAUDE.md #2).** The new wallet runs **one-way SQLite schema migrations** on `wallet.db` during the health probe, BEFORE the verdict. Rollback restores old *binaries* but the old wallet then can't open the migrated DB → **funded wallet unreachable**. → Owner decision pending (snapshot-and-restore the DB as part of the rollback unit, vs no-migrate-until-healthy). See §8.
- **B2 — Recovery still depends on `HodosBrowser.exe` launching.** The `--resume` watchdog lives *inside* the browser; a half-restored or half-installed tree (old `HodosBrowser.exe` + new `libcef.dll` = ABI crash) crashes before the watchdog runs → permanent brick. → (a) restore must be **crash-atomic** (stage to `.restore-tmp\`, swap per-file via `MoveFileEx`/`ReplaceFile`, the `HodosBrowser.exe`+`libcef.dll` coherent pair LAST) AND (b) a **browser-independent OS recovery hook** (per-user `RunOnce` / Scheduled Task → `helper --resume`) so recovery never needs the browser to start.
- **B3 — Lock-first + split liveness from state (collapses ~6 lock findings).** The D.0 gate runs BEFORE `update.lock` is created → TOCTOU lets a sibling launch into an in-progress apply and the helper then `taskkill`s the **shared singleton wallet** out from under it. AND pid+heartbeat-in-JSON is not a safe single-flight (pid-0 sentinel, heartbeat-bulldoze of a live-but-slow supervisor, non-atomic two-supervisor claim, arbitrary-live-pid DoS). → **(1) create `update.lock` BEFORE D.0; re-enumerate after; sibling present → delete lock + continue as normal browser (safe defer).** **(2) `update.lock` = an EXCLUSIVE (`share=0`, `DELETE_ON_CLOSE`) handle the bootstrap opens and passes by INHERITANCE to the helper** (persists across P0 exit, auto-removed on crash/power-via-reboot). Liveness = "can I exclusively open it?" (OS guarantee), NOT a JSON guess. **`apply.json` (atomic temp+rename writes) = the persistent transaction state** for the watchdog. The 6a honor-at-launch changes from `GetFileAttributes` presence to an exclusive-open probe (co-lands here per forward-flag #2).
- **B4 — Expected-NEW signed manifest (the design's "Inno decides, unknowable" premise is FALSE).** The installer is built from `staging\HodosBrowser\`, so the exact new file set + hashes ARE known at build time. → ship a **build-time `expected-new-manifest.json`, EdDSA-signed**, and verify the post-install `{app}` against it. Catches half-install version-skew, truncated data files (`icudtl.dat`/`*.bin` aren't even PE-signed so Authenticode can't see them), and cross-file ABI coherence — instead of relying on the health probe to notice a crash after the fact.
- **B5 — Backup the COMPLETE `[Files]` closure.** The Phase A.6 set (3 exes + libcef + paks + locales + frontend) MISSES `*.bin` (V8 snapshot), `*.dat` (`icudtl.dat`), and peer DLLs (`chrome_elf`, `libEGL`, `libGLESv2`, `vk_swiftshader`, `vulkan-1`, `d3dcompiler_47`) that `hodos-browser.iss:55-59` installs. Old `libcef.dll` + new `v8_context_snapshot.bin` = instant crash on rollback. → back up the exact `[Files]` closure, one manifest drives backup + verify.

### HIGH (fix in the revised design)
- **H1 — Health-probe relaunch can resolve into PICKER mode → false rollback on the shipped default.** P3 is launched with no `--profile`; the resolver sets `g_picker_mode` (picker is default-ON for multi-profile per project memory); picker never writes the healthy marker → false rollback + auto-update-pause for every multi-profile user. → supervisor passes explicit `--profile <P0's resolved profileId>` AND the bypass is gated on `!g_picker_mode`.
- **H2 — 6c at `:~3962` runs BEFORE `SettingsManager::Initialize` (`:3983`)** → can't read `autoUpdateMode`; and `autoUpdateMode` is *per-profile* while the apply is *fleet-wide*. → eligibility (silent on/off, paused) comes from the **global `update-state.json`** (cross-profile, available at the seam), not the per-profile setting.
- **H3 — `taskkill /F` without `/T` orphans P3's CEF render/GPU subprocs** (NOT in the wallet/adblock jobs) → they hold `libcef.dll`/`*.pak` → restore fails → brick. → `taskkill /F /T` (tree) + **exclusive-open-poll the files for ACTUAL unlock before restoring** (death ≠ unlocked).
- **H4 — Server-side retraction can't stop an already-STAGED build.** A retracted bad build sitting in `pending\` on the fleet still applies at next cold boot. → supervisor (Phase B, before installer) fetches a **signed kill-list** (own EdDSA sig + monotonic generation), checks `marker.buildNumber` not listed, **fail-OPEN on network-down**.
- **H5 — Apply-time anti-rollback (high-water) not wired; `update-state.json` is user-writable.** Local attacker stages an old validly-signed build + zeros `highWaterBuildNumber` → silent downgrade. → assert `marker.buildNumber > max(installed-exe VERSIONINFO, highWater)` at apply time; **derive the anti-rollback floor + `signerThumbprint` from Authenticode-verifying the live `{app}\HodosBrowser.exe`**, not from the deletable JSON (JSON is a cache, the on-disk signed binary is the trust root).
- **H6 — Rollback kill must be GRACEFUL-FIRST.** Phase C `taskkill /F` of P3 hard-kills its wallet (job-close) with no `/shutdown` — bypassing the commit-5 graceful-exit. WAL+`synchronous=FULL` make it *recoverable not corrupting*, but free to do better: POST `:31301//:31302/shutdown` (hard ≤2s) + bounded PID-exit wait BEFORE `/F /T`.
- **H7 — Health timeout vs funded-wallet startup recovery → false rollback that kills a healthy busy wallet.** A large funded wallet's startup reconciliation can exceed 60s; the safety mechanism then triggers the very mid-write kill it exists to prevent, on a GOOD build. → define `/health` as **fast local liveness (port bound + DB openable), NOT gated on full recovery / adblock filter-list download**; make the timeout generous/adaptive; measure real funded-wallet recovery in the MUST-TEST.

### MEDIUM (fold into the revision)
- **M1 — wait-for-PID is PID-reuse-vulnerable** → pass an **inheritable process HANDLE** (`--bootstrap-handle`), not a PID. Combine with B3's inherited lock handle (carve out exactly these inheritable handles; reconcile with "no inherited handles").
- **M2 — `CREATE_BREAKAWAY_FROM_JOB` fails with `ERROR_ACCESS_DENIED` if `HodosBrowser.exe` is launched inside a job lacking `BREAKAWAY_OK`** (AV sandbox/Citrix). → check `CreateProcess(helper)` return; retry WITHOUT the flag on `ACCESS_DENIED`; **delete `update.lock` before `_exit` on any spawn failure** (else orphaned lock).
- **M3 — Free-space precheck (≥2× tree) + re-hash `rollback\` vs manifest before arming** (disk-full-mid-backup → short backup → brick on later restore). "Rollback verified complete" is a precondition of spawning the helper.
- **M4 — SAC may block the new low-reputation `hodos-update-helper.exe` itself** → circular brick (recovery tool blocked). → consecutive-failure counter → `paused`+notify; **CI smoke that SAC doesn't block the helper before enabling `HODOS_SILENT_AUTOUPDATE`**.
- **M5 — Helper can't delete its own running image / CWD** (it runs from `pending\helper\`) → set helper CWD OUTSIDE `pending\` and `{app}`; delegate `pending\` cleanup to the healthy P3 (runs from `{app}`) or a detached `cmd /c` after exit.
- **M6 — D.0 detector = Toolhelp count-by-PID-excluding-self matched on FULL MODULE PATH under prod `{app}`** (not bare image name; a dev build's path differs). The instance MUTEX is NOT the D.0 detector (self always keeps it alive) — it exists ONLY for the Inno `AppMutex`. State this explicitly.
- **M7 — `apply.json` state transitions must be atomic** (temp + `MoveFileEx` rename) — the supervisor reads it cross-process; a half-written state mis-rollbacks. Add a distinct **`state="installing"`** (written right before `CreateProcess(installer)`, cleared on captured exit) so power-loss-mid-install is NOT misclassified as `"armed"` (= "never ran, clean & boot the Frankenstein"); `installing` → always re-spawn helper to restore.
- **M8 — EdDSA must hash the FULL RAW file bytes** (`Sha256File` from offset 0), documented, so a CVE-2013-3900 padded-cert can't slip past a PE-section-only hash.
- **M9 — `_exit(0)` nit:** `fflush`/Logger-flush the "spawned helper" forensic lines first (stdout is `freopen`'d to `debug_output.log`; `_exit` skips stdio flush).
- **DOC — stale line numbers** (~40-line drift): honor-check `:3859`, mutex `:3877`, picker `:3912`, `TryAcquireInstance` `:3925`, `StartListenerThread` `:3961`, `AcquireProfileLock` `:3965`, `LaunchWallet/Adblock` `:3978-3979`, `SettingsManager::Initialize` `:3983`, `CefInitialize` `:4311`, mutex close `:4561`. 6c inserts before `TryAcquireInstance` (`:3925`), inside `!g_picker_mode`. Reconcile parent §D.1 "after SingleInstance check" wording (it reads as after the listener thread — wrong).

### CONFIRMED OK
- `_exit(0)` at the `:~3922` seam is safe (no CefInitialize yet, no children, no profile.lock) — but must be `_exit` not `return` (return would continue to LaunchWallet/CefInitialize). CVE-2013-3900 dual-gate reasoning sound (with M8). I1/I5/I6/I7 hold *as designed* once B1-B5/H1-H7 land; I2/I3/I4 are the violated ones the revisions above restore.

**NEXT:** owner decides B1 (money-DB) + B2's recovery-hook mechanism (§8) → I revise this doc end-to-end → **second design review** on the revised structural shape → only then code 6b. The revision is large enough that a fresh adversarial pass on it is warranted before implementation.

---

## 🔁 SECOND-PASS REVIEW VERDICT (2026-06-29) — v2 shape VALIDATED; bounded v3 fix list (no new owner decisions)

Two focused skeptics attacked §9 v2. **The structural shape holds:** the inherited-`DELETE_ON_CLOSE`-handle
lock model is a correct single-flight/liveness primitive (kernel reasoning confirmed); H3 (`/F /T` + verify-
unlock) and H6 (graceful-first kill) are CLOSED. But the **money-DB snapshot joint** has 4 new v2-specific
defects, plus HIGH lock/eligibility gaps. v3 corrections (all mechanical / factual — implement, no decisions):

**Money-DB joint (the weak point — was the owner-chosen snapshot path, mechanics were wrong):**
- **V3-1 (was F4, factual) — wallet.db is at `%APPDATA%\HodosBrowser\wallet\wallet.db` (ROAMING), NOT under `{app}`.** The §1 diagram conflated `{app}` (Local) with the data dir. Snapshot SOURCE + restore TARGET = the Roaming path (confirmed by `hodos-browser.iss WalletExists()`/CLAUDE.md). Roaming may be redirected (enterprise) → cross-volume COPY (fine for a snapshot; the §D.5 orphan-RENAME stays Local-only, unaffected).
- **V3-2 (was F1/F2) — snapshot only AFTER the wallet is proven DEAD, and as a RAW copy with NO checkpoint.** Reinstate the D.2 exclusive-open poll on `{app}\hodos-wallet.exe` (process-dead proxy) in Phase A BEFORE the snapshot (it was wrongly deferred to Phase B). NO `wal_checkpoint` (no legitimate opener — C++ opening the money DB = CLAUDE.md #2 violation + second-writer hazard). Copy `wallet.db` + `-wal` ONLY (omit `-shm` — regenerable; stale `-shm` misleads recovery); restore replays the WAL. Closes the "two live wallets on one 31301 DB" hazard.
- **V3-3 (was F3) — restore the money DB FIRST (or atomic group), binaries after.** v2 restored binaries-then-DB → power-loss between = old-binary+new-schema-DB = the exact B1 brick in the restore path. DB-first makes that intermediate state unreachable (new-exe+old-DB re-migrates harmlessly; old-exe+old-DB fine). New invariant **I9**.
- **V3-4 (was F5, behavior) — rollback discards money writes made during the health window** (a tx received/broadcast in the ~60s probe). On-chain-recoverable via wallet rescan; set a post-rollback rescan flag. Document as accepted (rare + recoverable).

**Lock / eligibility (HIGH):**
- **V3-5 (was N1) — the honor-at-launch PROBE uses permissive share, NOT `share=0`.** Probe = `GENERIC_READ, OPEN_EXISTING, FILE_SHARE_READ|WRITE|DELETE, NO DELETE_ON_CLOSE`, close immediately. Only bootstrap/supervisor open `share=0`. (Literal "exclusive-open probe" would make two concurrent launchers false-defer on each other; `DELETE_ON_CLOSE` on a probe would delete the supervisor's lock.) Bootstrap lock disposition = `CREATE_ALWAYS` (re-arms a power-loss remnant) (was N4).
- **V3-6 (was N2) — every entry point (bootstrap, RunOnce `--resume`, in-browser watchdog) FIRST action = create `update.lock` `share=0` or abort on `SHARING_VIOLATION`.** After a crash the `DELETE_ON_CLOSE` lock is already gone, so two `--resume` paths can race concurrent `{app}` restores → corruption. Lock-first-or-abort is the single-flight.
- **V3-7 (was N3) — global `silent` eligibility writer + fresh-install default.** Missing `update-state.json` ⇒ NOT eligible (fail-safe-off). A normal post-`SettingsManager` startup mirrors `silent=(autoUpdateMode=="silent")` into the global file (the supervisor only writes high-water/thumbprint/paused). First eligible boot is conservatively notify-only.

**Pipeline / packaging (HIGH/MED):**
- **V3-8 (was F6) — expected-new-manifest: EdDSA-sign the WHOLE manifest, verify-before-use with the embedded key, GENERATE IT IN `release.yml` FROM THE POST-Authenticode-signed staging tree** (hashing pre-signing → hashes never match installed files → fleet-wide permanent `paused` on first silent release). Add a "manifest absent" (legacy staged build) → degrade-to-notify path. Add to the `pending\` inventory.
- **V3-9 (was F7) — the installer doesn't ship the helper.** `[Files]` globs `*.dll/*.bin/*.dat/*.pak/*.json` + 3 named exes — no line installs `hodos-update-helper.exe`. Add an explicit `[Files]` line (else A.5 "copy helper OUT" finds nothing).
- **V3-10 (was F8 + §1/§6 contradiction) — move the whole update working area OUT of the `{app}` root into `%LOCALAPPDATA%\HodosBrowser\update\` (`pending\`, `rollback\`, `update.lock`, `update-state.json`).** Today they sit at the `{app}` root, so the `*.json` backup glob captures `update-state.json` and the installer/uninstaller can touch them. A dedicated `update\` subtree is naturally excluded by the root-level (non-recursive) backup globs. **Updates `AppPaths::GetPendingUpdateDir()` (shipped in commit 4, flag-off/unshipped → safe to repath now)** + add `GetUpdateLockPath()`/state path under `update\`.

**Robustness (MED):**
- **V3-11 (was M1/N5) — pass an inherited bootstrap PROCESS HANDLE (`--bootstrap-handle`), not `--bootstrap-pid`** (PID-reuse); use `STARTUPINFOEX` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` to inherit EXACTLY `{update.lock, bootstrap-handle}` (was N9). Reconcile §4's stale `--bootstrap-pid`/"no inherited handles" wording.
- **V3-12 (was N7) — exclusive-open-poll `{app}\HodosBrowser.exe`+`libcef.dll` for ACTUAL unlock after the P0-handle signals, BEFORE the installer** (death ≠ unlocked — symmetric with H3).
- **V3-13 (was F9/M3) — free-space precheck ≈ `installer + 3× tree` on the Local volume** (`{app}` partial-new + `rollback\` + `.restore-tmp\` + wallet snapshot), not 2×.
- **V3-14 (was F10/M7) — write `apply.json` (state `preparing`) BEFORE arming RunOnce; `--resume` default branch for absent/corrupt apply.json = clear RunOnce + no-op.** (RunOnce is armed at A.6 before A.7 writes apply.json.)
- **V3-15 (was F11/H7) — add an "alive-but-migrating" heartbeat from the wallet `/health` so a slow large-DB migration on a GOOD build doesn't trip the health timeout → false rollback.** Snapshot timing already protects correctness (pre-migration); this protects availability. Measure real large-funded-DB migration in MUST-TEST.
- **V3-16 (was N8, accepted) — RunOnce fires at next LOGON.** For the `installing`-crash Frankenstein case the in-browser tripwire can't run, so recovery waits for logon — but a user whose browser won't open will reboot/re-login (= a logon) → RunOnce fires. Adequate; a boot-triggered Scheduled Task is a possible future hardening, not now (owner chose RunOnce).
- **DOC — mark §3 (pid+heartbeat JSON lock) and §4's `--bootstrap-pid`/"no inherited handles" as SUPERSEDED by §9** (was N5/N6 — coder-traps); fix the ~40-line-stale citations.

**Disposition:** B3/M2/H3/H6 CLOSED. B1/B2/B4/B5/H2/M1/M3/M7/H7 → CLOSED once V3-1..V3-15 land. The money-DB
joint (V3-1..V3-4) is the part to get exactly right; **a focused third micro-review on Phase A wallet-quiesce/
snapshot + Phase C restore-ordering is warranted after the v3 rewrite, before code.** No new owner decisions.

---

## 0. Why a separate exe at all (the structural reason, from the revision)

A running process **cannot overwrite its own image**, and an OS-blocked (Smart App Control) or
power-loss-truncated `HodosBrowser.exe` **never runs** — so any "new exe self-restores from rollback on
next launch" design can't recover the exact worst case (the new exe is the thing that's broken). The
**only** process that BOTH runs after the install AND can overwrite `HodosBrowser.exe` is a *separate*,
*signed*, *low-churn* helper that lives **outside `{app}`**. That is `hodos-update-helper.exe`.

---

## 1. Cast of processes & artifacts

```
{app} = %LOCALAPPDATA%\HodosBrowser\               (Inno install dir, per-user, no UAC)
  HodosBrowser.exe, hodos-wallet.exe, hodos-adblock.exe, libcef.dll, *.pak, ...

%LOCALAPPDATA%\HodosBrowser\
  update.lock                  (presence = "apply in progress"; carries supervisorPid+ts; staleness-guarded)
  update-state.json            (GLOBAL, cross-profile: highWaterBuildNumber, signerThumbprint, paused, lastFailure)
  pending\
    HodosBrowser-<v>-setup.exe (commit-4 staged, verified installer)
    update-info.json           (commit-4 StagedUpdateMarker: buildNumber, sha256, edSignature, signer, thumbprint)
    apply.json                 (NEW 6b: the "armed, awaiting health" record the new browser + watchdog read)
    rollback\                  (NEW 6b: full pre-apply {app} backup + manifest.json content-hashes)
    helper\hodos-update-helper.exe  (NEW 6b: copy of the helper OUT of {app} so the installer can't lock/replace it)

Processes, in order:
  (P0) bootstrap browser  = the cold-boot HodosBrowser.exe that runs MaybeApplyStagedUpdate (6c) at :3922
  (P1) hodos-update-helper.exe = the supervisor (6b) — spawned by P0, OUTLIVES it
  (P2) HodosBrowser-<v>-setup.exe = Inno installer /VERYSILENT (spawned by P1)
  (P3) new browser        = {app}\HodosBrowser.exe after install, launched by P1 with the health-probe arg
```

**Key timing fact (verified in code):** 6c runs at `cef_browser_shell.cpp:~3922`, BEFORE
`LaunchWalletProcess`/`LaunchAdblockProcess` (~3937-3938) and BEFORE `AcquireProfileLock` (~3924). So **P0
has no children and holds no `profile.lock`** — the only `{app}` files P0 image-locks are
`HodosBrowser.exe` + `libcef.dll` + paks (itself). Job objects don't exist yet, so P1 is spawned into **no
job** (the revision's "spawn helper not in any job" is satisfied for free; add `CREATE_BREAKAWAY_FROM_JOB`
defensively + a comment).

---

## 2. The apply transaction — full timeline

### Phase A — 6c bootstrap decision (in `HodosBrowser.exe`, `MaybeApplyStagedUpdate()` at :3922)
Runs only when: `!g_picker_mode`, `HODOS_SILENT_AUTOUPDATE` compiled in, `autoUpdateMode=="silent"`, NOT
paused (`update-state.json`), and a verified `pending\update-info.json` + matching installer exist.
1. **D.0 all-instances-gone gate.** Prove P0 is the SOLE `HodosBrowser.exe`: (a) Toolhelp enumerate
   `HodosBrowser.exe` images, require count==1 (self, by PID); (b) `!g_picker_mode`; (c) the picker counts
   as a sibling (OD-D) — if a `.picker` instance is up, defer. If any sibling → **DEFER** (return, continue
   normal startup), no apply.
2. **Acquire `update.lock`** = create the file with `{schema:1, supervisorPid:0, stagedBuild:N, ts}` (pid 0
   = "claimed by bootstrap, helper not yet up"). Created while P0 is provably the sole instance, so no race.
3. **Re-verify at apply time (OD-B):** re-hash the staged installer == marker.sha256 AND re-run Authenticode
   (`WinVerifyTrust`) AND re-verify `edSignature` over the installer bytes (marker now carries `edSignature`).
   Any mismatch → abort, delete `update.lock`, keep old version, retry next launch.
4. **Signer-continuity (OD-E) at STAGE time, re-checked here:** if marker `signerThumbprint` !=
   `update-state.json.signerThumbprint` (the currently-installed signer) → **DEGRADE to notify**: do not
   silent-apply; write a notify request; delete `update.lock`; continue normal startup.
5. **D.2 exclusive-open poll** (zero-retry, ~10s cap, `Sleep(250)`): exclusive `GENERIC_WRITE share=0` on
   `{app}\HodosBrowser.exe` (will FAIL — P0 holds it; that's expected & handled in Phase B by wait-for-PID),
   `hodos-wallet.exe`, `hodos-adblock.exe`, `libcef.dll`. Purpose here: confirm the THREE non-self exes +
   libcef are free (no orphaned children). `HodosBrowser.exe` self-lock is expected; the helper waits for
   P0's PID to release it. If a non-self exe stays locked past the cap with zero siblings → truly-orphaned
   child → D.5 orphan-only rename path (out of 6b core; documented).
6. **Take FULL `{app}` rollback backup (D.3):** copy all three exes + libcef.dll + *.pak + locales + frontend
   to `pending\rollback\`, write `rollback\manifest.json` = sha256 of every CURRENT `{app}` file (the
   "known-good old tree"). Also stamp the EXPECTED-new manifest is NOT known yet (Inno decides) → the D.6
   integrity check validates the new tree by Authenticode + the installer's own file list, not a pre-known
   hash (see §5).
7. **Copy the helper OUT:** `{app}\hodos-update-helper.exe` → `pending\helper\hodos-update-helper.exe` (so the
   installer can freely replace `{app}\hodos-update-helper.exe`).
8. **Write `pending\apply.json`** = `{schema:1, fromBuild, toBuild, installer, rollbackDir, manifest, stagedAt,
   state:"armed"}`.
9. **Close the instance mutex** (`CloseHandle(g_instance_mutex)`) — BEFORE spawning anything, so Inno
   `AppMutex` won't see P0 (revision "mutex release ordering"). NOTE: P0's image is still locked until P0
   exits; AppMutex only checks the mutex, not the image — fine.
10. **Spawn the helper** `pending\helper\hodos-update-helper.exe` with args (see §6), `CREATE_BREAKAWAY_FROM_JOB
    | CREATE_NO_WINDOW | DETACHED_PROCESS`, NOT inheriting handles. Capture P1 PID, write it into
    `update.lock` (supervisorPid=P1).
11. **`_exit(0)`** (skip atexit/CefShutdown — P0 never ran CefInitialize; nothing to flush; releases the
    `HodosBrowser.exe`/libcef image locks promptly).

### Phase B — supervisor (P1, `hodos-update-helper.exe`)
1. **Re-assert it owns the window:** read `update.lock`; if `supervisorPid` already names a DIFFERENT live
   process → another supervisor is active → exit (single-flight). Else write its own PID. Re-write
   `update.lock` ts (heartbeat) — see staleness, §3.
2. **Wait for P0 to exit (wait-for-PID):** `OpenProcess(SYNCHRONIZE, P0pid)` → `WaitForSingleObject` bounded
   (~15s). When P0 is gone, `{app}\HodosBrowser.exe` + `libcef.dll` are unlocked. (If P0 won't die in the
   cap → abort: delete `update.lock`, leave old install intact, exit; next launch retries.)
3. **Bounded child-shutdown safety net (absorbs E.3):** belt-and-suspenders — POST `:31301/shutdown` +
   `:31302/shutdown` with HARD `WinHttp` timeouts (≤2s each), then bounded exclusive-open poll on the two
   child exes (~10s). If still locked → `taskkill /F /IM hodos-wallet.exe /IM hodos-adblock.exe` LAST resort.
   (In the normal path the children are already dead — D.0 + D.2 proved it — so this usually no-ops.)
4. **Spawn the Inno installer (P2):** `pending\HodosBrowser-<v>-setup.exe /VERYSILENT /SP- /SUPPRESSMSGBOXES
   /NORESTART`, `CREATE_BREAKAWAY_FROM_JOB`. `WaitForSingleObject(P2)` bounded (~120s). Capture exit code.
   - Inno `AppMutex` no longer fires (P0 gone, mutex closed). `SetupMutex` prevents a 2nd installer.
   - If exit code != 0 OR timeout → **ROLLBACK** (Phase C) — the tree may be half-written.
5. **Integrity-check the new tree (D.6):** for every exe + libcef.dll in `{app}`: Authenticode-verify
   (`WinVerifyTrust`, signer == Marston) AND non-empty/non-truncated (size>0, valid PE header). The Rust exes
   carry VERSIONINFO (§H.3) → cross-check ProductVersion == toBuild where available. If ANY fails (present-
   but-truncated after power loss is the headline case) → **ROLLBACK** (Phase C).
6. **Launch the new browser (P3)** `{app}\HodosBrowser.exe --post-update-health-probe`
   (CREATE_BREAKAWAY_FROM_JOB; normal window). Set `apply.json.state="awaiting-health"`.
7. **Wait bounded for the `first-run-healthy` marker** (~60s): poll for `pending\apply.json.state=="healthy"`
   (the new browser writes it — see Phase D). While waiting also detect P3 early-exit (crash-loop) via its
   PID.
   - **Marker appears in time** → **SUCCESS** (Phase E).
   - **Timeout, OR P3 exited without writing healthy, OR P3 crash-looped** → **ROLLBACK** (Phase C).

### Phase C — ROLLBACK (the reason this exe exists)
1. **Kill P3 if still alive:** the new (hung/blocked) browser may hold `{app}\HodosBrowser.exe`. `taskkill
   /F` P3 by PID; its job object kills any children it spawned; wait for P3 dead. `profile.lock`
   (`FILE_FLAG_DELETE_ON_CLOSE`) auto-releases on P3 death.
2. **Restore `rollback\` over `{app}`:** copy every file from `pending\rollback\` back over `{app}` (the
   helper CAN overwrite the now-non-running browser). Verify each restored file's sha256 == `manifest.json`.
   ALSO restore `{app}\hodos-update-helper.exe` from rollback (it was replaced by the new installer).
3. **Set paused:** `update-state.json.paused=true`, `lastFailure={toBuild, reason, ts}`. The high-water
   build-number is NOT advanced (the failed build must not become the floor).
4. **Delete `update.lock`** (apply window over).
5. **Relaunch the OLD build:** `{app}\HodosBrowser.exe` (no probe arg → normal launch; update.lock gone so it
   won't defer). Best-effort notify (a small flag the old browser reads → shows "an update failed and was
   rolled back; auto-updates paused").
6. **`_exit(0)`.** Leave `pending\rollback\` + `apply.json` for the watchdog (§5) as a tripwire if step 2
   itself was interrupted.

### Phase D — new browser first-run health (6d, in `HodosBrowser.exe`)
On launch WITH `--post-update-health-probe` AND a valid `apply.json.state=="awaiting-health"` for the
just-installed build (double-gate): **bypass `update.lock` honor** (6a seam), run normal startup
(AcquireProfileLock + LaunchWallet + LaunchAdblock), and AFTER (a) `profile.lock` acquired AND (b) both
children pass `:31301`/`:31302` health AND (c) the new build's own version == `apply.json.toBuild`, write
`apply.json.state="healthy"`. If the probe arg is present but `apply.json` is absent/mismatched → ignore the
arg, behave as a normal launch (defends against a stray/forged arg).

### Phase E — SUCCESS cleanup
Supervisor: advance `update-state.json.highWaterBuildNumber=toBuild`, set
`signerThumbprint=marker.signerThumbprint`, `paused=false`; delete `pending\` (incl. `rollback\`, installer,
markers); delete `update.lock`; `_exit(0)`. The healthy new browser (P3) is already the running session.

---

## 3. `update.lock` semantics + staleness (the 6a forward-flag #2 resolution)
- **Presence = "apply in progress."** Honor-at-launch (6a) defers any normal launch while present.
- **Format (schema 1):** JSON `{schema, supervisorPid, stagedBuild, createdTs, heartbeatTs}`.
- **Staleness self-heal (closes the silent-brick):** a launch that finds `update.lock` must treat it as
  STALE (and delete it + proceed) if ANY of: (a) `supervisorPid` is not a live process; (b)
  `now - heartbeatTs > 90s` (the supervisor heartbeats every ~10s across its bounded waits). Only a
  fresh+live lock defers. So a crashed supervisor cannot wedge launches forever.
- **User-visible persistence:** if a launch defers on a fresh lock more than ~twice in a row / the window
  exceeds a cap, show a one-line "Hodos is updating…" splash rather than a silent exit-0.
- **The honor bypass is ARG-based, not marker-only (REVISES the 6a recommendation).** The 6b design pass
  found marker-only detection can't distinguish "supervisor's health-probe launch" from "user double-clicked
  during the window" — both see lock+apply.json and would both bypass → two browsers race `profile.lock`.
  Fix: the supervisor passes `--post-update-health-probe`; only that arg bypasses, AND it's double-gated on a
  valid `apply.json.state=="awaiting-health"` so a stray arg without a real pending apply is inert. A user
  double-click (no arg) always defers. (Forged-arg risk is negligible: an attacker who can spawn our signed
  exe with args in the ~60s window can do worse, and the worst outcome is a premature "healthy" write =
  confirming an update, not a security boundary.)

---

## 4. The helper exe itself
- **Source:** new `cef-native/` target (tiny C++/Win32, OpenSSL for sha256/EdDSA reuse via shared
  `UpdateStager` pure funcs, WinTrust for Authenticode). Built + Azure-signed by `release.yml` like the other
  exes; shipped inside `{app}` by Inno (`[Files]`).
- **Why low-churn:** it must keep working across browser versions, so it stays minimal and its on-disk copy
  (`pending\helper\`) is the one that runs an apply (taken from the CURRENTLY-installed, known-good build —
  NOT the new one, so a broken new helper can't break recovery).
- **Spawned NOT in any job** (CREATE_BREAKAWAY_FROM_JOB; at :3922 no jobs exist anyway), DETACHED, no
  inherited handles. Uses `_exit(0)`.
- **Args (one source of truth, passed by 6c):** `--app-dir`, `--pending-dir`, `--bootstrap-pid`,
  `--installer`, `--from-build`, `--to-build`, `--health-timeout`. All paths are absolute, non-ASCII-safe
  (wide). Everything else (marker, manifest, state) it reads from files.
- **Logging:** its own `pending\helper\helper.log` (the browser's Logger isn't available); never writes
  inside `{app}` (being replaced).

---

## 5. Watchdog / armed-but-unconfirmed (6e tie-in, but the invariant lives here)
If ANYTHING kills P1 mid-transaction (P1 crash, machine power-loss between Phase B step 4 and Phase E), the
NEXT cold-boot browser must detect the half-done state and self-heal **without** P1:
- On startup, before the silent-apply check, the browser inspects `pending\apply.json`:
  - `state=="armed"` but `update.lock` stale/gone and `{app}` integrity OK → installer never ran or fully
    rolled back; just clean `pending\` and continue.
  - `state=="awaiting-health"` and no `state=="healthy"` and the lock is stale → **the supervisor died after
    install but before confirming health.** The browser CANNOT overwrite its own image to restore — so it
    **re-spawns the helper** (`pending\helper\hodos-update-helper.exe --resume`) which kills any stray P3,
    restores `rollback\`, pauses, relaunches old. (This is why the helper copy + rollback\ survive until
    success.)
  - `state=="healthy"` → success cleanup was interrupted; finish it (advance high-water, delete pending).
- **Integrity check (D.6) is BY AUTHENTICODE + TRUNCATION, not a pre-known new-hash:** Inno decides the new
  file set, so we validate "every exe/DLL is validly signed by Marston AND not truncated," plus Rust
  VERSIONINFO == toBuild where present. `rollback\manifest.json` is the OLD tree (for restore verification),
  not the new.

---

## 6. Settling questions for the owner (before code)
1. **`update-state.json` as the global paused/high-water store** (new file at `{app}`-parent root). OK to
   introduce, or fold into an existing file? (It must be cross-profile + outside `{app}` so the installer
   doesn't clobber it. Recommend the new file.)
2. **Helper logs in `pending\helper\helper.log`** (cleaned on success). OK?
3. **Health timeout = 60s; installer timeout = 120s; P0-exit wait = 15s; child-shutdown poll = 10s.** Sane
   first cut for soak, then tune. Agree to start here?
4. **Notify-on-rollback UX:** a small flag file the relaunched old browser reads → shows a one-time banner
   "An update failed and was rolled back; automatic updates are paused. [Retry] [Keep paused]." Acceptable
   for 6b, or defer the banner to 6d?
5. **`--resume` watchdog re-spawn (Phase C via the next browser)** is the belt-and-suspenders for a dead
   supervisor. Confirm it belongs in 6b's helper (vs. punting all watchdog logic to 6e). Recommend: the
   helper supports `--resume` in 6b (so the recovery code path exists + is tested), 6e only adds the
   M-consecutive-failures escalation.

---

## 7. Invariants the review must confirm hold
- **I1.** No path leaves the user with NO browser: every abort/rollback ends by relaunching a runnable build
  (old on failure, new on success) OR continuing normal startup (defer).
- **I2.** No path overwrites a RUNNING `HodosBrowser.exe` (P0 waited-for; P3 killed before restore).
- **I3.** The wallet money-DB is never `TerminateProcess`d mid-write on the HAPPY path (children already dead
  via job-close before the helper runs; the helper's taskkill is last-resort on an orphan only). On ROLLBACK,
  P3's children are killed by job-close on P3 death — quantify the residual risk + lean on WAL + the wallet
  graceful-exit.
- **I4.** A crashed supervisor cannot wedge launches forever (lock staleness) nor silently brick (watchdog
  `--resume` from the next browser).
- **I5.** A staged build whose signer/thumbprint differs from the installed one NEVER silent-applies
  (degrade-to-notify at both stage time and apply time).
- **I6.** The failed build's build-number never becomes the high-water floor (only a HEALTHY apply advances
  it), so a known-bad build can be superseded by a fixed one with the same/next number.
- **I7.** Dev (`HODOS_DEV=1`) is fully inert: no helper, no installer, no lock.

---

## 8. OWNER DECISIONS blocking the revision (from the adversarial verdict)

### D-B1 — Money-DB rollback strategy (the headline; touches the wallet → CLAUDE.md #2 sign-off)
The new build's wallet runs one-way `wallet.db` schema migrations during the health probe; a binary-only
rollback then strands a funded wallet on a too-new schema. Two viable strategies:

- **Option (b) — Snapshot-and-restore the money DB as part of the rollback unit (RECOMMENDED).** At apply
  time (all instances gone → DB quiescent), checkpoint WAL `TRUNCATE` then copy `wallet.db`(+`-wal`/`-shm`)
  into `pending\rollback\wallet\`. On rollback, restore it atomically alongside the binaries. **Wallet code
  unchanged** (no money-path edit). Cost: one extra DB copy (personal wallet = MBs) + restore on the rare
  rollback. Cleanest correctness (atomic old-tree+old-DB unit).
- **Option (a) — No-migrate-until-healthy.** The health-probe launch opens the money DB **read-only / with
  migrations DEFERRED**, confirms health, writes the marker; migrations run only on the first NORMAL launch
  after success. No DB copy, but **requires a wallet change** (a "probe / no-migrate" open mode) on the money
  path — more risk, more review, per CLAUDE.md #2.

### D-B2 — Browser-independent recovery hook mechanism (so recovery never needs the browser to launch)
- **Per-user `RunOnce` (HKCU\…\RunOnce → `helper --resume`) (RECOMMENDED).** No admin, fires once at next
  logon, self-deletes. Set when arming; cleared on success. Simplest per-user no-UAC fit.
- **Per-user Scheduled Task.** More robust (survives logon timing, can retry) but heavier to create/clean and
  more AV-conspicuous for a new exe.

(Lower-stakes settling questions — `update-state.json` as the global store, helper log location, the
timeout first-cut values, rollback-notify UX — are in §6; I'll lock them to the recommended defaults unless
you flag one.)

**Owner decisions (2026-06-29): D-B1 = (b) snapshot-and-restore the money DB. D-B2 = per-user RunOnce.**

---

## 9. REVISED DESIGN v2 (AUTHORITATIVE — supersedes §2/§3/§5 above; folds in the verdict + owner decisions)

**Two-handle model (B3).** `update.lock` is an **exclusive (`share=0`, `FILE_FLAG_DELETE_ON_CLOSE`) handle** =
the *liveness* token. `apply.json` (atomic temp+`MoveFileEx` writes) = the durable *transaction state*.
Liveness is an OS guarantee (can you exclusively open `update.lock`?), never a pid/heartbeat guess. The 6a
honor-at-launch becomes an **exclusive-open probe** (`SHARING_VIOLATION` ⇒ live apply ⇒ defer; opens or
`NOT_FOUND` ⇒ no live apply ⇒ continue, and if `apply.json` shows an unfinished txn, run the watchdog).

**Global eligibility (H2).** Silent-on/paused/high-water/signerThumbprint live in cross-profile
`update-state.json` at the `HodosBrowser` root (NOT per-profile settings, which aren't loaded at the seam).
`signerThumbprint` + anti-rollback floor are **derived by Authenticode-verifying the live `{app}\HodosBrowser.exe`**
(the signed binary is the trust root; the JSON is only a cache — H5).

### Phase A — bootstrap (`MaybeApplyStagedUpdate()`, inserted BEFORE `SingleInstance::TryAcquireInstance` `:3925`, inside `!g_picker_mode`)
Eligible only if: `!g_picker_mode`, `HODOS_SILENT_AUTOUPDATE` compiled in, `update-state.json` silent && !paused,
a verified `pending\update-info.json` + installer exist. (Note: this seam is before `SettingsManager::Initialize`
— that's WHY eligibility reads the global file, H2.)
1. **LOCK-FIRST (B3):** open `update.lock` exclusive + `DELETE_ON_CLOSE`, **inheritable**. If it
   `SHARING_VIOLATION`s → another apply live → continue normal startup. Hold the handle.
2. **D.0 all-instances-gone (re-ordered AFTER the lock):** Toolhelp enumerate, count==1 by **full module path
   under prod `{app}`** excluding self (M6); picker `.picker` pipe counts as a sibling (OD-D). Any sibling →
   close `update.lock` (auto-deletes) → continue normal startup (safe defer). *(The lock now precedes the
   count, so a sibling either appears in this snapshot or hit the lock at its own honor probe — TOCTOU closed.)*
3. **Apply-time verify (B-gates, all fail-closed):** re-hash installer == `marker.sha256`; **EdDSA over the full
   raw installer bytes** (`Sha256File` from 0, M8) with the embedded key; **Authenticode** (`WinVerifyTrust` +
   pin signer CN+thumbprint == live-exe thumbprint); **anti-rollback** `marker.buildNumber > max(live-exe
   VERSIONINFO, update-state.highWater)` (H5); **kill-list (H4):** fetch signed kill-list, reject if
   `buildNumber` listed (fail-OPEN on network-down); **signer-continuity (I5):** `marker.signerThumbprint !=
   live-exe thumbprint` → degrade to notify (no silent apply). Any failure → close lock, keep old, retry/notify.
4. **Backup the FULL `[Files]` closure + the money DB (B5+B1):** copy the exact `hodos-browser.iss [Files]`
   set (`*.exe *.dll *.bin *.dat *.pak *.json` + `locales\` + `frontend\`) to `pending\rollback\`; WAL-
   checkpoint(TRUNCATE) then copy `wallet.db`(+`-wal`/`-shm`) to `pending\rollback\wallet\`. Write
   `rollback\manifest.json` (sha256 of every backed-up file). **Verify the backup complete (M3):** free-space
   precheck ≥2× tree; re-hash `rollback\` vs manifest; any miss → close lock, abort (no arm).
5. **Copy helper OUT:** `{app}\hodos-update-helper.exe` → `pending\helper\`.
6. **Arm recovery (B2):** set HKCU `RunOnce\HodosUpdateResume = "pending\helper\hodos-update-helper.exe --resume"`.
7. **`apply.json` (atomic) → `state="armed"`** with fromBuild/toBuild/paths/manifest.
8. **Close the instance mutex** (so Inno AppMutex won't see P0).
9. **Spawn helper** `pending\helper\hodos-update-helper.exe <args>` with an **inherited** `update.lock` handle
   (B3) + an **inherited bootstrap process handle** (M1, not a PID) + `CREATE_NO_WINDOW|DETACHED_PROCESS`;
   `CREATE_BREAKAWAY_FROM_JOB` **with ACCESS_DENIED→retry-without (M2)**. **Check the return**; on irrecoverable
   spawn failure → clear RunOnce, close lock (auto-delete), continue normal startup (M2).
10. `fflush`+Logger-flush the forensic lines (M9), then **`_exit(0)`** (launches/holds nothing).

### Phase B — supervisor (`hodos-update-helper.exe`); CWD set OUTSIDE `pending\`/`{app}` (M5)
1. **Single-flight by the inherited exclusive `update.lock` handle** (B3) — no second supervisor can hold it.
2. **Wait for the bootstrap via the inherited HANDLE** (M1, PID-reuse-immune), bounded ~15s; on timeout abort
   (old install intact), close lock, exit.
3. **Belt-and-suspenders child-shutdown (E.3) — only after re-confirming no sibling (F6/F9):** re-enumerate;
   sibling present → abort+rollback, NEVER `taskkill` the shared wallet. Else POST `:31301//:31302/shutdown`
   (hard ≤2s) + bounded exclusive-open poll; `taskkill /F` children only as last resort on a true orphan.
4. **`apply.json` → `state="installing"` (M7) BEFORE** spawning the installer. Spawn `…-setup.exe /VERYSILENT
   /SP- /SUPPRESSMSGBOXES /NORESTART`; **poll-wait** `WaitForSingleObject(P2, 10s)` in a loop so the supervisor
   stays responsive (no 120s blind block — the old heartbeat conflict is moot under the exclusive-handle model
   but keep the loop for abort/logging), ~120s cap. Non-zero exit / timeout → ROLLBACK.
5. **Integrity-gate the new tree against the BUILD-TIME signed `expected-new-manifest.json` (B4):** every
   installed file's sha256 == manifest AND every PE Authenticode-valid (Marston). Catches half-install skew +
   truncated `icudtl.dat`/`*.bin`. Any miss → ROLLBACK.
6. **`apply.json` → `state="awaiting-health"`** (atomic). Launch P3 `{app}\HodosBrowser.exe --post-update-health-probe
   --profile <P0's resolved profileId>` (H1: explicit profile, forces `!g_picker_mode`).
7. **Wait ~adaptive (H7, generous) for `apply.json.state=="healthy"`**, also watching P3 PID for crash-loop.
   Healthy in time → SUCCESS (Phase E). Else → ROLLBACK.

### Phase C — ROLLBACK (crash-atomic; B2/H3/H6)
1. **Graceful-first kill of P3 (H6):** POST P3's `:31301//:31302/shutdown` (hard ≤2s) + bounded wait, THEN
   `taskkill /F /T <P3>` (tree — H3, kills CEF render/GPU subprocs too). **Then exclusive-open-poll
   `HodosBrowser.exe`+`libcef.dll`+3 exes for ACTUAL unlock** (death ≠ unlocked, H3).
2. **Crash-atomic restore (B2):** stage `rollback\` into `{app}\.restore-tmp\`, swap per file via
   `ReplaceFile`/`MoveFileEx`, **`HodosBrowser.exe`+`libcef.dll` coherent pair LAST**; restore the money DB
   from `rollback\wallet\` (B1); restore `{app}\hodos-update-helper.exe`. Verify each vs `manifest.json`.
3. `update-state.json`: `paused=true`, `lastFailure={toBuild,reason,ts}`; **do NOT advance highWater** (I6).
4. Clear HKCU `RunOnce`. Relaunch the OLD `{app}\HodosBrowser.exe` (no probe arg). Notify flag for the banner.
5. **Close `update.lock`** (auto-deletes). `_exit(0)`. (Leave `pending\` for forensics; next healthy boot cleans.)

### Phase D — new browser first-run health (6d, in `HodosBrowser.exe`)
Launched with `--post-update-health-probe` AND `apply.json.state=="awaiting-health"` for the just-installed
build AND `!g_picker_mode` (double/triple-gate). Bypass the `update.lock` honor; run normal startup; AFTER
`profile.lock` acquired + both children pass **fast local `/health` (port bound + DB openable — NOT full
recovery/filter-list, H7)** + own version == `apply.json.toBuild`, **atomically** set `apply.json.state="healthy"`.
Probe arg without a matching armed `apply.json` ⇒ ignore arg, behave normally (defends a stray/forged arg).

### Phase E — SUCCESS
Supervisor: advance `update-state.json.highWater=toBuild`, `signerThumbprint`, `paused=false`; clear HKCU
`RunOnce`; delete `pending\` **but not its own running image** — delegate `pending\` removal to the healthy P3
(runs from `{app}`) or a detached `cmd /c` after exit (M5); close `update.lock`; `_exit(0)`.

### Watchdog / `--resume` (B2; the RunOnce target + the next-boot in-browser check)
The **RunOnce** fires `helper --resume` at next logon **independent of the browser** (closes B2's circular
brick). `--resume` reads `apply.json`: `installing` or `awaiting-health` (+ no `healthy`) ⇒ kill any stray P3
(graceful-first), **crash-atomic restore from `rollback\`+`rollback\wallet\`**, pause, relaunch old; `armed`
with integrity OK ⇒ clean; `healthy` ⇒ finish success cleanup. Single-flight via the exclusive `update.lock`.
The in-browser check stays as a SECONDARY tripwire but is no longer the sole recovery path.
