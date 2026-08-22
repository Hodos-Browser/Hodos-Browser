# BRC-73 manifests — spec reference, adoption survey, and the deferred recommendations modal

**Filed 2026-08-22** out of beta.3 Phase 0.8's kickoff.

⚠️ **Scope changed the same day.** This ticket originally carried the BRC-73 *parser* work for
beta.4. The owner merged that back into **Phase 0.8**, which now does the whole job — see
`0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` §3 for the reasoning.

**What is left here:**
1. §2–§3 — the spec citations and the adoption survey, kept in one place because Phase 0.8, the
   fixtures, and any future manifest work all cite them.
2. §4 — the **"App ABC recommends these settings"** modal, which is genuinely deferred and has no
   target release yet.

**Owner steer (2026-08-22):** keep our own auto-approve engine; BRC-73 is an *input* to it, never a
replacement.

## 2. The specs — settled, with citations

- **BRC-73, "Group Permissions for App Access"** (`bitcoin-sv/BRCs`, `wallet/0073.md`) — the schema.
  - *"The current interoperable manifest namespace is `metanet.groupPermissions`. Wallets may continue
    to read the legacy `babbage.groupPermissions` namespace for backwards compatibility, but it is
    deprecated."*
  - *"Applications SHOULD serve a W3C web-app manifest at `https://{originator}/manifest.json`."*
    BRC-73 rides **inside** that document; it does not replace it.
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

Different layers, not alternatives. This closes the `reference_brc116_manifest_research` TODO open
since 2026-06-09.

**Our top-level `permissions` shape appears in neither spec.** It is our own invention
(`PERMISSION_UX_DESIGN.md` §5).

## 3. Adoption — MEASURED 2026-08-22

Ten hand-picked BSV/BRC-100 properties, both locations, one-shot GETs
(`demos/manifest-shapes/probe_manifests.py`):

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
probably lower, not higher. A signal, not a survey.

**What it tells us:**

1. **1 of 10 serves a BRC-73 manifest**, at **both** paths.
2. **No site uses `spendingAuthorization`, `basketAccess` or `certificateAccess`.** Only
   `protocolPermissions` exists in the wild — which is why §4 is deferred.
3. **No site uses our top-level `permissions` shape.** Zero observed adopters.
4. bitgenius publishes **both** `metanet` and `babbage` with identical content — belt-and-braces for
   older wallets. Reading `metanet` is sufficient today; reading `babbage` as a fallback is cheap.
5. ⛔ **`200` does not mean "manifest."** Several SPAs return `200 text/html` for unknown paths. A
   fetch must parse-and-reject, never trust the status code. Our parser already returns `None` for
   non-JSON — keep that property. Fixture: `demos/manifest-shapes/not-a-manifest.html`.

**Worth re-running** before building §4, or whenever a new BRC-100 app appears. If
`spendingAuthorization` shows up in the wild, §4 acquires a real driver.

## 4. Deferred — the "App ABC recommends these settings" modal

**Owner's requirement (2026-08-22):** *the user definitely needs to see and approve this themselves* —
a modal saying the app recommends these settings, which the user can accept or adjust manually.

Agreed as a design. Deferred because **no site in the survey declares `spendingAuthorization`**, so
there is nothing to recommend. Building a modal against zero real inputs is how you get the wrong
modal. Phase 0.8 parses the category anyway
(`demos/manifest-shapes/brc73-all-categories.json`), so the data will be there the moment it is worth
showing.

Two mismatches to resolve whenever it is built:

- **Units and period.** BRC-73's `amount` is *monthly satoshis*. Ours are *per-transaction* and
  *per-session* **USD cents**. Neither the unit nor the period lines up, and satoshis→USD moves.
- **Direction of trust.** A site's declared figure must never pre-fill our caps.

⭐ **Recommended resolution:** show the site's declared figure as **information** beside our own
defaults, and let the user set ours. That keeps *"a site can never set its own caps"* literally true,
sidesteps the conversion entirely, and still tells the user what the app is asking for. Phase 0.8's
`P0.8-A5` asserts the invariant this rests on.

**Prerequisite before building:** at least one real site declaring `spendingAuthorization`, so the
modal is designed against a real input rather than a hypothetical one.

## 5. Related

- `0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` — where the parser work actually lives.
- `demos/manifest-shapes/` — the fixtures and the re-runnable probe script.
- `bitcoin-sv/BRCs` — `wallet/0073.md`, `wallet/0116.md`.
- `PERMISSION_UX_DESIGN.md` §5 — where our non-standard shape is documented.
- `test-fixtures/manifest-dapp/` — the existing HTTPS connect-bundle fixture and its constraints.
