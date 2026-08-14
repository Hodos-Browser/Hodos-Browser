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
| R6 | Popup navigated to a real URL | ✅ | **MEASURED** (was CODE-READ) | `farbling_realm_matrix.py` | both arms VOID with farbling off |
| R7 | **Dedicated worker** | ✅ **CLOSED by P4f** | **MEASURED** on `g9ccef04` | `farbling_worker_probe.py --auto` — exit **1 → 0**, same box, same command | control arm collapses main==worker; see A.4/A.7 |
| R8 | Nested worker (worker → worker) | ✅ **CLOSED by P4f** | **MEASURED** on `g9ccef04` | `farbling_realm_matrix.py` — canvas2d, WebGL **and** convertToBlob all keyed | 8/8 realms VOID with farbling off |
| R9 | Shared worker | ⏸️ **owner-signed 2026-08-14** (§F) | CODE-READ | no install point exists; keying genuinely ambiguous | same-origin only |
| R10 | Service worker | ⏸️ **owner-signed 2026-08-14** (§F) | CODE-READ | no install point exists; no top frame, cross-site trap §A.3 | same-origin only |
| R11a | **AudioWorklet** global scope | ✅ **no §B surface exists here** | **MEASURED** (was ❓) | `farbling_realm_matrix.py` — worklet reports `OffscreenCanvas=false, navigator=false` over its own `port` | page-side control proves the channel |
| R11b | **PaintWorklet** global scope | ✅ **no §B surface exists here** | **MEASURED** (was ❓) | same, via the CDP console channel — see A.5 | page-side control line |
| R12 | Fenced frame | ❓ | **UNKNOWN — and the container EXISTS** | see A.6 — **owner gate** | — |
| R13 | Sandboxed iframe (opaque origin) | ✅ | **MEASURED** (was ❓) | `farbling_realm_matrix.py` — child == top frame's farbled value | VOID with farbling off |
| R14a | `document.write` document | ✅ | **MEASURED** (was ❓) | `farbling_realm_matrix.py` | VOID with farbling off |
| R14b | `javascript:` URL document | ✅ | **MEASURED** (was ❓) | `farbling_realm_matrix.py` | VOID with farbling off |
| R15 | bfcache-restored page | ✅ | **MEASURED** (was ❓) | `farbling_realm_matrix.py` — and bfcache is **proven to have engaged**, see A.5 | VOID with farbling off |
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

### A.5 — E5 closed: how the remaining ❓ realms were settled, and the two instruments that lied

All measured 2026-08-14 on `Chrome/150.0.7871.187` by **`farbling_realm_matrix.py`**.
Every realm is judged **two-sidedly** — it must equal the top frame's *farbled* value to
pass, not merely differ from native. The one-sided version would let a realm keyed on its
**own** origin through, which is precisely the wrong-model outcome the design rejects.
Realms are compared only against a reference taken **with the same probe in the same arm**,
because a DOM-canvas hash and an `OffscreenCanvas` hash differ for ordinary reasons.

Negative control: `--negative-control` runs both arms with farbling off, and all **7/7**
realms then report VOID (the top-frame reference stops differing, so no realm can be
judged) — the rig is shown to report the feature's absence.

```
references   dom: farbled=e865bafb  native=2fad2e1a
R6  popup on a real URL     KEYED      e865bafb == top frame's farbled value
R8  nested worker           UNKEYED    2fad2e1a == native
R13 sandboxed iframe        KEYED      e865bafb
R14a document.write         KEYED      e865bafb
R14b javascript: URL        KEYED      e865bafb
R15 bfcache restore         KEYED      e865bafb
```

⛔ **R15 needed a control or it would have been trivially green.** "Navigate away, come
back, still farbled" is *also* satisfied by an ordinary reload, which takes the plain
main-frame path and proves nothing about bfcache. The harness stamps
`window.__hodos_bfcache_marker` before leaving and requires it to **survive** the back
navigation — the same `LocalDOMWindow` being restored is what makes it a bfcache test at
all. If the marker is lost the run reports "bfcache did NOT engage", which is a failure to
*test*, not a failure of farbling.

⛔ **Two instruments produced confident wrong answers and were caught by their own
controls.** Both are worth naming because both looked fine:

1. **`addModule()` rejection (PaintWorklet).** The probe asked each capability question as
   "throw unless the API exists" and read the promise's fate. It reported
   `OffscreenCanvas`, `navigator` **and `document`** all present in a
   `PaintWorkletGlobalScope` — and `document` cannot be true there, which is the tell. A
   forced-failure arm (a module body that *always* throws) confirmed it: **`addModule()`
   resolved anyway**, so the observable had no negative direction and every answer it gave
   was unfalsifiable. Discarded entirely.
2. **The replacement had to carry its own control too.** `PaintWorkletGlobalScope` has no
   `port`, no `postMessage` and no `fetch`, so the capability answer is captured over
   CDP's console channel. But "no line arrived from the worklet" is indistinguishable from
   "console is not forwarded out of that realm", so the page logs the same shape first.
   With that control green, the worklet's own line reads `OffscreenCanvas=false,
   navigator=false, document=false`.

⇒ **R11 is not a gap in either half.** Neither worklet scope can read a §B surface at all,
so there is nothing there to key. That is a stronger result than "unkeyed", and it is
measured rather than assumed.

### A.7 — P4f: R7 and R8 CLOSED, measured red → green on one box

Built at fork `9ccef044f` (`CEF 150.0.43-7871.3576+g9ccef04`, Chromium unchanged at
`150.0.7871.187`), 2026-08-14.

⛔ **The Chromium version does NOT identify this engine.** `7dd0357` and `9ccef04` are both
Chromium `150.0.7871.187`, so `engine_version()` — which every harness uses as its subject
assertion — proves only that two *arms* are the same build. It cannot prove *which* build,
and a run against a stale binary would report the pre-fix engine string and look entirely
correct. The chain that does identify it, verified by artifact:

- `cef-binaries/include/cef_version.h` → `150.0.43-7871.3576+g9ccef04`
- `cef-native/build/bin/Release/libcef.dll` **md5-identical** to the distribution's
  (`8c761ddc87d3461dabb7e03087975d8f`) ⇒ the app provably loads the P4f engine.

A `cef_version()` helper now exists in `farbling_seed_rotation_check.py` for this reason.

```
R7  farbled arm   main   canvas=e865bafb cores=10     worker canvas=e865bafb cores=10  ✅
    control arm   main   canvas=2fad2e1a cores=24     worker canvas=2fad2e1a cores=24
    -> farbling_worker_probe.py --auto: exit 1 BEFORE, exit 0 AFTER. Same box, same command.

R8  nested worker      canvas KEYED  webgl KEYED  convertToBlob KEYED
T3  worker in a subframe canvas KEYED  webgl KEYED  convertToBlob KEYED
```

⭐ **T3 is the row that would have caught the wrong keying model.** A worker created *inside*
a subframe takes its key from the **iframe's** window, not the top frame's. It must still come
out equal to the **top frame's** farbled value. A same-origin subframe keyed on its own origin
would look correct on every other test in the suite — the two origins are identical — and is
caught here only because the assertion is `== the top frame's farbled value` rather than
`!= native`.

**T11 — nothing else moved.** All seven pre-existing main-frame values are byte-identical to
the baseline recorded before the build (`PLAN_P4f…` §5.1), with `convertToBlob` the only
change. That matters because C3 was edited: `HodosFarbleSnapshot` moved out of an anonymous
namespace so `convertToBlob` could share one definition. A pure code move that alters a single
hash is not a code move — it is a regression on the vector users are already enrolled on.

**T12 — worker-start perf: no detectable regression, and the honest caveat.** Marginal cost
`-407.5 µs` against the pre-build baseline. ⚠️ But the first budget was **50 µs, inherited
from the iframe vector, and it is ~6× below this instrument's noise floor**: eight runs of the
identical command spanned `-236.7 … +1289.8 µs` (sd ≈ 520 µs). Inflating the budget until it
passed would have been a treadmill — every fresh sample widened the estimate. The fix was to
the *instrument*: the marginal metric now runs N=10→N=50 instead of N=1→N=50, so both ends are
multi-worker and per-worker fixed costs cancel. Post-fix runs straddle zero
(`+155, −325, −310 µs`). **Read the gate for what it is:** at this resolution it excludes a
gross regression only. P4f's worker-side addition is a 32-byte memcpy, six orders of magnitude
below the floor, so PASS here never means "measured to cost nothing".

**Other gates on the same build:** rotation **PASS** (A→B→A round trip
`0e4e6251/16ac1f08/0e4e6251`) with its negative control RED on 7 assertions; battery **7/7**;
auth exemption **PASS** 5/6 attempted with the non-exempt control differing; both matrix
harnesses' negative controls green (8/8 realms VOID; positive control NATIVE).

### A.6 — ⚠️ R12 stays ❓, and the container is REAL — owner gate

`HTMLFencedFrameElement`, `FencedFrameConfig`, `navigator.runAdAuction`,
`window.sharedStorage` and `sharedStorage.selectURL` are **all present in this build**. So
this is a live container, not an absent one, and per §G.3 it may not be called done,
deferred, or shipped around on my say-so.

Two things bound how much it matters:

- A fenced frame is **not scriptable by its embedder** — that is its entire purpose. So it
  is **not a page bypass** in the way R7 is. It is a tracker-visibility question, in the
  same class as R4.
- The P4e design already intends to cover it: D1 resolves `GetOutermostMainFrame()`
  precisely *because* `GetMainFrame()` stops at inner pages and would key a fenced frame's
  children on the fenced root rather than on what the user sees in the omnibox.

Measuring it needs a config minted through Shared Storage / Protected Audience, which a
plain local page cannot produce. **Owner decision required:** stand up that fixture, or
accept R12 as a documented gap in the manner of R3 (§F).

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
| **`OffscreenCanvas.convertToBlob`** | `OffscreenCanvas::convertToBlob` (P4f) | ✅ **CLOSED by P4f** | **MEASURED** | `92a26986` native → `a1fb54de` farbled — i.e. now **equal to `toBlob`'s farbled value**, which is the right answer: same image, same perturbation, same encoder. Green in the **main frame and in a worker**. |
| WebGL `readPixels` (WebGL1 + 2) | `WebGLRenderingContextBase::ReadPixelsHelper` | ✅ | MEASURED | — |
| WebAudio `AudioBuffer.getChannelData` | `AudioBuffer::getChannelData` | ✅ | MEASURED | bindings overload only, deliberately |
| WebAudio `AnalyserNode.getFloatFrequencyData` | `AnalyserNode::getFloatFrequencyData` | ✅ | MEASURED | — |
| **`AnalyserNode.getByteFrequencyData`** | `AnalyserNode` + `PerturbBytes` (P4f) | ✅ **CLOSED by P4f** | **MEASURED** | `b680346b` → `d266f578`; **39/1024 elements moved (3.8%), deltas ∈ {−1,+1}** — see B.3 |
| **`AnalyserNode.getFloatTimeDomainData`** | `AnalyserNode` + `PerturbAudioSamples` (P4f) | ✅ **CLOSED by P4f** | **MEASURED** | `f522dfc7` → `919d4979` (float path — the multiplier is correct *here*) |
| **`AnalyserNode.getByteTimeDomainData`** | `AnalyserNode` + `PerturbBytes` (P4f) | ✅ **CLOSED by P4f** | **MEASURED** | `57085a52` → `000947b2`; **60/2048 moved (2.9%), deltas ∈ {−1,+1}** — see B.3 |
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

### B.3 — ⛔ the byte analyser paths needed a DIFFERENT mechanism, and the obvious one is broken

`getByteFrequencyData` and `getByteTimeDomainData` return values already quantised to
`[0, 255]`. The audio multiplier `x * (1 ± δ)`, `δ ∈ [2⁻²³, 2e-7]`, on an integer that small:

```
b * (1 + δ)  ->  NEVER moves the byte   (b would have to exceed 5,000,000)
b * (1 - δ)  ->  ALWAYS drops it by 1   (the store truncates toward zero)
```

and the sign is **one bit, fixed per profile+domain for the whole session**. So hooking these
the way `getFloatFrequencyData` is hooked — the obvious fix, and the one that works for every
float vector — produces a coin flip between **bit-identical to native** (~50% of
profile+domain pairs, no protection at all) and **every non-zero byte minus exactly one**
(~50%, a uniform structure-preserving shift a fingerprinter inverts with a subtraction).

⚠️ **That is the C5 float32 defect one type-width along** — the one that shipped in every
release the feature ever appeared in — and it would have been reintroduced *by copying C5's
own fix*. They therefore use `HodosSessionCache::PerturbBytes`: a keyed low-bit flip on ~3% of
entries, the same construction `PerturbPixels` uses on the other quantised surface we farble,
with a separate `Stream` each. `getFloatTimeDomainData` is float32 and keeps the multiplier.

⛔ **And the acceptance test needed an extra assertion, or the defect would have passed it.**
The uniform −1 shift *moves the hash*, so "farbled ≠ native" is satisfied by the broken
version. `farbling_vector_matrix.py` compares the two arrays **element-wise** and rejects a
constant offset. Measured on the fix: `39/1024` and `60/2048` elements moved, deltas ∈ {−1,+1}
— the low-bit-flip signature, not a shift.

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
| Any realm ⛔ or ❓ that is **page-scriptable**, **or any §B vector unhooked in a ✅ realm** | ❌ **No fingerprinting claim of any kind.** The protection is bypassable by the page it protects. |
| All page-scriptable realms ✅ **and** every §B vector hooked in them; shared/service workers ⏸️ | "Fingerprint protection on pages and their frames. Background service workers are not yet covered." |
| Every realm ✅ | "Fingerprint protection across all execution contexts." |

⛔ **Row 1's condition was amended 2026-08-14, because as originally written the ladder
could not see the gap we actually found.** It spoke only of *realms*. But
`OffscreenCanvas.convertToBlob` is unhooked **and reachable on the main thread** — i.e.
inside R1, a realm marked ✅. So a page needs no exotic container at all: three lines on
the top-level document read out an unfarbled encoding of the same canvas that
`getImageData` farbles. A realm-only ladder would have rated that as row 2. **A ✅ realm is
only as covered as its least-covered vector.**

**As of 2026-08-14 (P4f, fork `g9ccef04`) we are on ROW 2.** All three reasons that held us
on row 1 are measured closed:

1. ~~R7 dedicated workers~~ — ✅ keyed (§A.7)
2. ~~R8 nested workers~~ — ✅ keyed, plus T3 (worker inside a subframe) ✅
3. ~~`OffscreenCanvas.convertToBlob`~~ — ✅ farbled, main frame **and** worker

and every §B vector is now measured FARBLED, so the amended row-1 clause is satisfied too.

⇒ **beta.2 MAY claim:** *"Fingerprint protection on pages and their frames. Background
service workers are not yet covered."*

⚠️ **Row 3 is not available and will not be soon.** R9/R10 are owner-signed ⏸️ (§F) and R12 is
still ❓. Wording draft: `RELEASE_NOTE_farbling_draft.md`, **owner approval required**.

### D.1 — the three residuals any release note must state

1. **Widgets on non-exempt sites** — captcha/payment iframes are now farbled where they were
   native. *Not established as harmful*; the incoherent farbled-parent/native-child combination
   that shipped before P4e is the configuration these scorers actually reject.
2. **D5 exemption inheritance** — on all 37 `IsAuthDomain` hostnames, **every** embedded third
   party reads native values. ⚠️ **Unchanged from beta.1** — pre-P4e those frames were native
   too (unkeyed). Do not describe it as new; that would be inaccurate in the direction that
   costs the most trust.
3. **Workers** — dedicated (R7) and nested (R8), both ⛔ **measured**, both page-scriptable.
   This is the largest residual and was **absent from the release-note ask entirely** until
   2026-08-14. Shared (R9) and service (R10) workers are separate and unsigned.
4. ⚠️ **`OffscreenCanvas.convertToBlob`, in the main frame** — added 2026-08-14. Not a
   worker-only issue and not a container issue: it is an unfarbled canvas readback on the
   top-level document itself. **If the note claims canvas protection while this is open,
   the claim is false on the most-cited vector in the feature.**

⇒ the release-note ask is **four** residuals, not three. Each of 3 and 4 alone is
sufficient to hold the claim at row 1.

---

## E. Ordered work list

| # | Item | Blocks a claim? | Status |
|---|---|---|---|
| E1 | Re-measure **R7** on the current engine → RED baseline | yes — no baseline, no meaningful green | ✅ **DONE** 2026-08-14 (§A.4) |
| E2 | Implement **R7** dedicated-worker key inheritance (+ R8) | **yes** | ✅ **DONE** — fork `9ccef04` (§A.7) |
| E3 | Hook `OffscreenCanvas.convertToBlob` | **yes** | ✅ **DONE** — green in main frame **and** worker |
| E4 | Verify `getByte*` audio paths; hook if unfarbled | yes | ✅ **DONE** — all three hooked, two mechanisms (§B.3) |
| E5 | Measure R6, R8, R11–R15 → move each out of ❓ | yes for any page-scriptable one | ✅ **DONE** except **R12** (§A.6, owner gate) |
| E6 | Owner decision on R9/R10 (defer vs implement) | no — ⏸️ is a valid end state | ✅ **SIGNED 2026-08-14** — both deferred (§F) |
| E7 | Owner decision on WebGL renderer/vendor strings | no | **owner** |
| E8 | Release-note wording, **four** §D.1 residuals | yes | ✅ **drafted** — `RELEASE_NOTE_farbling_draft.md`, **owner approval owed** |
| E9 | T6 regression basket + soak, both platforms | yes | Windows gates green (§A.7); **macOS owed** |
| **E10** | **Owner decision on R12** — build a Shared-Storage fixture, or sign it as a documented gap | no, if signed | **owner** (new) |

**E2 + E3 + E4 are one Blink patch and one build cycle.** Batch them; the build dominates.
E4 is **three** hooks, not one — `getByteFrequencyData`, `getFloatTimeDomainData` and
`getByteTimeDomainData` each go straight to `AnalyserHandler` and each must be shown to
move.

---

## F. Deferrals — owner-signed only

A deferral is valid only with a signature and a date. An unsigned row is ⛔, not ⏸️.

| Realm / vector | Deferred? | Owner | Date | Rationale |
|---|---|---|---|---|
| R9 shared workers | **⏸️ DEFERRED** | owner | **2026-08-14** | Keying is genuinely ambiguous — many documents under *different* top frames may connect to one worker, and "which top frame's key" has no correct answer. Needs cross-process machinery, unlike dedicated workers. **Reachable same-origin only**, so narrower than R7 was. Revisit when a keying model exists, not before. |
| R10 service workers | **⏸️ DEFERRED** | owner | **2026-08-14** | No top frame at all, and outlives every document. ⛔ The natural fix — keying on the SW's own origin — is *wrong*: a third-party iframe can register a SW for its own origin and would then be handed a **stable cross-site** fingerprint, which is exactly the model rejected for iframes (§A.3). **Reachable same-origin only.** |
| R3 same-site cross-origin iframe | **accepted as documented gap** | both sessions | 2026-08-13 | unmeasurable without a two-host server; R4 covers the model |
| **R12 fenced frame** | pending | — | — | container **exists** in this build (§A.6); needs a Shared-Storage / Protected-Audience fixture to mint a config. Not embedder-scriptable, so not a page bypass |
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
