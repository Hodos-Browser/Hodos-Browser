# Support BRC-73 `metanet.groupPermissions` manifests

**Filed 2026-08-22**, split out of beta.3 Phase 0.8 at kickoff.
**Target: beta.4.** ⛔ Not beta.3 — Phase 0.8 closes the *safety* half without needing any of this.

**Owner steer (2026-08-22):** keep our own auto-approve engine; add BRC-73 as an input to it, not a
replacement for it.

## 1. What this is

Our manifest parser reads a **top-level `permissions`** object — our own invention, documented in
`PERMISSION_UX_DESIGN.md` §5. The BSV ecosystem uses **BRC-73 `metanet.groupPermissions`**. A site
using the standard shape parses to zero permissions, which is how the Phase 0.8 defect happened.

Phase 0.8 makes that failure honest (fall back to `domain_approval`). This ticket makes it *work*:
read what the site actually declared and show it.

## 2. The specs — settled, with citations

- **BRC-73, "Group Permissions for App Access"** (`bitcoin-sv/BRCs`, `wallet/0073.md`) — the schema.
  - *"The current interoperable manifest namespace is `metanet.groupPermissions`. Wallets may continue
    to read the legacy `babbage.groupPermissions` namespace for backwards compatibility, but it is
    deprecated."*
  - *"Applications SHOULD serve a W3C web-app manifest at `https://{originator}/manifest.json`."*
  - Four categories: `protocolPermissions`, `spendingAuthorization`, `basketAccess`,
    `certificateAccess`.
  - `protocolPermissions[]`: `protocolID` (BRC-43 tuple `[securityLevel, protocolName]`),
    `counterparty` (required for specific Level 2; MAY be omitted at Level 1), `description`.
  - `spendingAuthorization`: `amount` — *"the authorized **monthly** spend limit in **satoshis**"* —
    plus `description`. (`duration` is deprecated.)
  - `basketAccess[]`: `basket`, `description`.
  - `certificateAccess[]`: `type`, `fields`, `verifierPublicKey`, `description`.
- **BRC-116, "Wallet Permissions and Counterparty Trust"** (`wallet/0116.md`) — the authoritative
  *lifecycle* spec (fetch, prompt routing, persistence, renewal, revocation, counterparty trust).
  BRC-73 explicitly defers to it and is *"not a complete permission-system specification by itself."*

They are different layers, not alternatives. This closes the `reference_brc116_manifest_research`
TODO open since 2026-06-09.

## 3. Adoption — MEASURED 2026-08-22

Ten hand-picked BSV/BRC-100 properties, both locations, one-shot GETs:

| Domain | `/manifest.json` | `/.well-known/wallet-manifest.json` | Content |
|---|---|---|---|
| bitgenius.net | 200 | 200 | `metanet.groupPermissions[protocolPermissions=4]` **and** `babbage.groupPermissions[protocolPermissions=4]` |
| socialcert.net | 200 | 200 | `babbage` key present, **no** `groupPermissions` |
| projectbabbage.com | 200 | 200 | not JSON (SPA catch-all → HTML) |
| coolcert.babbage.systems | 200 | 200 | not JSON |
| peerpay.babbage.systems | 200 | 200 | not JSON |
| messagebox.babbage.systems | 401 | 401 | — |
| now.bsvblockchain.tech | 404 | 404 | — |
| 1sat.market | 404 | 404 | — |
| metanetapps.com | 404 | 404 | — |
| toolbelt.babbage.systems | DNS fail | DNS fail | dead |

⚠️ Small, hand-picked, one-shot sample — biased *toward* likely adopters, so real-world adoption is
probably lower, not higher. Treat as a signal, not a survey.

**What it tells us:**

1. **1 of 10 serves a BRC-73 manifest**, and it serves it at **both** paths.
2. **No site uses `spendingAuthorization`, `basketAccess` or `certificateAccess`.** Only
   `protocolPermissions` appears in the wild.
3. **No site uses our top-level `permissions` shape.** Our invention has zero observed adopters.
4. bitgenius publishes **both** `metanet` and `babbage` with identical content — belt-and-braces for
   older wallets. Reading only `metanet` is sufficient today; reading `babbage` as a fallback is cheap.
5. ⛔ **`200` does not mean "manifest."** Several SPAs return `200 text/html` for unknown paths. Any
   fetch must parse-and-reject, never trust the status code. Our parser already returns `None` for
   non-JSON — keep that property.

## 4. Scope

**In:**
1. Parse `metanet.groupPermissions` (+ legacy `babbage.groupPermissions`) into the existing
   `Manifest` struct — a **translation layer**, so the engine is untouched. Both layers move
   together: `rust-wallet/src/manifest.rs :: parse_manifest` and
   `cef-native/src/core/ManifestFetcher.cpp :: ParseFromJson`.
2. Keep the legacy top-level `permissions` shape as a fallback (zero observed adopters, but it is
   ours and costs nothing to retain).
3. Add `/manifest.json` as a fetch location alongside `/.well-known/wallet-manifest.json`.
   ⚠️ Cost/benefit: our only known adopter serves both, so this buys nothing *today* — but
   `/manifest.json` is the spec location, so future adopters will use it and only it. Sequential
   fallback, both under the existing 3 s / 64 KB caps, only on the unknown-domain path.
4. Map BRC-43 `protocolID` tuples into whatever the connect modal displays, with the site's
   `description` shown verbatim as the site's own words.

**Deferred, deliberately** — see §5:
5. The "App ABC recommends these settings — accept, or adjust yourself" modal.

**Out:** any change to the decision engine itself. BRC-73 is an *input*.

## 5. The recommended-settings modal — why it waits

Owner's requirement (2026-08-22): *the user definitely needs to see and approve this themselves* —
a modal saying the app recommends these settings, accept or adjust manually.

Agreed as a design, but **no site in the sample declares `spendingAuthorization` at all**, so there
is nothing to recommend yet. Building it now means designing a modal against zero real inputs.

Two mismatches to resolve whenever it is built:

- **Units and period.** BRC-73's `amount` is *monthly satoshis*. Ours are *per-transaction* and
  *per-session* **USD cents**. Neither the unit nor the period lines up, and satoshis→USD moves.
- **Direction of trust.** A site's declared figure must never pre-fill our caps.

⭐ **Recommended resolution:** show the site's declared figure as **information** beside our own
defaults, and let the user set ours. That keeps *"a site can never set its own caps"* literally true,
sidesteps the conversion entirely, and still tells the user what the app is asking for. Phase 0.8's
`P0.8-A5` asserts the invariant this depends on.

## 6. Test plan

Inherits the two evidence rows moved out of Phase 0.8:

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT |
|---|---|---|---|
| `A1` | bitgenius.net's real manifest parses to **4 protocols** | ⛔ Pre-fix **0** — measured 2026-08-21 | The `📦 Triggering manifest_connect_bundle … (N protocols…)` log line |
| `A2` | The modal **displays** those four with their descriptions | ⛔ Pre-fix it displays none | The rendered overlay, not the parse count |
| `A6` | A `/manifest.json`-only site is found | ⛔ Remove the second location → not found | A fixture served at `/manifest.json` only |
| `A7` | An SPA returning `200 text/html` yields **no** manifest | ⛔ Trust the status code → HTML treated as a manifest | Any of the four SPA domains in §3 |

⛔ Negative control per parser and per layer: revert the shape support and the bitgenius fixture must
report 0 again. Check both Rust and C++ — a fix in one with a green test driven by the other proves
nothing, and the count in the log line comes from the **C++** parse of the bytes Rust embeds.

Use the real bitgenius manifest as a checked-in fixture (3358 bytes) so the test does not depend on a
live site.

## 7. Related

- `0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` — the safety half; do first.
- `bitcoin-sv/BRCs` — `wallet/0073.md`, `wallet/0116.md`.
- `PERMISSION_UX_DESIGN.md` §5 — where our non-standard shape is documented.
- Probe script: `scratchpad/probe_manifests.py` (re-runnable; ten candidates, both paths).
