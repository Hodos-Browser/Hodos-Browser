# CEF/Chromium Full-Build Runbook (Tier 1)

**Created:** 2026-06-01
**Status:** 🚧 DRAFT skeleton — grounded in `../build_hodos_cef.bat` + `../build_hodos_cef_mac.sh`;
sections marked **TODO** need to be filled/verified before we rely on this for a real build.
**Owner:** DevOps/CI-CD · **Covers:** A1 (self-build), A2 (latest stable), A3 (dependency bump), A5 (Tier 1)

> **Read this first — terminology.** We are a **CEF-based browser that does custom Chromium builds.**
> CEF is not an alternative to Chromium; CEF's `automate-git.py` downloads the full Chromium source,
> applies the CEF layer, and compiles `libcef`. Our shell (`cef-native/`) is built against that.
> "Full build" = this Tier-1 process: produce fresh CEF binaries. It is **expensive and infrequent**.
> The fast Tier-2 path (bug-fix app release that *reuses* these binaries) is in `BUILD_AND_RELEASE.md`.

## Why we self-build (settled — do not relitigate)

Stock CEF binaries are built `ffmpeg_branding=Chromium` → **no H.264/AAC/MP3** → video/audio broken
across the open web. We build with `proprietary_codecs=true ffmpeg_branding=Chrome` to fix that.
Self-build is **mandatory for codecs**, and is *also* the only way to do renderer-layer farbling (B1).
Widevine premium DRM (Amazon/Netflix) is a **separate** VMP-signing concern — see §6.

## Current known-good configuration (from our scripts)

| Setting | Value | Source |
|---------|-------|--------|
| CEF branch | `7103` (CEF 136 / Chromium 136) — **currently ~6 mo old** | both scripts |
| GN_DEFINES | `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0` | both scripts |
| Build tool | CEF `automate-git.py` (`--minimal-distrib --client-distrib --no-debug-build --force-build`) | both scripts |
| Win toolchain | VS 2022 BuildTools; `DEPOT_TOOLS_WIN_TOOLCHAIN=0` | `.bat` |
| Mac | Python 3.9–3.11 (NOT 3.12+); arch auto-detect (`--arm64-build`/`--x64-build`) | `.sh` |
| Resources | ~100 GB disk, 16 GB RAM min (32 rec), 4–6 hr first build | `.sh` header |
| Output | `chromium_git/chromium/src/cef/binary_distrib/cef_binary_136.*` | both scripts |

---

## The full-build checklist

### Step 0 — Decide WHY this full build is happening
Trigger is one of: (a) Chromium/CEF version bump (A2), (b) new/changed farbling patches (B1),
(c) codec/flag change, (d) Widevine/VMP change. Record the trigger in the build's changelog entry.

### Step 1 — Choose the CEF branch (A2: latest stable)
- CEF branches map 1:1 to Chromium milestones. Pick the latest **stable** CEF release branch (not
  beta/dev) from CEF's branch list. `7103` = M136; newer stable branch = newer Chromium.
- **TODO:** document exactly where we read "latest stable CEF branch" (CEF release page / `cef-builds`
  CDN / `magpcss` branch list) and how we map branch number → Chromium milestone.
- **Compatibility gate (A2):** before committing to a new milestone, list what a Chromium jump may
  break — CEF API changes (handler signatures), removed flags, V8/Blink behavior, our patch rebase.
  **TODO:** build a per-bump "what could break" checklist; diff CEF's release notes between branches.

### Step 2 — Apply OUR source modifications (before build)
1. **Codec flags** — confirm `GN_DEFINES` includes `proprietary_codecs=true ffmpeg_branding=Chrome`.
2. **Farbling patches (B1)** — NEW step once B1 lands. Apply our Blink farbling patches via CEF's
   `cef/patch/patch.cfg` mechanism (add our `.patch` files + register them) so they're applied to the
   Chromium source before compile. **TODO (B1 design session):** finalize the patch set (Canvas,
   WebGL, AudioBuffer, navigator) + worker-context hook + session-seed model. "Same as last build,
   or with these changes: ___" — log per build.
3. **Extensions** — **N/A on CEF.** Extensions are chrome-layer; self-build does NOT unlock them. Do
   not add extension patches here. (Strategic future item; see `0.4.0/B4-extensions.md`.)
4. **Any other custom patches** — list and version them. **TODO:** enumerate any existing patches.

### Step 3 — Build
- Windows: `build_hodos_cef.bat` (from `C:\cef\chromium_git\`). Mac: `build_hodos_cef_mac.sh`.
- **A1 pain-reduction (the real A1 work) — TODO/research:**
  - **sccache / caching** to avoid full recompiles.
  - **Remote/cloud build execution** or a dedicated build machine (GitHub-hosted runners CANNOT do a
    full Chromium build — disk + 6 hr limit). Options: self-hosted runner, large cloud VM (spot),
    EngFlow/BuildBuddy-style remote execution. Cost/benefit to be researched in the A1 deep-dive.
  - Linux: **placeholder only** — not a current target.

### Step 4 — Stage binaries
- Copy `cef_binary_136.*` output → `cef-binaries/`. Rebuild `libcef_dll_wrapper`, then `cef-native`.
- Publish the binary distribution to the `cef-binaries` GitHub release so the Tier-2 app pipeline
  (`release.yml`) consumes it. **TODO:** document the exact publish step + naming.

### Step 5 — Dependency reconciliation (A3)
After a Chromium/CEF bump, re-check everything pinned to the old engine and **annotate** what needs
updating (we deliberately don't bump these in isolation):
- Frontend: React/Vite/TypeScript + any browser-API-dependent JS/TS. **TODO:** list known-coupled deps.
- Rust (`rust-wallet`, `adblock-engine`): crates sensitive to platform/CEF (e.g. adblock 0.10.3 pinned
  for Rust 1.85). **TODO:** confirm.
- C++: vcpkg deps (nlohmann-json, sqlite3), quirc, OpenSSL.
- Record a per-bump "dependencies touched / deferred" table.

### Step 6 — Widevine / premium DRM (separate track)
- Basic DRM (CDM auto-download) works on the codec build already.
- Premium (Amazon/Netflix) needs **VMP signing** of our binaries. **TODO (if premium is a goal):**
  scope Castlabs commercial path vs Google MLA; this is its own mini-spike, not part of the routine build.

### Step 7 — Verify (acceptance gate)
- **Codecs:** `video.canPlayType('video/mp4; codecs="avc1.42E01E"')` → `'probably'`. Smoke video/audio
  on x.com, reddit, LinkedIn, YouTube.
- **Farbling (once B1 lands):** CreepJS / fingerprintjs detection sites show no "lie"; logins that
  broke before now work; **workers** report farbled values (the current gap).
- **Regression:** the standard site basket (CLAUDE.md Testing Standards) on **both Windows and macOS**.
- **TODO:** turn this into a concrete pass/fail acceptance checklist.

### Step 8 — Record the build
- Changelog entry: CEF branch, Chromium milestone, GN_DEFINES, patch set version, deps touched,
  verification results, build duration. This is the institutional memory for the next full build.

---

## Open TODOs to make this real
- [ ] Fill every **TODO** above from the real scripts/process (some live only in Matt's head + the .bat/.sh).
- [ ] A1 deep-dive: cloud/CI build feasibility + caching → kill the ~2-week pain.
- [ ] A2: latest-stable-CEF sourcing + per-bump breakage checklist.
- [ ] B1: farbling patch set + `patch.cfg` integration (its own design session).
- [ ] Decide whether premium DRM (VMP) is a product goal (own mini-spike).
