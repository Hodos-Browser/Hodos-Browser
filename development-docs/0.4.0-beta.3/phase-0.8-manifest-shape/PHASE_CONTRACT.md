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
- [ ] Fixtures checked in (§6) and driven by both test suites, each with a negative control.

## 6. Test JSON fixtures

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

## 7. Evidence table

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

⚠️ **Subject-correctness, twice bitten already this sprint.** `A3` must assert *which modal opened*,
not a parse count — a count assertion passes the instant the parser changes at all, whether or not
the prompt improved. `A1` must be driven through the C++ parse, because that is what the log line
measures.

**Live acceptance:** bitgenius.net end to end, from an unapproved state (revoke via right-click →
Manage Site Permissions first, or the domain is already trusted and nothing will be fetched).
⚠️ `test-fixtures/manifest-dapp/README.md` records the constraint: the manifest fetch is **HTTPS-only
for bare hosts**, so a localhost demo cannot exercise the real fetch path — the fixtures test the
parsers, bitgenius tests the flow.

## 8. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-PERIM` | Privacy-perimeter gates re-evaluate after a connect approval | `HttpRequestInterceptor.cpp` deliberately does **not** propagate `X-User-Approved` for `domain_approval` / `manifest_connect_bundle` so the kind gates run fresh; its comment calls propagating it a privacy-perimeter bypass. A richer bundle must not start granting those. |
| `R-CAPS` | A site can never set its own spending caps | `A5`. BRC-73's `amount` is **monthly satoshis**; ours are **per-tx / per-session USD cents** — neither unit nor period matches, so it is displayed, never written. |
| `R-CONNECT` | An unknown domain still gets a working connect flow | The fallback must not become a dead end. |
| `R-NOFETCH` | An approved domain is never re-asked for a manifest | Adding a second fetch location must stay behind the `trust == "approved"` short-circuit. |

## 9. Out of scope

- The *"app recommends these settings"* modal → `../../TICKET_brc73_group_permissions_manifest.md` §5.
- Any change to the decision engine.
- Chromium's Local Network Access prompt branding → Phase 0.9.

## 10. Related

- `../../TICKET_brc73_group_permissions_manifest.md` — spec citations, full survey, deferred modal.
- `bitcoin-sv/BRCs` — `wallet/0073.md`, `wallet/0116.md`.
- `test-fixtures/manifest-dapp/` — the existing HTTPS connect-bundle fixture and its constraints.
- `demos/README.md` — where runnable fixtures live and why.
