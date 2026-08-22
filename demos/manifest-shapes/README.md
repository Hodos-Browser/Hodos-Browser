# manifest-shapes — wallet manifest fixtures

Fixtures for **beta.3 Phase 0.8** (`development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/`).

These are the **canonical copy** — one home, not duplicated into a test folder. Both test suites
read these same files:

- `rust-wallet/src/manifest.rs` — `include_str!`, so **deleting a fixture breaks the build**
- `cef-native/tests/manifest_fetcher_test.cpp` — read from disk via `HODOS_MANIFEST_FIXTURE_DIR`,
  set in `cef-native/tests/CMakeLists.txt`. ⛔ A missing fixture **fails and names the path**; it
  never skips. (That guard earned its keep on 2026-08-22: a wrong relative path in a throwaway
  probe made one assertion pass *vacuously* on an empty string, and this is what caught it.)

Both parsers re-parse the same bytes at runtime (Rust fetches and embeds the manifest in the 202;
C++ re-parses it to build the modal), so **a fix in one layer with a test driven by the other proves
nothing.** Drive every fixture through both.

## Serving them

Static files — same pattern as `../qr-codes/`:

```
npx serve demos/manifest-shapes
```

⚠️ **A localhost server cannot exercise the real fetch path.** `manifest.rs :: manifest_url` is
**HTTPS-only for bare hosts**, by design — manifests must not be served in plaintext. See
`test-fixtures/manifest-dapp/README.md` for the HTTPS-host requirement. These fixtures test the
**parsers**; `https://bitgenius.net/app` tests the **flow**.

## The fixtures

| File | What it is | Expected |
|---|---|---|
| `bitgenius-live-capture.json` | The real BitGenius manifest, captured 2026-08-22 (3358 bytes). Publishes `metanet` **and** legacy `babbage` with identical content. | 4 protocols. The acceptance case, checked in so the test does not depend on a live third-party site. |
| `brc73-metanet-protocols.json` | The standard shape: `metanet.groupPermissions.protocolPermissions`. Mixes a Level-1 declaration with **no** `counterparty` and Level-2 declarations that name one — both are legal per BRC-73. | 4 protocols, itemised connect modal. |
| `brc73-babbage-legacy.json` | Deprecated `babbage` namespace only. | 2 protocols. Must still parse, and log a deprecation warning (BRC-116 Backwards Compatibility). |
| `brc73-all-categories.json` | All four BRC-73 categories, including `spendingAuthorization`. **No surveyed site serves this** — it is the future-use-case guard. | Parses all four. ⛔ The declared spend must **never** reach `domain_permissions` (`P0.8-A5`). |
| `brc73-both-namespaces.json` | `metanet` and `babbage` present with **different** content. | `metanet` wins. If the modal shows "STALE legacy protocol", precedence is wrong (`P0.8-A8`). |
| `hodos-legacy-permissions.json` | Our own non-standard top-level `permissions` shape (`PERMISSION_UX_DESIGN.md` §5). Zero adopters in the survey, but it is ours. | 1 protocol, 1 basket, `$100/tx` + `$1000/session`. ⚠️ **This yielded 0 protocols before Phase 0.8** — the parser only read `protocolID:[level,name]` while our own documented example uses the flattened `name` + `securityLevel` spelling, so the "regression guard" was guarding a shape that never worked. Both spellings parse now. |
| `unrecognised-shape.json` | Valid JSON, plain W3C web-app manifest, **no** permission namespace at all. | ⛔ **Not a manifest.** Rust returns `None`, C++ returns `valid=false`, and the interceptor's existing fallback opens plain `domain_approval` (`P0.8-A3`). Pre-fix it returned "valid" and rendered a permission-free itemised bundle — the defect the phase exists to fix. |
| `not-a-manifest.html` | An SPA catch-all page. **Four of ten** surveyed BSV sites return exactly this, with **HTTP 200**, for both manifest paths. | No manifest. ⛔ `200` does not mean "manifest" (`P0.8-A7`). |

## Why these shapes

**BRC-73, "Group Permissions for App Access"** — *"The current interoperable manifest namespace is
`metanet.groupPermissions`. Wallets may continue to read the legacy `babbage.groupPermissions`
namespace for backwards compatibility, but it is deprecated."* Categories: `protocolPermissions`,
`spendingAuthorization`, `basketAccess`, `certificateAccess`. Apps SHOULD serve a W3C web-app
manifest at `https://{originator}/manifest.json` — BRC-73 rides **inside** that document, it does not
replace it, which is why every fixture here is a valid web-app manifest too.

**BRC-116, "Wallet Permissions and Counterparty Trust"** is the authoritative *lifecycle* spec;
BRC-73 is the schema and defers to it.

⚠️ `spendingAuthorization.amount` is **monthly satoshis**. Our caps are **per-transaction** and
**per-session USD cents**. Neither the unit nor the period matches, which is why a site's declared
figure is shown as information and never written (`R-CAPS` / `P0.8-A5`, asserted in both parsers).

## What "parses" means since Phase 0.8

🚨 A document that declares **no recognised permission** is **not a manifest**, whatever else it
contains. `parse_manifest` returns `None` and `ParseFromJson` returns `valid=false`. Both used to
succeed on any JSON object — `ParseFromJson` said so out loud, *"Mark valid even if some sections
are empty"* — and that is precisely why bitgenius, which declares four protocols, produced a connect
modal itemising zero while the site went on to hold the access. ⛔ Do not relax this back.

## Re-running the adoption survey

`probe_manifests.py` (co-located here) probes ten BSV/BRC-100 properties at both locations and reports which
namespaces and categories each serves. Results as of 2026-08-22 are in
`development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` §3.
