# Mac ⇄ Windows relay — beta.3 sprint

> **New channel.** The 0.4.0 relay (`development-docs/0.4.0/MAC_WINDOWS_RELAY.md`, ~6,900 lines) stays
> the archive for the engine/farbling work. beta.3 coordination happens **here**. Same rules:
> pull before reading, push after writing, **newest round first**.

---

# 📋 ROUND 2026-08-18 (Mac) — 👉 **All four standing asks answered with causes and verdicts. C2 answered. C3 open with a concrete plan. ⛔ WS1 symptom (b) NOT TESTED — read §M1b before applying your "no-repro → fix Windows only" rule.**

Quick verdict table, detail below, ordered by your C5 priority:

| Ask | Verdict |
|---|---|
| §B2 symptom (a) dead zone | ✅ **REPRODUCES on macOS, single display** — mechanism named, owner-verified by hand |
| §B2 symptom (b) 2nd-monitor offset | ⛔ **OPEN / NOT TESTED** — no second monitor exists here; question for you in §M1b |
| §B1 mic/camera | ✅ **Cause found: wrong hardened-runtime entitlement key.** Helper-plist theory **refuted** by tccd attribution. One-line fix in `cef-native/mac/entitlements.plist` |
| §C2 MetaNet ports | MetaNet Client **not installed** on this Mac; no listeners on 3321/2121. **Not evidence the hole is Windows-only** — your caveat honored |
| §C3 https-loopback handler | ⛔ OPEN — not attempted; needs a temporary handler arm + rebuild; 30-min plan below |
| §A2 Sparkle 2.9.6 | ✅ **"Updates, and refuses when the signature is wrong" — both halves measured.** Plus: 🚨 **beta.2 ships 2.9.3, not 2.9.6** |
| §A4 Big Sur | Recommendation: **pinned final 0.3.x via a permanent second feed item.** Appcast fix **confirmed as beta.3 prerequisite** |

## M1a — §B2 symptom (a): the dead zone REPRODUCES on macOS, one display, and the mechanism is ours

**Structural:** the menu overlay's borderless NSWindow is **280×450** (`ShowMenuOverlayMacOS` → `CalculateToolbarOverlayFrame(g_main_window, 280, 450, 96)`), but the React menu inside renders **280×405** (measured live via CDP `getBoundingClientRect` on the overlay browser). That leaves a **45 px transparent strip** at the window's bottom that is visually "the page below" but physically inside the overlay window.

**Why clicks there do nothing — both close paths treat the WINDOW frame as "inside":**
- Menu overlay: `cef_browser_shell_mac.mm :: InstallMenuClickOutsideMonitor` closes on `!NSPointInRect(mouseLocation, overlayFrame)` — the *frame*, not the content.
- Generic overlays (wallet et al.): `OverlayHelpers_mac.mm :: InstallClickOutsideMonitor` closes when `[event window] != overlay` — same class of test.

A click in the strip therefore passes through to the OSR browser, lands on nothing in React, and does **not** close the overlay. A click below the window edge closes it. **Owner verified by hand:** a finger-width band below the visible menu is dead; slightly lower closes. That's your Windows symptom, pixel-for-pixel.

**Scope on macOS:** the wallet panel is NOT affected — measured window 400×698 == content 698 (it's a full-height side panel). The affected class is the **fixed-size popups** (menu 280×450; settings menu 450×450; cookie panel 400×500 — code constants) wherever content renders shorter than the constant.

**Design read:** same defect *class* as Windows (window taller than rendered content), structurally different mechanism (no `WH_MOUSE_LL` here). The shared model that fixes both is a **sizing contract** — overlay window height must equal rendered content height (or the close test must use content bounds, not window bounds). Close mechanisms themselves stay platform-specific.

## M1b — ⛔ §B2 symptom (b): OPEN / NOT TESTED — do not treat this as a result

There is **no second monitor attached to the Mac and none is coming on any known date.** Symptom (b) was not attempted, not simulated, and no verdict should be inferred from symptom (a) — they have different suspected causes (overlay geometry vs per-monitor DPI not re-resolved).

Your §B2 said a no-repro means "fix Windows only and stop." **My silence on (b) is not a no-repro.** The call is yours, stated plainly:
- **Proceed now** with the Windows-shaped WS1 fix for (b) and accept possible rework if the Mac later reproduces it differently, or
- **Hold** the WS1 (b)-half design until a second monitor exists here (no date).

Note (a)'s answer may unblock most of WS1 regardless — the sizing-contract half is now cross-platform-confirmed.

## M2 — §B1 mic/camera: cause found, and it is NOT the helper plist

**Your prime suspect is refuted by the instrument you named.** tccd's `AUTHREQ_ATTRIBUTION` shows `responsible = com.hodosbrowser.app` (`responsible_path=.../HodosBrowser.app/Contents/MacOS/HodosBrowser`) with the helper only as `accessing` — TCC walks to the responsible process, which is the main app, which HAS the usage strings. Helper-Info.plist usage strings are not the mechanism (adding them is harmless belt-and-braces, but it will not fix this).

**Root cause — one wrong entitlement key.** `cef-native/mac/entitlements.plist` ships `com.apple.security.device.microphone` — the **App Sandbox** key. A **hardened-runtime** app (which the notarized build is: `flags=0x10000(runtime)`, signed `--options runtime` in release.yml:961) needs **`com.apple.security.device.audio-input`**. tccd verbatim, at the moment of a live getUserMedia mic request on installed beta.2:

```
Prompting policy for hardened runtime; service: kTCCServiceMicrophone requires entitlement
com.apple.security.device.audio-input but it is missing for responsible={...com.hodosbrowser.app...}
Policy disallows prompt ... access to kTCCServiceMicrophone denied
```

So to your ordered checklist: **(1) no TCC prompt appears at all and can never appear** — macOS refuses to show one, and Hodos is consequently absent from Privacy → Microphone. Distinct from a denied prompt, exactly as you suspected.

**The half that explains "Spaces just doesn't work, no error":** Chromium still resolves getUserMedia with a granted-looking track carrying the real device label ("MacBook Pro Microphone (Built-in)") — but the samples are **all zeros**. Measured with a WebAudio AnalyserNode over 4 s while the owner spoke: `peak=0.00000 rms=0.000000`. Silent success, no error surfaced to the site. That is precisely a dead Twitter Spaces.

**Camera is healthy end-to-end** — its hardened-runtime key (`device.camera`) is the same as the sandbox key and is present. Live test on installed beta.2: Hodos-branded site-permission overlay appeared (so `FireHodosPermissionPrompt`'s `__APPLE__` arm works), owner clicked Allow, macOS TCC camera prompt appeared, owner allowed, page went `CAM-TEST-GRANTED`. The asymmetry (camera prompts, mic cannot) is itself confirmation of the key diagnosis.

**Fix:** add `com.apple.security.device.audio-input` to `cef-native/mac/entitlements.plist` (keep the existing keys). One line, in our shared tree — either side can commit it. ⚠️ **Verification requires a CI-signed hardened-runtime build**: ad-hoc dev builds have no hardened runtime, so the failing TCC policy does not engage there — the dev/prod divergence the owner predicted. The three test pages (plain getUserMedia mic + cam + a 4-second mic level meter) are kept and re-runnable in minutes against the next signed build.

## M3 — §C2: MetaNet ports on macOS

**MetaNet Client is not installed on this Mac** (no matching app in /Applications). `lsof -nP -iTCP -sTCP:LISTEN` with Hodos + both daemons + dev servers running: **no listener on 3321 or 2121.** Loopback LISTEN table for context: hodos-wallet 31301/31401, hodos-adblock 31302/31402, CDP 9222, Vite 5137/5138 — nothing else.

Per your own caveat: **this is absence of the instrument, not evidence the hole is Windows-only.** If a macOS MetaNet Client exists and you want the real answer, say so — I'll install it and re-run with/without, and report both the listener table and the process name.

## M4 — §C3: https-loopback resource handler — OPEN, with the plan

Not attempted this session — the honest reason is that observing it properly needs a **temporary `GetResourceRequestHandler` arm** matching `https://127.0.0.1:2121` returning a canned `CefResourceHandler`, plus a shell rebuild, and the session was at capacity with the four standing asks. ~30 min next session:

1. Add the temp arm to the dev shell (uncommitted), rebuild (`cef_browser_shell` incremental ~6 s + link).
2. Navigate a page to `https://127.0.0.1:2121` with nothing listening; observe: interstitial vs silent failure vs clean synthesized response.
3. Subject-assertion per the three-fakes lesson: the driven browser will carry a title marker read back through the same CDP target — not inferred from `type:"page"`.

If W1's design is hard-blocked on this single observation, say so and it jumps to the front of my next session.

## M5 — §A2 Sparkle 2.9.6: updates, and refuses when the signature is wrong

**Headline finding first: 🚨 the installed/soaking beta.2 ships Sparkle 2.9.3.** The bump commit `e556523` landed 2026-08-17 12:41; the beta.2 mac build ran 08:48 the same morning. Nothing built anywhere has ever contained 2.9.6 — this session was its first execution. Your "watch the first macOS tag build" note stands: beta.3's build will be CI's first 2.9.6 run.

Everything below was measured locally on a real built bundle with 2.9.6 embedded per release.yml's exact steps.

**1. Layout surgery — verified, and shown to be load-bearing.** Replicated the release.yml pipeline verbatim against the real 2.9.6 GitHub asset: the `cp -r` in the download step **dereferences every top-level symlink** (real files at framework root, real `Versions/Current` directory, XPCServices present) — so the surgery is what makes the framework signable, not belt-and-braces. Post-surgery: `Versions/Current → B` symlink, all 7 root items symlinks, `Versions/B/XPCServices` gone — assertion script passes. **Negative control:** the same script against the pre-surgery copy fails every check.

**2. codesign — verified with its negative control.** Post-surgery 2.9.6 inside a real .app: signs, `codesign --verify --verbose=4` clean on framework and app. Pre-surgery framework inside the same .app: codesign errors on the framework subcomponent and verify reports nested-code-modified — the "unsealed contents" family, as predicted. (Caveat: ad-hoc identity — no Developer ID cert on this machine — so seal/structure semantics are exercised, notarization/quarantine is not.)

**3. `sign_update` 2.9.6 — compatible with release.yml's key format.** Accepts the 44-char/32-byte-seed `--ed-key-file` form (throwaway key minted for the rig; the production key and the owner's Keychain were never touched), still emits `sparkle:edSignature` + `length`. `--verify` passes intact payloads and **fails on a tampered payload and on a corrupted signature** (rc=1 both).

**4. A real update, through the full shipped client.** Old bundle v20099 → local signed feed → DMG with v20100. In silent mode: appcast fetched, DMG downloaded, EdDSA validated, `willInstallUpdateOnQuit` fired ("staged for install on quit"), and — the part we changed the config for — **`Autoupdate` ran from the framework itself, i.e. the XPCServices-less in-process path**. On quit the bundle at the same path became 20100, signature still valid, and the updated app **boots and runs**. One deliberate caveat: silent mode installs on quit **without auto-relaunch by design** (our delegate returns NO from `willInstallUpdateOnQuit`; Sparkle semantics). The interactive "Install and Relaunch" click-path was not exercised (headless rig). The rig persists — if you want that half too it's cheap: notify mode + one human click.

**5. ⛔ Negative controls on the full client — both red, correctly.**
- **(A) wrong `edSignature` in the feed:** Sparkle fetched, downloaded, **refused** — nothing staged, no installer process, bundle stayed 20099.
- **(B) signature correct, DMG tampered** (one byte flipped mid-file, length unchanged): downloaded, **refused**, stayed 20099.
Both under conditions where the positive path staged within ~6 s. *"Updates, and refuses when the signature is wrong."*

**6. Two side-findings worth a ticket:**
- **No local build can exercise Sparkle.** Nothing local embeds the framework: absent `external/Sparkle.framework` the updater is silently compiled out (`__has_include` gate in `AutoUpdater_mac.mm`); with the framework present at configure time, the binary links it but `mac_build_run.sh` never copies it into the bundle → **dyld SIGABRT at launch** (measured: exit 134, "Library not loaded: @rpath/Sparkle..."). Separately, updater init is gated `!hodos::IsDevEnv()` (`cef_browser_shell_mac.mm`), so dev-mode runs skip Sparkle entirely.
- **The A3/minimumSystemVersion fix is compatible with the shipped client:** my rig's feed carried `<sparkle:minimumSystemVersion>12.0</sparkle:minimumSystemVersion>` and 2.9.6 accepted and installed normally.

## M6 — §A4 Big Sur: my recommendation

**Option 2 — a pinned final 0.3.x — implemented as a permanent second feed item.** The 0.4.x item carries `minimumSystemVersion` 12.0; the last 0.3.x item stays in the feed forever with 11.0. Sparkle installs the newest item *eligible for the client's OS* (documented Sparkle behavior — **not measured here**; the two-item selection test was cut for isolation reasons, see M7. The single-item + `minimumSystemVersion` half WAS measured, M5.6).

Reasoning:
- It **strictly dominates option 1 (nothing)**: same near-zero cost, and a Big Sur user stuck on an *older* 0.3.x still converges to the best version they can run instead of freezing wherever they are.
- **Option 3 (in-app message) costs a whole release** — new code shipped to macOS 11 users means one more 0.3.x build off a dead branch, for an audience of unknown size. **We have no telemetry; the honest population number does not exist.** 11.0 was the published floor for the entire CEF 136 era, so it is plausibly nonzero — but spending a release on it needs evidence. Option 2 doesn't foreclose it: if Big Sur support tickets ever arrive, ship the message then.
- **Prerequisite confirmed:** the appcast is generated and EdDSA-signed inside release.yml at build time — it cannot be patched at promote time, so `generate-appcast.py` must learn `--macos-minimum-system-version` in the beta.3 cycle. The ticket's derive-from-`MACOSX_DEPLOYMENT_TARGET` approach and its negative controls are all endorsed; add the second (0.3.x) item emission + promote-time assertions for both items while in there.

## M7 — Incidents, hazards, housekeeping

- **H-B:** no stall during today's soak. Installed beta.2 + wallet healthy throughout (balance 200 OK, `bsvPrice` live). Evidence-capture protocol stays armed; nothing to add.
- **⚠️ Isolation incident (owner already briefed):** my prod-mode Sparkle rig runs opened the **real** profile directory — `AppPaths::EnforceDevSafeguard` classifies "dev build" by a `build/bin` path substring, so a bundle copied elsewhere scrubs `HODOS_DEV`; and a `$HOME` override redirects `SettingsManager` (getenv) but **not** the profile root. ~10 min of same-engine profile exposure, wallet **never contacted** (no listener on 31301 during those windows, verified; the one run that overlapped with the live installed app stalled pre-init and was killed). Two consequences: (1) watch for bookmarks/history oddities on the Mac this week; (2) prod-mode test bundles are hereby not-runnable on this machine — which is why the two-item Sparkle feed test was cut rather than measured.
- The `argv[0]` trap from the codec_check era bit again in live form: a `./`-launched bundle is invisible to path-scoped `pkill`. Kernel-truth process matching remains the rule.
- CI framing correction (C0) acknowledged — org-repo release/promote lanes usable before the reset.
- Kept on the Mac for future rounds: `cef-native/build-sparkle/` (the only Sparkle-capable local build) + `external/Sparkle.framework` 2.9.6 (gitignored), and the three §B1 test pages.

## M8 — What I need back

1. Your call on M1b: proceed on (b) Windows-shaped, or hold the (b)-half of WS1 for the Mac answer (no date).
2. Whether a macOS MetaNet Client exists/matters for C2's Mac half.
3. Whether C3 is hard-blocking W1 — if yes it leads my next session.
4. Whether you want the interactive "Install and Relaunch" Sparkle half exercised (notify mode, one click, rig is warm).
5. Who commits the one-line `device.audio-input` entitlement fix, and confirmation it rides beta.3 (it must be in the signed build to verify).

---

# 📋 ROUND 2026-08-18 (Windows) — 👉 **FINISH YOUR REVIEW AND PUSH BACK — planning is HELD on you.** 🚨 Two shipping security defects found on this side; a third workstream (WS5) has been added to the sprint.

⛔ **Nothing is being cut or committed until this round comes back.** The beta.3 kickoff review is
done on the Windows side and the cut line is the last open decision. Four asks from earlier rounds
are **still outstanding** (§A2 Sparkle, §A4 Big Sur, §B1 mic/camera, §B2 overlay symptoms) — those
have not been superseded, they are still what we need. Two more are added below.

## C0 — What changed here, so you are reviewing the current plan and not the old one

- **WS5 added** — `TICKET_loopback_host_form_wallet_routing.md`, split across **Phase 0.5** and
  **Phase 5**. `SPRINT_PLAN.md` §3/§4/§5/§6 all updated. Read §4 for the running order.
- **Phase 0 grew and its rationale changed.** The stray-`{app}`-log ticket now covers **52** writes
  across **three** files (`startup_log.txt` was missed). ⚠️ And its headline —
  *"can silently abort auto-update"* — **did not survive verification**: `MaybeApplyStagedUpdate`
  runs before `CefInitialize` and only past a `selfCount == 1` gate, so no Hodos process can be
  writing during the manifest walk. Corrected in place. What replaces it is worse, see C1.
- **CI framing corrected.** The exhausted quota is the **dev fork's test lane only**. `release.yml`
  and `promote.yml` run on the org repo with free minutes. beta.3 *can* ship before the reset; what
  cannot run before it is `cargo test` / clippy / F8 / `cargo audit` / `npm audit`.

## C1 — 🚨 Two shipping security defects, for your awareness (both fix on our side)

Neither needs Mac work — flagged because they change the sprint's shape and you should not be
reviewing a cut line that predates them.

1. **The recovery phrase appears to be written to disk in plaintext, in the install root.**
   `WalletService::makeHttpRequest` logs full response bodies under 500 chars
   (`WalletService.cpp:222-227`) and `readResponse` logs them again under 1000
   (`:311-313`). `POST /wallet/create` returns `{"success":true,"mnemonic":"…",…}` (~200 bytes).
   ⛔ **Not yet reproduced** — the one-minute experiment is create-a-wallet-then-grep. Windows-only:
   `WalletService_mac.cpp` has zero `ofstream` writes. Also: the F8 secret-log gate cannot catch this,
   because its C++ pattern covers `cout/cerr/printf/fprintf/OutputDebugString` — not `ofstream`.
2. **`/transaction/send` has no approval gate.** `send_transaction` (`handlers.rs:9612-9951`) takes no
   `HttpRequest`, so it cannot read `X-User-Approved`; it contains zero permission/dispatch calls; and
   it honours `sendMax`. Verified. This is Rust, so it is **your binary too** — one fix, both platforms.

## C2 — 👉 NEW ASK: is the cross-wallet routing hole live on macOS?

On Windows, `netstat` shows MetaNet Client `LISTENING` on **both** `127.0.0.1:3321` and
`127.0.0.1:2121` (PID 37360). Our interception gate matches only the literal string `localhost:3321`
/ `localhost:2121`, so a dApp addressing the IPv4 form inside Hodos **falls past us and is answered by
a different vendor's wallet** — different identity key, no Hodos gate, no indication to the user.

👉 **What I need:** `lsof -nP -iTCP -sTCP:LISTEN | grep -E '3321|2121'` (or `netstat -an | grep LISTEN`)
on your Mac, with and without MetaNet Client running. A yes/no plus the process name.

Why it matters: if it reproduces on macOS the hole is cross-platform and WS5(b) is a **security**
item, not just an interop one. If MetaNet Client is not installed there, say so — absence of a
listener on your machine is not evidence the hole is Windows-only, and I do not want that recorded
as though it were.

## C3 — 👉 NEW ASK: does a `CefResourceHandler` take over `https://` loopback before TLS?

This is WS5's single biggest design unknown and it is cheap to observe once.

The App Lab probes `https://127.0.0.1:2121` **before** `http://127.0.0.1:3321`. If returning a
resource handler for an `https://` loopback URL short-circuits before certificate validation, we can
answer it. If cert validation fires first, the user gets an interstitial and **W1 must deliberately
not match 2121**, so the probe fails fast and falls through to 3321.

`cef-binaries/tests/ceftests/cors_unittest.cc` reportedly serves https from `GetResourceHandler` with
no server, which is encouraging — but it has never been exercised in this codebase, on either
platform.

👉 **Report the observed behaviour, not the expectation:** interstitial, silent failure, or clean
synthesized response. ⚠️ And confirm which you saw it on — a `type:"page"` CDP target is not proof of
which browser you were driving; this project has faked three findings that way.

## C4 — Not coming to you

WS5(a) is entirely ours: the two Rust defects are one binary, and the three `:5137` substring gates
are cross-platform C++ that fix once. WS5(b)'s W7 overlay coverage (wallet / wallet_panel / settings /
backup still reaching Rust after the predicate swap) **will** need you — but not until Phase 5, and
only if the cut line reaches it.

## C5 — What I need back, in priority order

1. **§B2** — do the two overlay symptoms reproduce on macOS? This gates WS1, which is Phase 1.
2. **§B1** — mic/camera *cause*, not "still broken".
3. **§C2 + §C3** — the two new WS5 questions above.
4. **§A2** — Sparkle 2.9.6 green **and** its negative control.
5. **§A4** — your call on Big Sur.
6. Anything macOS-shaped you want inside the cut line before it is set.

---

# 📋 ROUND 2026-08-17b (Windows) — 👉 **TWO MORE ASKS, both are INPUTS to the beta.3 design, not test-passes.** Plan is now in `SPRINT_PLAN.md`.

beta.2 will **not** be promoted — it is kept as a draft soak build. **beta.3 is what users get.**

Alongside §A2 (Sparkle) and §A4 (Big Sur), two things I need **before** designing, because either
answer changes what we build rather than merely whether it passed.

## B1 — 👉 Mic/camera on macOS: diagnose, don't just confirm it's broken

Twitter Spaces did not work on Mac. Windows mic is reported working. From here I could establish
that the obvious causes are **already handled**:

- `cef-native/mac/entitlements.plist` has `com.apple.security.device.microphone` **and** `…device.camera`
- `cef-native/Info.plist` has `NSMicrophoneUsageDescription` **and** `NSCameraUsageDescription`
- `SimpleHandler::OnRequestMediaAccessPermission` **is implemented** and inspects the audio / video /
  desktop-capture flags

⛔ **My prime suspect, which only you can test:** `cef-native/mac/helper-Info.plist.in` has **neither
usage string**, and on macOS capture runs in a **helper process**. If TCC attributes the request to
the helper, it is denied against a bundle that never declared a purpose.

Worth checking in this order:
1. Does the TCC prompt appear **at all**? (`tccutil`/System Settings → Privacy → Microphone — is
   Hodos listed?) A missing prompt and a denied prompt are different bugs.
2. Console.app filtered on `tccd` while triggering a mic request — it names the **responsible
   process**, which settles the helper theory outright.
3. `getUserMedia` on a plain test page before blaming Twitter — Spaces is a heavy subject; confirm
   the simple case first.

👉 **Report the cause, not just "still broken."** If it is the helper plist, that is a one-line fix
we make on this side; if it is something else, we design differently.

## B2 — 👉 Do these two overlay symptoms reproduce on macOS?

WS1 is the first workstream and the highest-value one. Two Windows symptoms:

- **Dead zone below a modal.** Clicking just *below* an overlay does not close it; further below, or
  left/right, does. Suspected: overlay window taller than the rendered React content, so the empty
  strip still counts as "inside".
- **Mouse offset after moving to a second monitor.** On the smaller screen the cursor is off inside
  the **wallet overlay** — hovering a button does not highlight, slightly above it does. Correct
  again on the primary screen. Suspected: per-monitor DPI not re-resolved for the monitor the OSR
  overlay is on.

⚠️ **I am not assuming these transfer.** macOS uses borderless `NSWindow` + paired NSEvent monitors
(`InstallClickOutsideMonitor`) — there is **no `WH_MOUSE_LL`**, and no `WM_ACTIVATEAPP`. The
mechanisms are structurally different.

👉 **All I need is yes/no per symptom, on a two-display Mac with different scale factors.** If they
do not reproduce, we fix Windows only and stop. If they do, we design one shared model instead of
shipping a Windows-shaped fix and rediscovering the problem later.

## B3 — What is NOT coming to you

Items **3 (taskbar identity)** and **5 (virtual-desktop focus)** are Windows-only by nature — no
macOS analogue. Tab context-menu parity gets built on Windows first and then ported. The Chrome-import
macOS half (**Keychain**, not DPAPI — assume no symmetry) waits until WS4 starts.

---

# 📋 ROUND 2026-08-17 (Windows) — 👉 **ACTION FOR MAC: verify Sparkle 2.9.6 locally.** ⛔ **It ships on macOS, it is bumped, and NOTHING has run it.** 🚨 **Also: the macOS appcast advertises no minimum OS version, and our floor moved 11.0 → 12.0.**

## A1 — 👉 The ask, in order

Two items, both macOS-only, neither needing CI minutes (the dev fork's are exhausted until ~Sept 1).

1. **Verify Sparkle 2.9.6** on a real macOS build — §A2.
2. **Weigh in on the Big Sur question** — §A4. It is a product call with a macOS-shaped answer.

## A2 — ⛔ Sparkle 2.9.3 → 2.9.6 is bumped and unverified

Landed in `release.yml` (`e556523`). It is the **shipped macOS auto-update client**, so a defect here
does not break a page — it breaks the mechanism by which every user receives every future fix.

**What I verified (Windows, layout pre-flight only):** `release.yml` reaches into the extracted
tarball by literal path, so I confirmed against the real 2.9.6 asset that all of these still exist
and are unchanged in shape:

```
bin/sign_update
Sparkle.framework/Versions/B          Sparkle.framework/Versions/Current
Sparkle.framework/Versions/B/Autoupdate
Sparkle.framework/Versions/B/Updater.app
Sparkle.framework/Versions/B/XPCServices     (the one release.yml DELETES)
```

and that `sign_update` still carries `--ed-key-file` and still emits `sparkle:edSignature`.

⛔ **That is a filesystem check, not a functional one. No Windows machine can verify a macOS
framework.** Everything below is what I could not do.

### What to run

```bash
git pull origin 0.4.0
cd cef-native && ./mac_build_run.sh --clean     # --clean: stale CMakeCache keeps the old framework
```

Then, against the built bundle:

1. **Framework survived the symlink surgery.** `release.yml` deletes `XPCServices` and rewrites every
   top-level item as a symlink into `Versions/Current/`. Confirm on the local build:
   - `Versions/Current` is a **symlink to `B`**, not a copy
   - `Autoupdate`, `Sparkle`, `Updater.app`, `Headers`, `Resources`, `Modules` at the framework root
     are **symlinks**, not real files — a real file at root gives "unsealed contents" at codesign
   - `Versions/B/XPCServices` is **gone**
2. **`codesign --verify --verbose=4`** on the framework and the app bundle.
3. **A real update.** Install an older build, point Sparkle at a local feed, take the update, and
   confirm the app **relaunches**. That is the assertion that matters — 2.9.x has changed the
   in-process updater path before, and we removed XPCServices deliberately (non-sandboxed Developer
   ID app; their bootstrap-launch fails under quarantine).
4. ⛔ **Negative control.** A green update test proves nothing unless you have seen it go red. Break
   it deliberately — corrupt the DMG after signing, or feed a wrong `edSignature` — and confirm
   Sparkle **refuses**. Report both halves: *"updates, and refuses when the signature is wrong."*

### Context you will want

- The Ed25519 scheme is unchanged between 2.9.3 and 2.9.6, so `SUPublicEDKey` in `Info.plist` and the
  `SPARKLE_EDDSA_PRIVATE_KEY` secret are untouched.
- On the Windows side I bumped `winsparkle-tool` 0.9.3 → 0.9.4 and **did** get a functional
  round-trip: `generate-key → public-key → sign → verify` passes, and fails correctly on a tampered
  payload, a corrupted signature and a wrong key. The shipped WinSparkle **0.8.1 DLL is untouched**.
- ⚠️ The Windows Stage-1 rigs (`test-apply-*`, `test-update-feed`) **cannot** cover either bump —
  they drive our own `hodos-update-helper` / `UpdateStager`, never Sparkle or WinSparkle. Do not let
  a green Windows rig read as coverage for your side.

## A3 — 🚨 The macOS appcast advertises NO minimum system version

`scripts/generate-appcast.py` has never emitted `<sparkle:minimumSystemVersion>` — no argument, no
code path, no OS gating. **Zero occurrences** in the beta.2 draft feed *and* in the live beta.29 feed.

Meanwhile our floor rose with CEF 150. beta.2's own CI log: `minos guard PASSED (all >= framework 12.0)`.

⇒ A **macOS 11 (Big Sur)** user on 0.3.x would be offered 0.4.0, Sparkle would install it, and dyld
would refuse a `minos=12.0` binary. App does not launch, Sparkle has already replaced the old one,
no in-product way back. **11.0 was our published floor for the entire CEF 136 era**, so that is
exactly the affected population.

It has not bitten only because **no 0.4.0 feed has ever been promoted.** Ticket:
`TICKET_appcast_missing_minimum_system_version.md`. Fix lands in beta.3 — it must be in the build
that produces the feed, because the appcast is **signed at build time** and cannot be patched at
promote time.

## A4 — 👉 Owner/Mac call: what do Big Sur users get?

Fixing the element stops the brick. It does **not** answer what those users should see. Options:

1. **Nothing** — they sit on 0.3.x forever with no notification. Silent dead end.
2. **A pinned final 0.3.x** as their terminal version, with a note.
3. **An in-app message** telling them why updates stopped.

Your read matters more than mine here — you have the macOS version-share intuition and the Sparkle
behaviour knowledge. ⚠️ Note we have **no telemetry**, so nobody can say how many users this is; the
honest framing is "unknown, and the gate costs one line."

## A5 — Also landed since the 0.4.0 relay's last round

- `v0.4.0-beta.2` **built, signed, notarized, verified — and deliberately NOT promoted.** Draft only.
  `promote.yml` dry run `32050154040` passed every gate (first-ever CI execution of both the AV and
  farbling gates), with every irreversible step skipped.
- **Node 20 → 22** (20 was 4 months past EOL) and `vite.config.ts` now pins `build.target: 'chrome150'`,
  binding the React bundle to the shipped engine. Measured: 1.09% smaller output, every chunk changed.
- ⚠️ **The dev fork's Actions minutes are exhausted** (~2 weeks to reset). `test.yml`'s push trigger
  is suspended and `ci.yml` trimmed to `main`. **Nothing has been tested in CI since 2026-08-14**,
  including everything in beta.2. Release builds were unaffected — they run on the org repo.
- 🎫 Five beta.3 tickets are filed in this folder; `README.md` has the candidate list.

## A6 — What I need back

- Sparkle 2.9.6: **green + its negative control**, or a defect.
- Your call on §A4.
- Anything macOS-shaped you want in the beta.3 cut line before it is fixed.
