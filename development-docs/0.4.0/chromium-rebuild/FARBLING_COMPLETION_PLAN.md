# Farbling completion plan — finishing P4

> **Written 2026-08-09.** This is the **execution** plan: what is left, in what order, what each unit
> costs, and what proves it. The **design** already exists and is not restated here — read
> `PLAN_farbling_blink.md` §6 (C1–C7 edit points), §7 (the per-value farble-vs-omit table, already
> owner-signed), and §8 (P4a–P4e landing order). Where this doc and that one disagree about
> *sequencing*, this one wins; where they disagree about *what to patch*, that one wins.

---

## 1. Where we actually are

> **STATUS 2026-08-10 — the batch is landed and measured on Windows.** Table refreshed below;
> the plan text that follows is kept as the record of how it was sequenced.

| Unit | What it covers | State |
|---|---|---|
| **C1** Supplement on `ExecutionContext` | the foundation every other C-step hangs off | ✅ landed, compiled both platforms |
| **C2** key delivery | browser→renderer `[Sync]` pull at `OnContextCreated` | ✅ landed, behaviourally proven on Windows |
| **C3** Canvas 2D | `getImageData` / `toDataURL` / `toBlob` | ✅ landed, proven on Windows (seed-rotation gate) |
| **C4** WebGL `readPixels` | one hook in `ReadPixelsHelper` | ✅ landed `743e5f322`, **behaviourally proven** on Windows |
| **C5** WebAudio | `getChannelData`, `getFloatFrequencyData` | ✅ landed `743e5f322`; the ±2e-7 delta was found to be a **no-op below float32 resolution** and floored at `2^-23` in `c63654654` |
| **C6** Navigator | `deviceMemory`, `hardwareConcurrency` | ✅ landed `743e5f322`, **behaviourally proven** on Windows |
| **C7** auth-domain exemption at source | move `IsAuthDomain` + per-site toggle into the `enabled` bit | ✅ **no fork change needed** — `OnBeforeBrowse` already collapses all three inputs into C2's `enabled` bit, which is what Q3 §2.1 specifies. Only the teardown remained |
| **Teardown** | delete the dead JS fragments, retire `FingerprintScript.h` | ✅ done — `FingerprintScript.h` deleted, both seed caches and both IPC names removed, orphan sweep clean, Privacy Shield control preserved |
| **P4e** | OOP shared/service workers + cross-site iframes | ⏸️ **DEFERRED past beta.1 (owner decision 2026-08-09)**, logged as a known gap — and **wider than this plan assumed**: see the note below |

> ### ⚠️ P4e is larger than "OOP workers + cross-site iframes"
>
> §8 and the C3 patch comment both claim P4a closed the window-vs-worker canvas mismatch. The hook is
> in the right place, but the **key never reaches a worker**: the only install site is
> `blink_glue::SetHodosFarblingKey(blink::WebLocalFrame*, …)`, called from
> `CefFrameImpl::MaybeApplyHodosFarblingKey` at `CefFrameImpl::OnContextCreated` — **frame contexts
> only**. A `DedicatedWorkerGlobalScope` is a different `ExecutionContext`, gets a fresh key-less
> Supplement, and fails closed to native.
>
> If that reading holds, **in-process workers are unfarbled too**, so §11's worker row is red for a
> reason unrelated to OOP. Recorded as **reasoned from the code, not yet measured** — the owner
> declined a dedicated worker harness for now, so this stays a logged gap rather than a claim.

~~**Today, WebGL / audio / navigator are farbled by nothing at all.**~~ **Superseded 2026-08-10.**
All four values are now farbled natively on Windows dev, and the JS path that nominally owned them is
deleted. **macOS is off M136** as of 2026-08-09 (CEF 150 at `dfe5a2343` staged, gate green with its
negative control) — but Mac has **not** yet taken the C4/C5/C6 batch, so on macOS canvas is farbled
and WebGL/audio/navigator are not, until it builds `c63654654`.

**Release builds are still M136**, so none of this reaches users yet; that remains gated on the CI
`cef-binaries` asset carrying 150 (`FARBLING_RELEASE_GATE.md` §3).

---

## 2. The one planning fact that drives everything: builds are the cost

Every C-step is a Chromium patch or fork-`libcef` change, so **each one needs a full CEF rebuild** —
~4h50m on Windows, ~5h on the M1. Landing C4, C5, C6 and C7 as four separate builds costs **~20
hours of machine time per platform, 40 total**, and buys nothing: they touch four disjoint sets of
files and have no ordering dependency on each other (all depend only on C1, which is done).

> ### ⭐ Therefore: land C4 + C5 + C6 + C7 as ONE fork commit and ONE build per platform.
> Four builds become one. ~30 hours of machine time saved across both platforms.

**Why batching is safe here, specifically:**

- The four patches are in disjoint files (`webgl_rendering_context_base.cc`, `audio_buffer.cc` +
  `realtime_analyser.cc`, `navigator_device_memory.cc` + `navigator_base.cc`, and libcef browser-side
  for C7). A failure in one does not mask another.
- Each lands as its **own `.patch` file** in `patch.cfg`, so the patcher reports them individually
  and a bad one can be dropped without touching the others.
- Verification is per-symbol in the built binary (see §4), not "did the build go green" — the same
  artifact-level check the Mac session used, which is what makes a batch auditable at all.

**What batching does NOT excuse:** landing them as one *commit* in the shell. Keep the fork commit
batched and the acceptance testing per-value.

---

## 3. Order of work

### Step 0 — Mac establishes a baseline *(in parallel, machine time)*
Rebuild at `dfe5a2343`, then run the seed-rotation gate + negative control
(`FARBLING_RELEASE_GATE.md`). This is the first proof of farbling *behaviour* on macOS.
**Do not fold this into the batch** — see relay A4. Runs unattended while Step 1 is authored.

### Step 1 — Author C4 + C5 + C6 + C7 in the fork *(Windows, no build yet)*
Four patch files, one commit. Edit points are in `PLAN_farbling_blink.md` §6; the value decisions are
already made in §7 and must be followed exactly:

- **C4 WebGL** — patch `readPixels` only. **Do NOT touch `getParameter`** — FB-2 closed as *drop*
  vendor/renderer. Random GPU strings are more identifying than the truth.
- **C5 WebAudio** — per-sample fudge equivalent to today's `*= 1.0 + (rng()-0.5)*4e-7`.
- **C6 Navigator** — `deviceMemory` constrained to `{4,8,16,32}` (desktop-plausible; **not** Brave's
  mobile-inclusive `{0.25…8}`), `hardwareConcurrency` **reduce-only, ≤ real core count**. Never
  inflate: an inflated core count is cross-referenceable against real timing and is a detection
  vector in itself. `navigator.plugins` and `navigator.webdriver` are **done** (BOT-1) — do not
  re-add them.
- **C7 auth exemption** — re-home `FingerprintProtection::IsAuthDomain`'s allowlist **and** the
  per-site `IsSiteEnabled` toggle into C2's `enabled` bit. ⚠️ `IsSiteEnabled`/`SetSiteEnabled` and the
  `fingerprint_get/set_site_enabled` IPC are **shipped user-facing control — re-home, never delete**.
  `hodos-unbreak.txt` and adblock scriptlet exemptions are a different subsystem: do not touch.

### Step 2 — One CEF build, Windows *(~5h, unattended)*
`--force-cef-update --force-build`. Then verify per §4 before believing anything.

### Step 3 — Teardown in the shell *(no rebuild)*
Delete the WebGL / audio / navigator JS fragments, retire `FingerprintScript.h` and the JS-injection
half of `FingerprintProtection.h` (`GetDomainSeed`, `s_domainSeeds`, `s_fpDisabledUrls`, the
`fingerprint_seed` / `fingerprint_site_disabled` IPC).

> **The atomic-swap rule (I-4) is cheaper to obey than to reason about.** It exists to stop the JS
> layer re-perturbing already-native-farbled values. Right now the JS path is fail-closed and injects
> nothing, so the hazard is *currently* dormant — but that is an invariant several layers away from
> this code, and "it's fine because something else is broken" is not a property to build on. Delete
> each fragment in the same change that lands its native replacement.

Keep byte-identical: the adjacent adblock scriptlet block and the `window.chrome` stub.
Then grep-sweep for orphaned symbols (Q2 T8).

### Step 4 — Acceptance, Windows
Per-value gates in §4, then the full §11 acceptance rows, then the standard site basket.

### Step 5 — Mac takes the batch
One build at the batched commit, then the same gates. Mac already has the baseline from Step 0, so a
failure here is attributable to the batch rather than to the platform.

### Step 6 — Decide P4e *(owner call, recommend DEFER)*
P4e covers out-of-process shared/service workers and cross-site iframes. It is a genuinely separate
delivery mechanism (worker-start hook + subframe commit hook), it is where Brave needed multiple
follow-up releases, and it is the largest remaining unit.

**Recommendation: defer past beta.1 and log it as a known gap, not a silent one.** P4a already closed
the CreepJS dedicated-worker column, which is the high-signal case. Shipping the four values above on
window + same-site frames + in-process workers is a large, verifiable improvement; holding beta.1 for
OOP coverage trades a lot of calendar for a narrower vector. **This needs an explicit owner decision**
because §11's cross-site-iframe row cannot go green without it.

---

## 4. What counts as proof

**Do not accept a green build as evidence of anything.** Two independent layers, both required:

**Layer A — the code is in the binary.** Grep the built `libcef` for the symbols each patch
introduces, exactly as the Mac session did for C1/C3 (`blink::HodosSessionCache`,
`HodosFarbleSnapshot`, `PerturbPixels`). A patch that silently failed to apply produces a perfectly
green build.

**Layer B — the behaviour changed, and it changes back when disabled.** Per value:

| Value | Measurement | Negative control |
|---|---|---|
| WebGL | `readPixels` hash, farbled vs auth-exempt page | per-site Privacy Shield off ⇒ hashes converge to native |
| Audio | `OfflineAudioContext` + `DynamicsCompressor` → `getChannelData` hash | same |
| deviceMemory / hardwareConcurrency | value ∈ allowed set; cores **≤** real cores | same |
| all of the above | **seed rotation A→B→A** (`farbling_seed_rotation_check.py`) | `--negative-control` |

~~`farbling_audio_check.py` currently asserts the *fail-closed* state...~~ **RESOLVED 2026-08-10 by
DELETING it, not inverting it.** Two defects made it unusable as a gate: it selected its target as
"first page target that is not 127.0.0.1:5137" (harness defect #3 verbatim — an overlay satisfies
that after its first navigation), and it never exited non-zero, so it could not gate anything. Its
subject — the injected JS surface — no longer exists after the teardown. Audio and WebGL are now
covered by `farbling_seed_rotation_check.py`, which drives a verified tab and has a negative control.

⛔ **Every one of these must be demonstrated to fail with the feature off** (`CLAUDE.md` → Testing
Standards). Three harnesses in this project would have passed with farbling entirely absent, and two
more defects were found in the seed-rotation harness itself *by running it*. Report both halves.

Subject assertions that are non-negotiable, all already encoded in the harnesses: drive a **tab**,
not one of the ~14 overlays (identify chrome once by CDP target id); launch with an explicit
`--profile=<id>` or picker mode disables the CDP port; kill browsers **by path** and verify the kill
matched something.

---

## 5. Risks worth naming up front

| Risk | Why it matters | Mitigation |
|---|---|---|
| A patch silently fails to apply | Green build, zero farbling — the exact failure that has bitten us twice | Layer-A symbol grep, mandatory |
| `--force-cef-update` omitted | Rebuilds the OLD fork copy, green, with none of the new code | It is unconditional in both build scripts; do not "optimise" it out |
| Batch makes a defect hard to isolate | Four patches at once | Disjoint files + per-patch `.patch` + per-value Layer-B gates |
| `hardwareConcurrency` inflated | Implausible vs real timing ⇒ a *new* detection vector | Reduce-only clamp, asserted in the gate |
| Deleting `IsSiteEnabled` during teardown | Removes a **shipped user-facing control** | Re-home into the `enabled` bit; the Privacy Shield toggle must keep working |
| Release builds still M136 | None of this reaches users until the CI `cef-binaries` asset carries 150 | Tracked in `FARBLING_RELEASE_GATE.md` §3 |

---

## 6. Definition of done for P4

- C4/C5/C6/C7 landed, symbols verified in `libcef` on **both** platforms.
- JS fragments deleted, `FingerprintScript.h` retired, no orphaned symbols, Privacy Shield per-site
  toggle still works.
- Seed-rotation gate green **with its negative control** on both platforms.
- Per-value Layer-B gates green, each shown to fail with farbling off.
- ~~`farbling_audio_check.py` updated~~ — **deleted**; its coverage moved into `farbling_seed_rotation_check.py`.
- Standard site basket (Minimal at minimum) green on both platforms.
- P4e explicitly **decided** — shipped or logged as a known gap with owner sign-off.
