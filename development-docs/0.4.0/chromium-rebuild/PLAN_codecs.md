# PLAN — Proprietary Codecs on the Target CEF/Chromium Build

**Status:** DETAILED PLAN (Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3a + §7 codec gate). Research + design only — **NO code, NO builds.**
**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**What this plans:** How to confirm proprietary codecs (H.264 / AAC / MP3 / VP9 / AV1, plus the already-inherited HEVC/H.265) still build and function after the 136 → TARGET bump — the exact GN flags, the codec-flag changes to watch, the media smoke matrix, and the relationship to `is_official_build` and to Widevine (separate). **TARGET is a placeholder** (per outline §2 / Step 0): resolve the exact CEF stable version + branch from `cef-builds.spotifycdn.com/index.json` at plan time — do NOT bake a milestone number into this doc. Per the runbook (`CEF_BUILD_RUNBOOK.md` lines 80–85) current stable at time of writing is **CEF 149 / Chromium 149 (branch 7827)** with **M150 LTS** as the pin candidate; the codec audit window must be re-anchored to whichever milestone Step 0 resolves. **Feeds research question Q5** (full reconciled edit list). This is the smallest of the rebuild docs by design — the codec path is a *carry-forward*, not a redesign; the value is in the drift audit + smoke gate.

> **Authoritative inputs:** `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (§"Why we self-build", Step 5.5, Step 6, Step 7), `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md`, `scripts/build_hodos_cef.bat` / `scripts/build_hodos_cef_mac.sh`, `reference_cef_self_build_reason`, `BRAVE_FORK_FEASIBILITY.md` §4. Widevine/DRM lives in the sibling `Q4-amazon-drm.md` — **out of scope here except the boundary note in §5.**

---

## 1. What this answers (one screen)

- **Do our codec GN flags still take effect on TARGET?** — Yes, the mechanism is unchanged from M136 through current stable (CEF 149 / M150-LTS candidate): codecs are governed by `proprietary_codecs=true ffmpeg_branding=Chrome`, set via `GN_DEFINES` in both build scripts. **Re-verify by smoke, not by faith** — a flipped Chromium default or a renamed flag ships a *green build with no codecs* (`CEF_BUILD_RUNBOOK.md` Step 5.5).
- **Headline recommendation:** carry the four flags forward verbatim; add **one explicit HEVC decision** (see §3 — HEVC platform decode is *already inherited on our M136 build* and simply carries forward, correcting the earlier "out of scope" mental model) and keep the existing canPlayType + real-playback smoke as a **hard gate** on P5/P6.
- **Relationship to Widevine:** independent build systems. Codecs (this doc) = container/codec decode via FFmpeg + platform decoders. Widevine = encrypted-media CDM, auto-downloaded by the component updater. Neither flag affects the other. See §5.

---

## 2. The flags — carry forward verbatim

Current known-good (from `scripts/build_hodos_cef.bat` / `_mac.sh`, confirmed in `CEF_BUILD_RUNBOOK.md` line 50):

```
set GN_DEFINES=is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0
```

| Flag | Role for codecs | Action on bump |
|---|---|---|
| `proprietary_codecs=true` | Master switch. Makes `//media` + `canPlayType()` advertise + accept H.264, AAC, MP3, and gates the HEVC/Dolby build flags (§3). | **Keep.** Verify it is still a recognized GN arg (`gn args --list`). |
| `ffmpeg_branding=Chrome` | Selects the FFmpeg config that *ships the decoders* (H.264/AAC/MP3 software + demuxers). `Chromium` branding ships none. | **Keep.** The two flags are coupled: `proprietary_codecs=true` + `ffmpeg_branding=Chromium` is a **build-time assertion failure** (media/BUILD.gn), so a mismatch fails loudly — a useful safety net, not a silent trap. |
| `is_official_build=true` | Optimized/branded build. **Does not itself enable proprietary codecs** (that's the two flags above), and **does not gate AV1** — AV1 decode via `dav1d` (`enable_dav1d_decoder`, defaults true) ships in all Chromium builds regardless of official-build. Its one codec-adjacent effect: it flips `ffmpeg_branding`'s *default* to `Chrome` on Chrome-branded builds. We are CEF (not Chrome-branded) so we set `ffmpeg_branding` **explicitly** regardless. | **Keep.** Note the decoupling in §4. |
| `chrome_pgo_phase=0` | Disables PGO (unrelated to codecs; needed for sccache determinism — drops `/Brepro`). | Keep. Out of scope for codecs. |

> **Why we do it via `GN_DEFINES`, not the `--proprietary-codecs` automate-git flag:** more reliable and explicit (`CEF_BUILD_RUNBOOK.md` line 111). Keep this approach on TARGET.

**Primary-source status:** CEF still documents `proprietary_codecs=true ffmpeg_branding=Chrome` as the enabling args (see Sources). The claim that `proprietary_codecs` / `ffmpeg_branding` semantics and the `Chrome`-vs-`Chromium` assertion are unchanged on TARGET is **to be re-confirmed at plan time** against current `media/media_options.gni` + `media/BUILD.gn` — §7 step 1 (`gn args --list`) + step 2 (assertion check) do exactly this. Do not treat it as already confirmed; the only in-repo citation for the assertion is a 2016 codereview CL, which is a design reference, not proof of current state.

---

## 3. Codec-flag changes to watch on the bump

The four flags above are stable, but the *milestones between M136 and TARGET* (M136 → whichever milestone Step 0 resolves — CEF 149 / M150-LTS candidate as of this writing) may have changed defaults for the newer codecs. **The "Change vs M136" column below was originally audited over a narrower window and MUST be re-run M136 → the confirmed TARGET milestone before the "none" rows can be trusted complete** — that drift audit is the value of this section. Per-codec status on the TARGET proprietary build:

| Codec | canPlayType target | How enabled | Change vs M136 mental model | Beta.1 scope |
|---|---|---|---|---|
| **H.264** (`avc1.42E01E`, and High `avc1.64…`) | `probably` | `proprietary_codecs`+`ffmpeg_branding=Chrome` (SW) + platform HW | none | **GATE** |
| **AAC** (`mp4a.40.2`) | `probably` | same | none | **GATE** |
| **MP3** (`audio/mpeg`) | `probably` | same | none | **GATE** |
| **VP9** (`vp09…`) | `probably` | free codec, always in Chromium | none | **GATE** |
| **AV1** (`av01…`) | `probably` | free codec; `dav1d` SW decoder (`enable_dav1d_decoder`, defaults true) bundled in **all** Chromium builds — not gated on `is_official_build` | none (already present on M136) | **assert decode presence** |
| **HEVC / H.265** (`hev1…`/`hvc1…`) | *see note* | `enable_hevc_parser_and_hw_decoder = proprietary_codecs && (is_win \|\| is_apple \|\| …)` and `enable_platform_hevc = proprietary_codecs && (…)` — **both default-ON when `proprietary_codecs=true`** | **CARRY-FORWARD (not a bump change):** these flags have defaulted on since ~M107 (2022), so our **existing M136 build already inherits** HEVC platform/hardware decode via `proprietary_codecs=true`. TARGET simply carries it forward, no extra flag. It is **hardware/OS-decoder only** (no software fallback shipped). | **SMOKE-ONLY, non-gating** (see §3.1) |
| **Dolby (AC-3/EAC-3/AC-4), Dolby Vision** | `""` | separate `enable_platform_ac3_eac3_audio` / Dolby flags, licensing-gated | not enabling | **OUT** (record explicitly) |

### 3.1 HEVC — the one real decision this doc surfaces

The outline (§3a M3) says "HEVC/H.265 (`enable_platform_hevc`) and Dolby are OUT of scope for beta.1." That was written against a mental model where HEVC needed an extra opt-in. **Primary-source reality:** `enable_hevc_parser_and_hw_decoder` and `enable_platform_hevc` are *derived from* `proprietary_codecs` and have defaulted to true on Windows/macOS since ~M107. So HEVC hardware/platform decode is **not a bump-introduced surprise — our current M136 build already inherits it**, and TARGET carries it forward. This is a mental-model correction, not a version delta, and it is **testable on the existing M136 build right now** (confirm `canPlayType('video/mp4; codecs="hvc1…"')` on the live build before the bump).

Implications the P5 plan must handle:
- **Do not fight it.** Removing HEVC would require an *extra* override (`enable_platform_hevc=false`) and a patch — added churn for no benefit. HEVC is hardware-decoder-only (no FFmpeg SW decoder shipped), so it carries no additional binary-size/licensing surface beyond the OS decoder already on the machine.
- **But do not gate on it either.** HEVC playback depends on the *user's GPU/OS* decoder; a machine without an HEVC-capable decoder returns `""`. Gating beta.1 on HEVC would create machine-dependent red builds.
- **Recommended default:** **leave HEVC at its inherited default (present, hardware-only); SMOKE-test it (record `probably`/`maybe`/`""` per test machine); do NOT gate.** Update `CEF_VERSION_UPDATE_TRACKER.md`, the outline **§3a M3** note, **and the outline §7 readiness-checklist codec row** (which currently says "HEVC/Dolby explicitly out-of-scope (M3)") to reflect "HEVC = inherited-on hardware-only, non-gating" rather than "out of scope" — otherwise the two docs disagree. Confirm the derived-default at plan time with `gn args out/Release_GN_x64 --list --short | findstr hevc`.

---

## 4. Relationship to `is_official_build`

- `is_official_build=true` is about **build type/optimization/branding**, not codec licensing. It turns on full optimization and official branding. It does **not** turn on AV1 — the `dav1d` AV1 decoder (`enable_dav1d_decoder`, defaults true) is present in all Chromium builds regardless of official-build, so the §6.3/§7 "assert AV1 presence" gate is machine- and build-type-independent.
- It **does not** enable H.264/AAC/MP3 — those need `proprietary_codecs=true ffmpeg_branding=Chrome`. Do **not** assume "official build ⇒ codecs"; the pairing is what matters.
- The one coupling: on a *Chrome-branded* official build, `ffmpeg_branding` *defaults* to `Chrome`. Because CEF is **not** Chrome-branded, we set `ffmpeg_branding=Chrome` **explicitly** — which we already do. Keep it explicit; never rely on the default.
- Consequence for the drift audit: if a future change ever flips `is_official_build` to false (e.g. a debug build), the codec flags still hold **only because they are explicit**. This is why Step 5.5's "confirm the flag still takes effect" check reads the *resolved* `args.gn`, not the script input.

---

## 5. Relationship to Widevine (separate — boundary note only)

- **Codecs ≠ DRM.** This doc covers unencrypted container/codec decode. Widevine covers *encrypted* media (EME/CDM). They are independent build systems and independent flags.
- `enable_widevine=true` is set automatically by CEF's build system; the `widevinecdm.dll` is **not** in our output and auto-downloads via the component updater at runtime (`CEF_BUILD_RUNBOOK.md` Step 6).
- **Do not conflate a codec failure with a DRM failure during smoke.** YouTube/Twitch/X non-DRM playback exercises *codecs* (this doc). An Amazon Prime **movie** failing is a *DRM/CDM* problem (Q4), even though both look like "video won't play." The smoke matrix (§6) is deliberately DRM-free so a red result unambiguously implicates codecs.
- Full Widevine plan: `Q4-amazon-drm.md`. Nothing in this doc changes DRM behavior.

---

## 6. Media smoke matrix (the P5/P6 acceptance gate)

Two layers: (A) `canPlayType` capability probe (fast, deterministic), then (B) real-playback smoke (catches decoder-present-but-broken).

### 6.1 Layer A — `canPlayType` capability probe

Run in the built browser devtools console (or a tiny local HTML harness). **Expected `'probably'` unless noted.**

```javascript
const v = document.createElement('video'), a = document.createElement('audio');
v.canPlayType('video/mp4; codecs="avc1.42E01E"')   // H.264 baseline  → probably   [GATE]
v.canPlayType('video/mp4; codecs="avc1.640028"')    // H.264 High      → probably   [GATE]
a.canPlayType('audio/mp4; codecs="mp4a.40.2"')      // AAC-LC          → probably   [GATE]
a.canPlayType('audio/mpeg')                          // MP3             → probably   [GATE]
v.canPlayType('video/webm; codecs="vp09.00.10.08"') // VP9             → probably   [GATE]
v.canPlayType('video/mp4; codecs="av01.0.05M.08"')  // AV1             → probably   [assert present]
v.canPlayType('video/mp4; codecs="hvc1.1.6.L93.B0"')// HEVC/H.265      → probably|maybe|"" (machine-dependent, NON-gating)
```

A `""` on any **[GATE]** row = codec build regressed → **block the bump**, re-audit `args.gn` per Step 5.5 (§7).

### 6.2 Layer B — real-playback smoke (both Windows and macOS)

| Site | Exercises | Pass = |
|---|---|---|
| **youtube.com** | VP9/AV1 (+ H.264 fallback), AAC/Opus | plays, seeks, audio present |
| **x.com** | H.264 MP4 video **+ animated GIF (really MP4)** | both play (GIF-as-MP4 is the classic proprietary-codec canary) |
| **reddit.com** | H.264 video | plays (no infinite spinner — the classic no-codec symptom) |
| **twitch.tv** | H.264/AAC live | live stream plays |
| **linkedin.com** | H.264 feed video | plays |
| **soundcloud.com** (any public track) | AAC/MP3 audio | audio plays (named for reproducibility; if unavailable, substitute any stable public MP3/AAC embed and record which) | 

> These are the sites `CEF_BUILD_RUNBOOK.md` §"Why we self-build" names as codec canaries (x.com, Reddit, Twitch), plus the seed's media set (YouTube, LinkedIn). Reconcile Win vs Mac results in `CHROMIUM_BUILD_RELAY.md`.

### 6.3 Acceptance criteria (maps to outline §7 "Codecs / media")

- [ ] Layer-A: all **[GATE]** rows return `'probably'`; AV1 decode presence asserted (`'probably'`).
- [ ] Layer-B: all six sites play real audio+video on **both** Windows and macOS.
- [ ] HEVC result **recorded** (per test machine) but **not gating**; Dolby explicitly out.
- [ ] `libcef` size sanity: our official codec build's `libcef.dll` was ~239 MB vs the ~224 MB **prebuilt Spotify CEF** (~15 MB larger) per `CEF_BUILD_RUNBOOK.md` line 61. Note this delta is **not a clean codec-only measurement** — it also folds in official-build/branding differences vs the prebuilt — so it's a loose corroboration that a codec-bearing build was produced, not a precise codec-size check. Optional but cheap.

---

## 7. Step-by-step verification procedure (for the later implementing session)

1. **Pre-bump (on TARGET args, before full build):** run `gn gen` then `gn args <out> --list --short` and confirm the resolved values:
   `proprietary_codecs=true`, `ffmpeg_branding="Chrome"`, and inspect the HEVC derivations
   (`enable_platform_hevc`, `enable_hevc_parser_and_hw_decoder`). Record them. This catches a *renamed/removed* flag before a 10–12 hr build.
2. **Confirm the coupling guard:** verify `media/BUILD.gn` still asserts on `proprietary_codecs=true` + `ffmpeg_branding=Chromium` (our fail-loud safety net). If the assertion moved/changed, note it in the tracker.
3. **Post-build Step 5.5 drift audit (codec slice):** diff pinned `GN_DEFINES` vs the new CEF defaults; confirm `ffmpeg_branding=Chrome` still resolves through (`CEF_BUILD_RUNBOOK.md` Step 5.5, lines 245–247). A flipped default is exactly the silent-green-no-codec failure mode.
4. **Layer-A probe** (§6.1) in the freshly built browser.
5. **Layer-B smoke** (§6.2) on Windows; Mac runs the same and reports to the relay.
6. **Record** in `CEF_VERSION_UPDATE_TRACKER.md`: CEF branch, Chromium milestone, resolved codec args, per-codec canPlayType results, HEVC per-machine result, site-smoke pass/fail, build duration (Step 8).

---

## 8. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Flipped Chromium default silently drops a codec (green build, no playback) | Low-Med | Step 5.5 resolved-`args.gn` diff + Layer-A probe **before** shipping (§7 steps 1,3,4) — this is exactly why we smoke every bump. |
| A codec-flag was **renamed** between M136 and TARGET (e.g. `ffmpeg_branding` value set changes) | Low | `gn args --list` pre-build catches it before the 10–12 hr build (§7 step 1). |
| HEVC inherited-on causes an *unexpected* behavior (e.g. a site serves HEVC and a user's GPU lacks a decoder → black frame instead of fallback) | Low | Non-gating smoke records it; if it bites, add `enable_platform_hevc=false` override — a one-line GN change, documented as the escape hatch. |
| Codec failure misread as DRM failure (or vice-versa) during smoke | Med | DRM-free smoke matrix (§5, §6.2) isolates codecs; Amazon **movie** = Q4, not this doc. |
| `is_official_build` assumed to imply codecs | Low | §4: pairing is explicit; drift audit reads resolved args, not script input. |

---

## 9. Open questions (with recommended defaults)

| # | Question | Recommended default |
|---|---|---|
| CQ-1 | HEVC is inherited-ON (hardware-only) since ~M107 — keep or force off? | **Keep inherited-on, smoke-only, non-gating.** Removing it adds an override + churn for no benefit; it's hardware-decoder-only so no size/licensing surface. Update outline **§3a M3**, the outline **§7 readiness-checklist codec row**, and the tracker to say "HEVC = inherited hardware-only, non-gating" (supersedes "out of scope") so the docs don't disagree. |
| CQ-2 | Assert **AV1** as a gate or presence-only? | **Presence-only** (`'probably'`), non-gating — AV1 is a free codec already present on M136; a regression would be surprising but shouldn't block on a decode-perf basis. |
| CQ-3 | Add an automated Layer-A probe to CI? | **Yes, later** — a tiny headless canPlayType harness is a cheap regression guard, but it needs the built binary on the self-hosted host; defer to the Step-5.5 automation TODO already tracked in the runbook (line 323). Not a beta.1 blocker. |
| CQ-4 | Dolby (AC-3/EAC-3/AC-4, Dolby Vision)? | **OUT for beta.1** — licensing-gated, separate flags, no current demand. Record explicitly (already outline §3a M3). |
| CQ-5 | Ship the `libcef` +15 MB size check as a gate? | **No, corroboration only** — useful sanity signal, not a hard gate (delta shifts with each Chromium version). |

---

*Feeds `Q5-full-edit-list.md` (§3a row) and the outline §7 "Codecs / media" gate. This doc stops at a followable plan; the implementing session runs §7 against the real TARGET build.*

---

### Sources (primary)
- CEF — Branches & Building (proprietary_codecs / ffmpeg_branding=Chrome build args): https://chromiumembedded.github.io/cef/branches_and_building.html
- Chromium — Audio/Video build config (media_options.gni; ffmpeg_branding roles): https://www.chromium.org/audio-video/
- Chromium — media/BUILD.gn assertion (proprietary_codecs=1 + ffmpeg_branding=Chromium fails): https://codereview.chromium.org/1569053002/
- CEF issue #3559 — proprietary codecs via OS decoding; flag details: https://github.com/chromiumembedded/cef/issues/3559
- HEVC build-flag derivations (`enable_hevc_parser_and_hw_decoder = proprietary_codecs && (is_win||is_apple||…)`, `enable_platform_hevc`): https://github.com/StaZhu/enable-chromium-hevc-hardware-decoding
- CEF current stable surface + version→branch lookup (use `index.json`, not the wiki, per outline §2): https://cef-builds.spotifycdn.com/index.json · https://www.nuget.org/packages/cef.sdk
- Chrome release schedule (milestone stable/expected dates — confirms current stable is M150/M151 in July 2026, not ~147): https://releases.sh/google/chrome
- In-repo authoritative version anchor: `development-docs/DevOps-CICD/CEF_BUILD_RUNBOOK.md` lines 80–85 (CEF 149 / Chromium 149 current stable; M150 LTS target)
