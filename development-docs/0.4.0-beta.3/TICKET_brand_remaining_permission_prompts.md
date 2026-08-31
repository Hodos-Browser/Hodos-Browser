# Brand the remaining 21 permission prompts

**Status:** 🔴 **OPEN** — 21 prompts, inventory in `PROMPT_BRANDING_INVENTORY.md`. Filed `9aae090`. Labelled 2026-08-31 (had no status line).
**Sprint:** 📌 Phase 7 (consent surface) — bundled 2026-08-31.

**Opened 2026-08-24** after Phase 0.9. **Status:** 🔵 SCHEDULED — ⛔ **blocked on Phase 1 (DPI)**, see §2.
**Inventory:** `PROMPT_BRANDING_INVENTORY.md`. **Precedents:** `phase-0.9-chromium-prompt-branding/PHASE_CONTRACT.md` §7.

---

## 1. Goal

**No Hodos user should ever see a stock Chrome permission bubble.**

Chromium can raise **28** permission types. We brand **7**. The other 21 fall through to Chrome's UI
because `OnShowPermissionPrompt` returns `false` for anything unmapped.

---

## 2. ⛔ Why this is blocked on Phase 1 (DPI)

Not a scheduling preference — a dependency.

`TICKET_modal_buttons_unclickable_small_screen.md` reports a consent modal whose buttons could not be
clicked at one display scale, **still unreproduced under instrumentation**. Its own framing:

> a user who aims at *Deny* and whose click is silently translated elsewhere could land on *Allow* —
> a security bug wearing a layout bug's clothes.

Every prompt branded here becomes another modal carrying that exposure. Branding 21 more before the
hit-test is measured multiplies the surface area of an unmeasured defect. Phase 1 also owns overlay
input, which is where **most of Phase 0.9's cost actually went** (§4.1).

---

## 3. Design — one mechanism, not 21 features

⛔ **Do NOT build 21 bespoke prompts.** The estimate that follows from that framing (~1–2 h each) is
both wrong and the wrong shape.

**3a. Map every type; route unmapped ones to a Hodos generic prompt.**
`BRC100AuthOverlayRoot.tsx` already has the fallback —
`PERM[permCode] || { icon: '🔐', label: 'access a device feature' }` — and it is currently
**unreachable**, because C++ never sends an unmapped type. Give all 28 types stable
`SitePermissionType` ids and a wire code, and the fallback starts doing its job.

Result: no stock Chrome permission bubble can appear, **including for a permission type a future
Chromium bump adds** — the new type gets Hodos branding automatically instead of silently reverting
to Chrome's UI. That fail-safe direction is the main reason to do it this way.

**3b. Then a table of 28 label/icon strings.** Specific copy per type, replacing the generic label.
This is a copy exercise, not 21 engineering tasks.

**3c. ⚠️ The generic prompt must never be VAGUER than Chrome's.**
"wants to access a device feature" for **File system access** is *worse* than Chrome's specific
wording — we would have replaced an accurate prompt with a bland one and called it an improvement.
That is a consent-surface regression, the exact defect class Phase 0.8 and 0.9 kept producing.

⛔ **Rule:** the six high-risk types below get specific copy **in the same commit** that routes them
away from Chrome. They may never sit on the generic label, even temporarily:

File system access (24) · Window management (23) · Sensors (28) · Local fonts (7) ·
Idle detection (11) · Protected media identifier (18)

---

## 4. ⚠️ Risk list — written BEFORE any code

Phase 0.9's post-mortem was written at hour eight. Had it existed at hour one, the shared overlay
would have been known as the hazard before anything was touched. This section is that list, up front.

### 4.1 🔴 Shared notification overlay contention — the big one

One overlay multiplexes wallet modals, payment confirmations, certificate disclosures and permission
prompts. In Phase 0.9 this produced, in order: a modal painting over a live prompt; a latch fix that
read a value already cleared in the same millisecond; a jam that made **every** permission
browser-wide fall through to Chrome; and an invisible click-eating full-window overlay surviving up
to 300 s. **~60% of the phase's cost.**

More prompt types means more chances for one to collide with a money-path modal.
**Mitigation:** Phase 1 first; re-read `PHASE_CONTRACT.md` §7.4 and the pre-emption latch before
touching `FireHodosPermissionPrompt`.

### 4.2 🟠 Write-through is mandatory, not optional

Anything added to `kSitePermCaps` **must** mirror to Chromium's content setting
(`MirrorNetworkPermissionToChromium`). Chromium consults its own setting before calling us again, so
a Site-controls toggle writing only our SQLite changes a row nobody reads — and displays "Block"
while the site keeps working. See `TICKET_site_permission_dual_store.md`.

### 4.3 🟠 CEF has no "grant once"

`ACCEPT` persists. Any type on the `OnShowPermissionPrompt` path must show **two** buttons, not
three (`noOnce`). Media-path types (camera/mic) are the only exception.

### 4.4 🟡 Stored ids are forever

`SitePermissionType` values are written into `site_permissions.db` with no migration and no checksum.
Renumbering silently re-interprets every existing row. Extend the frozen-id test in
`site_permission_mapping_test.cpp` for every new id. ⚠️ These integers have already been reassigned
once in this project's history.

### 4.5 🟡 Some of the 21 may be unreachable in our build

WebXR (AR/VR/hand tracking), MIDI sysex, protected media identifier and others may never fire,
depending on enabled features. **Do not claim a type is branded because the code compiles.** Either
demonstrate the prompt, or mark it explicitly as *mapped but unexercised*.

### 4.6 🟡 macOS is written-but-unrun

Every mac arm from Phase 0.9 compiles and none has executed. This ticket adds more of the same.
Budget a relay round; do not treat mac as done because it builds.

### 4.7 🟢 State is no longer a hazard

`reset_test_state.py` exists. ⛔ Use it, and require `verify` to exit 0 before any acceptance run. A
fresh profile is **not** a fresh test (one wallet DB serves all profiles).

---

## 5. Adversarial review criteria

Run a review **before** merge, against these specific claims. Each is phrased so it can be attacked;
a reviewer that finds nothing should say which it could not break and why.

1. **No stock Chrome permission bubble can reach the user.** Find any path where
   `OnShowPermissionPrompt` returns `false` for a type in the table — mixed masks, multi-type
   requests, insecure origins, `hasPending()`, an unsupported platform.
2. **The callback is answered exactly once.** Never zero (hang / jam), never twice. Attack via tab
   close, navigation, browser close, the 60 s watchdog, `OnDismissPermissionPrompt`, overlay
   pre-emption, and a modal closing without a decision.
3. **The prompt never destroys another consent surface**, and never leaves an overlay stranded.
   Specifically re-attack the Phase 0.9 finding: a non-connect modal owning the overlay while a
   permission prompt wants it.
4. **The label always matches the capability.** Hunt for any type reaching the generic label when
   §3c says it must not, and for any label claiming less access than the permission grants.
5. **Every type in `kSitePermCaps` is genuinely revocable** — the Site-controls toggle changes the
   setting that actually governs, not just ours. Verify against the *site's behaviour*, not the panel.
6. **Stored ids match the frozen-id test**, and no id was reused or renumbered.
7. **Two-button rule holds** for every prompt-path type; three buttons only on the media path.

---

## 6. Out of scope

- `OnJSDialog` and `GetAuthCredentials` — **different machinery** (blocking dialogs, a password
  field, different lifecycle). Separate time-boxed spike; do not fold in, or this ticket becomes
  open-ended. See `PROMPT_BRANDING_INVENTORY.md` Group C.
- Group B Chrome-UI bubbles (save password, save card, translate). No CEF hook exists; patch-only.
- Changing behaviour of the existing 5 — covered by `TICKET_prompt_denials_should_not_persist.md`
  and `TICKET_site_permission_dual_store.md`.

---

## 7. Test plan

**Unit** — extend `site_permission_mapping_test.cpp`: every new id frozen; every type yields a
non-empty unique wire code; the six high-risk types do **not** resolve to the generic label.
Negative control: delete a mapping arm and watch exactly the right tests go red.

**Live** — after `reset_test_state.py verify` exits 0, trigger at least the six high-risk types from
a real page and confirm a Hodos prompt. For each, assert the **effect**, not the dialog: allow →
capability works; deny → it does not; and per §4.5, mark anything you could not trigger as
*unexercised* rather than passing it.

**Cross-DPI** — cells #4/#6/#9 per `DPI_RESOLUTION_TEST_MATRIX.md`. Inherited from `P0.9-A5`, which
was never run. ⛔ Negative control: the same prompt must be seen working at 100%, or the test cannot
distinguish a DPI bug from a broken prompt.

---

## 8. Standing rules inherited from Phase 0.9

- Browser logo, never the wallet logo — these are browser-level asks about sites and the device.
- Prompt denials are temporary (`DISMISS`); overt Site-controls actions persist.
- New stable ids, never a CEF bit value.
- Anything that grants more than its label says must **fail closed** — no grant without the
  disclosure actually rendering (`PHASE_CONTRACT.md` §7.2).
