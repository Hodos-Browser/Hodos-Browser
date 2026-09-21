# Phase 12 candidate (b) — the registry + renderer-pull patch

**Status:** ⬜ SCOPED, NOT STARTED. **Opened:** 2026-09-21, after the mitigation landed (`d79a869`).
**Prerequisite reading:** this folder's `README.md` — the measurement, the 2×2 factorial, and the
mitigation's measured residual. ⛔ Do not start from the original hypothesis; it is disproved.

---

## 1. What this fixes, in one paragraph

`OnBeforeBrowse` pushes the cosmetic scriptlet payload with
`frame->SendProcessMessage(PID_RENDERER, "preload_cosmetic_script")`. That is delivered to whichever
render process hosts the frame **at that moment** — the **source** document's. Every cross-site
navigation commits in a **different** render process, whose process-local `s_scriptCache` is empty, so
the pre-page-JS injection never happens. The mitigation in `d79a869` re-pushes from `OnLoadStart` and
injects on late arrival, which lands **25–41 ms after** the destination process created its V8 context.
⛔ **An inline `<script>` in the document head still wins that window.** This contract removes it.

## 2. The precedent — ⭐ we have already built this once, for the identical defect

`OnBeforeBrowse`'s **farbling** block, ~40 lines below the broken cosmetic block, carries this comment:

> *"This `SendProcessMessage` does **NOT** reach the renderer. libcef's browser side intercepts
> `hodos_farble_key` in `CefFrameHostImpl::SendProcessMessage` and files it in
> `hodos::FarblingRegistry`; the renderer then PULLS it at `OnContextCreated`. **That indirection is the
> fix for the delivery bug — a push from here is pre-commit, so it lands on the outgoing document.**"*

⇒ The shape is proven in production in this repo. The cosmetic path was simply never moved across.
Brave reaches the same answer independently: `ProcessURL` runs in **`ReadyToCommitNavigation`**, i.e. in
the frame that is about to commit — the destination process. (`PRIOR_ART.md`, 2026-09-21 row.)

## 3. ⚠️ The design decision this contract does NOT pre-make

Farbling's registry is **entirely internal to libcef** — libcef pulls the key and hands it to
`HodosSessionCache`, and no CEF API surface was added. Cosmetic scriptlets are different: the payload is
JS that must be **executed in the page**, and today that execution lives in **our** code
(`simple_render_process_handler.cpp :: OnContextCreated`). Two sub-options, and they differ in
maintenance cost, not correctness:

| | **b1 — expose a pull API** | **b2 — libcef injects, like farbling** |
|---|---|---|
| Patch files it in a `hodos::CosmeticRegistry` browser-side | ✅ | ✅ |
| Renderer half | new **public CEF API** our `OnContextCreated` calls | libcef executes the script itself at document start |
| Our C++ changes | `OnContextCreated` calls the new API instead of the map | the `preload_cosmetic_script` path is **deleted** |
| ⚠️ Cost | **adds public CEF API surface** — every Chromium bump re-litigates a signature we own | no new API, but the injection moves inside the patch where our tests cannot reach it |
| Testability | stays unit-testable on our side | ⛔ only testable through a built engine |

⭐ **Recommendation: b1.** It keeps the injection, the escaping and the logging in code we can test
without a Chromium build, and it is the smaller conceptual change. ⛔ **Owner decides before work starts.**

## 4. Cost

- **CEF fork patch** on `Hodos-Browser/cef`, branch `hodos/7871`, as `patch/patches/hodos_*.patch`,
  registered in the fork's `HODOS_PATCHES.md`.
- **Full Chromium+CEF rebuild** on the build host — see `DevOps-CICD/CEF_BUILD_RUNBOOK.md`. ⛔ There is
  **no local Chromium checkout**, so none of this is verifiable on the Windows dev box; the patch cannot
  be smoke-tested before the rebuild completes.
- **`CEF_CHECKOUT` bump** in both build scripts + the new SHA recorded. ⛔ A moving branch tip is not a
  reproducible build.
- **Per-Chromium-bump maintenance** forever, as with every patch in the set.

## 5. Scope notes

- ⚠️ The payload is **~34 KB and grows with the filter lists** (23 KB → 34 KB between 2026-09-19 and
  2026-09-21, from the engine's 6-hourly update). The farbling key it is modelled on is **32 bytes**.
  Whatever the registry does about lifetime and eviction must be decided deliberately, not inherited.
- ⚠️ **The CSS half rides along.** `inject_cosmetic_css` is pushed from the *late* path only, so element
  hiding is also post-load today. A registry that carries the scriptlets should carry the selectors.
- ✅ **`P12-A3` measured 2026-09-21 — no extra work needed.** The new-tab arrival
  (`OnBeforePopup` → `CreateNewTabWithUrl`) is the **same defect**, and the shipped mitigation already
  covers it (2/2 trials, distinct URLs). ⭐ One difference, and it makes this patch **easier**: on a
  navigate-in-place arrival the early push reaches the **wrong** renderer, but on a new-tab arrival it
  reaches **no renderer at all** — the brand-new browser has no render process for that frame yet, so
  the message is dropped outright. A browser-side registry the renderer **pulls** from is indifferent to
  both, because nothing is pushed. ⇒ **No special keying for the new-tab case.**
- ⭐ Two guards Brave has and we do not, both cheap, both worth porting with the patch:
  **(1)** a **fallback key** — when the URL is empty/invalid/`about:blank`, fall back to the frame's
  security origin, so a miss is impossible rather than silent; **(2)** at document start, **wait** on the
  payload when it has not arrived rather than skipping.

## 6. Acceptance — ⛔ every row needs its negative control

| ID | GREEN | RED (control) |
|---|---|---|
| `P12-B1` | cross-process arrival (`github.com` → `youtube.com/watch`) injects **inside `OnContextCreated`**, i.e. **0 ms** late — not the mitigation's `late-arrival inject` line | disable scriptlets for the host ⇒ no injection, **while the YouTube V8 context is still created** |
| `P12-B2` | same-process arrival unchanged, still pre-JS, still exactly **one** injection | as above |
| `P12-B3` | new-tab arrival via `OnBeforePopup` injects **pre-JS** (today it only gets the mitigation's ~30 ms late inject — measured 2026-09-21) | as above |
| `P12-B4` | an **inline `<script>` at the top of `<head>`** observes the **mutated** surface — the property the mitigation cannot deliver | revert the patch ⇒ it observes the un-mutated surface |
| `P12-B5` | minimal basket (youtube, x, github) clean, both platforms | — |

⛔ **`P12-B4` is the row that distinguishes this work from the mitigation.** If it is not measured, the
patch has not been shown to buy anything over what already shipped.

⛔ **Fresh URL per trial.** `s_scriptCache` is URL-keyed and one-shot; a leftover entry is consumed by
the next navigation to the same URL and scores a false GREEN. That artifact produced a false 2-of-3
during the Phase 12 measurement — `README.md` §"The near-miss".

⚠️ **Instrument:** the `[RENDER]` lines are in `cef_debug.log` (truncated per launch), **not**
`debug_output-<pid>.log`. `hodos::LogSafeUrl` is origin-only, so the renderer **PID** is the only
discriminator that distinguishes the processes.

## 7. Does this also close Phase 13?

⚠️ **Unknown, and do not assume it.** The early path's stated purpose is that anti-bot scripts observe
the mutated surface, and it has been measured **never to run on a cross-site arrival** — which raises
Phase 13's prior considerably. But Phase 13 **step 0** still stands: the CAPTCHA report was filed against
**0.3.x injected-JS farbling**, which carried a `toString` tamper tell that 0.4.0 deleted. Re-test on the
current build before treating Phase 13 as downstream of this patch.
