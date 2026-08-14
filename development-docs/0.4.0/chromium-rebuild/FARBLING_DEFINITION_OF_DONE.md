# FARBLING — DEFINITION OF DONE

> **This document is the scope. Nothing else is.** If a realm or a vector is not in a table
> below, it is not covered, and saying otherwise in a release note is a defect.
>
> **Status legend.** ✅ green (measured) · ⛔ open (measured) · ⏸️ deferred (owner-signed, §F)
> · ❓ **unknown — never measured** (treat as ⛔ until measured; an unknown cell may not be
> called done, deferred, or shipped-around).
>
> **Every claim here carries its evidence class:** `MEASURED` · `CODE-READ` · `UNKNOWN`.
> A CODE-READ cell is a hypothesis. This project has had code reads turn out both right
> (the iframe bypass) and wrong (`SW_SECURE_DECODE`), so the class is load-bearing.

---

## 0. Why this document exists

Three consecutive review rounds each discovered a *new* unfarbled container:

| Round | Discovered | Cost |
|---|---|---|
| 2026-08-10 | cross-site iframes unfarbled | scope line rewritten |
| 2026-08-13 | **every** iframe unfarbled, same-origin included — a **bypass** | release held |
| 2026-08-13 | `window.open()` popups unfarbled — same bypass, different container | patch reworked mid-flight |
| 2026-08-14 | dedicated workers still open, and they were **never hard** | a week of avoidable delay |

The root cause was not any individual miss. It was that **nobody enumerated the realms up
front**, so scope was discovered one measurement at a time and each discovery invalidated the
previous release wording.

⛔ **The specific failure worth naming**, because it is the one that cost most: the
2026-08-09 deferral bundled *all* workers together as "needs cross-process machinery." That is
true of shared and service workers. It is **false of dedicated workers**, which are in-process
and inherit from a parent context that already holds the key. `PLAN_P4e_iframe_farbling.md` §5
repeated the bundle unexamined, and the cheapest remaining fix sat deferred behind the two
expensive ones for a week. **Never defer a group; defer each realm on its own evidence.**

---

## A. Realm coverage — where JavaScript can read a fingerprint surface

The key is `HMAC(profile_seed, registrable_domain_of_top_frame)`. `HodosSessionCache` is
fail-closed: **no key ⇒ native values**. So "unfarbled" always means "no key was delivered
here", never "the perturbation failed".

| # | Realm | State | Evidence | Proven by | Negative control |
|---|---|---|---|---|---|
| R1 | Top-level document | ✅ | MEASURED | `farbling_acceptance_battery.py` 7/7 | `--negative-control` |
| R2 | Same-origin iframe (`about:blank`, `srcdoc`, `blob:`, `data:`) | ✅ | MEASURED both OS | `farbling_subframe_check.py --vector iframe` | ran RED pre-fix (exit 2) |
| R3 | Same-site cross-origin iframe | ❓ | **UNKNOWN** | — see §A.1 | — |
| R4 | Cross-site iframe (OOPIF) | ✅ | MEASURED | `farbling_iframe_check.py` (strong S3) | reference-not-farbled guard |
| R5 | Popup `window.open()` (origin-inheriting) | ✅ | MEASURED both OS | `farbling_subframe_check.py --vector popup` | ran RED pre-fix (exit 2) |
| R6 | Popup navigated to a real URL | ✅ | CODE-READ | it is a top frame ⇒ R1 path | — owed |
| R7 | **Dedicated worker** | ⛔ | **MEASURED on the shipping engine** (E1 closed 2026-08-14) | `farbling_worker_probe.py --auto` | control arm (opt-out) collapses main==worker; see A.4 |
| R8 | Nested worker (worker → worker) | ❓ | UNKNOWN | — | — |
| R9 | Shared worker | ⛔ | CODE-READ | no install point exists | — |
| R10 | Service worker | ⛔ | CODE-READ | no install point exists | — |
| R11 | AudioWorklet / PaintWorklet / other worklets | ❓ | UNKNOWN | — | — |
| R12 | Fenced frame | ❓ | UNKNOWN | `GetOutermostMainFrame()` should escape it | — |
| R13 | Sandboxed iframe (opaque origin) | ❓ | UNKNOWN | gets top-frame key by construction | — |
| R14 | `javascript:` / `document.write` document | ❓ | UNKNOWN | — | — |
| R15 | Prerendered / bfcached page | ❓ | UNKNOWN | — | — |
| R16 | Hodos internal UI (header, ~15 overlays) | ✅ **never farbled, by design** | MEASURED | localhost skip in `MaybeApplyHodosFarblingKey` | startup cost gate |

### A.1 — R3 is deliberately untestable by both existing harnesses

A same-site cross-origin child has **no separate CDP target** (same site ⇒ same process) and
**no `contentWindow` access** (cross-origin). Settling it needs a two-hostname local server.
**Decision (2026-08-13, both sessions): do not build one.** The strengthened R4 test asserts
`child == its own parent's farbled value`, which covers the keying model R3 was a proxy for.
R3 stays ❓ as a *documented* gap, not a silent one.

### A.2 — R7 is the live bypass and it is not hard

A dedicated worker is created by a document, runs **in-process**, and is constructed from a
parent `ExecutionContext` that already holds the key. `GlobalScopeCreationParams` already
carries inherited state from parent to worker global scope.

⇒ **Fix: carry the 32-byte key + `enabled` bit in those params and install into
`HodosSessionCache` at worker-global-scope construction.** No browser round-trip, no IPC, no
perf question, no keying ambiguity. Nested workers (R8) inherit transitively for free.

⚠️ Unlike P4e's iframe half — which was libcef-only and cost **zero** Chromium rebase surface —
this is a **Blink patch** and joins the per-bump maintenance budget. That is the real cost, and
it is the only reason this is not trivially free.

### A.4 — E1 closed: R7's RED baseline on the engine we actually ship

The prior R7 measurement was macOS, fork `c63654654` — two engines and one OS away from
what ships. Re-measured **2026-08-14, Windows, `Chrome/150.0.7871.187` (fork `7dd0357`)**:

```
FARBLED arm   main   canvas=e865bafb  cores=10  mem=32
              worker canvas=2fad2e1a  cores=24  mem=32   <- NATIVE
CONTROL arm   main   canvas=2fad2e1a  cores=24  mem=32
              worker canvas=2fad2e1a  cores=24  mem=32
```

All four controls green: both arms on the same engine; the opt-out verified **off disk**
in each arm; farbling-off collapses main==worker (so the two `OffscreenCanvas` paths do
render identically and a difference really does mean farbling); and the main thread is
demonstrably farbled (`2fad2e1a → e865bafb`). `cores` carries the finding independently of
canvas (`10` on the main thread vs the native `24` in the worker).

⚠️ `mem` is `32` everywhere here — that is the §B.2 draw collision, not evidence. It is
why this probe reports three vectors and not one.

**Two subject assertions were added to the probe before this run, and neither is optional:**

1. **Engine.** The owner's *installed* browser holds CDP **9222** while a `--dev` build
   holds **9322**. The port was a bare int with no engine check, so one wrong flag would
   have measured the shipped pre-P4e engine and produced a RED that says nothing about the
   build under test — the same defect class as the harness that drove an overlay.
2. **The opt-out actually landed.** The control arm's entire meaning is "farbling is off
   here". A silently-failed settings edit (the natural hand-edit, a bare `false` instead of
   `{"enabled": false}`, is ignored) makes *both* arms farbled runs, in which `main ==
   worker` holds for the ordinary reason and the probe prints a confident verdict about
   nothing.

Both assertions were themselves shown to fire — engines differing, engine unreadable,
opt-out missing, and farbled-arm-opted-out each refuse to render a verdict, while a clean
pair passes through. They can only ever refuse, never turn a red into a green.

### A.3 — why R9/R10 are genuinely harder (and not the same job)

- **R9 shared worker:** many documents under *different* top frames may connect to one worker.
  "Which top frame's key" has no correct answer.
- **R10 service worker:** no top frame at all, outlives every document. ⛔ **The trap:** keying
  on the SW's own origin looks natural but is wrong — a third-party iframe (`evil.com` inside
  `shop.com`) can register a SW for `evil.com`, which would then hand out a **stable cross-site**
  fingerprint. That is exactly the model rejected for iframes.

Both are reachable **same-origin only**, so the bypass is narrower than R7's.

---

## B. Vector coverage — which surfaces are actually perturbed

**Every row below was re-measured on 2026-08-14** against engine `Chrome/150.0.7871.187`
(fork `7dd0357`, Windows) by **`farbling_vector_matrix.py`** — one browser session, each
vector judged on its own evidence. Negative control: `--negative-control` runs both arms
with farbling off, and the run's own positive control then reports `NATIVE`, i.e. the rig
demonstrably reports the feature's *absence* rather than its presence. **No row in this
table is a code read any more.**

| Vector | Hook | Covered | Evidence | Gap |
|---|---|---|---|---|
| Canvas 2D `getImageData` | `BaseRenderingContext2D::getImageDataInternal` | ✅ | MEASURED | — (shared base ⇒ OffscreenCanvas too) |
| Canvas `toDataURL` | `HTMLCanvasElement::ToDataURLInternal` | ✅ | MEASURED | — |
| Canvas `toBlob` | `HTMLCanvasElement::toBlob` | ✅ | **MEASURED** (was CODE-READ) | — `92a26986` native → `a1fb54de` farbled |
| **`OffscreenCanvas.convertToBlob`** | — | ⛔ | **MEASURED** (was CODE-READ) | **unhooked, confirmed.** `92a26986` in *both* arms — byte-identical to `toBlob`'s **native** output, so the two endpoints encode the same image and only `toBlob` is perturbed. Reachable **on the main thread today**, so this is not gated behind R7. |
| WebGL `readPixels` (WebGL1 + 2) | `WebGLRenderingContextBase::ReadPixelsHelper` | ✅ | MEASURED | — |
| WebAudio `AudioBuffer.getChannelData` | `AudioBuffer::getChannelData` | ✅ | MEASURED | bindings overload only, deliberately |
| WebAudio `AnalyserNode.getFloatFrequencyData` | `AnalyserNode::getFloatFrequencyData` | ✅ | MEASURED | — |
| **`AnalyserNode.getByteFrequencyData`** | — | ⛔ | **MEASURED** (was ❓) | **unhooked, confirmed.** `b680346b` in both arms |
| **`AnalyserNode.getFloatTimeDomainData`** | — | ⛔ | **MEASURED** (was ❓) | **unhooked, confirmed.** `f522dfc7` in both arms |
| **`AnalyserNode.getByteTimeDomainData`** | — | ⛔ | **MEASURED** (was ❓) | **unhooked, confirmed.** `57085a52` in both arms |
| `navigator.deviceMemory` | `NavigatorBase` | ✅ | MEASURED | shared base ⇒ `WorkerNavigator` too. ⚠️ see B.2 |
| `navigator.hardwareConcurrency` | `NavigatorBase` | ✅ | MEASURED | reduce-only, floored at 2 (`24 → 10`) |

⛔ The three analyser rows are **one gap, not three**: `getByteFrequencyData` and both
time-domain readers each go straight to `AnalyserHandler` and never touch the hooked float
path. They are listed separately because §G.2 forbids deferring a group, and because a fix
must be shown to move each one.

### B.2 — ⛔ the small-codomain trap: `deviceMemory` measured NATIVE and was **not** a bug

The first run of `farbling_vector_matrix.py` reported `navigator.deviceMemory` as an
**unhooked vector**. It is not. `FarbleDeviceMemory` *draws* from `{4, 8, 16, 32}`, this
machine's native value is `32`, and the draw for `example.com` came out `32` — so a fully
live hook produced farbled == native. One domain in four does this.

The discriminator, now permanent in the harness: re-draw the scalar on **other registrable
domains** (the key is `HMAC(seed, registrable_domain)`), which resolved it immediately —
`example.net=16`, `example.org=8`, `iana.org=32`. Agreement on one domain is a coin flip;
agreement on four is 1 in 256.

⚠️ **This is the §D "green while broken" attack question firing in the opposite
direction** — a *false ⛔* that would have sent a whole build after a bug that does not
exist. Hash-valued vectors cannot do this (codomain 2^32); only small-codomain scalars
need the control, and only they carry it. **Any future scalar vector must ship the same
discriminator.**

### B.1 — deliberately NOT farbled (scope boundary, not an oversight)

Listing these is part of being done. Each is a real fingerprinting surface we do **not** touch:
screen dimensions · installed fonts · `mediaDevices.enumerateDevices` · WebRTC IP · timezone ·
`navigator.language` · `navigator.plugins` · WebGL `UNMASKED_RENDERER/VENDOR` strings · WebGPU ·
speech-synthesis voices · User-Agent / Client Hints.

⚠️ **WebGL renderer/vendor strings are the most valuable of these** and are trivially readable.
Farbling `readPixels` while leaving the GPU model in a string is a coherence question worth an
explicit owner decision, not silence.

---

## C. Cross-cutting contracts — must hold in **every** ✅ realm

| # | Contract | Proven by |
|---|---|---|
| C-1 | **Determinism** — same site + same session ⇒ identical values (the login guarantee) | rotation gate A→B→**A** round trip |
| C-2 | **Unlinkability** — different sites ⇒ different values | rotation gate; R4 strong assertion |
| C-3 | **Seed rotation** — new profile seed ⇒ all values change | `farbling_seed_rotation_check.py` (release gate) |
| C-4 | **Fail-closed** — no key ⇒ native, never a constant | code invariant; the shipped constant-seed bug is why |
| C-5 | **Exemption inheritance** — exempt top frame ⇒ every child native | `farbling_d5_residual_check.py` (MEASURED) |
| C-6 | **No tamper tell** — patched methods still report `[native code]` | battery |
| C-7 | **Windows / macOS parity** — every ✅ proven on both | both sessions, same pin |
| C-8 | **Negative control** — every test shown to FAIL with the feature off | per-test, mandatory |

---

## D. Release-claim ladder — what we are allowed to say

Tie the wording to the coverage, so the claim cannot drift from reality.

| Coverage | Permitted claim |
|---|---|
| Any realm ⛔ or ❓ that is **page-scriptable** | ❌ **No fingerprinting claim of any kind.** The protection is bypassable by the page it protects. |
| All page-scriptable realms ✅; shared/service workers ⏸️ | "Fingerprint protection on pages and their frames. Background service workers are not yet covered." |
| Every realm ✅ | "Fingerprint protection across all execution contexts." |

**Today we are on row 1** — R7 dedicated workers is open and page-scriptable, so **beta.2 must
not carry a fingerprinting claim.**

### D.1 — the three residuals any release note must state

1. **Widgets on non-exempt sites** — captcha/payment iframes are now farbled where they were
   native. *Not established as harmful*; the incoherent farbled-parent/native-child combination
   that shipped before P4e is the configuration these scorers actually reject.
2. **D5 exemption inheritance** — on all 37 `IsAuthDomain` hostnames, **every** embedded third
   party reads native values. ⚠️ **Unchanged from beta.1** — pre-P4e those frames were native
   too (unkeyed). Do not describe it as new; that would be inaccurate in the direction that
   costs the most trust.
3. **Workers** — the only remaining unfarbled realm, and page-scriptable. This is the largest
   residual and was **absent from the release-note ask entirely** until 2026-08-14.

---

## E. Ordered work list

| # | Item | Blocks a claim? |
|---|---|---|
| E1 | Re-measure **R7** on the current engine → RED baseline | yes — no baseline, no meaningful green |
| E2 | Implement **R7** dedicated-worker key inheritance (+ R8 free) | **yes** |
| E3 | Hook `OffscreenCanvas.convertToBlob` | **yes** — E2 is incomplete without it |
| E4 | Verify `getByte*` audio paths; hook if unfarbled | yes |
| E5 | Measure R6, R8, R11–R15 → move each out of ❓ | yes for any page-scriptable one |
| E6 | Owner decision on R9/R10 (defer vs implement) | no — ⏸️ is a valid end state |
| E7 | Owner decision on WebGL renderer/vendor strings | no |
| E8 | Release-note wording, all three §D.1 residuals | yes |
| E9 | T6 regression basket + soak, both platforms | yes |

**E2 + E3 + E4 are one Blink patch and one build cycle.** Batch them; the build dominates.

---

## F. Deferrals — owner-signed only

A deferral is valid only with a signature and a date. An unsigned row is ⛔, not ⏸️.

| Realm / vector | Deferred? | Owner | Date | Rationale |
|---|---|---|---|---|
| R9 shared workers | pending | — | — | keying genuinely ambiguous |
| R10 service workers | pending | — | — | no top frame; cross-site trap in §A.3 |
| R3 same-site cross-origin iframe | **accepted as documented gap** | both sessions | 2026-08-13 | unmeasurable without a two-host server; R4 covers the model |
| B.1 non-farbled vectors | pending | — | — | scope boundary |

---

## G. Process rules — so this does not happen a fourth time

1. **Enumerate before implementing.** A new realm is added to §A *before* any code, with state ❓.
2. **Never defer a group.** Each realm carries its own cost and its own decision. The
   all-workers bundle cost a week.
3. **❓ is not ✅.** An unmeasured realm may not be called done, deferred, or shipped around.
4. **Every ✅ names a test and a negative control.** A test never seen to fail proves nothing.
5. **The claim follows the matrix** (§D), never the other way round.
6. **CODE-READ never upgrades itself.** Only a measurement moves a cell to ✅.
