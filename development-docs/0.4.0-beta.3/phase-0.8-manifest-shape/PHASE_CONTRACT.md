# Phase 0.8 — the connect modal must show what the site actually asked for

**Opened 2026-08-21.** Owner-requested after testing `https://bitgenius.net/app`.

## 1. The defect — MEASURED

Clicking **Continue with Metanet** on bitgenius.net produced, in the browser log:

```
📦 Triggering manifest_connect_bundle for bitgenius.net
   (app=BitGenius, 0 protocols, 0 baskets, 0 certs, 0 counterparties)
🛡️ engine Prompt (domain-trust) … type=ManifestConnectBundle reason=new_domain_with_manifest
```

**Zero protocols — while the site declares four.** `https://bitgenius.net/.well-known/wallet-manifest.json`
carries them at:

```json
"metanet": { "groupPermissions": { "protocolPermissions": [
   {"protocolID":[1,"identity key retrieval"], …},
   {"protocolID":[2,"server hmac"], "counterparty":"self", …},
   {"protocolID":[2,"auth message signature"], "counterparty":"0279887c…", …},
   {"protocolID":[2,"3241645161d8"], "counterparty":"0279887c…", …}   // BRC-29 payment derivation
] } }
```

Our parser reads a **top-level `permissions`** object (`rust-wallet/src/manifest.rs`, `obj.get("permissions")`
→ `protocols` / `baskets` / `certificates` / `spending` / `counterparties`; mirrored in
`cef-native/src/core/ManifestFetcher.cpp :: ParseFromJson`). bitgenius uses the BRC-100 /
wallet-toolbox `metanet.groupPermissions.protocolPermissions` shape. **Shape mismatch ⇒ empty
manifest ⇒ a connect modal listing nothing.**

**Consequence.** The user is shown a manifest-flavoured consent dialog with no itemised
permissions — reading, in the owner's words, as *"allow identity and allow operations without
asking"* with no payment guardrails — and approves it. The site then holds protocol access it was
never shown. `ParseFromJson` is lenient by design (unknown fields ignored, never throws), which is
correct for robustness and **exactly what makes this failure silent.**

⚠️ **Which shape is canonical is an open question, not a settled one.** BRC-116 vs the
wallet-toolbox `groupPermissions` convention — `reference_brc116_manifest_research` in memory is
the prior art. Settle it before writing the parser, or we will support the wrong one twice.

## 2. Second defect, same trace — one click, THREE modals

```
11:06:17.720  approvalId=7850bb41…  🔔 Creating notification overlay (manifest_connect_bundle)
11:06:17.738  approvalId=19437cff…
11:06:17.776  approvalId=36fc4859…
```

Three 202s and three overlays in 56 ms for a single user click, all on `/getVersion`. Stacked
modals are also what the owner had to dismiss by hand.

## 3. Done means

- [ ] Decide the canonical manifest shape (BRC-116 vs `groupPermissions`), **written down with the
      reason**, then parse it. Supporting both is acceptable; guessing is not.
- [ ] A manifest whose permissions we cannot parse **must not render as a permission-free connect**.
      Fail loud: either show "this site's manifest could not be read" or fall back to plain
      `domain_approval`, which at least does not imply an itemised grant was reviewed.
- [ ] Duplicate connect prompts for one gesture are coalesced to one.
- [ ] ⛔ Confirm what a manifest-connect grant actually **writes** to `domain_permissions` — whether
      the row inherits the user's safe defaults ($1 per-tx / $10 per-session) or anything the
      manifest declared. A site must never set its own caps through a manifest. **Not yet measured.**
- [ ] Unit test per shape, with a **negative control**: revert the parser and the fixture must show 0.

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| `P0.8-A1` | bitgenius.net's real manifest parses to **4 protocols** | ⛔ Pre-fix **0** — measured 2026-08-21 | The `📦 Triggering manifest_connect_bundle … (N protocols…)` log line | T1 |
| `P0.8-A2` | The modal **displays** those four with their descriptions | ⛔ Pre-fix it displays none | The rendered overlay, not the parse count | T2 |
| `P0.8-A3` | An unparseable manifest fails **loud**, never as an empty grant | ⛔ Pre-fix it renders a permission-free bundle | Feed a deliberately malformed manifest | T1 |
| `P0.8-A4` | One click → **one** modal | ⛔ Pre-fix three in 56 ms — measured | Notification-overlay creation count | T1 |
| `P0.8-A5` | A manifest-declared `spending` block does **not** raise the stored caps | ⛔ Stub the clamp → the row takes the site's numbers | The `domain_permissions` row after approval | T1 |

## 5. Related

- `cef-native/src/core/ManifestFetcher.cpp` is described in CLAUDE.md as re-parsing the bytes Rust
  embeds in the `manifest_connect_bundle` 202 — **both parsers must move together**.
- Chromium's Local Network Access prompt appears alongside this flow. Branding it is separate work;
  `CEF_PERMISSION_TYPE_LOOPBACK_NETWORK` (`cef_types.h`) makes it interceptable via the
  `CefPermissionHandler` path we already use for camera/mic/location.
