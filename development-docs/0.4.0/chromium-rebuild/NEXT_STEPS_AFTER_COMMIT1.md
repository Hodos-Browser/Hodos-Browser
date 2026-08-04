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

## S1 — Commit 3: icon + VERSIONINFO ⭐ USER-VISIBLE, DO IT EARLY

**Confirmed by inspection on 2026-08-04:** `HodosBrowser.exe` is byte-identical to CEF's
`bootstrap.exe` and carries **CEF's** icon and version resources; our `hodos.ico` lives in
`HodosBrowser.dll`. That is why the taskbar shows the wrong logo today.

Split the two icons clearly, because they have different fixes:

| Icon | Source | Status |
|------|--------|--------|
| Window icon (title bar, Alt-Tab) | `LoadImage(g_hResourceModule, MAKEINTRESOURCE(1), …)` reading `hodos.rc` out of the DLL | **Fixed in commit 1** — needs a visual confirm |
| Exe icon (taskbar button, Explorer, pinning) | the `.rsrc` of `HodosBrowser.exe` = bootstrap's | ❌ **Commit 3** |

Implementation (already decided, do not re-litigate): a CMake `POST_BUILD` step that opens the
copied `HodosBrowser.exe` with `BeginUpdateResource` / `UpdateResource` / `EndUpdateResource` and
stamps in our `RT_GROUP_ICON` + `RT_ICON` + `VS_VERSIONINFO`. Patching CEF's `bootstrap.rc` through
the P3 patch toolchain was **considered and rejected** — it needs a full Chromium rebuild per icon
change and welds branding into CEF.

Watch for: the taskbar also consults the per-profile **AUMID** set by `SetupTaskbarProfile`, so
verify with more than one profile before calling it done.

---

## S2 — Commit 2b: turn the Chromium sandbox ON

Currently `settings.no_sandbox = true` on **both** platforms (Windows explicitly since commit 1;
macOS unconditionally in `cef_browser_shell_mac.mm`). The bootstrap already hands us a real
`sandbox_info` — commit 1 deliberately ignores it.

- Pass `sandbox_info` through from `RunWinMain` and drop `settings.no_sandbox = true`.
- Add `SET_LPAC_ACLS`-equivalent handling: CEF's own cmake applies LPAC ACLs to the output dir for
  Windows sandbox support (`tests/cefclient/CMakeLists.txt`). Ours does not yet.
- **Expected failure shape is a renderer crash-loop at startup.** That is exactly why this is its
  own commit.
- Real exposure being closed: an unsandboxed renderer can socket straight to the wallet port,
  bypassing the C++ interception layer and every permission gate.
- Smoke per `DevOps-CICD/TESTING.md` §14.4: overlays render and take keyboard input; file dialogs
  open; downloads write; adblock + farbling still inject; wallet bridge round-trips.

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
