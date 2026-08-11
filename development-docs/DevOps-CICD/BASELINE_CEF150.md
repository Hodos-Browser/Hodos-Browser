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

| Signal | Reference | Produced by |
|---|---|---|
| Regression basket | **10/10** sites render | `regression_soak.py` |
| Renderer crashes | **0** in 140 loads (14 passes) and **0** in 30 loads (3 passes) | `regression_soak.py --log` |
| `getImageData` 200×50 farbled | **1.55×** native (0.0500 → 0.0775 ms) | `farbling_perf_check.py` |
| `readPixels` 32×32 farbled | **1.14×** native (0.8640 → 0.9820 ms) | same |
| Perf null-effect controls (above the size gate) | **1.01× / 1.12×** — must stay ≈1.0 | same |
| Farbling seed-rotation gate | PASS, incl. exempt control `53225ec8` and large control `0cdc9b48` | `farbling_seed_rotation_check.py` |
| Auth exemptions LIVE | **5/5** attempted (32 of 37 entries untestable — asset origins) | `farbling_exemption_check.py` |
| Codec Layer-A | 5/5 GATE rows `probably`, AV1 present | `codec_check.py` |
| Q2 farbling × adblock | T1/T2/T5/T6/T7/T8 PASS | `q2_farbling_adblock_check.py` |
| Navigator farbled pair | `(32, 10)` against 24 real cores — *range*, not the literal | `farbling_acceptance_battery.py` |

## ⛔ What must NOT be baselined

| Signal | Why not |
|---|---|
| **Per-site text lengths** | Sites redesign continuously. A nytimes layout change would read as a build regression, and chasing it teaches everyone to ignore the diff. `regression_soak.py --baseline` deliberately does not compare them. |
| **The farbled hash literals** (`farbled=…` in the rotation token) | Seed B is regenerated per run by design — that is the feature. Only the **controls** (`exempt=`, `large=`) are stable, and only on one machine. |
| **`deviceMemory` / `hardwareConcurrency` exact values** | Legal sets are tiny (`deviceMemory` has four values), so an equality baseline flakes. Assert set-membership and `cores ≤ real cores` instead. |
| **Wall-clock durations** | Machine load, not build quality. The perf gate ratios are same-machine, back-to-back, minimum-of-N precisely so they are not this. |

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
