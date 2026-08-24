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
modal (`DEFERRED_recommendations_modal.md`). Not for risk — because **no site in
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

## 3a. MEASURED RESULTS — 2026-08-22 (implementation session)

Every RED below was **seen to fail on the pre-fix binaries**, by running the new
assertions against the reverted sources before the fix landed. Baselines before any
change: `hodos_tests` 250 passed / 1 skipped; `cargo test --bins` 469 tests, green.

### The pre-fix RED, both layers

Rust (`cargo test --bins p08_red::` against the reverted `manifest.rs`) — **10 of 11 failed**:

| Probe | Pre-fix measurement |
|---|---|
| `A1` bitgenius | **0 protocols** (the site declares 4) — matches the 2026-08-21 production trace exactly |
| `A3` `unrecognised-shape.json` | `is_some() == true` → a permission-free bundle |
| `A6` locations | only `.well-known`; `/manifest.json` never probed |
| `A8` precedence | 0 protocols, from either namespace |
| bounds | 500 declared entries → **500 kept** |
| `iconUrl` | `javascript:alert(1)` passed straight through to the modal's `<img src>` |

C++ (`hodos_tests --gtest_filter=P08Red.*` against the reverted `ManifestFetcher.*`) —
**10 of 10 failed**. The decisive line:

```
RED A1: valid=1 protocols=0
```

⭐ That is the shipped defect in one line: the **C++** parse — the one the
`📦 Triggering … (N protocols…)` log reads and the one that builds the modal — reported
the manifest **valid** while itemising **nothing**.

### Two findings the RED run surfaced that were not in the plan

1. 🚨 **`R-CAPS` and `R-PROV` were already violated, and not hypothetically.**
   `BRC100AuthOverlayRoot.tsx` seeded `manifestPerTxCents` from
   `m.spending.perTransactionUsd * 100` and rendered it under the label
   *"**Default** payment limits: $X/tx"*. A site using our legacy shape set its own caps
   **and they were presented as the user's own defaults**. Measured directly:
   `RED legacy: protocols=0 perTxUsd=100` — the parser read the site's $100/tx while
   reading none of its protocols. This is `R-PROV` inverted, in shipped code.
2. ⛔ **Our own legacy shape never parsed its own documented example.**
   `hodos-legacy-permissions.json` yielded **0 protocols** pre-fix: the parser only read
   `protocolID: [level, name]`, while `PERMISSION_UX_DESIGN.md` §5's own example uses the
   flattened `name` + `securityLevel` spelling. The "regression guard" fixture was
   guarding a shape that was already broken. Both spellings now parse.

### A third finding, from the negative control itself

⭐⭐ **The first `A5` negative control did not trip, and the harness said so.** Driving it
with the fixture's 5,000,000 satoshis, the deliberately-broken build still produced
`perTxCents=100 (source=user)` — because 5,000,000 is above `usableUsd`'s magnitude sanity
cap and was rejected for being *absurd*, not for being *the wrong unit*. The assertion was
passing for the wrong reason. `T1f` now also drives BRC-73's own 10,000 example, which sits
inside the sanity range and isolates the unit rule; the control then trips cleanly
(`observed perTxCents=1000000 (source=site)`). **A negative control only proves the
property you thought of** — this one earned its place.

### Post-fix GREEN

| Suite | Before | After |
|---|---|---|
| `hodos_tests` (C++) | 250 passed / 1 skipped | **285 passed / 1 skipped** (286 total) |
| `cargo test --bins` | 469 | **502 passed**, 2 ignored |
| `T1f` (real `manifestConsent.ts` via node) | did not exist | 26 checks, all pass |

### Verified negative controls (each *seen* to fail, then restored)

| Control | Sabotage | Observed |
|---|---|---|
| `A5` Rust | map `spendingAuthorization.amount` → `per_transaction_usd` | `assertion failed … left: 5000000, right: 0` |
| `A5` C++ | same, in `applyGroupPermissions` | `Which is: 5000000` → `A5_SpendingAuthorizationNeverPopulatesOurCapFields` FAILED |
| `A5` frontend (`T1f --negative-control`) | `usableUsd(perTransactionUsd ?? monthlySatoshis)` | 10,000 sats/month became **$10,000 per transaction, `source=site`** — caught |
| fixture wiring | (accidental) wrong relative path in the RED probe | the "missing fixture FAILS, never skips" guard fired and named the path — and revealed `A3` was passing **vacuously** on an empty string |

---

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

- [x] `metanet.groupPermissions` parsed, in **both** layers (`manifest.rs :: parse_manifest`,
      `ManifestFetcher.cpp :: ParseFromJson`) — they re-parse the same bytes and must move together.
- [x] Legacy `babbage.groupPermissions` read as a fallback; `metanet` wins when both are present.
      Conditioned on `groupPermissions` being *present*, not merely the namespace key — socialcert.net
      serves a bare `babbage` object, and a `metanet: {schemaVersion:1}` must not shadow a populated
      `babbage` (BRC-116 Backwards Compatibility).
- [x] Our legacy top-level `permissions` shape still parses. ⚠️ It did **not** before: its own
      documented example uses the flattened `name` + `securityLevel` spelling, which the parser never
      read. Both spellings now work — see §3a finding 2.
- [x] All four BRC-73 categories parsed into the existing `Manifest` struct — a **translation layer**.
      ⛔ The decision engine is not modified. Confirmed: `PermissionContext` gained no field, and
      `a11_the_engine_context_carries_no_manifest_content` fails if one appears.
- [x] `/manifest.json` fetched in addition to `/.well-known/wallet-manifest.json`, sequential
      (standard location first), both under the existing 3 s / 64 KB caps, unknown-domain path only.
      ⭐ Hardened beyond the ask (`R-PATH`): an origin carrying a path, query, fragment, userinfo or
      interior whitespace is now **rejected**, not concatenated, and `http://` is upgraded to `https://`
      except on loopback — BRC-116 §9.2.
- [x] A manifest with **no recognised permissions** falls back to `domain_approval`.
- [x] Duplicate connect prompts for one gesture coalesced to one. The §2 hypothesis is **confirmed by
      construction**: `hasPendingForDomain()` and `addRequest()` take the manager's mutex separately.
      Replaced by `PendingRequestManager::addRequestIfFirstForDomain`, which does both under one lock.
      ⭐ Applied to **all three** openers (`domain_approval`, `brc100_auth`, `manifest_connect_bundle`),
      not only the reported one — fixing the reported arm and leaving its identical siblings is how the
      last one survived.
- [x] 🚨 **MEASURED, and it was a real violation.** The row did NOT inherit the user's defaults: the
      modal seeded its per-tx / per-session fields from `m.spending.perTransactionUsd` and rendered
      them as *"**Default** payment limits"*. This was a fix, not a regression test. See §3a finding 1.
      The four fields now start from the user's own settings, and the modal never had the user's real
      defaults before either — `/wallet/settings` did not serve `default_max_tx_per_session` at all
      (V13 column, never in the GET, never in the POST; now both).
- [x] ⭐ Every manifest-populated field is marked `suggested by site` — in colour, in a border, **and
      in words**, because colour alone fails a colour-blind user, a high-contrast theme or a
      screenshot. A one-click **"Use my defaults"** control sits in both the summary and the customize
      view and reverts all four fields. The rule itself lives in the pure, unit-tested
      `frontend/src/utils/manifestConsent.ts`, not inline in the component.
- [x] Migration **V24** written exactly as specified in §6b — `domain_manifest_snapshots` +
      `settings.default_prefill_from_manifest`, runner gate bumped to `< 24`, idempotent
      (`CREATE TABLE IF NOT EXISTS` + existence-checked `ALTER`). No deviation from the approved shape.
- [x] The approved manifest is stored as a snapshot, informational only. Bytes come from the stash
      `domain_trust_gate` fills at **fetch** time, so what is stored is what the modal was built from —
      "as approved, not live" (§6a rule 2). Write is strictly best-effort and never fatal.
- [x] The pre-fill toggle ships **off**, bottom-right of "Default Limits for New Sites", worded to name
      the risk rather than the feature. `P0.8-A12` proves it changes only *which* values are pre-filled.
- [x] All 8 fixtures driven by **both** suites from one canonical copy — Rust via `include_str!`
      (a deleted fixture breaks the build), C++ via `HODOS_MANIFEST_FIXTURE_DIR` from CMake, where a
      missing fixture **fails and names the path** rather than skipping.

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
3. **Schema change ⇒ owner approval** (CLAUDE.md invariant #2). ✅ Granted 2026-08-22; DDL in §6b.

**Scope split:** store the snapshot **in this phase** — it is small (bitgenius is 3.3 KB against a
64 KB cap) and storing late means the restore button can never be offered for sites approved before
it landed. The **restore-button UI on the site permission screen is beta.4**; it is purely additive
once the data exists.

## 6b. Migration V24 — ✅ APPROVED by the owner 2026-08-22

Approved as specified below. Invariant #2 is satisfied for **exactly this shape** — if the
implementation needs to deviate (an extra column, a different table), ask again rather than widening
it silently.

⛔ **It does not exist yet — it still has to be written.** The schema is currently at **V23** (`database/migrations.rs`, highest
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

## 8. Evidence table — RESULTS

Legend: 🟢 GREEN measured · 🔴 RED *seen* to fail on the pre-fix binary · ⚠️ OWED.
Detail and raw numbers in §3a.

| ID | Verdict | 🎯 What was actually measured |
|---|---|---|
| `P0.8-A1` | 🟢 **GREEN** | Both layers, plus the live flow. C++ parse of the real 3358-byte manifest → **4 protocols** with counterparties (`ManifestBrc73.A1_…`); Rust the same; and live against `https://bitgenius.net` the gate embedded 3358 bytes and chose `ManifestConnectBundle`. 🔴 RED: `RED A1: valid=1 protocols=0` on the pre-fix **C++** parse — valid, itemising nothing. |
| `P0.8-A2` | ⚠️ **OWED — owner visual** | The payload now carries what the modal needs (counterparty per Level-2 protocol, verifier + field list per certificate, `sourceNamespace`, `groupDescription`) and the markup renders them. **I could not drive the rendered overlay**: physical clicks are dropped in this environment ([[reference_sendinput_clicks_blocked_in_agent_env]]) and CDP reports every Hodos overlay as `type:"page"`, which faked a bug once already. Needs a human to look at it. `bitgenius.net` is left **unapproved** so that test will not be vacuous. |
| `P0.8-A3` | 🟢 **GREEN — right subject, and on a real site** | Asserts **which prompt opened**, not a parse count. Live: `socialcert.net` (publishes a `babbage` key with **no** `groupPermissions` — fixture 6's shape in the wild) → `promptType=domain_approval`, **no manifest in the payload**; `projectbabbage.com` (SPA `200 text/html`) → `domain_approval`, `reason=new_domain_no_manifest`. Fixture 6 and 7 also assert `valid == false` in both parsers. 🔴 RED: pre-fix `unrecognised-shape.json` returned `is_some()/valid == true` → the permission-free bundle. |
| `P0.8-A4` | 🟡 **PARTIAL — fix landed, browser-level count OWED** | Root cause **confirmed by construction, not hypothesis**: `hasPendingForDomain()` and `addRequest()` take `PendingRequestManager::mutex_` separately, so N concurrent requests all read "none pending". Fixed with `addRequestIfFirstForDomain` (one lock) and applied to **all three** openers. Measured precondition: 3 concurrent `/getVersion` still mint **3 distinct approvalIds** in Rust — by design; coalescing to one *modal* is the C++ side's job. ⛔ That count is browser-side and **not unit-testable**: `PendingAuthRequest.h` includes `cef_resource_handler.h`/`cef_frame.h`, so it cannot link into the CEF-free `hodos_tests`. The overlay-creation count needs a human clicking Connect. |
| `P0.8-A5` | 🟢 **GREEN + 3 verified negative controls** | Fixture 3's `spendingAuthorization.amount` (5,000,000) lands in `monthly_satoshis` and leaves `per_transaction_usd`/`per_session_usd` at **0**, in both parsers; the frontend rule refuses it in **both** toggle states. Live: the approved `domain_permissions` row read `(100, 1000, 30, 100)` — the user's defaults. 🔴 RED ×3: Rust (`left: 5000000, right: 0`), C++ (`Which is: 5000000`), frontend (10,000 sats/month → **$10,000/tx, `source=site`**). ⭐ The frontend control initially did **not** trip — see §3a. |
| `P0.8-A6` | 🟢 **GREEN** | Live: the manifest was served from **`https://bitgenius.net/manifest.json`** — BRC-73's canonical location, which the pre-fix build never probed. Unit: `ManifestUrls`/`manifest_urls` return both locations, standard first. 🔴 RED: pre-fix `manifest_url` produced only `.well-known`. ⭐ Hardened beyond the ask — `R-PATH` rejects any origin that is not a bare authority, and `http://` is upgraded except on loopback. |
| `P0.8-A7` | 🟢 **GREEN (retained property, not a fixed defect)** | Fixture 7 yields no manifest in both parsers, and live `projectbabbage.com` (`200 text/html`) produced `domain_approval`. ⛔ **Honest note: this one passed pre-fix too** — HTML fails `serde_json::from_str`, so status-code trust was never in our code. Recorded as a property now guarded by a regression test, not as a defect this phase closed. |
| `P0.8-A8` | 🟢 **GREEN** | Fixture 4 → `sourceNamespace == metanet`, 4 protocols, and no entry named `STALE legacy protocol`, in both parsers. 🔴 RED: pre-fix 0 protocols from either namespace. Also pinned: `babbage` is used when `metanet` carries no `groupPermissions` (socialcert's real shape). |
| `P0.8-A9` | 🟢 **GREEN — owner-verified live 2026-08-23** | Site-sourced fields are marked by a red `***` beside the label plus a legend line *"`***` = this site's numbers, not your defaults"*, and the summary heading *"Payment limits suggested by this site … These are not your defaults."* ⛔ Colour is never the only signal — an asterisk is a **character**, so it survives greyscale, screenshots, high contrast and colour-blindness; each marked field also carries a `title` with the words. 🚨 The RED here was **live in shipped code**: the summary read *"**Default** payment limits: $X/tx"* with X from the site's manifest. Read and critiqued by the owner, who rejected the first two designs (pill + red chrome) as reading like an error banner. ⚠️ The pill it replaced was **masking a defect** — see `T1g`.
| `P0.8-A10` | 🟢 **GREEN** | `T1f`: `useMyDefaults` restores all four values and clears every `'site'` mark. Present in both the summary and the customize view. |
| `P0.8-A11` | 🟢 **GREEN** | Two guards. (1) The engine decision is identical across the whole domain-trust matrix — a snapshot has no argument to pass. (2) `PermissionContext`'s `Debug` output is asserted to contain **`manifest_present`** and **not** `manifest_json`/`snapshot`/`raw_json`/`source_url`/`groupPermissions` — the positive assertion is what stops the negative ones being vacuous. Fails the moment anyone gives a snapshot a route in. |
| `P0.8-A12` | 🟢 **GREEN — rule + live, BOTH states** | `T1f` guards the rule. **Measured live 2026-08-23** against `test-fixtures/manifest-dapp/` (legacy shape, $1/tx + $5/session) over a cloudflared HTTPS tunnel, user defaults $10/$50: toggle **OFF** → fields carried **$10.00/$50.00**, unmarked, site's $1/$5 shown beside as a suggestion; toggle **ON** → fields carried **$1.00/$5.00**, still `***`-marked, *"Use my defaults"* reverting. ⭐ Each state is the other's negative control — same origin, same manifest, same click, opposite outcome. ⛔ **Vacuous against bitgenius.net**, which declares no `spendingAuthorization`.

### Also measured live, beyond the table

| Check | Result |
|---|---|
| **Migration V24 on a real DB** | Dev wallet at 299 addresses migrated **V23 → V24** cleanly; `schema_version` = 24, `domain_manifest_snapshots` created, `default_prefill_from_manifest` = **0**. |
| **Snapshot write** | After a real approve: one row, **3358 bytes**, `source_url=https://bitgenius.net/manifest.json`, `approved_at` set. |
| **`R-SNAPSHOT` cascade** | Revoking through the product's `DELETE /domain/permissions` dropped the snapshot with the row (1 → 0). ⚠️ A raw `sqlite3` DELETE did **not** cascade — Python does not enable `PRAGMA foreign_keys` per connection, while the wallet does. A test artifact, but it is why this was re-measured through the real endpoint instead of assumed. |
| **`R-NOFETCH`** | An approved domain (`socialcert.net`) returned `200` with no prompt and no fetch line. |
| **`R-CAPS` at the row** | `domain_permissions` after a manifest connect: `(100, 1000, 30, 100)` — the user's defaults. |
| **Counterparty at the row** | 🟢 **Before/after, same site, 2026-08-23.** Row 98 @ 17:11:08 (pre-fix): `demo messaging` — declared `counterparty: 0279887cdd…`, displayed as *"one specific party"* — landed `counterparty = NULL` (**any**). Row 99 @ 19:24:21 (post-fix): the same protocol landed `counterparty = 0279887cddd8cb…`, while `server hmac` (declared `"self"`) correctly stayed `NULL`. ⛔ The RED was not manufactured — it was the shipped behaviour, captured from a real connect before the fix. |
| **Request-time counterparty** | 🟢 **MEASURED 2026-08-23**, and it decided the fix. `createHmac` on a protocol the manifest declares `counterparty: "self"` logged `🛡️ engine Prompt (scoped) … kind=**ProtocolUse**` — not `CounterpartyUse`. Per `request_gate.rs :: call_kind` that mapping is 1:1, so the counterparty arrived as **None** (`peek_scoped_grant_scope_protocol` collapses `self`/`anyone`/`""`). ⛔ Storing the literal `'self'` would therefore compare against NULL, never match, and permanently re-prompt an approved call. This is why only concrete 66-hex keys are written. |
| **Limit-field contrast** | 🟢 `T1g`, negative control trips at **1.08:1**. Three low-contrast defects found on this consent screen 2026-08-23: the site-marked input at **1.07:1** (invisible spending cap — `background` overridden without `color` on a dark theme), the marked label at **1.5:1**, the provenance heading at **3.00:1**. All were invisible to `T1f`, which guards the consent *rule*, not the consent *surface*. |
| **Preflight** | `PREFLIGHT: PASS` (all T0 gates + T1a–T1f, `-Full`). `NEGATIVE CONTROL: PASS` — every gate, including the new T1f, was seen to fail. |

### What is NOT claimed

⛔ Three things are **owed to a human at the browser**, and no green above should be read as covering
them: the **rendered** modal (`A2`, `A9`, the visual half of `A12`) and the **one-gesture-one-overlay**
count (`A4`). Each needs a click on a real connect prompt. `bitgenius.net` was deliberately left
**unapproved** in the dev database so that test measures something.

## 8a. ⭐ The field gap — recommendation (owner-requested, 2026-08-22)

Our engine has **four** numeric controls. BRC-73 has **one**, and it matches none of ours:

| | Unit | Period | In BRC-73? |
|---|---|---|---|
| `per_tx_limit_cents` (100) | USD cents | per transaction | ❌ |
| `per_session_limit_cents` (1000) | USD cents | per browser session | ❌ |
| `rate_limit_per_min` (30) | count | per minute | ❌ |
| `max_tx_per_session` (100) | count | per browser session | ❌ |
| `spendingAuthorization.amount` | **satoshis** | **calendar month** | ✅ — the only one |

BRC-116 confirms the gap is deliberate and total, not an oversight we can read around:
*"Protocol permission grants are binary (grant or deny). There are no amount limits or ephemeral
flags"* (§4.1), the same for baskets (§4.3), and spending *"is tracked on a **calendar month**
basis"* with `expiry` always `0` (§4.2, §7.3). So the standard has **no rate limit, no
per-transaction cap and no session concept anywhere** — and we have **no monthly concept at all**.

### Direction 1 — do we adjust our engine? **Recommendation: NO for beta.3. Add a monthly cap in beta.4+, and only then.**

**Why not now.** Nothing is broken. Our four controls are strictly *narrower* than BRC-73's single
one: a wallet that enforces per-transaction and per-session ceilings already bounds monthly spend,
just not on a calendar boundary. Adding a fifth control is an **engine change**, which this phase
explicitly may not make, and it would need a new `domain_permissions` column, a new
`PermissionContext` field, a new Matrix-C branch, a month-rollover clock, and UI in three places.
That is a phase, not a patch.

**Why eventually yes.** Two reasons that will not go away:
1. **Our session concept is weaker than users think.** `per_session_limit_cents` resets when the tab
   closes — by design, recorded as such — so a user who closes and reopens a tab ten times has
   authorised ten sessions' worth of spend without a single extra prompt. A calendar-month ceiling is
   the only one of the five that a determined site cannot reset by asking the user to reload.
2. **It is the only field a BRC-100 app can actually declare.** As long as we have no monthly
   concept, `spendingAuthorization` can only ever be *displayed*, and the deferred recommendations
   modal has nothing it can honour.

⛔ If it is built: it is an **additional** ceiling, never a replacement, and never writable from a
manifest without an affirmative user act (`R-CAPS` survives).

### Direction 2 — do we propose fields upstream to BRC-73? **Recommendation: YES, but scoped to one field, and not before a real second implementation exists.**

We would be arguing from a shipping implementation, which is the strongest position to propose
from — but the survey found **1 of 10** sites serving *any* grouped-permission manifest and **0**
serving `spendingAuthorization`. Proposing rate limits to a schema nobody populates is noise. The
honest sequence is: ship this, get a second app publishing a manifest, *then* propose.

When we do, propose **one** field, not three. Per-transaction cap is the one with a real safety
argument that the standard cannot already express; rate limit and session scope are
implementation policy that BRC-116 §6.5 already leaves to the wallet
(*"Wallets MAY expose policy controls…"*), and proposing them invites a "that is your UX" rejection
that would take the useful field down with it.

#### Draft proposal text (for `bitcoin-sv/BRCs`, `wallet/0073.md`)

> **Proposed addition to `spendingAuthorization`: an optional `perTransactionAmount`.**
>
> ```json
> "spendingAuthorization": {
>   "amount": 10000,
>   "perTransactionAmount": 500,
>   "description": "For in-app purchases."
> }
> ```
>
> `perTransactionAmount` — OPTIONAL. The largest single spend, in satoshis, the application expects
> to request without a fresh prompt. MUST be ≤ `amount` when both are present. Wallets MAY ignore it;
> wallets that enforce a per-transaction ceiling SHOULD treat it as the application's *request*, and
> MUST NOT adopt it without user consent.
>
> **Rationale.** `amount` alone cannot distinguish an application that will spend 10,000 satoshis in
> a hundred small increments from one that will spend it in a single transaction. These have very
> different risk profiles for the user and, in wallets that enforce a per-transaction ceiling, very
> different prompt behaviour: the first runs silently, the second prompts on its first action. Today
> an application has no way to signal which it is, so a wallet must either prompt on a spend the
> application considered routine or stay silent on one the user would have wanted to see.
>
> An OPTIONAL field costs nothing to wallets that do not enforce per-transaction limits (BRC-116 §4.2
> defines no such ceiling, so ignoring it is conformant) and lets those that do — Hodos enforces a
> user-configurable per-transaction cap, default $1.00 — show the user a more accurate prompt.
>
> **Not proposed, deliberately:** rate limits and session-scoped caps. BRC-116 §6.5 already places
> prompting policy with the wallet, and neither has an interoperability argument — a wallet can
> enforce both without the application declaring anything.

⚠️ **Status: drafted, not submitted.** Submitting is an outward-facing act on a public standards
repo under the Hodos name and is the owner's call, not mine. Prerequisite before submitting: at least
one BRC-100 app other than bitgenius publishing a `groupPermissions` manifest, so the proposal is
evidence-backed rather than a lone implementer's preference.

### What this phase did instead, and why it is sufficient for beta.3

Displayed, never converted. The connect modal shows *"This site declares a monthly allowance of
10,000 satoshis / month. Hodos does not enforce monthly limits — the per-transaction and per-session
limits below are what will actually apply."* That is honest about the gap in both directions, needs
no exchange-rate conversion (which moves), and keeps `R-CAPS` literally true.

---

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

- The *"app recommends these settings"* modal → `DEFERRED_recommendations_modal.md` (no target release; blocked on a real site declaring `spendingAuthorization`).
- Any change to the decision engine.
- Chromium's Local Network Access prompt branding → Phase 0.9.

## 11. Related

- `DEFERRED_recommendations_modal.md` — the one item this phase deliberately did not build.
  ⚠️ `../../TICKET_brc73_group_permissions_manifest.md` was **archived** on 2026-08-22 when this
  phase closed: its spec citations and adoption survey are §3 above (one home per fact), and its
  deferred modal is the file named on the line above.
- `bitcoin-sv/BRCs` — `wallet/0073.md`, `wallet/0116.md`.
- `test-fixtures/manifest-dapp/` — the existing HTTPS connect-bundle fixture and its constraints.
- `demos/README.md` — where runnable fixtures live and why.
