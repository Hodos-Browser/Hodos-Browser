# Auto-Update UX + Code-Signing — 0.4.0 Research & Design

**Created:** 2026-06-22 · **Status:** Research/design (NO code yet) · **Owner:** DevOps/CI-CD
**Canonical home:** `development-docs/DevOps-CICD/` · **Per root CLAUDE.md Invariant #12.**

> **Relationship to `AUTO_UPDATE.md`:** that file is the prior decision record (keep Sparkle+WinSparkle, EdDSA, security requirements). **This doc supersedes it on one critical point** — `AUTO_UPDATE.md §3/§5.1` implies the owner's "silent apply-on-quit" UX is reachable on Windows with WinSparkle config + `installerArguments`. **That is false** (proven below). Everything else in `AUTO_UPDATE.md` still stands; this doc sharpens Goal 1 to the owner's exact spec and adds the Goal 2 signing review.
>
> **Method:** every claim below was verified against source (2026-06-22) and run through an adversarial multi-agent review (live web research on WinSparkle/Sparkle/Azure-Signing + a skeptic pass that tried to refute each planned implementation). Verdicts in §7.

---

## 0. The two goals (owner-defined, do not drift)

- **Goal 1 — Auto-update UX:** app updates **when the user closes and reopens the browser** (background download while running → apply on next launch; **no mid-session forced restart**). **ON by default.** User keeps an **opt-in** option (Settings → About) to be **notified + approve** before an update applies. Ship fixes+features **regularly without popup spam.**
- **Goal 2 — Signing / "don't get flagged":** the whole build → sign → release → update flow signed well enough to **not** be flagged by Windows Defender / SmartScreen / Smart App Control / anti-malware, and clean on macOS Gatekeeper/notarization.

---

## 1. TL;DR — the decisions

1. **Goal 1 is NOT symmetric across platforms.**
   - **macOS:** the owner's UX is **natively supported** by Sparkle 2 (3 Info.plist keys + one delegate). Low effort.
   - **Windows:** WinSparkle **cannot** do silent-download-apply-on-next-launch. It has **no install-on-quit API**, and every "found an update" path shows a window. Delivering Goal 1 on Windows requires a **hybrid custom updater**: use WinSparkle for *detection only*, then own the download + apply a staged Inno installer `/VERYSILENT` on next launch. Medium-high effort, multi-commit.
2. **Settings model must change.** `autoUpdateEnabled` is a **bool** across the whole stack; Goal 1 needs a **3-state** mode (`off` / `notify` / `silent`) with a legacy-bool→enum migration, and the `AutoUpdater` C++ interface must widen to `SetUpdateMode(...)` (the two platforms implement it differently).
3. **Signing mechanics are mostly right, with two real holes:** the **portable ZIP ships unsigned** (zipped before the signing step), and the two **Rust exes carry no `VERSIONINFO`** — both are prime Defender heuristic triggers. macOS signing/notarize/staple is production-grade as-is.
4. **SmartScreen reputation resets are recurring, not one-time.** Microsoft's own CA rotations (the 2026 EOC/AOC churn) wipe reputation with no warning and no transfer path; a personal→org identity change is a *second* full reset. We need an **operational runbook**, not a one-time migration.
5. **DSA→EdDSA on Windows is required** (WinSparkle 0.9.x deprecates DSA) **but must be a dual-signed transition** — an EdDSA-only appcast strands existing 0.8.1 users.
6. **The "CEF artifact bug" is not a bug.** The release-repo `cef-binaries-macos.tar.bz2` is **byte-identical (SHA256-proven) to the codec build**; the macOS release already uses codec-enabled CEF. The only defensible change is pinning to a versioned tag for supply-chain hygiene.
7. **The test-gate is buildable now** (Track E `test.yml` exists) but `needs:` can't cross workflow files — wire it via a `test: uses: ./.github/workflows/test.yml` job. And don't overclaim: it gates Rust/adblock/secret-log only, **not** the CEF binary, updater, frontend, or installer.

---

## 2. Verified current state (source-confirmed 2026-06-22)

| Piece | State |
|-------|-------|
| WinSparkle (Windows) | **0.8.1 — DSA only.** Pinned in **3 places**: `release.yml:116`, `build-release.ps1:108`, `AutoUpdater.cpp` (DSA pub key). `CheckForUpdatesInBackground` → `win_sparkle_check_update_without_ui` (still shows dialog on hit). Manual check → `win_sparkle_check_update_with_ui`. **No silent install-on-quit.** |
| Sparkle (macOS) | **2.9.0**, EdDSA appcast. `Info.plist` has `SUFeedURL` + `SUPublicEDKey` **only** — the silent-update keys (`SUEnableAutomaticChecks`, `SUAutomaticallyUpdate`, `SUAllowsAutomaticUpdates`) are **absent**, so macOS is notify-first today too. `AutoUpdater_mac.mm` pins `allowedChannelsForUpdater = [@"beta"]`. |
| Settings UI | `AboutSettings.tsx` shows `0.3.0-beta.15`; one **on/off** `Switch` bound to `browser.autoUpdateEnabled` (**bool** end-to-end). No silent-vs-notify choice. |
| Appcast | `generate-appcast.py`: Windows item emits `sparkle:dsaSignature`; macOS emits `sparkle:edSignature` + integer build-number. **No `sparkle:channel`.** Omits the signature attribute entirely when the arg is empty (→ can generate a structurally-valid **unsigned** appcast). |
| Feed publish | `release.yml:907` `git push \|\| echo WARNING` — **fail-open.** Empty-commit guard at `:906`. |
| Windows signing | Azure Trusted Signing signs `staging/HodosBrowser` `exe,dll` + the installer separately. **Portable ZIP built in `build-release.ps1` step 7, BEFORE signing → ships unsigned.** |
| Rust exes | `hodos-wallet.exe` / `hodos-adblock.exe` — **no `build.rs`, no winres → zero `VERSIONINFO`.** |
| macOS signing | Developer ID Application: **Matthew Archbold** (personal), hardened runtime, per-dylib/helper/framework signing, notarize **app + DMG** with `status: Accepted` gates, staple + validate. Solid. |
| CI tests | `ci.yml` + `test.yml` (Track E: Rust matrix, adblock tests, **F8 secret-log gate** blocking; clippy/audits informational) **now exist** — but `release.yml` has **no `needs:` test dependency**, so installers are still signed with zero tests. |

> **Doc drift fixed:** `AUTO_UPDATE.md` and `BUILD_AND_RELEASE.md` still say "there is NO `ci.yml`." Stale — both workflows exist as of Track E. `settings/CLAUDE.md` says About shows "1.0.0" — actually `0.3.0-beta.15`.

---

## 3. Goal 1 — Auto-update UX

### 3.1 The core finding — platform asymmetry

The owner's UX = **background download while running → apply on next launch → no popup by default → opt-in notify/approve.**

- **Sparkle 2 (macOS) has exactly this primitive.** `SUAutomaticallyUpdate=YES` downloads in the background and installs **on quit**, applying on next launch. No popup once configured.
- **WinSparkle has no equivalent.** Confirmed from WinSparkle source (`updatechecker.cpp` / `ui.cpp`) and issues #21/#31/#168:
  - `win_sparkle_check_update_without_ui()` suppresses the *checking* spinner but **still shows the full update-available dialog when an update is found.**
  - `win_sparkle_check_update_with_ui_and_install()` skips the *choice* dialog but **still shows a download-progress window** and then **installs immediately (mid-session restart)** — the opposite of "apply on next launch."
  - There is **no** `install_on_shutdown` / deferred-apply API. The shutdown callbacks fire **after** the user already approved in a dialog.
  - `sparkle:installerArguments` (`/VERYSILENT`) only silences the **installer's** UI, not WinSparkle's own progress window.

**Conclusion:** Goal 1 is config-only on macOS and a **custom build** on Windows.

### 3.2 macOS design (low effort, config + delegate)

1. Add to `Info.plist`: `SUEnableAutomaticChecks=YES`, `SUAutomaticallyUpdate=YES`, `SUAllowsAutomaticUpdates=YES`, keep `SUVerifyUpdateBeforeExtraction` (EdDSA). Explicit `SUScheduledCheckInterval=86400`.
2. **Notify/approve opt-in:** implement `SPUUpdaterDelegate updater:willInstallUpdateOnQuit:immediateInstallationBlock:`. Silent mode → call the block on quit. Notify mode → present a custom approve prompt first. Drive the mode by toggling `SUAutomaticallyUpdate` via `NSUserDefaults` at runtime from the Settings IPC.
3. **Keep `/Applications`** (user-owned bundle updates without auth). **Do NOT move to `~/Applications`** — the shipping DMG already does drag-to-`/Applications`; `~/Applications` solves a non-problem and breaks Spotlight/Launchpad. *(Corrects `AUTO_UPDATE.md §5.2.)*
4. **Resolve the channel landmine:** `AutoUpdater_mac.mm` filters to channel `beta`, but the appcast emits no `sparkle:channel`. At GA either drop the `[beta]` filter or have `generate-appcast.py` emit a matching channel — otherwise the silent updater may evaluate **zero eligible updates** and silently never update.
5. Bump **Sparkle 2.9.0 → 2.9.3** (closes CVE-2026-47121 delta-symlink + the `.app`-bundle-ID first-update bug); keep **binary-deltas OFF**.
6. The one-time macOS **"Privacy & Security → App Management"** prompt is a system TCC behavior, fires once, silent thereafter — **expected and unavoidable**; document it in onboarding rather than fighting it.

### 3.3 Windows design (the hybrid — medium/high effort)

WinSparkle for **detection only**, Hodos owns the rest:

1. **Detect:** upgrade WinSparkle 0.8.1 → **0.9.3** (all 3 pin sites), keep the periodic background check, use `win_sparkle_set_did_find_update_callback`. Migrate **DSA→EdDSA in the same change** (§5.3).
2. **Download:** on "found", fetch the installer via the existing `SyncHttpClient`/WinHTTP to a temp path; write a **pending-installer marker**.
3. **Apply on next launch:** on startup, **before** the main window opens and **before** `ProfileLock` is acquired (`cef_browser_shell.cpp:3910`), run the staged Inno installer `/VERYSILENT /SP- /SUPPRESSMSGBOXES /NORESTART`, then relaunch.
4. **Child-process shutdown is mandatory and currently missing.** The CEF shell spawns `hodos-wallet.exe` and `hodos-adblock.exe` detached, **not** Restart-Manager-registered and **not** sharing the app mutex — they hold image-file locks on `{app}\hodos-wallet.exe` / `{app}\hodos-adblock.exe`. Inno `CloseApplications` (Restart Manager) **won't find them**, so file replacement fails or schedules a reboot, risking a **half-updated, version-skew install**. The installer must: add `AppMutex` + `SetupMutex`, and either register the children with RM, put all three exes in one Job Object, or have `[Code] InitializeSetup` call the existing `/shutdown` endpoints (ports 31301/31302) and **wait for the children to release locks** before `[Files]`.
5. **Sequence:** detect → stage → on next launch: shut down old process tree → wait for `ProfileLock` + image-lock release → install → relaunch. (Naive `/RESTARTAPPLICATIONS` races the single-instance ProfileLock.)
6. **Silent path removes the user's "Run anyway" escape hatch** — so a SmartScreen/SAC-flagged build = a *silently broken* update for everyone. This couples Goal 1 to Goal 2 (§5) and to the test-gate (§6.2): **do not flip default to silent-auto-apply until the feed is fail-closed and the test-gate is wired.**

> **Fallback if the hybrid proves too costly:** ship **notify-mode as the Windows default for 0.4.0** (honest, no popup-spam beyond one dialog per release) and land the hybrid silent path in 0.4.x. Owner decision — see §9 OD-1.

### 3.4 Settings → About UX + data migration

- Replace the on/off `Switch` with a 3-state control: **Update automatically (silent, default)** / **Notify me & let me approve** / **Off**.
- `autoUpdateEnabled` (bool) → `updateMode` enum touches the **whole stack**: `SettingsManager.h`/`.cpp`, the nlohmann serializer, `simple_handler.cpp` IPC contract, `useSettings.ts`, `AboutSettings.tsx`. Define a deterministic **legacy migration** (`true → silent`, `false → off`) so existing installs don't silently stop updating (a security regression for a money-handling app).
- Widen `AutoUpdater.h` to a single `SetUpdateMode(off|notify|silent)`; macOS sets `SUAutomaticallyUpdate` + delegate behavior, Windows selects detect-only-vs-hybrid + the notify dialog. Do **not** pretend `with_ui`/`without_ui` covers it.

### 3.5 Mac vs Windows work split (Goal 1)

| Work item | macOS | Windows |
|-----------|-------|---------|
| Silent download + apply-on-next-launch | Info.plist keys (config) | **Custom hybrid** (detect → download → stage → apply on launch) |
| Updater framework bump | Sparkle 2.9.0 → 2.9.3 | WinSparkle 0.8.1 → 0.9.3 (3 pin sites) |
| Appcast signature | EdDSA already | **DSA → EdDSA migration (dual-signed transition)** |
| Notify/approve opt-in | `willInstallUpdateOnQuit` delegate | custom in-app prompt + notify dialog |
| Child-process / lock handling | n/a (Sparkle handles bundle swap) | **AppMutex + child shutdown + ProfileLock sequencing** |
| Channel hygiene | drop `[beta]` filter or emit `sparkle:channel` | tag-guard (channel is a no-op on WinSparkle) |
| Settings 3-state + migration | shared | shared |
| One-time TCC prompt | document in onboarding | n/a |

---

## 4. (reserved)

---

## 5. Goal 2 — Signing / don't-get-flagged

### 5.1 Strategy (confirmed by research)

- **Keep Azure Trusted Signing on Windows.** EV certificates **no longer bypass SmartScreen** (Microsoft removed that in 2024) — paying for EV buys nothing here. OV is no better than Trusted Signing's Public Trust. Trusted Signing is the recommended path.
- **Keep macOS Developer ID + notarize + staple + hardened runtime** — already production-grade.
- **Smart App Control (Win 11 22H2+)** is a *separate, stricter* cloud-reputation layer that can block low-reputation Chromium binaries even when validly signed; only install volume clears it, no per-app whitelist.

### 5.2 Windows holes to close (the real flagging risks)

1. **Portable ZIP ships unsigned.** `build-release.ps1` step 7 `Compress-Archive`s the staging dir **before** the Azure signing step in `release.yml`, yet `windows-portable/*` is attached to every release. **Fix:** move portable-ZIP creation into `release.yml` **after** exe/dll signing (or re-zip the signed staging dir). *Single clearest fix.*
2. **Rust exes have no `VERSIONINFO`.** Bare, metadata-less PEs inside a self-built Chromium app are a textbook Defender heuristic trigger (cf. the 2022 Chromium/Electron and 2025 `*.pak.info` false-positive precedents). **Fix:** add `build.rs` + winres/embed-resource to `hodos-wallet` and `hodos-adblock` (CompanyName=Marston Enterprises, FileVersion, ProductName, OriginalFilename) matching the C++ exe's `hodos.rc`. *(This was already specced in `BUILD_AND_RELEASE.md §2.5.4` but never implemented for the Rust crates.)*
3. **Reputation resets are recurring.** The 2026 Azure intermediate-CA rotations (EOC CA 02 → AOC CA 03 → EOC/AOC churn) **wiped SmartScreen reputation with no warning and no transfer path** (open issue `azure/artifact-signing-action#128`). **Fix:** a **reputation-reset runbook** in DevOps-CICD — per-release VirusTotal pre-submit, MS Security Intelligence portal submission, stable download + changelog URLs, and a "warnings suddenly returned → submit + wait" response. Don't treat it as one-time.
4. **Identity migration timing.** Personal ("Matthew Archbold") → org ("Marston Enterprises") on Azure is a **full reputation reset** (no merge). Do it on a **deliberately low-stakes beta**, absorbing the 2–4 week "unrecognized publisher" window there — **not** on the public 0.4.0 push. Validation itself can take 1–3 weeks; start it early.
5. **Cosmetic but track it:** installer `AppPublisher=Hodos` vs cert `CN=Marston Enterprises` vs product "Hodos Browser" — SmartScreen keys off the **cert identity**, so flagging is unaffected, but resolve the branding inconsistency.

### 5.3 Appcast keys — DSA→EdDSA (Windows), done safely

- WinSparkle 0.9.x deprecates DSA; once `win_sparkle_set_eddsa_public_key` is called, DSA signatures are ignored. So the bump **forces** the migration.
- Touch **all coupled sites in lockstep:** `AutoUpdater.cpp` (`win_sparkle_set_dsa_pub_pem` → `win_sparkle_set_eddsa_public_key`), `generate-appcast.py` (`sparkle:dsaSignature` → `sparkle:edSignature` for the Windows item), `release.yml` DSA-sign step → EdDSA via `winsparkle-tool` (new secret), and the 4th site `build-release.ps1:108`.
- **Dual-signed transition required:** an EdDSA-only appcast can't be verified by installed 0.8.1 (DSA-only) clients → they're stranded. Ship at least one release whose appcast carries **both** signatures (or a transition build) before dropping DSA. *(This is the piece `AUTO_UPDATE.md §5.1` glosses.)*

### 5.4 Feed integrity — fail-closed (3 sites, not 1)

- **Make publish fail-closed:** `release.yml:907` → plain `git push`, **keeping** the empty-commit guard at `:906` (a byte-identical re-run must not hard-fail).
- **Hard-fail on missing secret at all three warn-and-skip sites:** `release.yml:183-187` (DSA), `:714-718` (EdDSA), and the publish-job reads `:866/:872` — **and** make `generate-appcast.py` **exit non-zero on an empty signature** instead of silently omitting the attribute (today it can emit a valid-looking unsigned feed).
- **Beta isolation:** `sparkle:channel` is a **no-op on WinSparkle 0.8.1** — adding it alone creates false safety. The load-bearing fix today is a **tag-pattern guard**: skip the appcast regenerate+website-push for `*-beta*` tags (or write to a separate `appcast-beta.xml` the stable feed never references). Add `sparkle:channel` in the same commit so it's ready when WinSparkle 0.9.x lands.
- **Anti-rollback:** add a CI gate refusing to publish if the new tag's build-number ≤ last published (derive a Windows build-number like the macOS path at `release.yml:251`); serve appcast over HTTPS with cache headers that prevent stale-signed-appcast replay.

---

## 6. Release-flow review

### 6.1 The "CEF artifact bug" — DEBUNKED (not a bug)

The memory note claimed `release.yml`'s mac job downloads a stale/missing `cef-binaries-macos.tar.bz2`. **Refuted by SHA256:**
- Release-repo `cef-binaries-macos.tar.bz2` = **SHA256 `80c34bcc…6c141c`**, 112,049,609 B.
- Dev-repo codec **non-minimal** asset (tag `cef-136.1.7-macosarm64-codecs`) = **identical SHA256 and size.** The release asset is the codec build **renamed**, not stale stock CEF. The extracted framework contains H264/AVC1/X264/mp4a markers → genuinely codec-enabled.
- The "real artifact" the note called `*_minimal.tar.bz2` (111,171,315 B) is a **different, smaller** variant; switching to it could break the "Build CEF wrapper" step (minimal omits the wrapper sources).

**Action:** **no re-upload** — the macOS release already uses codec CEF. The only defensible change is **pinning both jobs to a stable versioned tag** (e.g. `cef-136.1.7-…`) instead of the ambiguous `cef-binaries` tag, for provenance/anti-silent-swap — reusing the *exact* current tarball (verify by SHA256). Separately, **verify the Windows `cef-binaries-windows.zip` codec status** the same way (this review only proved macOS).

### 6.2 Test-gate

- `needs:` **cannot reference another workflow file.** Implement by adding a `test: uses: ./.github/workflows/test.yml` **job** to `release.yml`, then `needs: test` on `build-windows`/`build-macos` (mirrors `ci.yml`).
- **Don't overclaim.** The gate blocks only on **Rust + adblock tests + the F8 secret-log grep**. The **C++/CEF ctest matrix, frontend Vitest, Playwright** are commented-out/staged in `test.yml` — so a release can be green while the **CEF binary, updater, frontend, or installer** is broken. It closes "signs without Rust-regression/secret-leak testing," a real but partial subset.
- **Prove `test.yml` green on Linux first** (its Rust suite's first Linux run is unproven per its own comments) via `workflow_dispatch`, so the hard gate fails on real defects, not CI-env quirks.

---

## 7. Adversarial review verdicts

| # | Planned implementation | Verdict | Why |
|---|------------------------|---------|-----|
| 1 | Windows silent-on-quit via WinSparkle + `installerArguments` | **REFUTED** | No install-on-quit API; child-process file locks; ProfileLock relaunch race. Need hybrid + AppMutex + child shutdown. |
| 2 | macOS silent-on-quit (SU* keys, ~/Applications, deltas off) | **PARTIAL** | Mechanism sound; ~/Applications wrong (keep /Applications); opt-in toggle missing; channel landmine; Sparkle not bumped. |
| 3 | Settings 3-state via `with_ui`/`without_ui` + install-at-shutdown | **REFUTED** | Windows mechanism doesn't exist; bool→3-state migration unaccounted; AutoUpdater interface must widen. |
| 4 | Windows DSA→EdDSA migration | **PARTIAL** | Right direction; same-release DSA removal strands 0.8.1 users (need dual-signed); misses 4th pin site. |
| 5 | "Signing is sufficient to avoid flags at GA" | **PARTIAL** | macOS solid; **portable ZIP unsigned**; **Rust exes no VERSIONINFO**; reputation reset is recurring. |
| 6 | Fail-closed feed + `sparkle:channel` | **PARTIAL** | 3/4 parts sound; channel is a no-op on WinSparkle 0.8.1 (use tag-guard); fail-closed must hit 3 sites + generator. |
| 7 | CEF mac artifact re-upload | **REFUTED** | Asset is byte-identical codec build (SHA256-proven). No-op. Only pin-to-versioned-tag is defensible. |
| 8 | Wire `test.yml` as `needs:` gate in `release.yml` | **PARTIAL** | `needs:` can't cross files (use `uses:` job); covers Rust/adblock/F8 only, not CEF/frontend/installer. |

*(Full agent reports incl. live sources: workflow `wf_8599c934-31f`, 2026-06-22.)*

---

## 8. Corrections to prior docs / memory (so they stop misleading)

1. **WinSparkle can NOT do silent apply-on-quit** — corrects the optimistic framing in `AUTO_UPDATE.md §3/§5.1` and the memory pointer. Windows Goal 1 = hybrid custom updater.
2. **CEF mac artifact is fine** — corrects the "known bug" in `project_autoupdate_signing_research_2026_06_22` memory. The asset is the codec build, SHA256-verified.
3. **`ci.yml`/`test.yml` exist** — corrects `AUTO_UPDATE.md`/`BUILD_AND_RELEASE.md` "there is NO ci.yml."
4. **About shows `0.3.0-beta.15`**, not "1.0.0" — corrects `settings/CLAUDE.md`.
5. **Latest upstream versions:** WinSparkle **0.9.3** (2026-05-18), Sparkle **2.9.3** (2024-06-08). We are on 0.8.1 / 2.9.0.

---

## 9. Sequenced plan + open decisions for the owner

**Recommended sequence (each is its own commit set; gate before automate):**
1. **Hygiene/safety first (low risk):** fail-closed feed (§5.4), sign the portable ZIP (§5.2.1), Rust `VERSIONINFO` (§5.2.2), wire the test-gate via `uses:` (§6.2), bump Sparkle → 2.9.3 (§3.2.5).
2. **macOS Goal 1 (config):** SU* plist keys + `willInstallUpdateOnQuit` delegate + channel fix (§3.2).
3. **Settings 3-state + migration + AutoUpdater interface widen** (§3.4).
4. **Windows updater track:** WinSparkle 0.9.3 + DSA→EdDSA dual-signed transition (§5.3) → hybrid detect/download/apply-on-launch + child-shutdown/AppMutex (§3.3).
5. **Identity + reputation:** start org-identity validation early; cut over on a low-stakes beta; stand up the reputation-reset runbook (§5.2.3/.4).

**Open decisions (need owner input):**
- **OD-1 — Windows 0.4.0 default:** ship **notify-mode** for 0.4.0 (honest, low-risk) and land the hybrid silent path in 0.4.x? Or block 0.4.0 on the full hybrid? *(Recommend: notify for 0.4.0, hybrid in 0.4.x — silent-auto-apply shouldn't ship before the feed is fail-closed + test-gate wired, and the hybrid is multi-commit.)*
- **OD-2 — org-identity cutover release:** which beta absorbs the SmartScreen/Gatekeeper reset? (Must not be the public 0.4.0.)
- **OD-3 — binary-deltas:** confirm OFF on both platforms for 0.4.0 (CVE-2026-47121 mitigation) — recommend yes.
- **OD-4 — "update while app fully closed for weeks"** (scheduled task / LaunchAgent): defer past 0.4.0? Recommend defer.

**This is a research/design doc — no code has been written. Mac-vs-Windows implementation ownership is split per §3.5 and the §5/§6 items; sequence in §9.**
