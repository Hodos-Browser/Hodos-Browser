# Phase 0.8 — the connect modal must show what the site actually asked for

**Opened 2026-08-21.** Owner-requested after testing `https://bitgenius.net/app`.
**Re-scoped twice on 2026-08-22.** First split to the safety half at kickoff; then **merged back**
on the owner's call, so the phase now carries BRC-73 parsing as well. §3 records why.

## 1. The defect — MEASURED

Clicking **Continue with Metanet** on bitgenius.net produced, in the browser log:

```
📦 Triggering manifest_connect_bundle for bitgenius.net
   (app=BitGenius, 0 protocols, 0 baskets, 0 certs, 0 counterparties)
🛡️ engine Prompt (domain-trust) … type=ManifestConnectBundle reason=new_domain_with_manifest
```

**Zero protocols — while the site declares four.**

**Consequence.** The user is shown a manifest-flavoured consent dialog with no itemised permissions —
reading, in the owner's words, as *"allow identity and allow operations without asking"* with no
payment guardrails — and approves it. The site then holds protocol access it was never shown.

### Root cause, confirmed mechanically at kickoff

Two independent lenient parsers each "succeed" on a manifest they did not understand:

- `rust-wallet/src/manifest.rs :: parse_manifest` returns `Some` for **any** valid JSON object; it
  returns `None` only on malformed JSON or a non-object. bitgenius → `Some` with 0 protocols →
  `manifest_present = true` in `permission_service/request_gate.rs :: domain_trust_gate` → the engine
  picks the bundle prompt.
- `cef-native/src/core/ManifestFetcher.cpp :: ParseFromJson` sets `m.valid = true` explicitly
  *"even if some sections are empty"*.

⭐ **The fail-loud fallback already exists.** In the `manifest_connect_bundle` arm of
`HttpRequestInterceptor.cpp :: tryHandlePendingResponse`:

```cpp
if (manifest.valid) { newRequestId = openManifestConnectBundleModal(...); }
else { LOG_WARNING_HTTP("… unparseable manifest … falling back to domain_approval");
       newRequestId = openDomainApprovalModal(modalCtx, resume); }
```

It never fires, because `valid` is true for a manifest with zero permissions. The safety half of this
phase is therefore a **tightening of what `valid` / `Some` mean**, not new machinery.

## 2. Second defect, same trace — one click, THREE modals

```
11:06:17.720  approvalId=7850bb41…  🔔 Creating notification overlay (manifest_connect_bundle)
11:06:17.738  approvalId=19437cff…
11:06:17.776  approvalId=36fc4859…
```

Three 202s and three overlays in 56 ms for a single user click, all on `/getVersion`.

**Hypothesis to measure, not assume:** `openManifestConnectBundleModal` already dedupes via
`PendingRequestManager::hasPendingForDomain` and logs *"Modal already pending for domain … request
queued"*. But `hasPendingForDomain(...)` and `addRequest(...)` are two separately-locked calls on the
singleton, so three concurrent `/getVersion` requests can each read "none pending" before any of them
registers — a check-then-act race. Fits the 56 ms window. ⛔ Confirm before fixing.

## 3. Scope — merged, 2026-08-22 (owner decision)

Kickoff proposed splitting BRC-73 parsing out to beta.4, keeping only the safety half here. **The
owner merged it back, and the deciding argument is sound:**

> Shipping only the A3 tightening would **downgrade bitgenius** — the one site in the whole survey
> doing the right thing — from "a bundle listing nothing" to "a plain domain_approval". Honest, but
> worse. And both parsers, the fixture harness and all of the spec context would be touched twice.

So this phase does the whole job: fail loud **and** understand the standard shape.

**One item stays deferred:** the *"App ABC recommends these settings — accept, or adjust yourself"*
modal (`../../TICKET_brc73_group_permissions_manifest.md` §5). Not for risk — because **no site in
the survey declares `spendingAuthorization`**, so there is nothing to feed it. Building a modal
against zero real inputs is how you get the wrong modal. The parser still reads the category (§6
fixture 3) so the data is there the moment it is worth showing.

### What the kickoff settled — recorded so it is never re-derived

- **Canonical shape: BRC-73, "Group Permissions for App Access"** (`bitcoin-sv/BRCs`, `wallet/0073.md`)
  — *"The current interoperable manifest namespace is `metanet.groupPermissions`. Wallets may continue
  to read the legacy `babbage.groupPermissions` namespace for backwards compatibility, but it is
  deprecated."* Categories: `protocolPermissions`, `spendingAuthorization`, `basketAccess`,
  `certificateAccess`. Apps SHOULD serve a W3C web-app manifest at `https://{originator}/manifest.json`.
- **BRC-116, "Wallet Permissions and Counterparty Trust"** (`wallet/0116.md`) is the authoritative
  *lifecycle* spec; BRC-73 defers to it and is *"not a complete permission-system specification by
  itself."* Different layers, not alternatives. Closes the `reference_brc116_manifest_research` TODO
  open since 2026-06-09.
- **Our top-level `permissions` shape is in neither spec** — our own invention
  (`PERMISSION_UX_DESIGN.md` §5), with **zero observed adopters**.
- **We already only fetch for unknown domains.** `request_gate.rs :: domain_trust_gate` short-circuits
  `if trust == "approved" { return Proceed }` — *"Short-circuit so we never fetch a manifest for an
  already-trusted domain."* The owner's "don't ask on every visit" requirement already holds.

### Adoption — MEASURED 2026-08-22 (10 properties, both locations)

| Result | Count |
|---|---|
| Serves a BRC-73 grouped-permission manifest | **1** — bitgenius.net, at **both** paths, publishing `metanet` **and** legacy `babbage` with identical content (4 `protocolPermissions`) |
| Uses `spendingAuthorization` / `basketAccess` / `certificateAccess` | **0** |
| Uses our top-level `permissions` shape | **0** |
| Returns `200 text/html` for both paths (SPA catch-all) | **4** |
| 404 / 401 / DNS-dead | **5** |

⚠️ Small, hand-picked, one-shot sample, biased *toward* likely adopters — a signal, not a survey.
⛔ **`200` does not mean "manifest."** Any fetch must parse-and-reject, never trust the status code.

## 4. Goals

1. A site publishing the **standard** shape gets an itemised connect modal showing what it declared.
2. A site publishing a shape we **cannot** read never renders as a permission-free itemised grant —
   it falls back to plain `domain_approval`.
3. One user gesture produces **one** modal.
4. A site can **never** set its own spending caps, whatever its manifest declares.
5. The parser is shaped so the three unused BRC-73 categories are **data we already carry**, not a
   future rewrite.
6. ⭐ **The user always knows whose numbers they are looking at.** A field carrying a site's
   suggested value must never be indistinguishable from a field carrying the user's own default.

## 5. Done means

- [ ] `metanet.groupPermissions` parsed, in **both** layers (`manifest.rs :: parse_manifest`,
      `ManifestFetcher.cpp :: ParseFromJson`) — they re-parse the same bytes and must move together.
- [ ] Legacy `babbage.groupPermissions` read as a fallback; `metanet` wins when both are present.
- [ ] Our legacy top-level `permissions` shape still parses (zero adopters, but it is ours).
- [ ] All four BRC-73 categories parsed into the existing `Manifest` struct — a **translation layer**.
      ⛔ The decision engine is not modified. BRC-73 is an *input* to it.
- [ ] `/manifest.json` fetched in addition to `/.well-known/wallet-manifest.json`, sequential, both
      under the existing 3 s / 64 KB caps, unknown-domain path only.
- [ ] A manifest with **no recognised permissions** falls back to `domain_approval`.
- [ ] Duplicate connect prompts for one gesture coalesced to one — **after** the race is measured.
- [ ] ⛔ Confirm what a manifest-connect grant **writes** to `domain_permissions` — the row must
      inherit the user's safe defaults ($1 per-tx / $10 per-session), never anything the manifest
      declared. **Not yet measured.** If it already cannot, this is a regression test, not a fix.
- [ ] ⭐ Any field populated from the manifest is **visibly marked as the site's suggestion**, and the
      modal states plainly that these are not the user's defaults. A one-click **"Use my defaults"**
      control reverts every suggested field.
- [ ] The approved manifest is stored as a **snapshot**, informational only (§6a).
- [ ] Fixtures checked in (§7) and driven by both test suites, each with a negative control.

## 6a. Consent provenance and the stored snapshot — owner requirement, 2026-08-22

### The modal carries two different kinds of content — keep them apart

| Kind | Whose it is | How it is shown |
|---|---|---|
| **What the site is asking for** — protocols, baskets, certificates | Inherently the site's | Itemised, with the site's own `description` strings, labelled as the site's request |
| **The limits we allow it under** — per-tx, per-session, rate/min, max-tx/session | **Ours** | Must default to the **user's** values |

⛔ **This is a security requirement, not styling.** If a modal auto-populated with the site's
suggested numbers looks identical to one carrying the user's defaults, the site has changed what the
user approves *without the user knowing*. That is a worse defect than the one this phase exists to
fix: today the modal shows **nothing**; an undifferentiated one would show **the site's numbers
dressed as the user's own**. If the differentiation is not built, **do not auto-populate at all.**

### ⚠️ Open design decision — which way round? (owner to settle)

- **(a) Populate with the site's values**, clearly marked, plus a "Use my defaults" button.
  Best first-run UX on a site whose recommendations are what make it work — but a user who simply
  clicks Approve accepts the site's numbers, which is exactly the approve-without-reading failure
  mode this phase exists to close.
- **(b) Populate with the user's defaults**, show the site's suggestion **beside** each field as
  information, plus an explicit "Use the site's recommended settings" button.
  The safe state is the default state, and adopting the site's numbers is an affirmative act.

### ✅ SETTLED 2026-08-22 — build both; (b) is the default, (a) is an opt-in toggle

**(b) ships as the default behaviour.** **(a) is available behind a user toggle** for someone who
understands the trade-off and would rather have the site's values pre-filled.

- **Toggle location:** bottom-right of the **"Default Limits for New Sites"** section —
  `frontend/src/components/wallet/ApprovedSitesTab.tsx:144`.
- **Wording must name the risk**, not just the feature. It is opting into having a *site* choose the
  starting numbers. Something like *"Pre-fill new-site limits with the site's recommended settings"*,
  off by default.
- **Persistence:** a global user setting, following the `default_identity_key_disclosure_allowed`
  precedent (V19) — a `default_*` column on `settings`, saved through the existing `/wallet/settings`
  GET/POST that this component already uses. Proposed key: `default_prefill_from_manifest`,
  default `0`. Part of migration V24 (§6b).
- ⛔ **The toggle changes which values are pre-filled, nothing else.** Even with (a) on, `R-PROV`
  still holds — every manifest-sourced field stays visibly marked as the site's suggestion, the
  sections still open expanded, and "Use my defaults" is still present. The toggle is a starting
  position, never a suppression of the marking.

Original reasoning, retained:

**Recommendation: (b)** — and the owner's own §6a stored-snapshot idea is what makes (b) affordable:
its cost (a first run under conservative caps may feel restrictive) is recoverable at any time from
the site's settings, by a user who now has grounds to trust the site. That progression — conservative
default, lived experience, informed adjustment — is better consent than front-loading a stranger's
numbers.

Either way, `R-CAPS` holds: nothing a site declares is *written* without an affirmative user act.

### The stored snapshot

Store the approved manifest so the user can later adopt the site's recommended settings from the
site's own permission screen, and so "what did this site ask for when I approved it?" is answerable.

⛔ **Three rules, all load-bearing:**

1. **Informational only — never a decision input.** The authoritative state stays
   `domain_permissions`; the snapshot only ever feeds **display** and a user-initiated "apply these"
   action.
   ⚠️ *Correction, 2026-08-22:* an earlier draft cited BRC-116's *"In-memory caches … MUST NOT be
   treated as authoritative permission state"* as if it bound this. It does not — that rule is about
   cached **permission state**, and a manifest snapshot is a record of what a site *asked for*, not a
   grant. We adopt the discipline **by choice** because it is the right one, not because the spec
   compels it here. Do not cite it as a requirement.
2. **Snapshot as approved, not live.** 🚨 Otherwise: a site publishes modest recommendations, the
   user approves, the site later publishes aggressive ones, and the user clicking "restore
   recommended" months later silently adopts numbers they never saw. Store what was on screen at
   approval time, with `fetched_at` and the URL it came from. A manifest that has **changed** is a
   **re-consent** event — BRC-116 models exactly this as a `renewal` flag — never a silent update.
3. **Schema change ⇒ owner approval** (CLAUDE.md invariant #2). Spelled out in §6b.

**Scope split:** store the snapshot **in this phase** — it is small (bitgenius is 3.3 KB against a
64 KB cap) and storing late means the restore button can never be offered for sites approved before
it landed. The **restore-button UI on the site permission screen is beta.4**; it is purely additive
once the data exists.

## 6b. Migration V24 — proposed, NOT YET APPROVED

⛔ **Nothing here exists yet.** The schema is currently at **V23** (`database/migrations.rs`, highest
fn `migrate_v22_to_v23`; the runner in `connection.rs :: WalletDatabase::migrate` gates on
`current_version < 23`). "V24" is simply the next migration number. CLAUDE.md invariant #2 — do not
change wallet DB schema without asking — so this needs an explicit owner yes before it is written.

It would carry **two** changes, both additive; no existing column or table is altered or dropped:

**1. New child table — the manifest snapshot**

```sql
CREATE TABLE IF NOT EXISTS domain_manifest_snapshots (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    domain_permission_id INTEGER NOT NULL,
    manifest_json        TEXT    NOT NULL,  -- raw bytes as fetched, ≤ 64 KB
    source_url           TEXT    NOT NULL,  -- which of the two locations served it
    fetched_at           INTEGER NOT NULL,
    approved_at          INTEGER,           -- NULL = fetched but not the approved snapshot
    FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE
);
```

Child table rather than a column on `domain_permissions`, per CLAUDE.md's documented reuse anchor
(*"extend via child tables joined by FK + CASCADE, mirroring the `cert_field_permissions` pattern"*).
It also lets a newer fetch sit beside the approved one for diffing, which is what rule 2 needs to
detect a changed manifest and treat it as re-consent. `ON DELETE CASCADE` means revoking a site
disposes of its snapshot — no orphaned record of a site the user removed.

**2. New settings column — the pre-fill toggle**

```sql
ALTER TABLE settings ADD COLUMN default_prefill_from_manifest INTEGER NOT NULL DEFAULT 0;
```

Mirrors `default_identity_key_disclosure_allowed` (V19). Default `0` = behaviour (b).

**Size:** bitgenius is 3358 bytes; the fetch cap is 64 KB; one row per approved domain with a
manifest. Negligible.

## 7. Test JSON fixtures

**Canonical home: `demos/manifest-shapes/`** — one copy, servable *and* unit-testable, mirroring the
`demos/qr-codes/` precedent (static files, `npx serve`). Rust reads them with `include_str!`; the C++
suite reads the same files. ⛔ Do not duplicate them into a second folder — the 2026-08-03 docs-truth
pass rule is one home per fact.

| # | Fixture | Purpose |
|---|---|---|
| 1 | `brc73-metanet-protocols.json` | The bitgenius shape: `metanet.groupPermissions.protocolPermissions`, 4 entries, mixing a Level-1 declaration with no `counterparty` and Level-2 declarations that name one. **The case that must work.** |
| 2 | `brc73-babbage-legacy.json` | Deprecated namespace only. Must still parse. |
| 3 | `brc73-all-categories.json` | All four categories including `spendingAuthorization`. **Future use case** — nobody serves this yet; it is how goal 5 is proved and how `A5` is driven. |
| 4 | `brc73-both-namespaces.json` | `metanet` **and** `babbage` present with *different* content. Pins precedence: `metanet` wins. |
| 5 | `hodos-legacy-permissions.json` | Our top-level `permissions` shape. Regression guard. |
| 6 | `unrecognised-shape.json` | Valid JSON, plain W3C web-app manifest, no permission namespace at all. **Must produce `domain_approval`, not an empty bundle.** |
| 7 | `not-a-manifest.html` | The SPA `200 text/html` case. Must yield no manifest. |
| 8 | `bitgenius-live-capture.json` | The real 3358-byte manifest, captured 2026-08-22, so the acceptance test does not depend on a live third-party site. |

## 8. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| `P0.8-A1` | bitgenius's real manifest parses to **4 protocols** | ⛔ Pre-fix **0** — measured 2026-08-21 | The `📦 Triggering manifest_connect_bundle … (N protocols…)` line. ⚠️ That count comes from the **C++** parse of the bytes Rust embeds — a Rust-only fix cannot move it | T1 |
| `P0.8-A2` | The modal **displays** the four with their descriptions | ⛔ Pre-fix it displays none | The rendered overlay, not the parse count | T2 |
| `P0.8-A3` | Fixture 6 renders **`domain_approval`** | ⛔ Pre-fix it renders a permission-free bundle | Which modal opens, **and** the absence of the `📦 Triggering` line — not the parse count | T1 |
| `P0.8-A4` | One click → **one** modal | ⛔ Pre-fix three in 56 ms — measured | Notification-overlay creation count, not the 202 count | T1 |
| `P0.8-A5` | Fixture 3's `spendingAuthorization` does **not** raise the stored caps | ⛔ Stub the clamp → the row takes the site's numbers | The `domain_permissions` row after approval | T1 |
| `P0.8-A6` | A site serving **only** `/manifest.json` is found | ⛔ Remove the second location → not found | A fixture served at `/manifest.json` only | T1 |
| `P0.8-A7` | Fixture 7 yields **no** manifest | ⛔ Trust the status code → HTML treated as a manifest | Parse result, driven by real `200 text/html` | T1 |
| `P0.8-A8` | Fixture 4: `metanet` content wins over `babbage` | ⛔ Swap precedence → legacy content shown | Parsed protocol set | T1 |

| `P0.8-A9` | Every manifest-populated field is visibly marked as the **site's** suggestion, and the modal says so in words | ⛔ Remove the marking → it is indistinguishable from the user's own defaults | The rendered modal, read by someone who did not write it — not the presence of a CSS class | T2 |
| `P0.8-A10` | One click reverts every suggested field to the user's defaults | ⛔ Stub the control → suggested values survive | The `domain_permissions` row after approval | T1 |
| `P0.8-A12` | With the pre-fill toggle **off** (default), limit fields carry the **user's** defaults; with it **on**, they carry the site's — and in **both** states the site-sourced fields stay marked and "Use my defaults" still works | ⛔ Toggle has no effect, or turning it on suppresses the marking | The rendered modal in both toggle states | T1 |
| `P0.8-A11` | A stored snapshot never changes an allow/deny outcome | ⛔ Feed the snapshot into the gate → a decision moves | Engine decision with and without a snapshot present, identical inputs | T1 |

⚠️ **Subject-correctness, twice bitten already this sprint.** `A3` must assert *which modal opened*,
not a parse count — a count assertion passes the instant the parser changes at all, whether or not
the prompt improved. `A1` must be driven through the C++ parse, because that is what the log line
measures.

**Live acceptance:** bitgenius.net end to end, from an unapproved state (revoke via right-click →
Manage Site Permissions first, or the domain is already trusted and nothing will be fetched).
⚠️ `test-fixtures/manifest-dapp/README.md` records the constraint: the manifest fetch is **HTTPS-only
for bare hosts**, so a localhost demo cannot exercise the real fetch path — the fixtures test the
parsers, bitgenius tests the flow.

## 9. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-PERIM` | Privacy-perimeter gates re-evaluate after a connect approval | `HttpRequestInterceptor.cpp` deliberately does **not** propagate `X-User-Approved` for `domain_approval` / `manifest_connect_bundle` so the kind gates run fresh; its comment calls propagating it a privacy-perimeter bypass. A richer bundle must not start granting those. |
| `R-CAPS` | A site can never set its own spending caps | `A5`. BRC-73's `amount` is **monthly satoshis**; ours are **per-tx / per-session USD cents** — neither unit nor period matches, so it is displayed, never written. |
| `R-PROV` | The user can always tell whose numbers a field carries | §6a. An undifferentiated auto-populated modal lets a site alter what is approved without the user knowing. |
| `R-SNAPSHOT` | A stored manifest is informational, never authoritative | BRC-116 forbids treating cached state as authoritative permission state. `P0.8-A11`. |
| `R-CONNECT` | An unknown domain still gets a working connect flow | The fallback must not become a dead end. |
| `R-NOFETCH` | An approved domain is never re-asked for a manifest | Adding a second fetch location must stay behind the `trust == "approved"` short-circuit. |

## 10. Out of scope

- The *"app recommends these settings"* modal → `../../TICKET_brc73_group_permissions_manifest.md` §5.
- Any change to the decision engine.
- Chromium's Local Network Access prompt branding → Phase 0.9.

## 11. Related

- `../../TICKET_brc73_group_permissions_manifest.md` — spec citations, full survey, deferred modal.
- `bitcoin-sv/BRCs` — `wallet/0073.md`, `wallet/0116.md`.
- `test-fixtures/manifest-dapp/` — the existing HTTPS connect-bundle fixture and its constraints.
- `demos/README.md` — where runnable fixtures live and why.
