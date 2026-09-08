# Session prompt — beta.3 Phase 7d: the management half of the consent surface

Paste the block below into a fresh session. Everything above the line is context for whoever is
assembling it; the prompt itself starts at **PROMPT**.

**Written 2026-09-08**, at the close of Phase 7c. Base commit: `9989935` (== `origin/0.4.0` locally;
⚠️ **nothing since `c0b0672` has been pushed** — six commits sit local-only, so a Mac relay or a
`git ls-remote` check will not see them).

---

## PROMPT

Start beta.3 Phase 7d — the **management** half of the consent surface. Phase 7a/7b/7c closed the
*connect* half; this is the screen the user visits afterwards.

⛔ Run the kickoff before any code. Read, in this order:
1. `development-docs/0.4.0-beta.3/TICKET_edit_limits_modal_usability.md` — **five** items
2. `development-docs/0.4.0-beta.3/TICKET_site_permission_dual_store.md`
3. `development-docs/0.4.0-beta.3/phase-7-consent-surface/PHASE_CONTRACT.md` §0.1 row **R4**
4. `development-docs/0.4.0-beta.3/phase-7c-quiet-mode/PHASE_CONTRACT.md` §5.3 and §9.2

⛔ **Inventory against the tree before believing any of it.** Every kickoff this sprint has found
plan claims that did not survive measurement, and Phase 7c found **four** in one document —
including a "fix" that was a provable no-op and a narrowing target forbidden by an owner-approved
invariant. Assume the same rate here.

### What Phase 7c changed underneath this ticket — read this before sizing anything

`domain_permissions.bundled_scope_grant` **no longer decides anything.** The engine reads only the
V18 child tables. Consequences that land directly on the surface 7d owns:

- The **"Granted permissions" list** in `DomainPermissionForm` is now the *entire* per-site state.
  Revoking a row there is the only thing that makes a scope prompt again. It went from informational
  to load-bearing, and its usability items (2, 3, 5) rose in value accordingly.
- The **Quiet mode toggle is gone** from that panel, from the connect screens, and from wallet
  settings. Do not re-add one. `bddd312` explains why in the code comments.
- Grants are now written `key_id = '*'` from **both** paths (manifest connect and Always-allow), so
  the list shows `key any` for everything. That is intended, not a display bug.

### Scope — the owner's stated order

⭐ **Item 5 first (the search box).** It is the cheapest and it makes the other four easier to test,
because reaching a specific site stops being a scroll. `ApprovedSitesTab.tsx` has no search, filter
or sort — measured 2026-09-05, the words do not appear in the file — and 15 approved domains already
exist in the dev profile.

⚠️ Two things to check before writing the filter, not after:
- Native `<input>`, not MUI — CEF overlay rule, root `CLAUDE.md` §CEF Input Patterns.
- **Whether any row action is keyed by list index rather than by domain + grant identity.** If it is,
  filtering silently makes **Revoke act on the wrong row** — and after 7c, a wrong revoke is a
  permission change, not a cosmetic one.

Then items 1–3. ⛔ **Item 4 (persist denials) is a schema change** — CLAUDE.md invariant #2, needs an
explicit owner yes, and it collides with `TICKET_prompt_denials_should_not_persist.md`. The
reconciling distinction is written in the ticket; do not re-derive it.

### 🚨 The one with teeth — the dual store

Setting **Location / Notifications / Clipboard** to *Block* in Site controls changes the **panel's
display** but not the **site's behaviour**. `MirrorNetworkPermissionToChromium` opens with
`if (type != Loopback && type != LocalNetwork) return;` and its own comment says the other three
"have the SAME defect".

⇒ **A gate that has silently stopped gating** — the same defect class 7c just closed, and the
`R-PERIM` SUBJECT rule applies in the same way: the subject is **the site's actual behaviour**
(what `Notification.requestPermission()` resolves to), never what the panel renders. The panel
showing "Block" *is the lie under test*.

⛔ Capture the RED **before** writing the fix: today the same steps leave the site allowed and no
prompt appears. That is shipped behaviour and it is the control.

⚠️ Camera/mic are the deliberate control on the other side — they must stay governed by our store
with **no** Chromium content setting written. `A7` in the Phase 7 contract.

### Instrument discipline — these cost real time this sprint

- `npm run build` or `preflight -Full`. ⛔ **Never `npx tsc --noEmit`** — it passes on code the build
  rejects.
- ⛔ **A skip is never a pass.** Bare `preflight.ps1` skips `T1d` and prints
  *"0 failed, 1 skipped. This is NOT a pass."* Use `-Full`.
- Any probe must **assert its own trigger fired** and say VACUOUS rather than print a clean zero.
  Phase 7c produced two vacuous probes in one hour: one read a `400` (serde rejecting a malformed
  body **after** the gate ran) as a silence, and one used `-SkipHttpErrorCheck`, which is PowerShell
  **7 only** — on 5.1 every row printed `OTHER ()`.
- ⚠️ `Invoke-WebRequest` treats **202 as success**, so a success/catch split labels a *prompt* as a
  *silence*. Branch on `StatusCode`.
- Exit **143** is SIGTERM (usually your own timeout), not a failure.
- ⛔ Do not pipe a long-running launcher through `head` — it closes the pipe and kills the server.

### Machine state

The owner's **installed** browser and wallet are normally running. `.\scripts\stop-dev.ps1` now works
with no arguments (fixed `9989935`) and is path-matched — it spares the installed build. Never stop a
Hodos process by image name.

Dev stack: `.\dev-wallet.ps1` (31401) · `cd frontend && npm run dev` (5137) · then Start-Process the
dev exe with `HODOS_DEV=1 --profile=Default --remote-debugging-port=9322`.
⚠️ A stray vite from an earlier session may already hold 5137 — check before starting a second one,
and verify the running server actually serves your edits (fetch the module and grep for a string you
removed) rather than assuming.

### Owed from the 7c boundary, not yet done

- `R-INTEXT` end-to-end (both halves, stubbed in both directions) — unrun.
- The 7c adversarial review.
- `TICKET_wallet_quiet_detector_blind_to_long_polls.md` — ⛔ its negative control needs a **first**
  connect to a manifest site with a slow auth step. A test on an already-approved site *cannot fail*,
  which is how the bug survived.

Hand me the contract with its §0 delta and the item-5 index-vs-identity finding, and wait for my
confirmation before the first commit.
