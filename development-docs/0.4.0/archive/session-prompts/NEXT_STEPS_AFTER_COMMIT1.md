# Next steps after commit 1 (bootstrap migration) — written 2026-08-04

**State:** Windows dev app **runs on CEF 150 / 7871** at `1f98dba`. Tree clean.
**Read first:** `SESSION_HANDOFF_7871_BUILD_GREEN.md` → `../MAC_WINDOWS_RELAY.md` → this.

The four-commit bootstrap plan is half done: **2a `83fe472`** (history off the renderer) and
**commit 1 `1f98dba`** (bootstrap migration) are landed. **2b** and **3** remain.

---

## S0 — Stage the 150 distribution into `cef-binaries/` ✅ **DONE 2026-08-04**

Owner decision (2026-08-04): **dev must run on the same binaries the next release ships.** So stop
passing `-DCEF_ROOT` and make the repo default correct.

`cef-binaries/` is **gitignored** (`.gitignore:27`, 0 tracked files), so the staging itself was a
local-machine operation, not a commit. But it was **not** only local — see S0.3, the part that bites.

All five steps are complete. What was done, and the two things that differed from this plan:

1. ✅ **Archived M136.** The pre-existing backup at `C:\cef\cef150\m136_cef_binaries_backup\` turned
   out to be **incomplete** — only `Release/` + `Resources/`, missing `include/`, `libcef_dll/`,
   `cmake/`, `bazel/` and `cef-binaries/CLAUDE.md`. The whole tree was instead **moved** to
   `C:\cef\cef150\m136_cef_binaries_FULL_backup` (lossless, and cheap since it is a rename).
2. ✅ **Copied the 150 distribution in**, including `build_wrapper/libcef_dll_wrapper/Release/`.
   ⛔ **It had to be a MOVE-then-copy, not a copy-over.** The wrapper probe checks
   `${CEF_ROOT}/libcef_dll/wrapper/build/Release` **before** `${CEF_ROOT}/build_wrapper/...`, and the
   150 dist has no first-path directory — so merge-copying would have left the **M136 wrapper**
   in place, where it wins the probe, links cleanly, and then corrupts memory at runtime.
   Reconfigured with `cmake -U CEF_ROOT` (it is a *cache* variable, so merely dropping the flag keeps
   the old value), default resolved, `bootstrap.exe` gate passed silently, full rebuild green, dev app
   launches on the default path — `CefInitialize success=true`, all 11 overlay browsers created, no errors.
3. ✅ **CI asset published — but as a NEW asset, not a replacement.** The `cef-binaries` release lives
   on **`Hodos-Browser/Hodos-Browser`** (the signing org repo), *not* on `origin`. Since
   `cef-binaries-windows.zip` is shared by every branch and `main`/`staging` are still on the
   pre-bootstrap CMakeLists, clobbering it would have broken their Windows build (CEF 150 dropped
   `cef_sandbox.lib` → `LNK1181`). So 150 went up as **`cef-binaries-windows-150.zip`** and
   `release.yml:118` now points at it **on this branch only**. macOS (`cef-binaries-macos.tar.bz2`)
   is untouched and still M136.
   > **When 0.4.0 lands on main:** collapse the pattern back to the unversioned name and re-upload
   > the M136 asset as 150, so there is one asset again.
   >
   > Note: the zip had to be hand-built with **forward-slash** entry names —
   > `ZipFile::CreateFromDirectory` emits backslashes on this host, which violates the ZIP spec and
   > makes `7z x` extraction tool-dependent.
4. ✅ **`AboutSettings.tsx` no longer carries a literal at all.** Rather than bumping 136→150, the
   engine label is now derived at runtime from `navigator.userAgent` (`Chrome/(\d+)`), which cannot
   rot across a bump. Verified against the running dev browser over CDP: renders `Chromium (CEF 150)`.
   This matches the warning already in that file about the *app* version literal having silently drifted.
5. ✅ Pin table in `cef-native/CLAUDE.md` updated (Windows now builds on the default staged path;
   `-DCEF_ROOT` no longer needed), plus the merge-copy and CI-asset hazards recorded there and in
   `cef-binaries/CLAUDE.md`.

---

## S1 — Commit 3: icon + VERSIONINFO ✅ **DONE 2026-08-04**

**Confirmed by inspection on 2026-08-04:** `HodosBrowser.exe` is byte-identical to CEF's
`bootstrap.exe` and carries **CEF's** icon and version resources; our `hodos.ico` lives in
`HodosBrowser.dll`. That is why the taskbar showed the wrong logo.

Both icons are fixed. Note the corrected status of the first row — it was **not** fixed in commit 1:

| Icon | Source | Status |
|------|--------|--------|
| Window icon (title bar, Alt-Tab, taskbar button of a running window) | `LoadImage(g_hResourceModule, MAKEINTRESOURCE(1), …)` reading `hodos.rc` out of the DLL | ✅ fixed here — see the `.rc` bug below |
| Exe icon (Explorer, pinning) | the `.rsrc` of `HodosBrowser.exe` = bootstrap's | ✅ fixed by `tools/stamp_win_resources.cpp` |

Implemented as decided: a CMake `POST_BUILD` step running `tools/stamp_win_resources.cpp`, which
opens the copied `HodosBrowser.exe` with `BeginUpdateResource` / `UpdateResource` /
`EndUpdateResource` and stamps `RT_GROUP_ICON` + `RT_ICON` + `VS_VERSIONINFO`. Patching CEF's
`bootstrap.rc` through the P3 patch toolchain was **considered and rejected**.

Three things worth carrying forward:

1. **⚠️ The window icon was never actually fixed in commit 1.** `hodos.rc` declared
   `IDI_ICON1 ICON "hodos.ico"`, and since windows.h does not define `IDI_ICON1`, RC treated it as a
   resource **name** — the DLL carried `RT_GROUP_ICON "IDI_ICON1"` while both consumers asked for
   `MAKEINTRESOURCE(1)`. `LoadImage` returned NULL with `ERROR_RESOURCE_TYPE_NOT_FOUND` (1813) and
   the `if (hIcon)` guards swallowed it. Commit 1's `g_hResourceModule` change was necessary but not
   sufficient. Fixed by making the id a bare integer `1`. **This is why the plan's "needs a visual
   confirm" mattered — the claim was wrong.**
2. **`BeginUpdateResource` must be called with `bDeleteExistingResources = FALSE`.** `TRUE` also
   wipes `RT_MANIFEST`, which under the bootstrap model carries the Win10/11 `supportedOS` GUIDs.
   Verified preserved byte-for-byte (978 bytes) against the pristine `bootstrap.exe`.
3. CEF's own icons are **enumerated and deleted**, not merely shadowed. Explorer picks the lowest
   `RT_GROUP_ICON` id, so adding ours alongside CEF's `#32512` would work today and break the day
   CEF renumbers.

**Verified:** exe resources go from CEF's (`RT_ICON #1-8`, `RT_GROUP_ICON #32512`) to ours
(`RT_ICON #1-4`, `RT_GROUP_ICON #1`), all four images byte-identical to `hodos.ico`; Windows'
own parser reads the version block (`Hodos Browser` / `0.4.1` / company / copyright); and the
per-profile **AUMID** path still works — three simultaneous taskbar buttons each showed the Hodos
gear with the correct badge (production **H**, dev `Dev_Env` **D** in red, dev `Test` **T** in gold).
Before the fix the dev button rendered a generic blank-window icon.

---

## S2 — Commit 2b: turn the Chromium sandbox ON ✅ **SOLVED 2026-08-04 (later session)**

Real exposure closed: an unsandboxed renderer could socket straight to the wallet port, bypassing
the C++ interception layer and every permission gate.

**Verified state:** 14 renderers at **UNTRUSTED**, GPU at **LOW**, a utility at UNTRUSTED; 11 browsers
created; 0 `[ERROR]` lines; 0 `Tab Killed`. github.com, youtube.com and x.com all render (x.com
resolves to `/home`, so the cookie jar survives). Uncommitted — awaiting owner review.

### ⭐ ROOT CAUSE: the dev safeguard was running in child processes

`RunHodosMain` ran `AppPaths::EnforceDevSafeguard()` **before** `CefExecuteProcess`, i.e. in *every*
process. **A sandboxed child does not see `HODOS_DEV`**, so the guard failed there and
`RunHodosMain` returned **1**. Every renderer exited instantly with `RESULT_CODE_KILLED`.

Why it was so hard to see:

- Exit code 1 is a **clean return**, not a crash — so **crashpad produced zero reports** even with
  CEF crash reporting explicitly enabled.
- The process died before Chromium's own logging was up, so the engine could only say
  `sad_tab.cc:258 Tab Killed`. `--v=1` / `--vmodule=` added nothing.
- The renderer never survived long enough to be sampled, so the process list showed **no renderer at
  all**. The earlier "they launch and are killed instantly" reading was **wrong** — the original
  "they never appear" observation was right.
- Only **UNTRUSTED**-tier children lose the environment. The GPU process (LOW) kept `HODOS_DEV`,
  passed the guard, and ran — which is exactly why this looked renderer-specific rather than
  environment-specific.

**Fix (`cef_browser_shell.cpp :: RunHodosMain`):** compute `is_child_process` from `--type=` on the
**command line** (never the environment — that is the very thing a sandboxed child loses) and run the
safeguard only when false. Nothing is lost: children are spawned by the browser process, which has
already enforced it, and no child-process code resolves the dev/prod namespace — every
`PortConfig`/`AppPaths` consumer (`HttpRequestInterceptor`, `WalletService`, `SingleInstance`,
`AdblockCache`, browser-side `simple_handler`) is browser-process only.

> ⚠️ **Generalise this.** Anything gated on an env var is a no-op in a sandboxed child. Three
> diagnostics in this very session were env-gated and silently never ran, producing three false
> "exonerations" that had to be redone against the command line. If a child must know something,
> pass it as a **switch** (the way `OnBeforeChildProcessLaunch` already passes `--profile=`).

### How it was isolated (reusable ladder — each rung is cheap and decisive)

1. **CEF's own `cefsimple` built WITH `CEF_USE_BOOTSTRAP` + sandbox ON** → renderers UNTRUSTED and
   *running*. Kills the leading hypothesis outright: **a sandboxed child CAN load a client DLL**, so
   this was never an upstream CEF-150 bootstrap+sandbox interaction. (Build: configure
   `cef-binaries/` itself with `cmake -S cef-binaries -B <dir> -G "Visual Studio 17 2022" -A x64`;
   `USE_SANDBOX` defaults ON, which is what defines `CEF_USE_BOOTSTRAP`. Build just the `cefsimple`
   target — cefclient is far larger and was never needed.)
2. **Ran that same `cefsimple.exe` from OUR `build/bin/Release`**, against our `libcef.dll`, our
   resources, our LPAC ACLs → still fine. Exonerates the whole environment: paths, ACLs, engine build.
3. **Passed our full switch set** (`--lang`, `--remote-allow-origins`, `--disable-gpu-compositing`,
   `--disable-features=…GlicActorUi`, `--js-flags=--expose-gc`, `--off-screen-rendering-enabled`,
   our `--log-file`) to cefsimple → still fine. Exonerates `CefSettings` and the command line.
4. **`CefRequestHandler::OnRenderProcessTerminated`** (temporarily added to `SimpleHandler`) →
   `status=TS_PROCESS_WAS_KILLED error_code=1 RESULT_CODE_KILLED`. **This is the single highest-value
   instrument for any renderer-death question — reach for it first next time.** It gave the answer in
   one run after cdb, crashpad and verbose logging had all produced nothing.
5. Skipped the pre-`CefExecuteProcess` prologue in children → renderers came back. Then narrowed to
   the safeguard alone by re-enabling `CoInitializeEx`, DPI awareness and `WindowManager` (all three
   are harmless in a sandboxed child).

### Verify with

`development-docs/0.4.0/chromium-rebuild/check-sandbox.ps1`. ⛔ `no_sandbox=0` is **not** proof —
read child **token integrity**. MEDIUM = unsandboxed; LOW/UNTRUSTED = sandboxed.

### ✅ LPAC ACLs: NOT required — tested 2026-08-05, no build or installer change needed

The previous session applied `icacls <out dir> /grant *S-1-15-2-2:(OI)(CI)(RX)` by hand and left it
in place, so it was never known whether the sandbox depended on it. It does not:

- The ACE was **removed entirely** (`icacls <dir> /remove:g "ALL RESTRICTED APPLICATION PACKAGES"`)
  and audited off `HodosBrowser.exe`, `HodosBrowser.dll`, `libcef.dll` and `resources.pak` — it had
  in fact **never been on the files at all**, only on the directory, because those files carry
  explicit non-inherited DACLs.
- With no LPAC ACE anywhere: **12 renderers, all UNTRUSTED, 0 errors, 0 `Tab Killed`.**

**Why:** LPAC ACLs only matter for **AppContainer** processes, and ours are not. The `AppContainer`
column in `check-sandbox.ps1` is empty for every process — Chromium is not applying the App
Container / LPAC renderer mitigation in this configuration. The directory is now on **default
user-profile ACLs**, which is a closer match to a real install than the hand-patched state was.

> 🔭 **Tripwire, already instrumented.** `check-sandbox.ps1` prints the `AppContainer` column and its
> header comment says so. If a future Chromium/CEF bump ever flips renderers to AppContainer, that
> column reads `yes` and LPAC ACLs become **mandatory in both `CMakeLists.txt` (cf. CEF's own
> `SET_LPAC_ACLS`) and the Inno installer**. Until then, adding untested ACL manipulation to the
> installer is risk without benefit.

### Verifying the sandbox on a REAL install (the only proof that counts for users)

`check-sandbox.ps1` now takes `-Path`, so it works against an installed build, not just dev:

```powershell
.\check-sandbox.ps1 -Path "$env:LOCALAPPDATA\HodosBrowser"
```

**PASS = every renderer UNTRUSTED, GPU LOW, ~12-14 renderers.** ZERO renderers is the classic
"sandbox on but broken" signature. Run this on the first installed 0.4.0 build — it folds naturally
into the N-1 → N update-apply test already owed before promote. For reference, running it against
the currently-installed 0.3.x prints MEDIUM everywhere with `--no-sandbox PRESENT`, which is exactly
what this commit changes.

### ✅ Startup cost: ~45 ms, measured

Same binary, sandbox toggled with `--no-sandbox` on the command line (`command_line_args_disabled`
is false), 8 runs each, medians:

| Milestone | Sandbox ON | Sandbox OFF | Δ |
|---|---|---|---|
| First paint (skeleton window) | 52 ms | 54 ms | **−2 ms** (control) |
| `CefInitialize` done | 210 ms | 200 ms | +10 ms |
| Entering message loop | 278 ms | 249 ms | +29 ms |
| First renderer V8 ready | 522 ms | 476 ms | **+46 ms** |

First paint happens *before* `CefInitialize` and therefore **cannot** be affected by the sandbox —
its ~0 delta is the control that validates the harness, and its 37–139 ms spread is the run-to-run
noise floor. Against the ~2 s first-paint budget from the startup-optimization work, +46 ms to first
content is ~2% and smaller than the noise between consecutive cold starts.

### Still owed on this commit

- The interactive half of the §14.4 smoke: overlay **keyboard input**, file dialogs, downloads.
  Automated here: overlays render, V8 + bridge inject, adblock + farbling active, real sites load.

### Historical: what the first attempt had already established



1. **Threaded `sandbox_info` through** from `RunWinMain` → `RunHodosMain` → **both**
   `CefExecuteProcess` and `CefInitialize`, and made `no_sandbox` conditional
   (`if (!sandbox_info) settings.no_sandbox = true;`), matching CEF's own
   `tests/cefclient/cefclient_win.cc :: RunMain`. **Necessary but NOT sufficient.**
2. **Added the LPAC ACLs** (`icacls <out dir> /grant *S-1-15-2-2:(OI)(CI)(RX)`, mirroring CEF's
   `SET_LPAC_ACLS`). Verified applied — `icacls` shows
   `ALL RESTRICTED APPLICATION PACKAGES:(OI)(CI)(RX)`.

**⭐ The load-bearing discovery: `settings.browser_subprocess_path` silently disables the sandbox.**
With it set (as it has always been, to the main exe path), the browser process reported
`no_sandbox=0` **and** `sandbox_info=non-null` at `CefInitialize` — and yet *every* child process was
spawned with `--no-sandbox` and ran at **MEDIUM** integrity. Removing it flipped that instantly:
`--no-sandbox` disappeared from every child and the GPU process came up at **LOW integrity —
genuinely sandboxed**. On Windows an empty value already means "use the main process executable",
which under the bootstrap model is what we want, so setting it buys nothing and costs the sandbox.

> Do not trust "no_sandbox = 0" as evidence the sandbox is on. It is not. Read the child process
> **token integrity level** — MEDIUM means unsandboxed no matter what the settings say. The helper
> used here is kept at `scratchpad/check-sandbox.ps1` (OpenProcessToken + TokenIntegrityLevel).

### The blocker as it appeared then

With the sandbox genuinely engaged, **zero renderer processes start** — they never appear at all.
All 11 overlay browsers log as "created" in the browser process and the window paints **completely
blank**. Browser + GPU + one utility survive; nothing else. *(This reading was correct; the later
"they launch and are killed instantly" revision below was the wrong one. The processes exit so fast
they are never sampled.)*

Ruled out:

- **`RendererCodeIntegrity`** — the obvious suspect, since an unsigned dev DLL being refused by a
  sandboxed renderer would fit the symptom exactly. Added to the `disable-features` list; **no
  change**. (Note it must be added to *that list* — `AppendSwitchWithValue` replaces, so a
  command-line `--disable-features` is silently discarded.)
- **Missing LPAC ACLs** — verified present, see above.

### ✅ Follow-up 2026-08-04 (later the same day): logging fixed, and TWO decisive results

**1. The engine's log is fixed** (`f0f3be5`) — `settings.log_file` now resolves through
`AppPaths::GetLogDir()`. Chromium can report again, and it immediately did:

```
WARNING:chrome\browser\ui\sad_tab.cc:258] Tab Killed: http://127.0.0.1:5137/
```

repeated ~10× on a 1-second cadence. ~~So the renderers launch and are killed instantly, i.e. a
genuine crash-loop.~~ **❌ That inference was WRONG** — see the root cause above. `Tab Killed` is the
browser process reporting that the *render process host* went away; it says nothing about whether a
process ever ran. The renderer exits within milliseconds via a plain `return 1`. The lesson:
`sad_tab.cc` is not evidence of a crash, and a crash-*loop* reading sent the next session hunting a
crash that does not exist.

**2. ⭐ CEF 150's own `cefclient.exe` sandboxes correctly on this machine.** Ran the prebuilt binary
from the `..._client` distribution: renderers at **UNTRUSTED**, GPU at **LOW**, a utility at
**UNTRUSTED** — and they all *run*. So the sandbox is **not** broken on this hardware, not broken in
CEF 150, and not blocked by machine policy. **The fault is in our embedder.**

That comparison also surfaced what looked like the structural difference that mattered: the prebuilt
`cefclient.exe` is a **standalone executable**, whereas ours is the **bootstrap + client-DLL** model,
so every sandboxed child has to load `HodosBrowser.dll` first. ~~Leading hypothesis: a sandboxed
child cannot load the client DLL.~~ **❌ REFUTED** — `cefsimple` built with `CEF_USE_BOOTSTRAP` loads
its client DLL in an UNTRUSTED renderer perfectly (step 1 of the ladder above). Worth keeping only as
a reminder that "the most structurally striking difference" and "the cause" are not the same thing.

### ⚠️ Historical: Chromium's own log was broken (FIXED — see above)

`settings.log_file` is set to the **relative** path `"debug.log"`, which Chromium rejects on every
launch with `Invalid logging destination: debug.log`. That means the engine cannot tell us why the
renderers die — the whole investigation above ran blind. Point it at an absolute path under the
profile's `logs/` directory before attempting the sandbox again. (Workaround used here: pass
`--enable-logging --log-file=<abs> --log-severity=verbose` on the command line; it produced 8167
lines with **no** sandbox diagnostics, only unrelated "Privacy Sandbox" component-updater noise.)

Remaining smoke gate, per `DevOps-CICD/TESTING.md` §14.4: overlay **keyboard input**, file dialogs,
downloads. (Overlays rendering, V8/bridge injection, adblock + farbling and real-site loads are all
verified above.)

> macOS still sets `no_sandbox = true` unconditionally in `cef_browser_shell_mac.mm` and was
> deliberately **not** touched — a separate change, on a platform this session could not test. See
> `../MAC_WINDOWS_RELAY.md` for what does and does not transfer.

---

## S3 — Logging cleanup (small, own commit, needs a smoke on every process type)

The `freopen_s` landmine is defused at the entry point, but the underlying defect is intact:
`Logger::Log` echoes **every** line to `std::cout` unconditionally, even when the log file is open.
That is both a double-write (if the redirect ever starts working) and a hard dependency on `stdout`
being alive in every process — renderer, GPU, utility, helper.

Recommended shape:
1. Make the `std::cout` echo a **fallback**, used only when `logFile` is not open.
2. Then decide the redirect's fate: it has **never once succeeded** (always `EACCES` — `Logger`
   already holds the file). Either delete it as dead code, or make it work by redirecting *before*
   `Logger::Initialize` opens the file, or by pointing it at a sibling `debug_stdout.log`.
3. Keep the `NUL` reopen regardless — `Logger` is not the only thing in the process that can write
   to `stdout` (CEF, OpenSSL, SQLite, third-party libs all can).

Do **not** fold this into an engine commit. It touches every process type and deserves its own smoke.

---

## S4 — Then, and only then, the actual 0.4.0 feature work

P3 patch toolchain (fork `chromiumembedded/cef` → `Hodos-Browser/cef`, `patch.cfg`, prove a NO-OP
patch applies and builds) → P4 farbling into Blink, C1..C7, **~30–60 min incremental rebuild each**,
each sub-step atomically deleting its JS counterpart. Budget ~3–5 h for all of P4, not ~25 h.

---

## New tickets from the 2a smoke (2026-08-04) — small, independent, good filler work

- **Omnibox cold-start race.** First thing typed after launch shows "No suggestions":
  `omnibox_update_query` is delivered ~400 ms before the overlay's React app mounts and is dropped.
  Fix by replaying the last query when the overlay signals ready, or holding the query until
  `allSystemsReady`. See `TESTING.md` §14.6.
- **`clearRange` leaves a stale `last_visit_time`.** Deletes visit rows without recomputing the
  URL's last visit, so cleared-range entries still show a timestamp inside the cleared window.
  **Pre-existing** — 2a never touched `HistoryManager.cpp`.
- **⚠️ DevTools / CDP hardening** — **owner-approved 2026-08-04**, design + open questions in
  `../DEVTOOLS_SECURITY_DESIGN.md`. Four decisions: keep DevTools on in production (D1), close the
  remote debugging port in release (D2), drop `--remote-allow-origins=*` (D3), scope DevTools away
  from the wallet/overlay origins (D4). D2+D3 are one small commit and nothing in the repo depends
  on the removed surface; D4 routes the four DevTools entry points through one guard. Does not block
  the CEF bump — land any time after S0/S1. Open: Q1 (dev-only vs default-off setting) and Q2
  (role-only vs role+URL) need owner answers.

## Standing debt (unchanged, still owed)

- **2a smoke on macOS** — ✅ done on Windows (`TESTING.md` §14.6), still owed on the Mac, where this
  path was previously dead and is expected to start working.
- **beta.29 AV seeding.**
- **The two CI asserts** from `TESTING.md` §14.5: cert-thumbprint equality across
  exe/dll/`chrome_elf`; staging manifest completeness (half done — `HodosBrowser.dll` is now in
  `build-release.ps1 $requiredFiles`).
- **A real N-1 → N update apply test before promote.** The `{app}` manifest changed shape: a partial
  update that replaces the exe but not the DLL now hard-`LOG(FATAL)`s where it used to survive.
- DEP-1a/b/c (vcpkg manifest, Inno 6.7.1, Brewfile) still first-exercised in CI.
