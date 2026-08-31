# The install-test batch — everything deferred because it needs a REAL install

**Opened:** 2026-08-31, by owner decision. **Owner:** Matthew. **Platform:** Windows (macOS rows noted separately).

> **The decision, in the owner's words:** *"Lets just wait on the install tests, I know that is not
> best practices but we can keep track of it and do everything that requires install tests at the end
> to save time."*
>
> ⭐ **This is a deliberate, reasoned trade, not a lapse — and it is only safe because this file
> exists.** Building an installer, installing it, testing, then uninstalling is the most expensive
> loop in the sprint, and several phases each owe one or two rows of it. Batching turns N installs
> into one. ⛔ The failure mode is **silent loss** — a row deferred with no home is a row that never
> runs. Every deferral lands here, with the phase that owes it named.

⛔ **A row in this file is OWED, not waived.** Nothing here may be reported as passed, skipped, or
"covered by unit tests". Per `HARNESS.md` §8, a skipped check is **SKIPPED** and its run is
**INCOMPLETE**.

---

## When this batch runs

**Before the 0.4.0 release candidate**, and after the last phase that adds to it. It is a **release
gate**, not a phase: several rows are the difference between "we think it installs" and "it installs".

⚠️ **It cannot be pushed past the RC.** Two of these rows (`P3-A6`, R-UPDATE) are about *upgrading
users who already have Hodos installed* — the one population that cannot be re-tested after shipping.

---

## The batch

| # | Row | Owed by | What it needs | Why it cannot be faked |
|---|---|---|---|---|
| **I1** | `P3-A4` — production single-profile taskbar reads **"Hodos Browser"**, not "HodosBrowser.exe", and groups with the pinned icon | Phase 3 | A real install, **single profile** | ⛔ `HODOS_DEV=1` takes the `.Dev` identity branch, and the owner's machine has 2 profiles. The broken configuration is *precisely* the one no dev build can enter. ⭐ Cheapest route: a **second Windows user account** — clean `%APPDATA%`, real install identity, no VM |
| **I2** | `P3-A6` — an **existing pinned shortcut** on upgrade | Phase 3 | Pin on the pre-fix build → upgrade over it → observe | 🚨 The identity changes, so the pin **will** stop matching. Decision (Q4, Option A): release-note *"re-pin Hodos once after updating"*. This row confirms that is the only consequence, not a broken launch |
| **I3** | `P3-A7` / **R-UPDATE** — a real **N−1 → N** staged update applies, and rolls back when the staged installer is corrupted | Phase 3 + every boundary | Two signed builds | 🟡 T1-only at **every** boundary so far (6 tests). `scripts/test-apply-forward.ps1` / `test-apply-rollback.ps1` drive our helper but **not** WinSparkle/Sparkle. ⛔ Phase 3 **changed the installer** (`[Icons]` now declare an AUMID), so this is more owed than usual |
| **I4** | Nothing new is created inside `{app}` | Phase 3, and `TICKET_stray_log_in_install_root` | The installed tree, before/after a run | A file inside `{app}` broke the silent-update backup hash once already. Phase 3 touched the installer, so the assertion is re-owed |
| **I5** | `TICKET_stray_log_in_install_root` — its owed **T2/T3** rows | Phase 0 (code complete) | Same install as I4 | ⭐ Folded here rather than into Phase 9: **same rig as I4**, and testing separately means installing twice |
| **I6** | `TICKET_appcast_missing_minimum_system_version` | Phase 9 | An appcast + a build | 🚦 **Promotion blocker.** Not strictly an *install* test, but it is discovered and proven in the same build-and-ship loop |
| **I7** | Uninstall removes the per-profile Start Menu sweep and the WinSparkle keys | Phase 3 | Install → uninstall | The uninstaller enumerates `Hodos Browser - *.lnk`. ⚠️ Nothing creates those any more (owner reversed that design), so this row is now **belt-and-braces for machines that ran an intermediate build** — verify it is harmless, not that it is load-bearing |

### macOS equivalents — relayed, not owned here

`MAC_RELAY_BETA3.md` / `MAC_RELAY_P2_ROUND.md` carry Sparkle 2.9.6 verification + its negative control,
and the Big Sur `minimumSystemVersion` call. ⛔ Do not assume the Windows batch covers them — the
updater, the signing chain and the bundle layout are all different.

---

## Rules for adding to this file

1. **Name the phase that owes the row.** An unattributed row has no one to chase it.
2. **Say why it cannot be tested without an install** — in one line. If that line is hard to write, the
   row probably does not belong here.
3. **Keep the negative control with the row.** Deferring a check must not defer its RED half.
4. ⛔ **Never move a row here to make a phase look finished.** The phase's sign-off cites the row and
   its state stays visibly owed.
