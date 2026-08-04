# Next steps after commit 1 (bootstrap migration) — written 2026-08-04

**State:** Windows dev app **runs on CEF 150 / 7871** at `1f98dba`. Tree clean.
**Read first:** `SESSION_HANDOFF_7871_BUILD_GREEN.md` → `../MAC_WINDOWS_RELAY.md` → this.

The four-commit bootstrap plan is half done: **2a `83fe472`** (history off the renderer) and
**commit 1 `1f98dba`** (bootstrap migration) are landed. **2b** and **3** remain.

---

## S0 — Stage the 150 distribution into `cef-binaries/` ⭐ DO THIS FIRST

Owner decision (2026-08-04): **dev must run on the same binaries the next release ships.** So stop
passing `-DCEF_ROOT` and make the repo default correct.

`cef-binaries/` is **gitignored** (`.gitignore:27`, 0 tracked files), so this is a local-machine
operation, not a commit. But it is **not** only local — see S0.3, which is the part that bites.

1. **Archive M136.** A backup already exists at `C:\cef\cef150\m136_cef_binaries_backup\` (438 MB)
   from the 7871 build session. Verify it before overwriting anything.
2. **Copy the 150 distribution over `cef-binaries/`** — the whole thing, including
   `build_wrapper/libcef_dll_wrapper/Release/`, because the CMake wrapper probe accepts that layout
   and it saves rebuilding the wrapper. Then reconfigure **without** `-DCEF_ROOT` and confirm the
   default path resolves. The `bootstrap.exe` existence gate should pass silently.
3. **⚠️ Replace the CI asset too — this is the step that is easy to miss.**
   `release.yml:113-128` does `gh release download cef-binaries --pattern "cef-binaries-windows.zip"`.
   Staging locally does **nothing** for CI. Until that release asset is re-uploaded with the 150
   distribution, the next release build downloads M136, and configure fails at the `bootstrap.exe`
   gate. That failure is *loud and correct* — but it will fail. macOS has the same coupling at
   `release.yml:440` (`cef-binaries-macos.tar.bz2`).
4. **Bump `AboutSettings.tsx:39`** — hardcoded `"Chromium (CEF 136)"` → 150. It must move *with* the
   binaries, not before. Better long-term: surface the real `CEF_VERSION_MAJOR` from C++ instead of
   a literal that silently rots every bump.
5. Update the pin table in `cef-native/CLAUDE.md` (currently documents both platforms side by side).

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

## Standing debt (unchanged, still owed)

- **Runtime smoke of 2a** — history page + omnibox suggestions. Built and typechecked, **never run**.
  Cheapest to do right now while a working dev app is in front of you.
- **beta.29 AV seeding.**
- **The two CI asserts** from `TESTING.md` §14.5: cert-thumbprint equality across
  exe/dll/`chrome_elf`; staging manifest completeness (half done — `HodosBrowser.dll` is now in
  `build-release.ps1 $requiredFiles`).
- **A real N-1 → N update apply test before promote.** The `{app}` manifest changed shape: a partial
  update that replaces the exe but not the DLL now hard-`LOG(FATAL)`s where it used to survive.
- DEP-1a/b/c (vcpkg manifest, Inno 6.7.1, Brewfile) still first-exercised in CI.
