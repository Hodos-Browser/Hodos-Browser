# beta.3 sprint — scope and candidate list

**Opened:** 2026-08-17, immediately after `v0.4.0-beta.2` was built, gate-rehearsed, and
**deliberately not promoted**.
**Status:** 🚧 SCOPING. Nothing below is committed until the owner picks the cut line.

> **Naming.** This folder is the sprint that produces **`v0.4.0-beta.3`**. It sits beside
> `development-docs/0.4.0/`, which remains the home of the 0.4.0 engine/farbling work itself —
> beta.3 continues that line rather than starting a new one.

---

## Where beta.2 left things

| | |
|---|---|
| `v0.4.0-beta.2` | built, signed, notarized, **draft**, verified — **not promoted, by choice** |
| Engine | CEF `150.0.43-7871.3576+g9ccef04` (P4f), asserted out of both CI artifacts |
| Farbling claim | §D ladder **row 2** — pages and their frames; workers not covered |
| AV seeding | done — VT report + Defender `915366c4-f34a-4f78-a36e-b31863146afd` |
| Farbling rotation gate | green + negative control, all four vectors |
| `promote.yml` dry run | `32050154040` — **all gates passed**, every irreversible step skipped |

beta.2 is therefore a *promotable* build being held deliberately. If beta.3 supersedes it, beta.2
never needs promoting; if beta.3 slips, beta.2 is ready. **Decide which of those we're doing** — it
changes whether the beta.2 draft is kept or deleted during cleanup.

---

## Candidate work

Nothing here is ordered by priority yet. The two 🎫 items have full tickets in this folder.

### A. Release-gate integrity — found during the beta.2 run

- 🎫 **`TICKET_farbling_gate_engine_binding.md`** — the farbling gate binds to *"Chromium major
  ≥150"*, not to our engine. A **P4e** token passes a **P4f** gate. The harness defines the correct
  instrument (`require_engine()`) and never calls it, and its own docstring claims it prints
  `CEF_VERSION` when it prints `Chrome/…`. Fix is backward-compatible with the current gate parse,
  so harness and gate can ship in either order. **Cheap, and it is the gate we lean on hardest.**
- **The rotation gate measures a dev build, not the promoted installer.** Same tree, same staged
  CEF, different bytes, and the gate cannot tell. At minimum state the gap in the gate's own comment
  and record the subject binary's identity in the token. (Detailed in the ticket above.)
- **Widen the rotation token beyond canvas.** The harness measures canvas + WebGL + audio +
  navigator; the token still carries canvas only, so the gate re-derives one vector out of four.
  The harness's docstring already flags this as a deliberate, explicit follow-up.
- **`release.yml`'s draft-lookup retry is unproven in CI.** Landed after the beta.2 failure
  (`bc700e9`) and unit-tested locally with negative controls, but it has never executed in a real
  tag build. It runs for free on beta.3's tag — just make sure someone *looks*.

### B. Security items that already have approved designs and no implementation

- 🎫 **`TICKET_cdp_port_open_in_release.md`** — `DEVTOOLS_SECURITY_DESIGN.md` decision **D2**
  ("close the remote debugging port in release") was **owner-approved 2026-08-04 and never built**.
  Release builds still bind CDP on `9222`, which can drive the wallet and auth overlays. Loopback
  only, so not an incident — but it is an unauthenticated local control channel in shipped builds,
  and the design work is already done.
- **The other three D-decisions in that same doc** — drop `--remote-allow-origins=*`, keep DevTools
  available, role-only Inspect gate. They were approved as a set; implementing D2 alone leaves the
  set half-done.
- **Open items from the docs audit** — no-PIN mnemonic reveal, `IsInternalOrigin("") == true`,
  overlay remote-navigation. Carried since the 2026-08-03 truth pass, still open.

### C. Farbling residuals — the §H backlog

Live list is `FARBLING_DEFINITION_OF_DONE.md` **§H**. The four that gate the release-note wording:

| # | Residual | State |
|---|---|---|
| 1 | Widgets farbled on non-exempt sites | MEASURED (P4e); behaviour change, not a defect |
| 2 | Exemption inheritance (D5) — 37 `IsAuthDomain` hosts leak native to embedded third parties | MEASURED; **the lever is shrinking the allowlist, an owner call** |
| 3 | Shared + service workers unfarbled | shared **MEASURED**; service **unmeasured and not measurable locally** |
| 4 | Fenced frames | UNKNOWN — R12 |

- **R10 service workers cannot be measured locally, ever** — `MaybeApplyHodosFarblingKey` skips
  `localhost` for main frames, so any local fixture yields a VOID comparison. Needs a real
  origin-hosted fixture, or it stays permanently unmeasured. **Decide which.**
- **R12 fenced frames** — measure it or sign it as a documented gap with a date. `❓` is not `✅`.
- **The exemption gate proves 5 of 37 allowlist hosts.** Not a regression, but "exemption PASS" has
  never meant the allowlist is verified. Either widen coverage or stop implying it.
- **E7 — WebGL `UNMASKED_RENDERER` / `VENDOR`** are deliberately unfarbled and trivially readable.
  Named in the release note's "deliberately not randomised" list; if E7 goes the other way, the note
  changes with it.

### D. Cross-platform

- **macOS is on the same engine but has fewer measured cells than Windows.** Keep the matrix honest
  about which platform each ✅ came from.
- **Per-profile CDP ports differ on macOS** — anything touching the port scheme must confirm there
  rather than extrapolate.
- **A macOS pre-P4e iframe baseline can never be created** — that engine is gone. Permanent gap,
  already recorded; don't let someone re-open it as a task.

### E. Housekeeping carried in

- `cef-binaries-backup-g7dd0357/` and `cef-binaries-backup-gc636546/` are sitting untracked in the
  working tree. Decide keep-or-delete deliberately — they are the only local copies of two engines.
- `CEF_VERSION_UPDATE_TRACKER.md` and `cef-native/CLAUDE.md` still describe the **pre-P4f** pins
  (`c636546` / `dfe5a23`) as current. Stale on the most consequential fact in the repo.
- The `MAC_WINDOWS_RELAY.md` is ~6,900 lines. It works, but rounds older than the P4 series are
  archaeology and could move to `0.4.0/archive/`.

---

## What "done" looks like for beta.3

To be filled in once the cut line is chosen. The one structural rule carried forward:

> ⛔ **The claim follows the matrix, never the other way round.** If beta.3 moves a cell, the
> release-note wording moves with it and `RELEASE_NOTE_farbling_draft.md` needs re-approval — the
> 2026-08-17 approval was of *those words against those measurements*, not a standing licence.

## Decisions owed before work starts

1. **Is beta.2 promoted, or superseded by beta.3?** Changes whether its draft is kept.
2. **Scope:** release-gate integrity only (A), or A + B security, or A + B + C farbling residuals?
3. **The `IsAuthDomain` allowlist** — residual 2's only real lever is shrinking it. Whether
   `amazon.com` warrants a fingerprinting exemption is a product call, not an engineering one.
4. **R10 service workers** — build a hosted fixture, or sign the gap?
