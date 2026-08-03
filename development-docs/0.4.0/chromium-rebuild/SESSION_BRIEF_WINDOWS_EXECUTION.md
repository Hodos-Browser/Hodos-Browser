# Session Brief — Chromium/CEF Rebuild, Windows Execution

**Written:** 2026-08-03, at the end of the planning session, for the *next* session.
**Read this first, then `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`.**

This brief carries **what changed since the plan docs were written, what the owner
has decided, and how to run the session**. It does **not** restate the plan — the
roadmap and the eleven `chromium-rebuild/` docs are authoritative and were
deliberately researched. Don't re-derive what they already answer.

---

## 1. Goal

Rebuild our custom Chromium/CEF from source, **Windows first**, bumping
CEF 136 → 150, with farbling moved from injected JS into a Blink source patch.
End state for this sprint line: a reproducible Windows build plus a handoff
brief that lets the macOS session run its own build.

**Authoritative docs** (read, don't summarize back):
- `development-docs/0.4.0/IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` — master, phase-ordered
- `development-docs/0.4.0/CHROMIUM_CEF_SPRINT_KICKOFF.md` + `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md`
- `development-docs/0.4.0/chromium-rebuild/` — 11 plan + Q docs
- `development-docs/DevOps-CICD/` — `CEF_BUILD_RUNBOOK.md`, `CEF_VERSION_UPDATE_TRACKER.md`, `DEPENDENCY_VERIFICATION.md`

---

## 2. Decided — do not re-litigate

### D1 — version target: **RESOLVED 2026-08-03**

The roadmap carried `7871` as the target *with a fallback to M149*, because on
2026-07-10 branch `7871` was still **CEF Beta** and we will not ship a
money-handling browser off a Beta binary. **That gate is now satisfied.**
Verified against `cef-builds.spotifycdn.com/index.json`:

| Branch | Channel | Newest |
|---|---|---|
| **CEF 150 / `7871`** | **stable** | **`150.0.17+g94c1726+chromium-150.0.7871.187`** |
| CEF 151 / `7922` | stable | `151.3.14` |
| CEF 149 / `7827` | stable | `149.0.6` |

- **Target = CEF 150 / branch `7871`, pinned to `150.0.17`** (newest security point-release, not `.0`).
- **The M149 fallback is no longer needed.**
- **Do NOT jump to 151** even though it is newer — **150 is the LTS milestone**
  (M138 → M144 → M150 → M156). 151 is not, and would force an early re-bump.
- **Still re-verify on build day** — a newer `7871` point-release may exist.

### D2 — Amazon / Widevine DRM: **no spend**

Owner: *"It is ok if we can't get Amazon videos working but we should look at how
Brave does it, we don't want to pay anything."*

- **Run Spike-1** (`Q4_widevine_amazon_drm.md` §7 — ~1 hour, $0) to classify the
  failure definitively.
- **Start with step 0**: audit whether *our own build* suppresses the component
  updater (`--disable-component-update`, `*.googleapis.com` blocklist). If it
  does, this is a **missing CDM** with a free fix — the one cheap outcome.
- **Document how Brave does it** (`Q4` §4). Short version to verify, not assume:
  Part A is an on-demand CDM install prompt — replicable, but **cosmetic**.
  Part B is that Brave ships a **VMP-signed** browser under its own Widevine
  license — that is what actually makes Amazon work, and it is not replicable.
- **Do NOT pursue castLabs** (paid). Note its free EVS tier is **Electron-only**
  and cannot sign CEF binaries — a documented trap.
- **Google MLA** is $0 in fees but a **4+ month opaque wait** plus 1–2 weeks of
  pipeline work. **Out of scope for this sprint**; record it as a future option.
- **Expected outcome: document the limitation and move on.** Ship the free
  component-updater path with an honest "Amazon movies unsupported" note.

### Build hosts — verified adequate

This Windows machine, measured 2026-08-03:

| Spec | Required | Actual |
|---|---|---|
| Cores | 8+ | **16 / 24 threads** (i9-12950HX) |
| RAM | 32 GB+ | **31.7 GB — at the floor** |
| Disk | 150 GB (200+ two trees) | **1204 GB free on C:**, NTFS |

**RAM is exactly at the minimum.** Chromium's link step is memory-hungry — close
other applications during builds. Use a short ASCII base path: **`C:\cef\`**.

**Builds run locally, not in CI.** GitHub-hosted runners cannot build Chromium
(6-hour cap, ~14–29 GB free).

---

## 3. Open — surface these, do NOT decide them

| # | Decision | Blocks |
|---|---|---|
| **D3** | macOS arch: universal2 vs arm64 vs x86_64 | the Mac build; also sets the C4 Mac GPU-string set |
| **D4** | WebGL `UNMASKED_VENDOR/RENDERER` — **drop** (recommended) vs Brave-parity GPU-string map | **P4b.** The roadmap calls this "the highest-risk value decision" |
| **D5** | C2 seed delivery channel — (A) mojo/commit-params per-navigation vs (B) ephemeral cmdline nonce | C7's "no new IPC" property holds only under (A) |
| **D7** | Apple individual→org signing sequenced before or after beta.1 | P7 prod build |

Bring these to the owner when the phase that needs them is reached, with a
recommendation. Do not pick them unilaterally.

---

## 4. How to run this session

### Phase 1 — kickoff review (FIRST, before any code)

Per the mandatory workflow in `/CLAUDE.md`. **Target ~30–60 minutes. This is a
divergence check, not a re-plan.** The docs were researched deliberately; assume
they are right and look for drift.

1. Re-read the phase docs for P0–P2.
2. **Verify cited code is current** — for each `file :: symbol` reference, grep
   and confirm it still exists with the documented shape. Fix the doc inline if
   it moved.
3. **Reuse-first audit** — prove no equivalent already exists before adding.
4. **Risk assessment** — especially the load-bearing UX safeguards in
   `/CLAUDE.md` (gold pill, right-click revoke, "Always notify", privacy
   perimeter gates, per-session counters).
5. **Hand back a tight summary** — open questions, assumptions, decisions
   needed. **Then WAIT for owner confirmation before the first commit.**

### The research loop — bounded

If the review surfaces a genuine gap:

- Dispatch **one agent per question**.
- Every question must resolve to exactly one of:
  - **ANSWERED** — with a primary source
  - **CANNOT ANSWER WITHOUT BUILDING** — defer to the phase that produces the evidence
  - **OWNER DECISION** — surface it, don't research it further
- **Cap: 2 rounds.** A question surviving 2 rounds is an owner decision by definition.
- **Check the plan docs first.** They answer most of this. Researching what
  `Q1`–`Q5` already settled is waste.

> Prior-session calibration: the docs truth pass burned ~7M subagent tokens
> partly for lack of a termination rule. Bound it.

### Phase 2 — build, in this order

⚠️ **Ordering correction.** The owner initially framed the Windows work as
"download, codecs, and farbling." **Farbling cannot come early.** It is a Blink
source patch and the patch mechanism does not exist yet — `cef/patch/patch.cfg`
is greenfield, zero patches today. **P3 is the serial linchpin and blocks all of
C1–C7.** Codecs are the opposite: already-on GN flags, so P5 is *verification*,
nearly free.

| Phase | What | Notes |
|---|---|---|
| **P0** | Provision | Largely satisfied (§2). Still verify: depot_tools, VS2022 + Win SDK, branch-matched Python (re-confirm the `.vpython3` ceiling — the M136-era 3.11 is not a carry-forward), Defender exclusions, pause Windows Update, **Siso-vs-Ninja resumability**, and whether M136 still builds |
| **P1** | Pin version/toolchain | Version resolved (§2 D1). Do VER-2/3 toolchain + CI runner pin (**never `*-latest`**), VER-4 minos scoping, D7 sequencing |
| **P2a** | 136 baseline, guarded | If M136 has bit-rotted, smoke the last-known-good environment instead — do not treat an unbuildable M136 as an unmeetable gate |
| **P2b** | Bump to `7871` | VER-1..6, DEP-1a..d **then** DEP-1, FEDCM-1, GN-5..8, **VER-5 drift audit** (14 milestones of drift — expect ≥1 changed resource; this feeds the P6 auto-update gate) |
| **P3** | **Patch toolchain — LINCHPIN** | Greenfield. CEF-1..5: fork → `Hodos-Browser/cef`, `patch.cfg`, **prove a no-op patch applies + builds**, drift-audit script, `HODOS_FARBLING` condition gate, check in `build_hodos_cef.{bat,sh}` |
| **P4** | Farbling C1..C7 + P4e | Incremental. Each sub-step builds, smokes, and **atomically deletes its own JS counterpart in the same commit** |
| **P5** | Codecs + DRM | Parallel with P4, forks off P2b. Includes the **Amazon spike** (§2 D2) |
| **P6 / P7** | Test / prod build | Per roadmap |

**Cold build ≈ 10–12 hours, no sccache benefit cold.** Farbling is ~5 builds, not
one. Plan sessions accordingly — kick long builds off in the background.

> **First milestone worth aiming at: a clean `7871` build with codecs verified,
> before any source patching.** That is P2b + P5, it is a real deliverable, and
> it de-risks everything after it.

### Phase 3 — the macOS handoff

After P2b (and again after P4), write **`development-docs/0.4.0/CHROMIUM_BUILD_RELAY.md`**
and push it so the macOS session can pull and work from it.

**It must NOT be a recipe.** The roadmap is explicit: *"Mac is a parallel build,
not an inherit-and-verify afterthought (I8)."* Structure it as:

- **What was done on Windows** — exact commands, exact pins, what surprised us
- **What Mac inherits** — the shared cross-platform patch set and GN config
- **What Mac OWNS and must decide itself** — the part a recipe would silently skip:
  - **D3** arch: universal2 (two per-arch builds + `lipo`) vs arm64
  - **VER-4 minos** entirely: `vtool`-measure, set `max(12.0, measured)` in all
    three places, wire the CI guard
  - the **framework embed list**
  - **Sparkle / notarization / EdDSA** signing leg
  - **Mac GPU strings for C4** if D4 lands on "map"
  - its own baseline + target builds
- **Open questions Mac must answer**, listed explicitly

### Phase 4 — meta-analysis, running not retrospective

Per `/CLAUDE.md` invariant #12. **Capture as you go** — a lesson reconstructed at
the end is a lesson half-lost.

Land findings in the **existing** DevOps-CICD docs, not a new one (one home per fact):

| Finding type | Home |
|---|---|
| Build steps that surprised us, broke, or needed a workaround | `CEF_BUILD_RUNBOOK.md` |
| The changelog: branch, milestone, `GN_DEFINES`, patch-set version, deps touched/deferred, wall-clock duration, **estimated per-bump patch-rebase hours (I10)** | `CEF_VERSION_UPDATE_TRACKER.md` |
| Dependency drift and re-pins | `DEPENDENCY_VERIFICATION.md` |

The goal is that the *next* Chromium bump is cheaper than this one. Write for
someone who has never done it.

---

## 5. Guardrails

- **Invariants #2 / #3** — no DB schema or crypto/signing/derivation changes
  without asking first.
- **Invariant #13** — on a test failure, determine whether the test or the
  production code is wrong. Test-only fixes may proceed; **if the production
  code looks wrong, STOP and ASK.**
- **Clean-room (M7)** — reimplement Brave's *technique* from spec, behavior, and
  the value tables in our own plan docs. **Do not read Brave's MPL-2.0 source.**
  Bromite (GPL-3) is forbidden. Record the clean-room boundary in each PR.
- **Do not run F4 (parking_lot) concurrently with the farbling patch set** — both
  are L/XL cross-cutting.
- **The gold pill must survive** — never a "green dot".
- **Never hardcode a backend port** — `PortConfig.h`.
- **Wait for owner confirmation after the kickoff review** before the first commit.

---

## 6. Loose ends inherited from the planning session

Not part of this sprint, but open:

- **beta.29 AV seeding is still owed** — it is the current public Latest with no
  VirusTotal or MS Defender submission. See `project-av-seeding-gate-2026-08-03`
  in memory and `BUILD_AND_RELEASE.md` §2.5.5.
- **The AV seeding gate in `promote.yml` is live but UNPROVEN in CI.** Its first
  real exercise is the next draft promote. `av_seeding_waiver` is the unblock if
  it misbehaves.
- **Test scratch in the repo root** — `artifacts/`, `stub/`, `err.txt`,
  `gate_step_*.sh`, `validate_step.sh`. All untracked, safe to delete.
