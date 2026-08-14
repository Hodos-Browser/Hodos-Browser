# PLAN — P4f: dedicated-worker key inheritance + the four unhooked vectors

> **Status:** drafted 2026-08-14 after Phase 0 closed. Scope is **E2 + E3 + E4** of
> `FARBLING_DEFINITION_OF_DONE.md` §E — one Blink patch set, one build cycle.
> Everything here is downstream of measurements taken on `Chrome/150.0.7871.187`
> (fork `7dd0357`); nothing in it rests on a code read alone.

---

## 0. What Phase 0 established, in one table

| Item | State | Evidence |
|---|---|---|
| R7 dedicated worker | ⛔ unkeyed | `farbling_worker_probe.py --auto` |
| R8 nested worker | ⛔ unkeyed | `farbling_realm_matrix.py` |
| `OffscreenCanvas.convertToBlob` | ⛔ unhooked, **main frame reachable** | `farbling_vector_matrix.py` |
| `AnalyserNode.getByteFrequencyData` | ⛔ unhooked | same |
| `AnalyserNode.getFloatTimeDomainData` | ⛔ unhooked | same |
| `AnalyserNode.getByteTimeDomainData` | ⛔ unhooked | same |
| R6 / R11a / R11b / R13 / R14a / R14b / R15 | ✅ | `farbling_realm_matrix.py` |

---

## 1. ⛔ THE FINDING THAT CHANGED THIS DESIGN — the byte analyser paths cannot use the audio multiplier

The obvious implementation of E4 is "call `PerturbAudioSamples` on the three arrays the
way `getFloatFrequencyData` already does." **For two of the three that is not a weak fix,
it is a broken one**, and it would have compiled, reviewed cleanly and shipped.

`getByteFrequencyData` and `getByteTimeDomainData` hand back `Uint8Array` — values already
quantised to integers in `[0, 255]` (`realtime_analyser.cc`, `static_cast<unsigned char>
(ClampTo(scaled_value, 0, UCHAR_MAX))`). The existing perturbation is a multiplier
`x * (1 ± δ)` with `δ ∈ [2⁻²³, 2e-7]`. Applied to a small integer:

```
b * (1 + δ)  ->  never changes the byte  (b would need to exceed 5,000,000)
b * (1 - δ)  ->  ALWAYS drops it by exactly 1 (truncation toward zero)
```

The sign is **one bit, fixed per profile+domain for the whole run**. So the outcome is not
"a small perturbation". It is a coin flip between two useless states:

- **~50% of profile+domain pairs: bit-identical to native.** No protection at all.
- **~50%: every non-zero byte reduced by exactly 1.** A uniform, structure-preserving,
  trivially invertible shift — the spectrum's entire shape survives, so a fingerprinter
  subtracts a constant and recovers the native value.

⚠️ **This is the C5 float32 defect repeating in a new domain.** That one — `x * (1+δ)`
rounding straight back to `x` for float32 — shipped in *every release the feature ever
appeared in* and was found only by comparing farbled output against native. The same class
of bug, one type-width away, was about to be reintroduced by copying the working fix.

⇒ **E4 is two mechanisms, not one, and it must not be treated as one item:**

| Path | Domain | Mechanism |
|---|---|---|
| `getFloatTimeDomainData` | float32, nominally [-1, 1] | existing `PerturbAudioSamples` — same domain as `getChannelData`, works |
| `getByteFrequencyData` | uint8 [0, 255] | **new** `PerturbBytes` — low-bit flip on ~3% of entries, its own stream |
| `getByteTimeDomainData` | uint8 [0, 255] | **new** `PerturbBytes`, a *different* stream |

`PerturbBytes` mirrors `PerturbPixels`, which is the proven pattern for quantised data:
deterministic, guaranteed to move a value it selects, and not invertible by a constant.
Distinct `Stream` ids per endpoint, for the reason the header already gives — a shared
stream lets a site correlate two surfaces and recover the seed.

---

## 2. E2 — dedicated-worker key inheritance

### The install point

`HodosSessionCache` is already a `Supplement<ExecutionContext>`, and its header already
names worker global scopes as an intended attachment point. Nothing about the cache needs
to change; only delivery does.

`DedicatedWorker::CreateGlobalScopeCreationParams` (`core/workers/dedicated_worker.cc`)
runs **on the parent thread** and already branches on the parent context type:

```cpp
if (auto* window = DynamicTo<LocalDOMWindow>(execution_context)) {
  // main thread creates a DedicatedWorker
} else {
  // a DedicatedWorker creates another DedicatedWorker (nested worker)
}
```

⇒ read `HodosSessionCache::From(*execution_context)` there, carry `{key32, enabled,
has_key}` in `GlobalScopeCreationParams`, and install in `DedicatedWorkerGlobalScope`'s
constructor.

### What that covers, and why each falls out rather than needing its own code

| Case | Covered because |
|---|---|
| R7 window → worker | parent is the keyed `LocalDOMWindow` |
| **R8 worker → worker** | the `else` branch above; the parent worker already holds the key, so it propagates transitively at any depth |
| **iframe → worker** | post-P4e the iframe's own window holds the **top frame's** key, so its workers inherit the top frame's key with no special case |
| exempt top frame → worker | `enabled=false` propagates, so the worker is exempt too — D5 inheritance stays coherent |
| internal UI → worker | the window has **no** key, so `has_key=false` propagates and the worker fails closed to native |

### ⛔ Fail-closed obligations (each is a way this goes silently wrong)

1. **Propagate `has_key` explicitly. Never synthesise a key.** A zero-filled key with
   `has_key=true` is the shipped constant-seed catastrophe reintroduced — every user
   farbled identically, which is a *worse* fingerprint than none.
2. **Propagate `enabled` separately from the key.** Collapsing them loses the exempt case,
   and the auth-domain exemption depends on a true native pass-through.
3. **A missing/short/garbled key installs nothing**, matching `MaybeApplyHodosFarblingKey`'s
   existing rule that a partially decoded key is a *different* fingerprint, not a weaker one.

---

## 3. E3 — `OffscreenCanvas.convertToBlob`

`core/offscreencanvas/offscreen_canvas.cc :: OffscreenCanvas::convertToBlob` is untouched by
any patch. It is the same shape as `HTMLCanvasElement::toBlob`, which **is** hooked — and
Phase 0 measured both producing the *same* native bytes (`92a26986`), with only `toBlob`
perturbed. Hooking here covers window and worker with one change, since `OffscreenCanvas`
is shared.

⚠️ **This deliberately changes main-frame behaviour**, which P4e's rule forbade. It is
correct here: the current state is the *incoherent* one — two encoders of the same canvas
disagreeing — and that incoherence is itself a signal. Call it out in the release note.

---

## 4. Phase-1 adversarial review — findings

> *What container is adjacent to the one I am fixing? If this fix works perfectly, what
> still reads native? Does it change any currently-green behaviour? What is bundled that
> should be split?*

| # | Finding | Disposition |
|---|---|---|
| **A1** | ⛔ The byte analyser paths cannot use the audio multiplier — §1. Would have shipped as a no-op for half of users and an invertible constant shift for the rest. | **design changed**; E4 split into two mechanisms |
| **A2** | E4 is three endpoints and two mechanisms, i.e. exactly the "never defer/treat a group as one" rule. Each must be shown to move **independently**. | three separate rows in the acceptance table |
| **A3** | If this works perfectly, what still reads native: **R9, R10** (unsigned), **R12** (unmeasured), **R3** (signed gap), and all of §B.1. | no change; these are the standing gates |
| **A4** | `convertToBlob` changes main-frame behaviour — the one thing P4e was forbidden to do. | accepted deliberately, §3; must appear in the release note |
| **A5** | Adjacent container check: **worklets are not adjacent risk** — measured to have no §B surface at all, so there is nothing to deliver a key to. This is why R11 is not in this patch. | closed by measurement, not assumption |
| **A6** | `GlobalScopeCreationParams` is also constructed by shared workers, service workers and worklets. Those paths pass **no** key, so they keep today's behaviour and fail closed. The change must not perturb them. | build-time check: those call sites compile unchanged |
| **A7** | WebGL-in-worker (`OffscreenCanvas.getContext('webgl')` → `readPixels`) is already hooked on `ExecutionContext`, so it turns green the moment E2 lands — **it is not separate work**, but it is also not proven until measured. | added to the acceptance table as its own row |
| **A8** | This is a **Blink** patch set, unlike P4e's libcef-only iframe half, so it joins the per-Chromium-bump rebase budget. That is the real cost and it should be stated when the work is reported, not discovered at the next bump. | stated |

---

## 5. Acceptance table — every row needs its own negative control

| # | Assertion | Harness |
|---|---|---|
| T1 | R7 dedicated worker == the document's farbled value | `farbling_worker_probe.py --auto` (today: exit 1 → must become exit 0) |
| T2 | R8 nested worker == the top frame's farbled value | `farbling_realm_matrix.py` |
| T3 | worker under an **iframe** inherits the **top frame's** key | new arm in `farbling_realm_matrix.py` |
| T4 | worker under an **exempt** top frame reads **native** (D5 inheritance) | `farbling_d5_residual_check.py` extension |
| T5 | worker of an **internal-UI** window gets no key | code + battery |
| T6 | `convertToBlob` moves, window **and** worker | `farbling_vector_matrix.py` |
| T7 | `getFloatTimeDomainData` moves | `farbling_vector_matrix.py` |
| T8 | `getByteFrequencyData` moves — **and not by a uniform −1** | `farbling_vector_matrix.py` + a shift-detector, see below |
| T9 | `getByteTimeDomainData` moves — same | same |
| T10 | WebGL `readPixels` in a worker moves (A7) | `farbling_realm_matrix.py` |
| T11 | main-frame values are **unchanged** by this patch except `convertToBlob` | rotation gate token comparison against the pre-build run |
| T12 | no perf regression at worker start | `farbling_iframe_perf_check.py` sibling |

⛔ **T8/T9 need an assertion the other rows do not.** "The hash moved" is satisfied by the
broken uniform −1 shift described in §1. So those two rows must additionally assert that
the farbled array is **not** a constant offset of the native array — otherwise the exact
defect this design exists to avoid would pass its own acceptance test.

### 5.1 — T11's baseline, recorded BEFORE the build

T11 says the main frame is unchanged except `convertToBlob`. That is only checkable against
numbers taken beforehand, and afterwards it is too late — so here they are, measured on
`7dd0357` / `Chrome/150.0.7871.187`, profile seed `5f64f039…`, host `example.com`:

| Vector | native | farbled |
|---|---|---|
| `getImageData` | `4b351e23` | `1c051e0e` |
| `toDataURL` | `f97ae288` | `4822baea` |
| `toBlob` | `92a26986` | `a1fb54de` |
| `readPixels` | `00dea785` | `9d838219` |
| `getChannelData` | `dc700148` | `48e9a242` |
| `getFloatFrequencyData` | `fbac0bce` | `773233f8` |
| `hardwareConcurrency` | `24` | `10` |
| `convertToBlob` | `92a26986` | `92a26986` ⛔ **must change** |

⚠️ **Every farbled value in this table must be IDENTICAL after the build**, `convertToBlob`
excepted. The C3 patch was edited (the shared helper moved out of an anonymous namespace),
and a pure code move that alters a single hash is not a code move — it is a regression in
the one vector users are already enrolled on. This table is what turns "I only moved the
function" from an assertion into a check.

⚠️ Note `toBlob` and `convertToBlob` share the native value `92a26986`. That is the point:
they encode the same image, and before P4f only one of them was perturbed.

---

## 6. Owner gates outstanding (none block this patch)

1. **R9 / R10** — implement or sign a §F deferral with a date.
2. **WebGL `UNMASKED_RENDERER` / `VENDOR`** — farble or document as a boundary.
3. **R12 fenced frame** — build a Shared-Storage fixture, or sign it as a documented gap.
4. **Release-note wording** — now **four** residuals (§D.1).
