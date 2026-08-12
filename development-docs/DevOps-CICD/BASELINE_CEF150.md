# CEF 150 Baseline — the reference the NEXT build is compared against

**Established:** 2026-08-11, Windows. **Owner:** DevOps/CI-CD.
**Why it exists:** the roadmap's stability row asks for a crash rate *versus M136*. That
comparison is **unobtainable** — M136 is not installed on the build host and we ship no
telemetry, so there is nothing to subtract. This is the comparison we *can* make: a recorded
150 result that the next 150 build diffs against.

⚠️ **Per-machine, not universal.** Several values below (canvas/audio hashes, timing ratios,
core counts) are properties of *this host*. Re-establish the baseline on any machine before
diffing on it. What transfers between machines is the **shape** of each row, not its literal.

---

## Reference build

| | |
|---|---|
| Engine | `Chrome/150.0.7871.187` |
| `CEF_VERSION` | `150.0.40-7871.3573+gc636546+chromium-150.0.7871.187` |
| Fork pin | `c63654654` on `Hodos-Browser/cef` (tag `pin-c636546/7871`) |
| App | `origin/0.4.0` |

---

## What IS safe to baseline

macOS column established 2026-08-11 by the Mac session (arm64, M1, 8 logical cores) — a
**different machine**, so the literals differ by design. Compare each platform against **its own**
prior run, never against the other's.

| Signal | Windows (x64, 24 cores) | macOS (M1, 8 cores) | Produced by |
|---|---|---|---|
| Regression basket | **10/10** | **10/10** | `regression_soak.py` |
| Renderer crashes | **0** in 140 loads / **0** in 30 | **0** in 120 loads (12 passes) | `regression_soak.py --log` |
| Crash detectors agreeing | probe + log | probe + log (2 `PROCESS_WAS_KILLED` at operator cleanup, correctly **not** classed as crashes) | same |
| `getImageData` 200×50 farbled | 50.0 → 77.5 µs (**+27.5 µs**, 1.55×) | 22.5 → 69.5 µs (**+47.0 µs**, 3.09×) | `farbling_perf_check.py` |
| `readPixels` 32×32 farbled | 0.8640 → 0.9820 ms (1.14×) | 0.1085 → 0.1070 ms (0.99×) | same |
| Perf null-effect controls (above the size gate) | 1.01× / 1.12× | 0.95× / 0.98× | same |
| Farbling seed-rotation gate | PASS — exempt `53225ec8`, large `0cdc9b48` | PASS 20/0 — exempt `a4f83858`, large `9c12d258` | `farbling_seed_rotation_check.py` |
| Auth exemptions LIVE | **5/5** attempted | **6/6** (`accounts.google.com` loaded there) | `farbling_exemption_check.py` |
| Codec Layer-A | 5/5 `probably` + AV1 | 6/6 `probably`; HEVC `probably`; Dolby Vision `""`; AC-3 control refused | `codec_check.py` |
| Q2 farbling × adblock | T1/T2/T5/T6/T7/T8 PASS | T1/T5/T6/T7/T8 PASS; **T2 FAIL — cosmetic/scriptlet is a `#elif __APPLE__` stub**, not a regression | `q2_farbling_adblock_check.py` |
| Navigator farbled pair | `(32, 10)` vs 24 real cores | `(8, 5)` vs 8 real cores — assert the *range*, never the literal | `farbling_acceptance_battery.py` |
| Cross-site iframe | ⛔ RED (unfarbled) | ⛔ RED (unfarbled) — reproduces exactly | `farbling_iframe_check.py` |

⭐ **Four independent routes agree on "native" per machine** — the auth exemption, the per-site hard
bypass, the global toggle, and the unfarbled cross-site iframe. That convergence is what rules out
the alternative T2 was built to exclude: a path farbling with a *fixed or zeroed* key would also be
constant, but it would not land on the value three other mechanisms independently identify as native.

## ⛔ What must NOT be baselined

| Signal | Why not |
|---|---|
| **Per-site text lengths** | Sites redesign continuously. A nytimes layout change would read as a build regression, and chasing it teaches everyone to ignore the diff. `regression_soak.py --baseline` deliberately does not compare them. |
| **The farbled hash literals** (`farbled=…` in the rotation token) | Seed B is regenerated per run by design — that is the feature. Only the **controls** (`exempt=`, `large=`) are stable, and only on one machine. |
| **`deviceMemory` / `hardwareConcurrency` exact values** | Legal sets are tiny (`deviceMemory` has four values), so an equality baseline flakes. Assert set-membership and `cores ≤ real cores` instead. |
| **Wall-clock durations** | Machine load, not build quality. The perf figures are same-machine, back-to-back, minimum-of-N precisely so they are not this. |
| **Perf RATIOS as a cross-platform gate** | ⛔ Measured 2026-08-11: a `3.0×` ratio budget **passed Windows and failed macOS** for the *same feature*, because the M1's native call is 2.2× faster and a ratio divides by it — **a ratio budget punishes the faster machine for being fast.** The gate is now `--max-delta-us` (absolute µs per call, default 100); the ratio is reported only. Both platforms sit well inside it (+27.5 µs / +47.0 µs). Note both effects are real — the M1's absolute overhead *is* also 1.7× larger — but a ratio conflates "our code got slower" with "this CPU is quicker at baseline" and blames the wrong one. |

---

## How to diff a future build against it

```bash
python regression_soak.py --exe <…> --data-root <…> --dev \
    --log "%APPDATA%\HodosBrowserDev\logs\debug_output.log" \
    --passes 14 --out run_new \
    --baseline development-docs/0.4.0/chromium-rebuild/baseline_150/report.json
```

Prints per-site pass/fail drift and a crash-count comparison. The other rows are separate
harnesses; run them and compare against the table above by hand — they are infrequent enough
that a unified runner would be more machinery than it saves.

## ⚠️ Crash counting has two detectors, and `--log` is not optional

- **Probe** — a dead renderer cannot answer JavaScript. Cannot distinguish "crashed" from
  "slow", and misses a crash that a retry papers over.
- **Log** — `SimpleHandler::OnRenderProcessTerminated` (added 2026-08-11) names the cause and
  survives a successful retry. Reads only bytes appended after the run starts.

Without `--log` the rig falls back to the probe alone **and says so**. Read that as *"no
evidence of crashes"*, never as *"no crashes"*.

**Both detectors were positive-controlled before this baseline was recorded**, because a
detector that always returns zero is indistinguishable from a clean run:
- the crash reader was pointed at the whole log and **found the 2 deliberate `chrome://crash`
  terminations**; pointed at end-of-file it correctly found **0**, so old crashes are not
  recounted as new;
- the crash handler itself was demonstrated to fire (`status=PROCESS_CRASHED`,
  `error_code=-1073741819`, `role=tab_1`) rather than assumed.

## Related

- `TESTING.md` — the canonical testing strategy, incl. the harness-discipline rules these
  baselines depend on.
- `../0.4.0/IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` §7 — the readiness checklist each row maps to.
- `FARBLING_RELEASE_GATE.md` — the seed-rotation gate is a *release* gate, not merely a baseline
  row; it blocks promotion on its own.
