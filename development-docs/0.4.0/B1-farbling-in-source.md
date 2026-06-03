# B1 — Farbling into Chromium Source (Brave-style)

**Status:** 🔓 Unblocked — A4 decided (STAY ON CEF). **Design done → see `B1-farbling-design.md`.**
**Type:** refactor (large) · **0.4.0:** candidate

> Design research complete (2026-06-01): `B1-farbling-design.md`. Headline: use a **persistent
> per-profile seed** (not Brave's per-session) to fix login breakage; ExecutionContext Supplement
> covers workers; re-implement (don't copy MPL Brave text) or base on fingerprint-chromium (BSD-3).

## A4 finding (see `../DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md`)
We do NOT fork Brave. **A self-built CEF is a Chromium build, so we patch Blink directly in our own
CEF build** via CEF's `patch.cfg` mechanism — the same technique Brave uses, without being Brave.
- Current JS-injection farbling is detectable (≥6 mechanisms; **workers get raw unfarbled values** —
  `OnContextCreated` doesn't fire for Web/Service Workers — the top breakage cause).
- **Quick win:** add a worker-context hook (`DidInitializeWorkerContextOnWorkerThread`) for coverage.
- **Full fix:** Blink patches (Canvas readback, WebGL `getParameter`, AudioBuffer, navigator), origin-
  bound session seed tied to our existing token. Adapt approach (not verbatim text — MPL-2.0 license
  check) from **helium-chromium** / **fingerprint-chromium** references. Commit to ~4–6 wk rebase cadence.
- Couples to **A1** (this is the real justification for self-building CEF).

## Summary
Move fingerprint farbling from runtime JS injection down into the Chromium source, the way Brave
does it. Current approach is **detectable by some sites (breaks logins)** and suspected slow.

## Known facts (verified locations)
- Current farbling is JS injected at `OnContextCreated`:
  - `cef-native/include/core/FingerprintScript.h` — embedded `FINGERPRINT_PROTECTION_SCRIPT`
    (Mulberry32 PRNG; Canvas/WebGL/Navigator/AudioContext farbling).
  - `cef-native/include/core/FingerprintProtection.h` — per-domain seed via session-token hash mix.
  - Injection in `cef-native/src/handlers/simple_render_process_handler.cpp` (`s_domainSeeds`).

## Open questions / research needed
- **Detectability:** which sites detect our farbling, and how (timing? known JS signatures? value distributions?).
- **Perf:** is the slowness from JS injection per context, or from the farbling math itself?
- **Brave parity:** what exactly does Brave patch in-source, and does building from Brave (A4) give it to us "for free"?
- If we stay on upstream CEF, is source-level farbling even possible without a custom Chromium build?

## Dependencies
A4 decides the implementation surface. **Do not design B1 until A4 lands.**

## To fill after research
Acceptance criteria · Reuse map · Risk table · Implementation order · Test plan (Win/macOS) · What this does NOT do.
