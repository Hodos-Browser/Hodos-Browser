# B1 — Farbling Design (Blink patches in our self-built CEF)

**Created:** 2026-06-01 · **Status:** ✅ Design research complete; ready for a build-time spike
**Depends on:** A1 self-build pipeline (we already have it). **Stub:** `B1-farbling-in-source.md`.
**Method:** primary-sourced (CEF patch.cfg/patcher.py, brave-core source, helium/fingerprint-chromium,
CreepJS, Mozilla MPL docs).

> Goal: replace detectable JS-injection farbling with **Blink C++ patches in our own CEF build** —
> undetectable, covers workers, and (critically) **stops breaking logins**.

## ⭐ The key design decision: PERSISTENT per-profile seed (not per-session)

This is the fix for the login breakage. Brave seeds farbling **per browser session** (random token
regenerated at each startup) → the same site sees a *different* fingerprint after every restart →
fingerprint-based re-auth reads us as a brand-new device → login friction.

**Hodos should instead use a persistent per-profile seed:**
```
profile_seed   = random, generated once per BROWSER PROFILE, stored in the profile's LOCAL BROWSER
                 DATA (C++ layer — ProfileManager / SettingsManager, %APPDATA%/HodosBrowser/<profile>/).
                 NOT the wallet DB — this is browsing-privacy state (like cookies/history), nothing to
                 do with keys or money.
domain_seed    = HMAC-SHA256(profile_seed, eTLD+1)      // stable per site, per profile
```
- **Stable across restarts** → a real, consistent browser fingerprint per site → logins don't break.
- **Different per site** (eTLD+1) → defeats cross-site tracking (the actual privacy goal).
- Reset only when the user clears that site's data / resets the profile.

This is a deliberate, justified divergence from Brave's default, matched to our product (a wallet
browser where users log in and stay logged in). Cross-*session* unlinkability is sacrificed; cross-
*site* unlinkability — the part that matters — is kept. (Confidence: HIGH this fixes the breakage.)

## Architecture: ExecutionContext Supplement (covers workers for free)

Implement a `HodosSessionCache` as a Blink `Supplement<ExecutionContext>`, mirroring Brave's pattern.
Every patched API calls `HodosSessionCache::From(*execution_context)`. Because `ExecutionContext` is
the base of `LocalDOMWindow`, `DedicatedWorkerGlobalScope`, `SharedWorkerGlobalScope`, and worklets,
**workers are covered automatically** — closing the single biggest current detection gap
(`OnContextCreated` never fires for workers, so today they leak raw values). Prefer this over CEF's
`OnWorkerContextCreated` (which races and needs IPC). (Confidence: HIGH)

## Where each piece lives (and why)

```
BROWSER PROCESS (C++)                      RENDERER PROCESS (Blink C++)
profile_seed                               receives profile_seed at startup
  • generated once per profile               │
  • stored in profile data (C++ layer)       ▼
  • NOT the wallet                          domain_seed = HMAC(profile_seed, eTLD+1)   (per page)
        │                                      │
        └─ passed to renderer ───────────►     ▼
           (cmd-line switch, like Brave's      farbling applied: patched Canvas/WebGL/
            --brave-session-token)             Audio/navigator + HodosSessionCache supplement
```
This mirrors Brave's split exactly. Brave generates the token **in memory, per session**, and passes
it to renderers; the HMAC + farbling run in Blink. **Our only divergence: persist the seed** in
profile data instead of regenerating each launch (the login fix). Farbling math is in Blink because
that's *below* JavaScript — the source of undetectability.

## On reusing Brave's code (can we just copy it?)

"We can't build Brave" (whole-browser fork — rejected) is different from "can we copy their farbling
code." We CAN read/adapt it (MPL-2.0, open), but it is **NOT a clean copy-paste**:
1. **Not standalone.** Brave's farbling files call Brave-only plumbing (`BraveSessionCache`, shields
   settings, session-token wiring, build-system include shadowing) that doesn't exist in our CEF tree
   — copied as-is they won't compile. We'd rebuild the glue (`HodosSessionCache`, our seed wiring) anyway.
2. **License.** Brave files are MPL-2.0 (file-level copyleft): copying their text obligates *those
   files* to stay MPL + be offered to users (doesn't infect adjacent code). Cleaner to **re-implement
   the technique** (algorithms aren't copyrightable) or start from **fingerprint-chromium (BSD-3)**.

**Plan: use Brave as the reference blueprint; reimplement ~5–8 patches fitted to our tree.**

## Blink files to patch (highest value first)

| Area | Files (`third_party/blink/renderer/...`) |
|------|------------------------------------------|
| Canvas 2D | `modules/canvas/canvas2d/base_rendering_context_2d.cc`, `canvas_rendering_context_2d.cc` (readback: getImageData/toDataURL/toBlob; measureText gate) |
| WebGL | `modules/webgl/webgl_rendering_context_base.cc`, `webgl2_rendering_context_base.cc` (getParameter incl. UNMASKED_VENDOR/RENDERER; getSupportedExtensions). **readPixels is a known gap** — consider platform `graphics/static_bitmap_image.cc` |
| WebAudio | `modules/webaudio/analyser_handler.cc`, `audio_buffer.cc`, `realtime_analyser.cc` |
| Navigator | `core/frame/navigator_device_memory.cc`, `core/execution_context/navigator_base.cc` (hardwareConcurrency), `modules/plugins/dom_plugin_array.cc` |

Start with **Canvas + WebGL** (highest fingerprint value), then Audio, then navigator.

## Build integration (CEF patch.cfg)
Fork `chromiumembedded/cef`; add our `.patch` files to `patch/patches/`; register them in
`patch/patch.cfg`; build via `automate-git.py --url=<our cef fork>`. `patcher.py` applies them to the
Chromium source before compile. Use a `condition` env gate if we want a build-time on/off. Ties into
the `CEF_BUILD_RUNBOOK.md` Step 2.

## License (do this right)
- **Re-implement the technique from scratch** using Brave as a *reference* (technique/algorithm isn't
  copyrightable) → no copyleft. OR base on **fingerprint-chromium (BSD-3)** — permissive.
- Brave files are **MPL-2.0** (file-level copyleft): copying their `.cc`/`.h` obligates *those files*
  to stay MPL and be offered to users. Adjacent proprietary files are unaffected, but cleanest is to
  not copy their text.
- **Do NOT use Bromite** (GPL-3 — would infect the product). Verify before touching any Bromite code.

## Maintenance
~2–8 h per Chromium/CEF version bump to rebase ~5–8 patches; canvas/WebGL/audio APIs are mature and
mostly stable. `base_rendering_context_2d.cc` is the riskiest (Blink refactors Canvas2D internals
occasionally). CEF's slower-than-Chromium cadence reduces rebase frequency.

## Verification (acceptance gate)
- **CreepJS**: zero "lies" on canvas/WebGL/audio (toString integrity intact since funcs are native);
  **worker column matches window column** (the JS-injection tell, now fixed).
- **browserleaks** webgl/audio/javascript: farbled values, consistent within a profile.
- **Cross-session login test (the important one):** create account → restart browser → revisit →
  should appear as the SAME device (persistent per-profile seed working), and logins should NOT break.
- Run on the standard site basket + the BRC-121 test site, Windows + macOS.

## Quick win before the full patch set
A worker-coverage hook alone (or shipping the Supplement with just Canvas) closes the highest-signal
detection vector (window-vs-worker mismatch) — a good first increment.

## Unknowns / to verify
- readPixels coverage strategy (Brave doesn't patch it; platform-layer option exists).
- Exact CEF 136 `OnWorkerContextCreated` availability (we prefer the Supplement path anyway).
- fingerprint-chromium's persistence model (per-session vs per-profile) before adopting its patches.
