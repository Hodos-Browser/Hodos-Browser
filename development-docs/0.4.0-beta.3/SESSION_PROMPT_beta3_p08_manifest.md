# Session prompt — beta.3 Phase 0.8 (manifest shape / connect modal)

Paste the block below to open the implementation session. Written 2026-08-22, after the kickoff
review scoped the phase and checked in the fixtures (`cc222d3`).

---

Start beta.3 Phase 0.8 — the connect modal must show what the site actually asked for. This is a
**consent-surface** phase: the user approves what this modal tells them, so a modal that understates
what it grants is the defect. Cross-layer: Rust parser + C++ parser + the connect overlay.

Phase 0.7 (UTXO reservation leak) is DONE and pushed (`e42cd04`) — do not reopen it.
Phase 0.8 was **scoped but NOT implemented** in the previous session (`cc222d3`).

## 0. Read first — all of it, before any code

1. **Auto-loaded MEMORY.md**, especially `project_p08_manifest_brc73_scoped_2026_08_22` (the kickoff's
   findings — the spec question is SETTLED, do not re-derive it) and
   `project_beta3_windows_mac_deconfliction_protocol` (git workflow: commit/push per phase, batch the
   relay, rebase before push, **never commit `X402_INTEGRATION.md`**).
2. `development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` — goals §4, done-means §5,
   **consent provenance + stored snapshot §6a**, **proposed migration V24 §6b**, fixtures §7,
   evidence table §8 (A1–A12), invariants §9.
3. `development-docs/TICKET_brc73_group_permissions_manifest.md` — spec citations, the adoption
   survey, and the deferred modal. **Archive this ticket when 0.8 closes.**
4. `demos/manifest-shapes/README.md` + all 8 fixtures.
5. **The BRC specs themselves**, not just our summary of them:
   `gh api repos/bitcoin-sv/BRCs/contents/wallet/0073.md --jq '.content' | base64 -d` (the schema)
   and the same for `wallet/0116.md` (the lifecycle). Read 0116 properly — the kickoff only skimmed
   it, and it is the authority on prompt routing, persistence, renewal and revocation, which is
   exactly the surface this phase touches.
6. `PERMISSION_UX_DESIGN.md` §5 — where our non-standard shape is documented.

## 1. Kickoff (CLAUDE.md mandatory) — verify, don't trust

The previous session verified these; **re-verify, they may have moved**:
- `rust-wallet/src/manifest.rs :: parse_manifest` — reads top-level `permissions`; returns `Some` for
  ANY valid JSON object.
- `cef-native/src/core/ManifestFetcher.cpp :: ParseFromJson` — sets `valid = true` *"even if some
  sections are empty"*.
- `HttpRequestInterceptor.cpp :: tryHandlePendingResponse` — the `manifest_connect_bundle` arm
  ALREADY has `if (manifest.valid) … else openDomainApprovalModal(...)`. ⭐ **The fail-loud fallback
  exists and simply never fires. Tighten what `valid` means; do not build a second fallback.**
- `rust-wallet/src/permission_service/request_gate.rs :: domain_trust_gate` — short-circuits
  `if trust == "approved" { return Proceed }`, so we already never re-fetch for a trusted domain.
- `rust-wallet/src/database/models.rs` — the `domain_permissions` control fields.

## 2. Goals to verify against (contract §4)

1. A site publishing the standard shape gets an **itemised** connect modal.
2. A shape we cannot read **never** renders as a permission-free itemised grant — it falls back to
   plain `domain_approval`.
3. One gesture → one modal.
4. A site can **never** set its own spending caps.
5. The three unused BRC-73 categories are data we already carry, not a future rewrite.
6. ⭐ The user always knows **whose numbers** a field carries — the site's suggestion is never
   indistinguishable from their own default.

## 3. Security / privacy review — explicit, not implied

- `R-PERIM`: `HttpRequestInterceptor.cpp` deliberately does **not** propagate `X-User-Approved` for
  `domain_approval` / `manifest_connect_bundle`, so the kind gates re-evaluate afterwards; its own
  comment calls propagating it a privacy-perimeter bypass. **A richer bundle must not start granting
  those.** Confirm identity-key reveal, key-linkage reveal and sensitive cert fields still prompt
  after a bundle approval.
- `R-CAPS`: a manifest-declared spend must never reach `domain_permissions` (`P0.8-A5`).
- A manifest is attacker-controlled input from an untrusted origin. Treat every string as hostile:
  the site's `description` strings are rendered in the modal — check the injection path the way P0.6
  did for BIP21 `amount` (that one reached arbitrary JS in the wallet overlay). Bound array lengths
  and string lengths; the 64 KB fetch cap is not a parse cap.
- Privacy: fetching a manifest is a request to a third-party origin. Confirm it still happens **only**
  for unknown domains, and that nothing about the user is sent.

## 4. Fit it into OUR auto-approve engine

⛔ **The engine is not modified.** BRC-73 is a *translation layer* that produces the input the engine
already consumes (`crates/hodos_permission_engine`, driven by `permission_service/`). Confirm this
holds — if a BRC-73 field appears to need an engine change, stop and ask.

## 5. ⭐ The field gap — evaluate and recommend

`domain_permissions` has **four** numeric controls: `per_tx_limit_cents` (100),
`per_session_limit_cents` (1000), `rate_limit_per_min` (30), `max_tx_per_session` (100), plus
`identity_key_disclosure_allowed` and `bundled_scope_grant`.

BRC-73 offers **one**: `spendingAuthorization.amount` = **monthly satoshis**. BRC-116 confirms the
gap is deliberate and total: *"Protocol permission grants are binary (grant or deny). There are no
amount limits or ephemeral flags"* (§Protocol Permissions), the same for basket grants, and spending
is tracked on a **calendar month** basis with no time-based expiry. So the standard has **no rate
limit, no per-transaction cap, and no session concept anywhere**, and its single number matches
neither our unit (USD cents) nor our period. We also have **no monthly concept at all** — worth
deciding whether to add one.

Produce a written recommendation covering both directions:
- **Do we adjust our engine?** (e.g. is a monthly cap worth carrying alongside per-tx/per-session?)
- **Do we propose fields upstream to BRC-73?** (rate limit per minute, per-transaction cap, session
  scope). If yes, draft the proposal text — we would be arguing from a shipping implementation,
  which is the strongest position to propose from.
Record the answer in the contract. This is owner-requested output, not optional.

## 6. Modal / UX

Reuse the existing connect-bundle modal (`frontend/src/pages/BRC100AuthOverlayRoot.tsx`, type
dispatch; sibling surface `frontend/src/components/wallet/ApprovedSitesTab.tsx`). ⛔ Do not add a new
overlay HWND — CLAUDE.md's overlay rules.

**Owner requirements (contract §6a — read it, it is the security argument):**
- If we auto-populate fields with the site's suggested settings, the collapsible sections **must be
  expanded on popup** — a user must not approve values hidden behind a collapsed section.
- ⛔ **A manifest-populated field must never be indistinguishable from the user's own default.** The
  modal must differentiate visibly *and say so in words*. This is a security requirement: an
  undifferentiated modal lets a site change what the user approves without the user knowing — worse
  than the defect this phase fixes, because today the modal shows nothing rather than showing the
  site's numbers dressed as the user's. **If the differentiation is not built, do not auto-populate.**
- A one-click **"Use my defaults"** control must revert every suggested field.
- ✅ **SETTLED (owner, 2026-08-22): build both.** (b) — the user's defaults pre-filled, the site's
  suggestion shown beside each field — is the **default**. (a) — pre-fill with the site's values —
  is available behind an **opt-in toggle at the bottom-right of "Default Limits for New Sites"**
  (`ApprovedSitesTab.tsx:144`), persisted as `default_prefill_from_manifest` on `settings`
  (default 0), following the `default_identity_key_disclosure_allowed` precedent. ⛔ The toggle
  changes only **which values are pre-filled** — even with (a) on, site-sourced fields stay marked,
  sections stay expanded, and "Use my defaults" still works (`P0.8-A12`).

Keep the two kinds of modal content apart: **what the site asks for** (protocols, baskets, certs — 
inherently the site's) versus **the limits we allow it under** (per-tx, per-session, rate/min,
max-tx/session — ours, and they default to the user's).

**Store the approved manifest as a snapshot** (contract §6a, schema in §6b). Three rules:
informational **only**, never a decision input (a discipline we adopt by choice — see §6a rule 1,
which corrects an earlier draft that mis-cited BRC-116 as compelling it); the snapshot is **as
approved**, not live, or a site can escalate its recommendations after the fact and have them
silently adopted later; and it is a **schema change** — ⛔ **migration V24 is PROPOSED, NOT APPROVED**
(contract §6b has the exact DDL). CLAUDE.md invariant #2: confirm with the owner before writing it.

⚠️ Deferred (do not build): the *"App ABC recommends these settings — accept or adjust"* modal, and
the restore-recommended button on the site permission screen (beta.4 — additive once the snapshot
data exists). No surveyed site declares `spendingAuthorization`, so there is no real input to design
the recommendations modal against. Parse the category anyway so the data is ready.

## 7. Tests — negative control is a hard rule

Evidence table A1–A8 in the contract. Drive **every** fixture in `demos/manifest-shapes/` through
**both** parsers.

- ⛔ **A1's count comes from the C++ parse** of the bytes Rust embeds in the 202 — a Rust-only fix
  cannot move that log line. A green A1 driven only through Rust proves nothing.
- ⛔ **A3 must assert which modal opened**, not a parse count. A count assertion passes the instant
  the parser changes at all, whether or not the prompt improved.
- ⛔ Every RED must be **seen** to fail on the pre-fix binary, by you.
- **Live acceptance: bitgenius.net end to end.** ⛔ Vacuous if the domain is already approved —
  `request_gate` short-circuits and never fetches. **Revoke first** (right-click → Manage Site
  Permissions), then confirm the modal itemises the four protocols with their descriptions.
- ⚠️ The manifest fetch is **HTTPS-only for bare hosts**, so a localhost demo cannot exercise the real
  fetch path (`test-fixtures/manifest-dapp/README.md`). Fixtures test the **parsers**; bitgenius
  tests the **flow**. Do not build a harness that appears to test the flow but cannot.

T1 = `cargo test` (rust-wallet) + the C++ suite (`build/bin/Release`), via `scripts/preflight.ps1`.

## 8. Workflow — research, plan, iterate

Do not one-shot this. Loop until the goals in §2 and the standards in §3/§7 are all met:
**research → plan → implement a slice → measure (RED then GREEN) → review your own work → repeat.**
Panel your own fixes — a negative control only proves the property you thought of. If research is
needed (BRC-116 lifecycle details, an ambiguous field, upstream precedent), do it before coding
rather than guessing. Hand back a tight summary of open questions before the first commit.

## 9. macOS considerations → relay

Note anything Mac Claude must verify, for the batched 0.6–0.9 relay:
- `ManifestFetcher.cpp` is shared core with no `_mac` arm today — confirm it stays that way.
- ⭐ The connect-bundle modal is the real risk: macOS overlays are **borderless NSWindows** with paired
  NSEvent click-outside monitors, not `WS_POPUP`. An itemised modal is **taller** than the current
  one and may auto-expand sections — sizing, scrolling and click-outside dismissal all need macOS
  verification. Flag explicitly if the overlay grows.
- Note any new file so the predictable `cef-native/tests/CMakeLists.txt` conflict is expected.

## 10. Close-out

- Commit + push per the deconfliction protocol (rebase first; never commit `X402_INTEGRATION.md`).
- Update the contract with measured evidence (§3a-style, as Phase 0.7 did).
- **Archive `development-docs/TICKET_brc73_group_permissions_manifest.md`** — fold its still-live
  content (the deferred modal, the survey) wherever it belongs, and do not leave two homes for the
  same fact.
- Update `CLAUDE.md` / layer docs if the manifest surface changed.
- Save memory and add the macOS items to the relay.
