# Phase 0.8 — a manifest we cannot read must not look like an itemised grant

**Opened 2026-08-21.** Owner-requested after testing `https://bitgenius.net/app`.
**Re-scoped 2026-08-22** at kickoff, after the canonical manifest shape was settled and real-world
adoption was measured. See §3 — the BRC-73 parser work moved out to
`../../TICKET_brc73_group_permissions_manifest.md` (beta.4).

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

Two lenient parsers each "succeed" on a manifest they did not understand:

- `rust-wallet/src/manifest.rs :: parse_manifest` returns `Some` for **any** valid JSON object. It
  returns `None` only on malformed JSON or a non-object. bitgenius → `Some` with 0 protocols →
  `manifest_present = true` (`permission_service/request_gate.rs :: domain_trust_gate`) → the engine
  chooses the bundle prompt.
- `cef-native/src/core/ManifestFetcher.cpp :: ParseFromJson` sets `m.valid = true` explicitly
  *"even if some sections are empty"*.

⭐ **The fallback this phase was going to build already exists.** `HttpRequestInterceptor.cpp`, in the
`manifest_connect_bundle` arm of `tryHandlePendingResponse`:

```cpp
if (manifest.valid) { newRequestId = openManifestConnectBundleModal(...); }
else { LOG_WARNING_HTTP("… unparseable manifest … falling back to domain_approval");
       newRequestId = openDomainApprovalModal(modalCtx, resume); }
```

It never fires, because `valid` is true for a manifest with zero permissions. **This phase is a
tightening of what "valid"/`Some` mean, not new machinery.**

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

## 3. Scope decision — 2026-08-22

**The safety fix does not require BRC-73.** The harm is that the user approves a dialog which
itemises nothing while the site gets protocol access. If an unrecognised manifest falls back to plain
`domain_approval`, the user sees an honest prompt that does not imply a reviewed itemised grant.
Harm closed. Parsing BRC-73 makes the modal *better* — informed, itemised consent — it does not make
it *safe*. The fallback does.

So this phase keeps the safety half and beta.3 stays shippable. **`P0.8-A1` and `P0.8-A2` move to
`../../TICKET_brc73_group_permissions_manifest.md`**, which also carries the spec citations, the
measured adoption survey, and the owner's "app recommends these settings" modal requirement.

### What the kickoff settled (recorded here so it is not re-derived)

- **Canonical shape: BRC-73, "Group Permissions for App Access"** — *"The current interoperable
  manifest namespace is `metanet.groupPermissions`. Wallets may continue to read the legacy
  `babbage.groupPermissions` namespace for backwards compatibility, but it is deprecated."*
  Categories: `protocolPermissions`, `spendingAuthorization`, `basketAccess`, `certificateAccess`.
- **BRC-116, "Wallet Permissions and Counterparty Trust"** is the authoritative *lifecycle* spec;
  BRC-73 defers to it and is explicitly *"the grouped-permission schema, not a complete
  permission-system specification by itself."* They are different layers, not alternatives. This
  closes the `reference_brc116_manifest_research` TODO open since 2026-06-09.
- **Our top-level `permissions` shape appears in neither spec.** It is our own invention from
  `PERMISSION_UX_DESIGN.md` §5, and the adoption probe found **no site using it**.
- **We already only fetch for unknown domains** — `request_gate.rs :: domain_trust_gate` short-circuits
  `if trust == "approved" { return Proceed }` with the comment *"Short-circuit so we never fetch a
  manifest for an already-trusted domain."* The owner's "don't ask every visit" requirement already
  holds; nothing to build.

## 4. Done means

- [ ] A manifest whose permissions we cannot parse **must not render as a permission-free connect**.
      Fall back to plain `domain_approval` — which the code already does once `valid` means what it
      should. Both parsers move together (Rust `parse_manifest`, C++ `ParseFromJson`).
- [ ] Duplicate connect prompts for one gesture are coalesced to one — **after** the race is measured.
- [ ] ⛔ Confirm what a manifest-connect grant actually **writes** to `domain_permissions` — whether
      the row inherits the user's safe defaults ($1 per-tx / $10 per-session) or anything the
      manifest declared. A site must never set its own caps through a manifest. **Not yet measured.**
      If it already cannot, this is a regression test rather than a fix.
- [ ] Unit test per parser, with a **negative control**: revert the tightening and the
      zero-permission fixture must render as a bundle again.

## 5. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| `P0.8-A3` | A manifest with no permissions we recognise renders **`domain_approval`**, never an empty bundle | ⛔ Pre-fix bitgenius renders a permission-free `manifest_connect_bundle` — measured 2026-08-21 | Which modal opens, **and** the `📦 Triggering manifest_connect_bundle` line being absent | T1 |
| `P0.8-A4` | One click → **one** modal | ⛔ Pre-fix three in 56 ms — measured | Notification-overlay creation count, not the 202 count | T1 |
| `P0.8-A5` | A manifest-declared spending block does **not** raise the stored caps | ⛔ Stub the clamp → the row takes the site's numbers | The `domain_permissions` row after approval | T1 |
| ~~`P0.8-A1`~~ | — | — | **Moved** → `TICKET_brc73_group_permissions_manifest.md` | — |
| ~~`P0.8-A2`~~ | — | — | **Moved** → same ticket | — |

⚠️ **A3 subject-correctness.** Assert the *modal that opens*, not the parse count. A parse-count
assertion passes the moment the parser changes at all, whether or not the user-facing prompt improved
— the same shape of harness that cost this sprint days on farbling.

## 6. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-PERIM` | Privacy-perimeter gates re-evaluate after a connect approval | `HttpRequestInterceptor.cpp` deliberately does **not** propagate `X-User-Approved` for `domain_approval` / `manifest_connect_bundle`, so the kind gates run fresh; its comment calls propagating it a privacy-perimeter bypass. Any change to this arm must preserve that. |
| `R-CAPS` | A site can never set its own spending caps | A5 is the assertion. |
| `R-CONNECT` | An unknown domain still gets a working connect flow | The fallback must not turn into a dead end. |

## 7. Out of scope

- BRC-73 / `groupPermissions` parsing, the `/manifest.json` fetch location, and the "app recommends
  these settings" modal → `../../TICKET_brc73_group_permissions_manifest.md` (beta.4).
- Chromium's Local Network Access prompt branding → Phase 0.9.

## 8. Related

- `../../TICKET_brc73_group_permissions_manifest.md` — the parser work and the adoption survey.
- `cef-native/src/core/ManifestFetcher.cpp` re-parses the bytes Rust embeds in the
  `manifest_connect_bundle` 202 — **both parsers must move together**.
