# Session kickoff prompt — drive farbling to its definition of done

> Paste everything between the rules into a fresh Windows session.

---

Drive the farbling workstream to completion against its definition-of-done document. Work the
list, test every step, and loop until the requirements are met or you hit an owner gate.

## Read first, in this order

1. `development-docs/0.4.0/chromium-rebuild/FARBLING_DEFINITION_OF_DONE.md` — **this is the
   scope and the source of truth.** Nothing outside it is in scope; nothing inside it is
   optional.
2. The top entry of `MEMORY.md`, then `CLAUDE.md` → Guidelines + Testing Standards (especially
   the ⛔ NEGATIVE CONTROL rule).
3. `development-docs/0.4.0/MAC_WINDOWS_RELAY.md` — top 3 rounds only.
4. `development-docs/0.4.0/chromium-rebuild/PLAN_P4e_iframe_farbling.md` §0 (the revision block).

Do the CLAUDE.md phase-kickoff review before writing code: verify every file/symbol the docs
cite still exists at the cited shape in the live fork at `C:\cef\cef150\chromium\src\cef`.
Hand back a short summary of assumptions and open questions before the first commit.

## The goal

Get every **page-scriptable** realm in §A of the definition-of-done to ✅ (measured), so the
release-claim ladder in §D permits a fingerprinting claim. Today we are on its bottom rung:
dedicated workers are open and page-scriptable, so beta.2 may not carry any such claim.

## ⛔ Adversarial review is a GATE at every phase boundary, not a one-off

Before starting each phase, attack the plan for that phase and write down what the attack
found. This is not ceremony — the pre-implementation review is what found the `window.open()`
bypass before it shipped as "closed", and a pre-*measurement* review would have caught the three
farbling harnesses that would each have passed with the feature entirely absent.

**The question to attack is different at each boundary. Use the right one:**

| Before | Attack with |
|---|---|
| **Phase 0** (measuring) | *Would this harness pass if the feature were absent?* Is it measuring the intended **subject** (right process, right browser, right realm)? Does it reach the realm **the way an attacker would**, or by a privileged path an attacker lacks? Is the "pass" observable at all, or unfalsifiable? |
| **Phase 1** (design) | *What container is adjacent to the one I am fixing?* If this fix works perfectly, what still reads native? Does it change any currently-green behaviour? What am I assuming is expensive that is actually cheap — or bundled that should be split? |
| **Phase 2** (build) | *Is every new path fail-closed?* Can any early return leave a constant or partial key? What did I hook, and what calls the same surface **without** going through it? Does main-frame behaviour change at all? |
| **Phase 3** (results) | *Could this be green while broken?* Is any pass trivially satisfied (both sides equal because both are broken)? Does any number sit inside the instrument's own noise? Am I about to change a gate because it failed — and if so, which side is actually wrong? |
| **Phase 4** (the claim) | *Does the evidence support the wording?* Is anything stated as measured that is only a code read? Is any residual described as new when it predates this change? |

Record each review's findings in the relay notes even when it finds nothing — a review that
never changes anything is a review that is not being run properly.

## Workflow — phases, in order. Do not reorder.

**Phase 0 — measure, no build.** Every ❓ realm in §A that can be measured on the current
engine. This costs nothing but time and it is what stops scope arriving late. Convert every ❓
to ✅ or ⛔ with a named harness and a negative control. Includes E1 (the dedicated-worker RED
baseline) and E4's audio verification. **Update §A/§B in the document as you go — the matrix is
the deliverable, not a side effect.**

**Phase 1 — design, once.** Cover everything Phase 0 found, in a single patch: E2 dedicated
workers (+ E8 nested, free), E3 `OffscreenCanvas.convertToBlob`, E4 any unhooked audio path,
plus any new realm Phase 0 surfaced. Adversarially review the design before implementing —
last time that found a second bypass container before it shipped.

**Phase 2 — build, once.** The build dominates the calendar; batch everything into it.

**Phase 3 — test, and loop.** Run the full suite. Any failure → back to Phase 1, and batch all
fixes into the *next* single build. Do not build per fix.

**Phase 4 — hand off.** Relay round for Mac, then stop.

## Hard rules

- **Negative control on every acceptance test.** A test never seen to fail proves nothing. For
  each one, state "passes, and fails when X is disabled."
- **Assert the subject**, not just the value. Hodos's header and ~15 overlays are separate CEF
  browsers that CDP all reports as `type:"page"`; `location.href` does not catch it.
- **❓ is not ✅.** An unmeasured realm may not be called done, deferred, or shipped around.
- **CODE-READ never upgrades itself.** Only a measurement moves a cell to ✅.
- **Never defer a group** — each realm on its own evidence. Bundling all workers cost a week.
- **If you discover a new realm, add it to §A as ❓ and tell the owner.** Do not quietly absorb
  it, and do not quietly skip it.
- Follow CLAUDE.md invariant #13 on failing tests: decide whether the test or the code is
  wrong, from independent evidence, and **ask before changing production code**.

## Traps that have each cost real time — do not rediscover them

- ⛔ **Author fork changes in the STANDALONE checkout `C:\cef\cef150\cef`**, not in
  `chromium\src\cef` (the in-tree COPY). The build passes `--force-cef-update`, which deletes
  and re-copies that directory — a patch living only in the copy is silently erased and the
  build goes green **with the fix absent**. Before every build:
  `git -C C:\cef\cef150\cef status --porcelain -- libcef/` → empty, and
  `git -C C:\cef\cef150\cef log --oneline origin/hodos/7871..hodos/7871` → empty after pushing.
- ⛔ `automate-git` leaves the checkout on a **detached HEAD**; a commit lands off-branch and
  `git push` reports "Everything up-to-date" while pushing nothing.
- ⛔ Pin branches must be named `pin-<sha>/7871`. `hodos/7871-<sha>` yields a plausible-but-wrong
  version string.
- ⛔ Never remove `--force-cef-update` to chase the fast incremental number.
- ⚠️ Incremental `autoninja -C out/Release_GN_x64 cef` against the warm tree is **~6 min** and is
  a free compile check before the ~40 min full script. Use it.
- ⚠️ Harnesses kill the browser **by executable path** — the owner's installed browser shares the
  image name and holds CDP 9222. Never kill by image name.
- ⚠️ Dev stack must be running for harnesses: frontend on 5137 and dev wallet on 31401, or the
  overlays fail to load and CDP shows no usable tab.
- ⚠️ **Actions minutes:** the dev fork is private and draws a 2,000-min monthly allowance;
  code pushes cost ~19 min each (Windows runners bill 2×). Docs are now `paths-ignore`d. Batch
  code pushes. Never push a `v*` tag to `origin` — it starts a build that burns minutes and dies.
- ⚠️ The release repo is public and its Actions are free; release builds are not the cost.

## Stop and ask the owner at these gates

Do not decide these yourself:

1. **R9 shared workers / R10 service workers** — implement or sign a deferral (§F needs a
   signature and a date; an unsigned row is ⛔, not ⏸️).
2. **WebGL `UNMASKED_RENDERER`/`VENDOR` strings** — farble, or accept as a documented boundary.
3. **Release-note wording** — all three §D.1 residuals, including that D5's residual is
   **unchanged from beta.1**, not new.
4. **Any newly discovered realm** that would extend the sprint.

## Notes for Mac — accumulate throughout, relay at the end

Keep a running section as you work; do not batch-reconstruct it at the finish. Capture:

- every fork commit + the **pin** Mac must build, and the `pin-<sha>/7871` branch name;
- every harness added or changed, and how to invoke it;
- every §A/§B cell that changed state, with the evidence;
- every trap hit and how it was resolved;
- anything **platform-specific** you could not verify from Windows.

⚠️ **Exception to "relay at the end": if a finding changes the DESIGN, relay it immediately.**
Mac builds from the pin, and a design change mid-flight invalidates a build they may have
started. The `window.open()` bypass was exactly this case.

**Mac's work, after this session finishes:**

1. Rebuild at the final pin and re-run their full gate suite (rotation, battery, Q2, exemption,
   perf, subframe/popup, plus every new Phase 0 harness).
2. Re-measure the realms that are platform-dependent rather than trusting Windows results.
3. Re-upload the macOS CI asset at the new engine, **versioned**
   (`cef-binaries-macos-<version>-g<sha>.tar.bz2`), and give the exact `release.yml` lines.
4. Confirm §C-7 parity for every ✅ cell.

## Definition of done for this session

- Every page-scriptable realm in §A is ✅ **or** carries an owner-signed deferral in §F.
- No ❓ remains in §A or §B.
- Every ✅ names its proving test and its negative control.
- The document's §D ladder is updated to the claim the coverage now permits.
- A relay round is written and pushed with Mac's complete work list.

State clearly at the end which rung of §D we are on and what claim beta.2 may make.
