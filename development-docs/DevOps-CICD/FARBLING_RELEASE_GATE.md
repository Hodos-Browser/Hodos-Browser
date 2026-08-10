# Farbling seed-rotation release gate

> **Owner decision, 2026-08-09:** the gate lives on the **build host**, not in GitHub-hosted
> CI, and `promote.yml` blocks without its result. This doc is the procedure.

---

## 1. What this gate is for

Fingerprint farbling shipped **broken in every release from `v0.3.0-beta.1` to
`v0.3.0-beta.29`**. The renderer never received its per-domain seed and fell back to
`std::hash<std::string>(url)`, so the "randomised" fingerprint was a per-URL **constant** —
identical across launches, across profiles, and across users. That is not weak farbling; it
is a precomputable, browser-identifying tag applied on top of the native values, i.e. worse
than shipping nothing. Full write-up: `development-docs/TICKET_farbling_constant_seed_shipped.md`.

It survived nine months of releases for one reason: **the bug is invisible to any
single-session check, and every check anyone ran was single-session.** The farbled value
differed from the exempt value, was stable across reads, and was stable across navigations.
Everything looked right.

Brave shipped the same class themselves (#49346). So the durable artifact here is not the
fix — it is the cross-session test. Hence a gate.

## 2. What it actually proves

One experiment, three browser restarts, two controls:

| | seed A | seed B | seed A again |
|---|---|---|---|
| auth-exempt page (control) | `53225ec8` | `53225ec8` | `53225ec8` |
| ≥65536px canvas (control) | `0cdc9b48` | `0cdc9b48` | `0cdc9b48` |
| **farbled page** | `0e4e6251` | **`4fa8c859`** | **`0e4e6251`** |

*(Real output, dev build, fork `dfe5a2343`, 2026-08-09.)*

- Both controls hold still ⇒ the farbled delta is attributable to the seed and not to
  render variance between runs or machines.
- Seed B changes the fingerprint ⇒ **per-user unlinkability**. This is the assertion the
  shipped bug fails, and the only one that catches its class.
- Seed A round-trips exactly ⇒ **determinism across restarts** — the login guarantee, and
  the reason Hodos deliberately diverges from Brave's per-session seed.

## 3. Why this is not a GitHub Actions job

It cannot be, today, and the reason is the CEF binary supply chain rather than the runner:

1. `release.yml` gets CEF via `gh release download cef-binaries --repo ${{ github.repository }}`.
   **That release does not exist on `BSVArchie/Hodos-Browser`**, where 0.4.0's CI runs.
2. The org repo's `cef-binaries-windows-150.zip` is dated **2026-08-04** — the `94c1726`
   build, which predates both C2 (the farbling registry) and C3 (native canvas farbling).

Against that asset, farbling is **entirely absent**: the canvas JS was deleted by C3, the
native replacement is not in the binary, and the JS audio/WebGL path correctly fails closed.
A hosted job wired to it would go red against **correct** code — which is precisely the
defect family that has already cost this project days (three harnesses, all of which would
have passed with the feature completely absent; see the header of
`development-docs/0.4.0/chromium-rebuild/farbling_canvas_check.py`).

If a farbling-capable `cef-binaries-windows-150.zip` is ever published to the development
repo, revisit this — but note the standing hazard: **the asset must be re-uploaded on every
fork bump**, or the job silently starts testing the wrong engine.

## 4. Running it

Prerequisites on the build host: the dev stack up (Rust wallet, `npm run dev`, browser),
`pip install websocket-client`, and a build whose CEF carries the C2/C3 patches.

```powershell
python development-docs/0.4.0/chromium-rebuild/farbling_seed_rotation_check.py `
    --exe "<repo>\cef-native\build\bin\Release\HodosBrowser.exe" `
    --profile-dir "$env:APPDATA\HodosBrowserDev\Default" `
    --port 9322 --dev `
    --log "$env:APPDATA\HodosBrowserDev\logs\debug_output.log"
```

Takes ~6 minutes (three kill/relaunch cycles). It restores the original
`fingerprint_settings.json` on every exit path, including failure.

On success it prints one line to paste into `promote.yml`:

```
FARBLING-ROTATION-v1 engine=Chrome/150.0.7871.187 exempt=…/…/… large=…/…/… farbled=…/…/… verdict=PASS
```

### ⛔ The negative control is not optional

`CLAUDE.md` → Testing Standards makes this a hard rule, and this feature is why. Before
trusting a green run, prove the harness can go red:

```powershell
python ...\farbling_seed_rotation_check.py <same args> --negative-control
```

That disables farbling for the farbled domain via the per-site Privacy Shield opt-out (which
gates the native path as well as the JS one) and **inverts the exit code**: it exits 0 only
if the assertions failed. If it stays green with farbling off, the harness is measuring
nothing and must be fixed before any green result from it is believed.

Report both halves: *"passes, and fails when farbling is disabled."*

## 5. The promote gate

`promote.yml` takes two new inputs:

| Input | Meaning |
|---|---|
| `farbling_rotation_token` | The `FARBLING-ROTATION-v1` line from the run above |
| `farbling_gate_waiver` | Reason to promote without it. **Expected for any M136 build**, where farbling is inert by construction and the rotation cannot pass |

⭐ **Unlike the MS Defender attestation, this is verified rather than trusted.** The token
carries the raw hashes and the engine string, and the gate re-derives all four contracts
from them — controls stable, A≠B, A=A′ — plus an engine check that rejects anything below
Chrome 150. The token's own `verdict=` field is **ignored**; a token claiming PASS whose
hashes say otherwise fails.

Verified against every failure mode (2026-08-09): good token passes; a constant-seed token
fails on unlinkability; a non-deterministic token fails on determinism; a moved control
fails as an invalid comparison; an M136 engine is rejected; an empty token is rejected.

Rehearse with `dry_run: true` — it runs every check against the real draft bytes and stops
before the flip.

## 6. When to tighten this

- ✅ **DONE 2026-08-09 — the harness now covers all four vectors** (canvas, WebGL `readPixels`,
  WebAudio, navigator), measured in one page visit so they share the same three restarts.
  `farbling_audio_check.py` is **retired, not extended**: it selected its target as "first
  page target that is not 127.0.0.1:5137" — harness defect #3 verbatim — and it never
  exited non-zero, so it was never a gate.

  > ### The negative control that came free, and what it caught
  >
  > The extended harness was run against the **`dfe5a2343`** build — C1/C2/C3 only, so
  > WebGL/audio/navigator farbling was genuinely absent. That is a true feature-off state,
  > and it can only be measured *before* a C4/C5/C6 build is staged. Result:
  >
  > | Vector | In binary? | Outcome |
  > |---|---|---|
  > | canvas | yes | all 5 assertions **PASS**, values reproducing §2's table exactly |
  > | WebGL | no | both presence assertions **FAIL** (farbled == exempt, A == B) ✅ |
  > | audio | no | both presence assertions **FAIL** ✅ |
  > | navigator | no | **all four assertions PASSED — the gap** ⛔ |
  >
  > `deviceMemory` read 32, which is native on that machine *and* a legal farbled value;
  > `hardwareConcurrency` read 24 == real, which reduce-only permits. So the C6 checks were
  > plausibility checks that could not go red with the feature off — precisely the defect
  > class this document exists to prevent, found in our own new harness by running it.
  >
  > Fixed by adding a **presence check**: the `(deviceMemory, hardwareConcurrency)` pair must
  > differ from the native pair for at least one of the two seeds. False-failure odds are
  > ~1 in 8,500 on a 24-core box (both values colliding with native, for both seeds), stated
  > in the code so it is a judgeable trade-off rather than a surprise.
  >
  > **Lesson worth generalising:** an assertion that a value is *plausible* is not an
  > assertion that the feature *ran*. Every vector needs at least one check that is false
  > when the code is absent.

  > ### ⛔ And then the green run found a second one: audio farbling was a no-op for ~15% of users
  >
  > With C4/C5/C6 in the binary, one assertion stayed red: `audio farbled != exempt`, for
  > seed A only — while `audio seed A != seed B` **passed**, proving C5 was running. The
  > cause is arithmetic, not delivery:
  >
  > Audio samples are float32 and therefore already exactly representable, so
  > `x * (1 + delta)` rounds straight back to `x` unless `|delta * x|` exceeds **half** the
  > gap to the neighbouring float32 — a relative threshold of at most `2^-24`. The spec'd
  > multiplier was uniform `1.0 ± 2e-7`; `|delta| < ~3e-8` moves **0.00%** of samples.
  > Measured: `delta = -4.95e-09` → **0 of 44100** samples changed, on a window with 5000
  > non-zero samples and peak 0.70, so not silence.
  >
  > ≈15% of profile+domain pairs got a complete no-op; ~30% were dead or degraded. **The
  > injected JavaScript had the identical hole**, so this shipped in every release the
  > feature ever appeared in — invisible because every check compared farbled against
  > exempt *within one session*, and when farbling is a no-op both sides are native.
  >
  > That is the same structural blind spot as the constant-seed bug, reached by a different
  > mechanism: **comparing two things that are both wrong in the same way.** The durable
  > defence is comparing against a known-native reference, which is what the
  > `copyFromChannel`-vs-`getChannelData` differential does on a single page and seed.
  >
  > Fixed by flooring `|delta|` at `2^-23` (fork `c63654654`) — one full ULP, 2× margin,
  > simulated at 100% of non-zero samples moving across the whole band, ceiling unchanged
  > so it is never louder than the original spec allowed (~-134 dB). The harness assertion
  > needed **no** change: it was right and the product was wrong.
- ~~**When macOS builds CEF 150.**~~ **DONE 2026-08-09.** Mac is off M136: CEF 150 at fork pin
  `dfe5a2343` is staged into `cef-binaries/`, and this gate **passes on macOS with its negative
  control red** — `engine=Chrome/150.0.7871.187`, farbled `6a0803ed`/`b3551928`/`6a0803ed` against a
  stable exempt `a4f83858`. **Mac promotions no longer use the waiver**; they carry a real token like
  Windows. Running it on Mac has two platform gotchas: export `HODOS_MAC_DEV_FLAGS=1` yourself (the
  harness sets only `HODOS_DEV`, and ad-hoc signed dev builds crash without `--in-process-gpu`), and
  CDP binds **only** for the profile literally named `Default`
  (`cef_browser_shell_mac.mm :: main` → `remote_debugging_port = (profileId=="Default") ? 9222 : 0`,
  `+100` under dev = 9322), so no other profile can be driven.
- **If the fork is rebased**, re-run before promoting. The gate binds to the engine major
  version, not the fork commit, so a rebase within 150 will not be caught automatically.

## 7. Related

- `development-docs/TICKET_farbling_constant_seed_shipped.md` — the shipped bug
- `development-docs/0.4.0/chromium-rebuild/PLAN_farbling_blink.md` — C2/C3 and the migration
- `development-docs/0.4.0/chromium-rebuild/farbling_canvas_check.py` — the three harness
  defects this project actually hit, documented at the top of the file
- `development-docs/DevOps-CICD/CEF_BUILD_RUNBOOK.md` — the general CDP-testing rule
  (identify browser chrome once by target id; never `PUT /json/new`)
