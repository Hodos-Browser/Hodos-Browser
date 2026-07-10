# Mac ⇄ Windows relay (0.4.0) — cross-device coordination hub

Both the Windows Claude session and the Mac Claude session coordinate through THIS doc (committed to
`origin/0.4.0`). Pull before reading; push after writing.

---

## CURRENT REALITY (2026-07-09) — auto-update saga CLOSED; channel repointed to the Chromium/CEF rebuild
- **Latest shipped = `v0.3.0-beta.26` (LATEST / live).** Nothing is in flight; the previous handoff
  round (beta.23 + mac dropdown-button consistency) is CONSUMED and archived below.
- **Windows SILENT auto-update is DONE + PROVEN LIVE** through the two-process profile picker
  (beta.25→26 applied silently on real hardware). macOS silent proven earlier (beta.21→22). The whole
  silent-update saga is complete: signer-continuity CN gate (beta.23), external rollback-supervisor,
  picker-gate exact-picker-exit-wait fix (commit `ae5beb6`, beta.26), `promote.yml` redirect-verify
  retry hardening, and `BUILD_AND_RELEASE` tag-derived version + draft→manual-promote gate.
- **Profile picker + per-profile-wallet architecture = SHELVED** (wallet stays SHARED). The
  same-process picker refactor is deferred. No picker work this sprint.
- Win10 overlay cluster (F1/F2/F3/F5), global settings across profiles, and bookmark favicon/delete
  all landed in beta.23. Mac dropdown-button consistency landed + smoked (see archive below).

## STANDING CHANNEL: Chromium/CEF rebuild sprint coordination
**This doc is now the standing Win⇄Mac coordination hub for the Chromium/CEF rebuild sprint.**
The sprint is RESEARCH + DESIGN first (NO code yet) — see the kickoff brief:
`development-docs/0.4.0/CHROMIUM_CEF_SPRINT_KICKOFF.md`.
- **Windows Claude = LEAD.** Mac Claude coordinates through this doc.
- Scope headlines: newest stable CEF, farbling→Blink-patch (owner committed), proprietary codecs,
  dependency/version bump. Open owner questions the design must answer: mac farbling, farbling×adblock,
  farbling×OAuth-preapproved, Amazon Widevine (on-demand CDM — OUT of beta.1 unless cheap).
- Deliverable target: `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` (outline → auto-chained detailed impl
  plans with adversarial review).

### → FOR THE MAC CLAUDE SESSION
1. `git pull origin 0.4.0` before reading; `git push origin 0.4.0` after writing.
2. Read `CHROMIUM_CEF_SPRINT_KICKOFF.md`. This sprint is docs/research only — do NOT write engine code
   until the roadmap lands and the owner greenlights.
3. Own the **macOS-specific research/design inputs**: mac farbling approach (Blink-patch parity vs the
   current JS-injection farbling), mac codec/build implications, and any mac blockers for the CEF bump.
4. Report findings + open questions in "MAC → WINDOWS REPORT-BACK" below, then push.

### → FOR THE WINDOWS / RELEASE SIDE (heads-up)
- Windows is LEAD on the rebuild design and owns `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`.
- Pull before consuming Mac's report-back; fold mac inputs into the roadmap.

---

## MAC → WINDOWS REPORT-BACK (Mac Claude fills this in + pushes)

_(Awaiting the next round — Chromium/CEF rebuild research inputs. Previous rounds archived below.)_

---

## ARCHIVE — consumed handoff rounds

### 2026-07-08 — beta.23 + mac dropdown-button consistency (SHIPPED, CONSUMED)
beta.23 shipped and is live; the mac dropdown-button consistency work landed + smoked and rode in it.
Profile picker was shelved that round and remains shelved.

**Mac commits:** (1) prior session M1–M3 build verify + Sparkle force-check-on-launch + picker full
flow + async server startup fix + port deconfliction (`MACOS_EXECUTION_RESULTS_2026_07_07.md`);
(2) dropdown button consistency — menu, profile, download brought to the 4-way reference pattern.

**Files:** `cef-native/cef_browser_shell_mac.mm` (menu overlay keep-alive helpers + dedicated
click-outside monitor with 0.3s debounce; `CreateMenuOverlayMac` + Show/Hide stubs → keep-alive
orderOut instead of destroy); `cef-native/src/handlers/simple_handler.cpp` (macOS IPC branches for
`profile_panel_show`/`menu_show`/`download_panel_show` → the 4-way
`if (!window) Create; else if (IsVisible) Hide; else if (WasJustHidden) suppress; else Show` pattern).

**Result:** clean macOS Release build (zero warnings/errors); all three dropdowns smoked (open, toggle-
close, click-outside close, keep-alive reuse); bookmark/site-info/tab-list reference branches untouched.
No blockers.

**Notes carried forward:** dev builds need ad-hoc signing after rebuild
(`codesign --force --deep --sign -`) to launch via `open`; direct terminal exec still works unsigned.
`AutoUpdater_mac.mm` force-check-on-launch stays enabled for all non-Off modes — Windows intentionally
narrowed this to Notify-only (WinSparkle shows prompts even in silent mode; Sparkle 2 handles silent
mode correctly), so the platforms differ here by design.
