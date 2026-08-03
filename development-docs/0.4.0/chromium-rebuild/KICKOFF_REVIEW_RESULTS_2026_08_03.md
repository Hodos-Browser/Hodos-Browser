# Kickoff Review Results — Chromium/CEF Rebuild, Windows Execution

**Run:** 2026-08-03 · **Input:** `SESSION_BRIEF_WINDOWS_EXECUTION.md` §4 Phase 1
**Status:** Kickoff COMPLETE. Doc fixes landed. **No build started. D4 + D9 awaiting owner.**

> **Read order for the next session:** `SESSION_BRIEF_WINDOWS_EXECUTION.md` → **this doc** →
> `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`. This doc records what the review *found* and what changed as
> a result. It does not restate the plan. Where this doc and an older plan doc disagree, **this doc
> wins** — the corrections have been landed inline in the plan docs too, but this is the ledger.

---

## 0. Verdict

The plan set is **sound and should be executed as written**. D1 is confirmed more strongly than the
brief claimed. Eight material divergences were found; the corrections are landed. **One genuine safety
hazard** was found in the P4 teardown (§4) — it must be handled before TD-1.

---

## 1. Machine state (measured, not assumed)

| Item | Value |
|---|---|
| CPU / RAM | i9-12950HX, **16C / 24T**, **31.7 GB** — RAM is exactly at the floor; close apps during link |
| Disk | **C: 1203 GB free** / 1862 GB, NTFS · D: 686 GB free |
| Toolchain | **MSVC 14.44.35207** (VS2022 17.14) ≥ CEF's stated 17.13.4 for `7871` |
| Windows SDK | **10.0.26100** + 10.0.22621 present; **Debugging Tools present** |
| Python on PATH | 3.10.11 (in range; depot_tools ships its own; `.vpython3` wants 3.11) |
| `depot_tools` | **TWO copies.** PATH resolves `gclient` → `D:\cef\depot_tools`; the build script prepends `C:\cef\depot_tools` and wins. **Footgun for any command run outside the script** |
| Defender exclusions | **UNVERIFIED — needs admin.** Owner action |
| Windows Update | **NOT paused — needs admin.** Owner action. The 2026-03-12 build was killed by a forced restart at 78,821/96,000 objects |

**Existing build trees:**

| Path | Size | State |
|---|---|---|
| `C:\cef\chromium_git` | **175.1 GB** (1,296,510 files) | **Complete M136 tree.** `binary_distrib` present with `cef_binary_136.1.7+g15882fe+chromium-136.0.7103.114_windows64*`; `cef/` is a live git repo at `7103`; `out/Release_GN_x64/args.gn` intact; `.ninja_log` dated 2026-03-12 |
| `D:\cef\chromium_git` | **117.3 GB** (999,393 files) | **Partial duplicate.** No `binary_distrib`; `cef/` is not a git repo. Looks abandoned. Reclaimable — **owner has not approved deletion** |
| `C:\cef\CEF_from_source` | 0.5 GB | The staged `cef-binaries-windows.zip` |

**The live M136 `args.gn` (ground truth, beats any doc):**
```
chrome_pgo_phase=0   enable_widevine=true   ffmpeg_branding="Chrome"
is_official_build=true   proprietary_codecs=true   is_component_build=false
enable_cdm_host_verification=true   enable_cdm_storage_id=true
```
Byte-identical to the documented `GN_DEFINES`. No `use_siso` line → M136 built under Ninja.
**Note for Q4/DRM:** `enable_cdm_host_verification=true` is already set — relevant to the VMP question.

---

## 2. D1 — confirmed, and a doc contradiction resolved

**Target: CEF 150 / branch `7871`, pinned `150.0.17+g94c1726+chromium-150.0.7871.187`.**

Two repo docs disagreed on whether CEF even *has* an LTS program. A 2026-06-17 note said no; the
2026-07-10 plan said yes. **Resolved from primary sources: LTS is real. The 2026-06-17 note is
retracted.**

Root cause of the confusion, and the trap: `cef-builds.spotifycdn.com/index.json` has **no `lts` enum**
— LTS builds are labelled `"stable"` (reproduced locally: only `stable`/`beta` across 1,195 `windows64`
builds). The June note read the JSON; the authority is the **website table**.
**⇒ Automation must key off branch number `7871`, never the JSON `channel` field.**

| Fact | Value |
|---|---|
| `7871` channel today | **Stable** — the build-day gate is **SATISFIED** (it was Beta on 2026-07-10) |
| M150 LTS window | stable 2026-06-30 · **LTC 2026-07-21** · **LTS 2026-10-06** · refresh ends **2027-04-13** |
| M149 / `7827` fallback | **DEAD** — already in CEF's *Unsupported* table |
| Why not 151 | M151 is not a 6th branch → **no LTS at all** |
| Coverage caveats | Platform-agnostic Chromium fixes only; **CEF's own fixes are not backported** (maintainer, issue #3947) |

Corroboration: M144 (`7559`, LTS) shipped a fresh build 2026-07-31 — newer than any M150/M151 build —
while M149 is already unsupported. An LTS branch outliving a newer stable one only makes sense if LTS
is real. Full record: `../../DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` §Version-lock.

---

## 3. Verified unchanged M136 → M150 (no action)

- **Codec GN args** — `proprietary_codecs` and `ffmpeg_branding` unchanged in `features.gni` /
  `ffmpeg_options.gni` @ 7871. No rename, no value change. Our `GN_DEFINES` is valid as-is.
- **Python** — `.vpython3` pins **3.11** on `7871`, same as `7103`.
- **CI runner pins** — no live `*-latest` anywhere. ⚠️ But `test.yml:138/145/151` carry
  **commented-out** `windows-latest`/`macos-latest` in a disabled `cpp:` job — a re-drift trap VER-3
  should clean up.
- **TD-1 line refs** — `simple_render_process_handler.cpp:581–627` (FP block) and `:567–579` (adblock
  scriptlet) are **exact today**. The older `:586-632` is the stale one.
- **A dynamic minos guard already exists** (`release.yml:621-645`) — reads the framework's real minos,
  so VER-4 is **fail-loud**, not silent-drift.

---

## 4. 🚨 The one real hazard — BOT-1

**`navigator.webdriver = false` is NOT at `:629-653`.** It lives at **`FingerprintScript.h:128-133` —
inside the block TD-1 deletes.** So does the **`navigator.plugins` spoof (`:99-126`)**. Both are
**anti-bot** surfaces, not farbling. Only the `window.chrome` stub is genuinely at `:629-653`, and it
survives (own `isExternalPage` gate).

**Performing TD-1 as written silently drops both.** No compile error, no test failure — field-only
bot-detection regressions.

Not hypothetical: `simple_app.cpp:92-94` records Cloudflare Turnstile rejecting detectable browsers on
**whatsonchain.com**, in our own BSV regression basket. Git history agrees — `4fad37b` *"Fix Cloudflare
bot detection blocking (B-5)"* landed the **same day** as `b514c30`, the refactor that removed WebGL
GPU-string spoofing.

**Required:** re-home `navigator.webdriver` + `navigator.plugins` into the independent `window.chrome`
stub **as its own commit, before any teardown step.** They must survive **both** the JS-block deletion
**and** a per-site farbling opt-out — a user disabling farbling on a site must not thereby announce
they are automated.

---

## 5. Divergences corrected (landed in this commit)

| # | Was | Now |
|---|---|---|
| 1 | VER-5 targets "hardcoded copy-lists in `cef-native/CMakeLists.txt`" | **No such list exists** — it's wholesale `copy_directory`. Real targets: **`installer/hodos-browser.iss:68-72` extension whitelist** (`*.dll`/`*.bin`/`*.dat`/`*.pak`/`*.json` + `locales\*` — anything else is **silently dropped at packaging**, invisible to a from-source smoke), `CEF_HELPER_APP_SUFFIXES` (`CMakeLists.txt:539-545`), `release.yml:604` |
| 2 | P3 "GREENFIELD" | Upstream CEF ships **105 patches** already applied every build. Mechanism proven; only **our fork + patches** are new |
| 3 | Runbook: Ninja `.ninja_log` resumability | **Siso is default** on a fresh `7871` out-dir. State = `.siso_fs_state` + journal. One Ctrl-C graceful, second aborts. Runs fully local |
| 4 | Runbook LTS text "retracted" | **Vindicated** — see §2 |
| 5 | Runbook "Python 3.12+ breaks" | Ceiling is per-branch `.vpython3`; both branches want 3.11 |
| 6 | `scripts/build_hodos_cef.bat` | Lives at **`development-docs/DevOps-CICD/scripts/`**. CEF-5 reduces to a *move* decision, not authoring |
| 7 | BOT-1 citation | §4 above |
| 8 | Runbook L82 → 7103 row; "runbook §7.3" | Row is L81; §7.3 belongs to the archived master plan |

---

## 6. Not yet done — carried forward

**DEP-1a..d: all four pins MISSING.** No `vcpkg.json` anywhere (CI is classic-mode, unpinned:
`release.yml:172`), no Inno `--version` (`release.yml:160`), no `Brewfile` (`release.yml:468` floats),
no `rust-toolchain.toml` (`@stable` live in 4 places). Two additions the plan doesn't cover:

- **`rust-wallet/Cargo.toml:29` `actix-web = "4.9"` is an unpinned caret** — undercuts the rustc-ceiling
  argument that justifies the adblock-engine exact pins. Only `Cargo.lock` holds it.
- The commented-out `*-latest` in `test.yml` (§3).

**VER-6: not single-sourced.** No `cargo-release`, no `shadow-rs` — neither exists in the repo. CI is
genuinely tag-derived, but three stale local-build fallbacks disagree: `hodos-browser.iss:5` =
`0.3.0-beta.18` (11 releases stale), `build-release.ps1:5` = `0.1.1-alpha.1`, `CMakeLists.txt:48` =
`0.2.0-dev`. Neither Rust crate's version participates.

**macOS floor 11.0 was never `vtool`-measured** (tracker marks it provisional). VER-4's
`max(12.0, measured)` therefore has no prior measurement to compare against. Mac-owned.

---

## 7. Reuse-first audit — every P4 anchor already exists

| Need | Exists at |
|---|---|
| Auth-domain allowlist (C7/TD-4) | `FingerprintProtection.h :: IsAuthDomain` (`:191-270`, ~35 entries). **C++ only — no Rust equivalent** |
| Per-site toggle (TD-5/C7b) | `IsSiteEnabled`/`SetSiteEnabled` (`:123-145`) + live IPC chain → `PrivacyShieldPanel.tsx`. **Do not delete before `ShouldFarble` consumes it** |
| Persistent per-profile seed | `fingerprint_settings.json` + `LoadSiteSettings`/`SaveSiteSettings` (`:149-187`), via `ProfileManager::GetCurrentProfileDataPath()`. **`profile_seed` slots in with zero new path plumbing** |
| Global toggle | `SettingsManager` `PrivacySettings::fingerprintProtection` → `SetEnabled` at startup |

**Consolidate, don't duplicate:** domain extraction has **two copies** —
`FingerprintProtection.h :: ExtractDomain` (`:275-282`) and an inline duplicate at
`simple_handler.cpp:7491-7502` — and **both are host-only, not eTLD+1**, which the plan's seed keying
assumes.

**CSPRNG:** `FingerprintProtection.h:47` uses **`CryptGenRandom`** (deprecated). The plan's call for
`BCryptGenRandom` is correct and the citation is exact.

**⚠️ Stale layer doc:** `cef-native/include/core/CLAUDE.md` claims `FingerprintScript.h` farbles
`WebGL (getParameter, readPixels)` and `Navigator (hardwareConcurrency, deviceMemory, plugins)`. **Only
`readPixels` and `plugins` are real.** Do not scope the Blink patch off that table.

---

## 8. Risk — the UX safeguards

**P0–P2 do not touch them.** Gold pill, right-click revoke, "Always notify", privacy-perimeter gates
and per-session counters are Rust + React; a CEF bump can only break them at compile time, loudly.

The real exposure is the **23 CEF interface types we implement** (12 on `SimpleHandler` alone) across a
14-milestone jump. `CefResponseFilter` — which strips YouTube ads — is already flagged LOW-stability in
the tracker; re-verify it still exists and still streams.

---

## 9. Decisions

### Closed by evidence
- **D4 (WebGL `UNMASKED_VENDOR`/`RENDERER`) — recommend DROP.** The current build **does not farble
  these at all**; there is no `getParameter` hook in `FingerprintScript.h`. "Drop" = exact status quo,
  zero regression risk, and it removes the Mac GPU-string set from D3's scope.
  **History:** we shipped GPU-name spoofing 2026-02-28 (`0b7288b`) and removed it 2026-04-01
  (`b514c30`), same day as the Cloudflare bot-detection fix. Rationale is in the file: a fake GPU name
  contradicts the real WebGL extension list and rendering behavior, so detectors catch the lie.
  **Brave, for reference (public docs only — no source read):** they tried per-site randomization
  (2020), found it made users *more* unique (2022: one user measured 1-in-223,253 on EFF's tool), stated
  in Nov 2024 that randomizing *"causes too much website breakage for not much privacy advantage"*, and
  in **May 2026 pivoted to returning the literal constant `"Brave"` for every user** (confirmed by
  web3dsurvey.com field telemetry at ~100% share). **That herd defense depends on a huge user base and
  does not transfer to us** — `"Hodos"` would be a strong identifier, not a weak one. Reporting the
  truth puts us in the largest available crowd for this signal. **Revisit if the user base grows.**

### Awaiting owner
| # | Decision | Recommendation |
|---|---|---|
| **D9** | **P2a: re-build M136, or invoke the I5 guarded fallback?** `7103` is unsupported upstream; local CEF HEAD reads *"Pin depot_tools version for out-of-support branch."* But a complete tree + shipped binary exist on disk | **Fallback.** Use the existing tree + shipped binary as the last-known-good baseline; run the codec Layer-A probe against the **live M136 build** to capture the pre-bump baseline; skip a 10–12 h re-build of a dead branch. Go straight to P2b |
| **D10** | **Siso vs Ninja** | **Build on Siso** (the default). Keep `use_siso=false` as a documented escape hatch, not the plan |
| **D11** | **Disk layout** | Keep the M136 tree; build `7871` into a **second tree** (`C:\cef\cef150\`). 1203 GB free makes this comfortable and keeps D9 reversible. Separately: fix the `depot_tools` PATH ambiguity. **D: duplicate deletion needs owner approval** |

**Deferred to their phases per the brief:** D3 (mac arch), D5 (C2 seed channel), D7 (signing sequencing).

---

## 10. New work item opened

**B5 — `window.CWI` injection privacy** → `../WALLET_PROVIDER_INJECTION_PRIVACY.md`.
We inject the wallet provider on **every external HTTPS main-frame page**, so any site detects Hodos
with `typeof window.CWI !== 'undefined'` — and learns the user holds a BSV wallet. This defeats the
entire P4 farbling effort for *identification* purposes. **Scoped as an open question with options, no
design committed. Sequenced AFTER P7** — it edits `simple_render_process_handler.cpp`, the same file
TD-1 edits, and landing both together risks silently dropping one.

---

## 11. Immediate next steps

**Owner (admin required — blocks the build):**
1. Defender exclusion for the build directory (2–5× speedup on millions of small files).
2. Pause Windows Update / disable auto-restart. **Hard-kill resume is unproven under Siso.**
3. Answer **D9** and confirm **D4**.

**Then (next session):**
4. P1 pin + **DEP-1a..d** as four small commits — pure prep, no build, gates P2b.
5. Codec Layer-A probe against the live M136 build (~10 min) — captures the pre-bump baseline.
6. P2b: new tree, `--branch=7871`, `gn args --list` **pre-flight before the 10–12 h build** (a renamed
   or flipped flag ships a green build with no codecs), then kick the build off in the background.

**First milestone worth aiming at (unchanged from the brief): a clean `7871` build with codecs
verified, before any source patching.**

---

## 12. Unresolved, non-blocking

- **Windows SDK conflict.** CEF's table says `10.0.26100.4654` for `7871`; Chromium's own 7871 docs say
  `10.0.26100.7705`. Provision from `build/vs_toolchain.py` on the synced branch. We have 10.0.26100.
- **VS2022/VC143 vs VS2026/VC145.** VC143 is still listed and supported, but VC145 is now Google's
  packaged and tested toolchain. **CANNOT ANSWER WITHOUT BUILDING** — the largest unverified risk in
  the current setup.
- **Siso hard-kill resume** — designed for it (journal), unproven for us.
- **CEF issue #4114** (open, zero comments): CEF's **stable**-channel policy after Chromium's Sept 2026
  two-week cadence is undecided. Doesn't threaten LTC/LTS; strengthens the case for anchoring on LTS.

---

*Kickoff per `/CLAUDE.md` "Phase kickoff workflow". Research bounded per the brief §4: 4 agents, 1
round, no question survived to a second. Corrections landed inline in the plan docs; this is the ledger.*
