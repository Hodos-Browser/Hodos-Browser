# Certificate UI & Lifecycle — Development Plan

> Issue-41: Certificate management, display, and lifecycle operations
>
> **Related docs:**
> - `Possible-MVP-Features/SOCIALCERT_DEEP_DIVE.md` — SocialCert research, OAuth flow details, certificate conceptual overview
> - `Possible-MVP-Features/CONTENT_SIGNING_AND_TIPPING.md` — Content signing & tipping feature (depends on certificates being fully functional)
> - Certificate debugging notes in memory: `certificate-debugging-2026-03-14.md`, `certificate-lifecycle-research.md`

## Current State

The CertificatesTab is a read-only table showing raw data: base64 type IDs, truncated hex pubkeys, field counts, and serial numbers. No human-readable labels, no actions, no status indicators.

**What works:**
- Certificate acquisition (CoolCert, SocialCert email/X) — fixed in commit `31f569d`
- Local storage in SQLite (`certificates` + `certificate_fields` tables)
- `listCertificates` endpoint returns certs with encrypted fields + keyring
- `proveCertificate` endpoint for selective disclosure
- `relinquishCertificate` endpoint for soft delete
- Identity resolution via overlay (`identity_resolver.rs`) for incoming certs

**What's missing:**
- Human-readable display (cert type names, certifier names, field values)
- Certificate actions (delete, publish/unpublish, prove)
- Status indicators (valid/revoked/expired, public/private)
- Overlay integration for public visibility

---

## Phase 1: Human-Readable Display

### 1.1 Certificate Type Mapping

Map base64 type IDs to human-readable names. Add to frontend as a constant:

| Type Base64 | Display Name | Icon |
|-------------|-------------|------|
| `vdDWvftf1H+5+ZprUw123kjHlywH+v20aPQTuXgMpNc=` | X (Twitter) | X logo or 𝕏 |
| `exOl3KM0dIJ04EW5pZgbZmPag6MdJXd3/a1enmUU/BA=` | Email | ✉ |
| `2TgqRC35B1zehGmB21xveZNc7i5iqHc0uxMb+1NMPW4=` | Discord | Discord icon |
| `z40BOInXkI8m7f/wBrv4MJ09bZfzZbTj2fJqCtONqCY=` | Government ID | 🪪 |
| `YoPsbfR6YQczjzPdHCoGC7nJsOdPQR50+SYqcWpJ0y0=` | Registrant | 🏢 |
| `AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=` | CoolCert | ✅ |
| Unknown | "Certificate" | 📜 |

### 1.2 Certifier Name Mapping

Map certifier public keys to human-readable names:

| Certifier PubKey | Display Name |
|-----------------|-------------|
| `02cf6cdf466951d8...` | SocialCert (p2ppsr) |
| `03daf815fe38f83d...` | Metanet Trust |
| `0220529dc803041a...` | CoolCert (p2ppsr) |
| Unknown | Truncated pubkey with copy button |

**Source:** These could come from the `manifest.json` `babbage.trust` field that certifier websites publish, or hardcoded initially.

### 1.3 Decrypted Field Display

Currently fields are stored encrypted. To display human-readable values, we need to decrypt them using the masterKeyring.

**Backend change needed:** New endpoint `POST /listCertificatesDecrypted` or add `decrypt=true` query param to existing endpoint. The wallet backend has the private key and masterKeyring — it can decrypt fields before returning.

**Fields to display per cert type:**
- X: `userName` (handle), `profilePhoto` (avatar URL)
- Email: `email` address
- Discord: `discordUsername`, `discordId`
- CoolCert: `cool` ("true")

**UI:** Show primary field (handle/email) inline. Expandable row or dropdown for all fields with info tooltips.

### 1.4 Revised Table Columns

| Column | Content | Notes |
|--------|---------|-------|
| **Type** | Icon + human-readable name (e.g. "𝕏 Twitter") | Mapped from base64 |
| **Identity** | Primary field value (e.g. "@bsvarchie", "user@email.com") | Decrypted from fields |
| **Certifier** | Human-readable name (e.g. "SocialCert") | Mapped from pubkey |
| **Status** | Badge: "Active" / "Revoked" / "Public" / "Private" | Revocation check + overlay check |
| **Actions** | Dropdown or icon buttons | Details, Prove, Publish/Unpublish, Delete |

### 1.5 Info Tooltips

Hoverable (ℹ) icons next to:
- **Type** — "Certificate type identifier. Determines what identity attributes this certificate proves."
- **Certifier** — "The organization that verified your identity and signed this certificate. Full key: {pubkey}"
- **Serial Number** — "Unique identifier for this specific certificate instance."
- **Status** — "Active = not revoked. Public = published to BSV overlay for others to discover."

---

## Phase 2: Certificate Actions

### 2.1 View Details

Expandable row or modal showing:
- All decrypted field values (name/value pairs)
- Full certifier pubkey (with copy button)
- Full serial number
- Revocation outpoint (with WhatsOnChain link)
- Signature (truncated, with copy)
- Issued date (from `created_at`)
- Raw certificate JSON (developer toggle)

### 2.2 Delete Certificate

**CRITICAL: Never delete locally if the cert is still publicly visible.**

**Correct flow:**
1. User clicks Delete on a certificate
2. Check if certificate is published on-chain (is the PushDrop UTXO unspent?)
3. **If published:**
   a. Spend the PushDrop UTXO to remove from overlay (unpublish)
   b. If unpublish fails → DO NOT delete → show error: "Certificate is still publicly visible. Unable to remove from public overlay. Try again later."
   c. If unpublish succeeds → proceed to step 4
4. **If not published (or successfully unpublished):**
   a. Try to contact certifier to request revocation (if endpoint exists)
   b. If certifier unreachable → warn: "Certifier ABC could not be reached. Certificate removed from wallet but the certifier may still have records."
   c. Delete from local DB (hard delete, not soft delete — data is gone)
5. Show success/error feedback
6. Refresh certificate list

**Current state of certifiers:**
- SocialCert: revocation endpoint DISABLED ("Route not supported!")
- CoolCert: revocation not implemented
- For now, we can only unpublish (spend PushDrop) and delete locally

**On-chain status of our test certs:**
- SocialCert X cert: HAS a real revocation outpoint on-chain (txid `1a526a71...bb`), but NOT on the overlay (empty results from `ls_identity`). The on-chain tx contains PushDrop-encoded cert data.
- Local test cert: placeholder outpoint, not on-chain
- CoolCert cert: placeholder outpoint, not on-chain

### 2.3 Selective Disclosure (Prove Certificate)

**What it does:** Creates a verifier-specific keyring that lets a third party decrypt specific fields.

**UI flow:**
1. User clicks "Prove" on a certificate
2. Modal: "Share this certificate with a verifier"
3. Input: Verifier's public key (or identity key)
4. Checkboxes: Which fields to reveal (e.g. ☑ userName ☐ profilePhoto)
5. Call `POST /proveCertificate`
6. Display the verifier keyring (JSON or base64) for sharing
7. Copy button

**Note:** This is an advanced feature. May want to defer to Phase 3.

---

## Phase 3: Public/Private Visibility — RESEARCH COMPLETE (2026-03-16)

### 3.1 Research Findings

All research questions answered via SDK source analysis (`IdentityClient.ts`, `SHIPBroadcaster.ts`, `PushDrop.ts`):

1. **How does a certificate become public?**
   - SDK's `IdentityClient.publiclyRevealAttributes()` does this:
     a. Call `proveCertificate()` with verifier = `PrivateKey(1).toPublicKey()` (the "anyone" key) to create a public keyring
     b. Build PushDrop script: `fields = [JSON.stringify({...certificate, keyring: keyringForVerifier})]`
     c. Lock with `protocolID: [1, "identity"]`, `keyID: "1"`, counterparty: `"anyone"`, `includeSignature: true`
     d. `createAction()` to build+sign the transaction (1 sat output)
     e. Submit to overlay via `TopicBroadcaster(['tm_identity'])` — NOT just blockchain broadcast

2. **Who pays for the transaction?**
   - The wallet owner (subject) pays the mining fee
   - Output amount: **1 satoshi** (SDK `tokenAmount: 1`) — NOT 600 sats
   - Total cost: 1 sat (output) + ~200 sats (fee) = ~$0.0001

3. **How do we check if a cert is public?**
   - Query overlay: `POST /lookup` with `{ service: "ls_identity", query: { identityKey, certifiers } }` (already in `identity_resolver.rs`)
   - Can also query by `serialNumber` for exact match

4. **How do we unpublish (make private)?**
   - SDK's `IdentityClient.revokeCertificateRevelation()`:
     a. Query overlay `ls_identity` by `serialNumber` to find the published UTXO BEEF
     b. `createAction()` with the published UTXO as input (spend it, no new outputs)
     c. `PushDrop.unlock()` to sign the spending input (same protocolID/keyID/counterparty)
     d. Submit spending tx to overlay via `TopicBroadcaster(['tm_identity'])` — overlay removes the output

5. **Do certificates expire?**
   - BRC-52 spec doesn't define expiration
   - Revocation (spending the revocation UTXO) is the mechanism for invalidation

6. **What overlay services/endpoints exist?**
   - **Lookup**: `POST {host}/lookup` — query for certificates (already used in `identity_resolver.rs`)
   - **Submit**: `POST {host}/submit` — submit BEEF transactions to overlay
     - Content-Type: `application/octet-stream`
     - `X-Topics` header: `["tm_identity"]`
     - Body: raw BEEF bytes
     - Returns: STEAK (admittance instructions per topic)
   - **Hosts**: US `https://overlay-us-1.bsvb.tech`, EU `https://overlay-eu-1.bsvb.tech`
   - **Topic**: `tm_identity` (all topics must start with `tm_`)
   - Host discovery available via SHIP lookup (`ls_ship` service), but hardcode known hosts initially

### 3.2 Key Technical Details

**PushDrop field content for publishing:**
```json
// Single field[0] = JSON string of:
{
  "type": "vdDWvftf1H+5+...",
  "serialNumber": "abc123...",
  "subject": "02abc...",
  "certifier": "02def...",
  "revocationOutpoint": "txid.0",
  "fields": { "userName": "<base64 encrypted>", ... },
  "signature": "3045...",
  "keyring": {
    "userName": "<base64 anyone-key-encrypted revelation key>",
    "profilePhoto": "<base64 anyone-key-encrypted revelation key>"
  }
}
```

**PushDrop locking key derivation:**
- `protocolID: [1, "identity"]` → BRC-43 invoice: `"1-identity-1"`
- `keyID: "1"`
- `counterparty: "anyone"` → `PrivateKey(1).toPublicKey()` = `0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798`
- Lock position: `Before` (pubkey + OP_CHECKSIG before fields)

> **CRITICAL — BRC-42 derivation direction for PushDrop locking:**
>
> The PushDrop output is locked to **OUR** derived child public key, so **WE** can spend it
> later to unpublish. Despite the "anyone" counterparty name, we are spending to ourselves.
> The "anyone" counterparty only means the *public keyring* (for field decryption) is
> derivable by anyone — the PushDrop UTXO itself is only spendable by us.
>
> BRC-42's `derive_child_public_key(sender_priv, recipient_pub, invoice)` returns the
> **recipient's** child pubkey. So to get OUR child pubkey:
>
> ```
> // CORRECT — locks to OUR child pubkey (we are the "recipient"):
> locking_pubkey = derive_child_public_key(anyone_privkey, master_pubkey, "1-identity-1")
>
> // WRONG — locks to ANYONE's child pubkey (unspendable by us!):
> locking_pubkey = derive_child_public_key(master_privkey, anyone_pubkey, "1-identity-1")
> ```
>
> To spend (unpublish), derive the matching private key:
> ```
> child_privkey = derive_child_private_key(master_privkey, anyone_pubkey, "1-identity-1")
> ```
>
> These match because ECDH is symmetric: `ECDH(anyone_priv, master_pub) == ECDH(master_priv, anyone_pub)`
>
> **Bug history:** First publish attempt used the WRONG derivation (locked to anyone's child
> pubkey). Result: 1-sat UTXO permanently unspendable. The overlay rejected it (bad BEEF
> format), so it was never publicly visible. Fixed 2026-03-16.

**Overlay submission flow (CORRECTED 2026-03-17):**
1. Build and sign the PushDrop transaction via `create_action_internal` (handles UTXO selection, change, BEEF ancestry)
2. createAction returns Atomic BEEF (BRC-95): `[01010101][32-byte txid][BEEF V1 bytes]`
3. Strip 36-byte Atomic header → plain BEEF V1 (`0100beef`) for overlay submission
4. Broadcast to BSV network via ARC (done by createAction internally)
5. POST plain BEEF V1 bytes to overlay (`{host}/submit`, `X-Topics: ["tm_identity"]`), US primary → EU fallback
6. Check STEAK response: `outputsToAdmit: [0]` = success, `outputsToAdmit: []` = rejected
7. **Known issue (2026-03-17):** Overlay returns `outputsToAdmit: []` — root cause under investigation (see Issues checklist)
8. **Known bug:** STEAK parsing treats empty outputsToAdmit as success — must fix before retesting

**SDK reference flow** (`IdentityClient.publiclyRevealAttributes()`):
1. `createAction()` — builds+signs PushDrop tx (default options, no noSend)
2. `TopicBroadcaster(['tm_identity']).broadcast(Transaction.fromAtomicBEEF(tx))` — submits to overlay
3. Note: SDK does NOT broadcast to ARC separately — `createAction` with default `acceptDelayedBroadcast: true` means wallet returns BEEF to caller, and `TopicBroadcaster` submits to overlay which handles miner broadcast. Our wallet behaves differently (broadcasts to ARC directly via `acceptDelayedBroadcast: false`).

**Unpublish = spend the PushDrop UTXO:**
1. The published output is locked to our derived child key (P2PK: `<our_child_pubkey> OP_CHECKSIG`)
2. To spend: derive our child private key (BRC-42, same invoice/counterparty), sign the input
3. Unlocking script is just `<signature>` — NOT `<signature> <pubkey>` (P2PK, not P2PKH)
4. Transaction needs a funding UTXO + change output (BSV requires >= 1 output, 1 sat alone can't cover fee)
5. Submit spending tx to overlay — overlay sees the output was spent and removes it

**PushDrop signing pattern (P2PK vs P2PKH):**
- Regular UTXOs use P2PKH: locking = `OP_DUP OP_HASH160 <hash> OP_EQUALVERIFY OP_CHECKSIG`, unlocking = `<sig> <pubkey>`
- PushDrop outputs use P2PK: locking = `<pubkey> OP_CHECKSIG [fields] [DROP]`, unlocking = `<sig>`
- When spending PushDrop for unpublish, we need the P2PK signing pattern, not P2PKH

**Failure rollback and publish states:**
- `publish_status` column values: `unpublished` (default) → `broadcast` (tx on-chain) → `published` (overlay confirmed)
- If broadcast succeeds but overlay fails → status stays `broadcast`, can retry overlay submit later
- If overlay succeeds but DB update fails → on next listCertificates, re-check overlay to reconcile
- Overlay submission is best-effort with retry — doesn't block the user

**Output tracking for unpublish:**
- The PushDrop output MUST be inserted into `outputs` table with:
  - `txid`, `vout=0`, `satoshis=1`, `locking_script` (the PushDrop script bytes)
  - `derivation_prefix = "1-identity"`, `derivation_suffix = "1"` (matching the BRC-43 invoice)
  - `spendable = 1` (so unpublish can find and spend it)
  - `basket` = "identity" or similar (to distinguish from regular UTXOs)
- On unpublish: verify output still unspent on-chain before attempting to spend

---

## Sprint Plan

### Sprint 1: Display — COMPLETE
- [x] Add type/certifier name constants to frontend
- [x] Add decrypted fields to `listCertificates` backend response
- [x] Redesign CertificatesTab with new columns
- [x] Add expandable row for field details

### Sprint 2: Delete Flow — COMPLETE
- [x] Delete button with confirmation modal
- [x] Overlay publish check before allowing delete
- [x] `relinquishCertificate` blocks if published (returns 409 Conflict)

### Sprint 3: Publish/Unpublish — BLOCKED (2026-03-17, see Issues & Fix Checklist below)

**3-pre. Verification tests** (do first — catch issues before building)
- [ ] Verify "anyone" public key: `PrivateKey(0x01).to_public_key()` = `0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798`
- [ ] Verify BRC-42 derivation with anyone counterparty produces valid key pair
- [ ] Unit test: encode PushDrop → sign with derived key → decode → verify signature (P2PK pattern)
- [ ] Unit test: P2PK unlocking script (`<sig>` only, no pubkey) against PushDrop locking script

**3a. Database Migration V12** — Add publish tracking columns
- [ ] `publish_status` TEXT DEFAULT 'unpublished' (values: `unpublished`, `broadcast`, `published`)
- [ ] `publish_txid` TEXT (txid of PushDrop tx)
- [ ] `publish_vout` INTEGER DEFAULT 0 (output index)
- [ ] CertificateRepository: `update_publish_status()`, `get_publish_info()`

**3b. POST /wallet/certificate/publish** — Publish cert to overlay
- [ ] Look up certificate by type+serial+certifier
- [ ] Check not already published (reject if `publish_status != 'unpublished'`)
- [ ] Call internal `proveCertificate` with anyone key → get public keyring
- [ ] Build cert JSON with public keyring included (matching SDK format)
- [ ] Derive PushDrop locking key: BRC-42 with `invoice: "1-identity-1"`, counterparty = anyone pubkey
- [ ] Encode PushDrop: `fields = [cert_json_with_keyring]`, locked to derived pubkey, `LockPosition::Before`
- [ ] Build transaction: select UTXOs, 1 sat PushDrop output + change output, sign P2PKH inputs
- [ ] Build Atomic BEEF from signed tx (BEFORE broadcast — need ancestry proofs)
- [ ] Broadcast to BSV network via ARC
- [ ] Update DB: `publish_status='broadcast'`, `publish_txid`, `publish_vout=0`
- [ ] Insert PushDrop output into `outputs` table (derivation_prefix=`"1-identity"`, suffix=`"1"`, spendable=1)
- [ ] Submit BEEF to overlay (`POST {host}/submit`, `X-Topics: ["tm_identity"]`), US → EU fallback
- [ ] On overlay success: update DB `publish_status='published'`
- [ ] On overlay failure: log warning, keep `broadcast` status (can retry later)

**3c. POST /wallet/certificate/unpublish** — Remove cert from overlay
- [ ] Look up certificate, verify published (`publish_status in ('broadcast', 'published')`)
- [ ] Get `publish_txid`/`publish_vout` from DB
- [ ] Verify output still unspent on-chain (WoC outspend check)
- [ ] Derive same private key (BRC-42, `"1-identity-1"`, anyone counterparty)
- [ ] Build spending transaction: input = published UTXO, P2PK unlocking (`<sig>` only)
- [ ] Build Atomic BEEF from spending tx
- [ ] Broadcast spending tx to BSV network
- [ ] Submit BEEF to overlay (`POST {host}/submit`, `X-Topics: ["tm_identity"]`), US → EU fallback
- [ ] Update DB: `publish_status='unpublished'`, clear `publish_txid`/`publish_vout`
- [ ] Mark PushDrop output as spent in `outputs` table

**3d. Frontend — Publish/Unpublish buttons**
- [ ] Add `publish_status` to `listCertificates` response
- [ ] Show "Publish" button for unpublished certs
- [ ] Show "Unpublish" button for published/broadcast certs
- [ ] Confirmation dialog with cost estimate (~200 sats fee)
- [ ] Loading state + disable button during transaction (prevent double-click)
- [ ] Success/error feedback toast

**3e. Delete flow integration**
- [ ] Update `relinquishCertificate`: auto-unpublish → then delete (instead of blocking with "coming soon")
- [ ] If unpublish fails → show error, don't delete

**3f. Retry/recovery** (nice-to-have)
- [ ] POST /wallet/certificate/retry-publish — for certs stuck in `broadcast` status
- [ ] On app startup or periodic check: find `broadcast` status certs and retry overlay submit

### Sprint 4: Selective Disclosure (future)
- [ ] Prove Certificate UI (field selection + verifier key input)
- [ ] Display verifier keyring result
- [ ] Copy/share functionality

---

## Open Questions

1. ~~**Should we auto-publish certificates?**~~ No — always require user action (Sprint 3)
2. ~~**Can we request revocation from the certifier?**~~ No — SocialCert and CoolCert revocation endpoints are disabled/unimplemented
3. ~~**What happens when the certifier's server goes down?**~~ Certs still work locally. Overlay lookup still works. Only new acquisition is affected.
4. ~~**What does the frontend need from C++ for overlay queries?**~~ Nothing — Rust calls overlays directly via reqwest
5. ~~**PushDrop signing**~~: Verified — P2PK pattern (`<sig>` only unlocking) needs separate code path from P2PKH. Unit tests in Sprint 3-pre.
6. ~~**BEEF before or after broadcast?**~~ Before — must build Atomic BEEF (tx + input proofs) before broadcasting. Can't reconstruct BEEF after broadcast without fetching from API.
7. **What exactly does SocialCert's "make public" checkbox do?** — Unclear. We know it triggers a `createAction` call on our wallet with description "Create a new Identity Token" (matching the SDK's `publiclyRevealAttributes` description). The tx is created as `nosend` — our wallet builds+signs but does NOT broadcast. SocialCert's browser JS is supposed to submit the BEEF to the overlay via `TopicBroadcaster`. We have zero visibility into whether that succeeds. Need to verify by monitoring BRC-100 calls during acquisition.
8. **How should we handle externally-triggered publish (noSend)?** — When an app (SocialCert) creates a publish tx via `createAction(noSend)`, our wallet tracks the tx but never learns if it was broadcast. Design gap: need timeout + chain verification for nosend txs.

---

## Sprint 3 Issues & Fix Checklist (2026-03-17)

Sprint 3 implementation revealed multiple issues during testing. The publish button creates
a transaction and broadcasts to miners, but the overlay does not index it. Additionally,
the noSend path (SocialCert-initiated publish) leaves transactions in limbo.

### What We Know (from DB + chain investigation)

| tx | DB status | on-chain? | overlay? | what it is |
|----|-----------|-----------|----------|------------|
| `f84e92a1...` | nosend | NO | NO | SocialCert's website called `createAction` ("Create a new Identity Token") during cert acquisition with "make public". Our wallet built+signed, returned BEEF. SocialCert's JS was supposed to submit to overlay. Failed — tx never on-chain. |
| `7d39a34d...` | completed | YES (block 940664) | NO | Our publish button created this. Broadcast to miners succeeded. Overlay returned `outputsToAdmit: []` (rejected) but our code incorrectly treated it as success. |
| `0515a300...` | failed (deleted) | NO | NO | Our second publish attempt. ARC returned ORPHAN_MEMPOOL because parent `7d39a34d...:1` was already spent by unknown tx `a1a0fa17...`. Rollback cleaned up correctly. |
| `a1a0fa17...` | not tracked | YES | NO | Unknown origin — spent both `7d39a34d...:0` (PushDrop) and `7d39a34d...:1` (change). Output went to our address 265 (`1Mb7k8T9...`) but that was also spent further. Old code didn't track this. |
| `20abe28b...` | unproven | YES (`SEEN_ON_NETWORK`) | REJECTED (`outputsToAdmit: []`) | Fresh publish from clean UTXO `f451427e...:1`. Broadcast succeeded. Overlay rejects on both US and EU. **Active P1 blocker.** PushDrop output tracked at vout 0, basket=identity_certificates. |

### Two Publish Paths

**Path A — External (SocialCert's website via BRC-100):**
1. SocialCert's JS calls `createAction` on our wallet → wallet builds+signs, returns BEEF (nosend)
2. SocialCert's JS calls `TopicBroadcaster.broadcast()` → submits to overlay → overlay broadcasts to miners
3. **Our wallet has zero visibility into step 2.** No callback, no notification.
4. **Risk:** nosend tx locks UTXOs. If external broadcast fails, UTXOs stuck.

**Path B — Internal (our publish button):**
1. Frontend calls `POST /wallet/certificate/publish`
2. Handler calls `create_action_internal` → builds, signs, broadcasts to miners
3. Handler calls `overlay::submit_to_identity_overlay` → submits BEEF to overlay
4. **Issues found:** STEAK parsing bug, no broadcast success check, overlay rejects with empty outputsToAdmit

### P0 — Blocks Everything — ALL COMPLETE (2026-03-17)

- [x] **#1 STEAK parsing bug** — `overlay.rs:116` treats `outputsToAdmit: []` as success. The fallback check `!steak.as_object().map(|o| o.is_empty()).unwrap_or(true)` returns true when the response has a `tm_identity` key even if outputs is empty. Fix: check `outputsToAdmit` array explicitly, treat empty as rejection.
  - File: `rust-wallet/src/overlay.rs:96-124`
  - Test: submit known-bad BEEF, verify returns `Ok(false)` not `Ok(true)`

- [x] **#5 Publish handler ignores broadcast failure** — `create_action_internal` returns HTTP 200 with signed tx even when broadcast fails (by design — tx is valid). Publish handler checks `ca_resp.status().is_success()` which is always true. Must also check if the txid is actually on-chain or at least if createAction reported broadcast success.
  - File: `rust-wallet/src/handlers/certificate_handlers.rs` (publish_certificate)
  - Fix: Parse the createAction response — if broadcast failed, the response will have `sendWithResults` or the tx status in DB will be `failed`. Check DB status after createAction returns.

- [x] **#9 publish_status unreliable** — Cert was marked "published" based on buggy STEAK parsing. Must reflect reality.
  - Fix: Only set "published" when STEAK response has non-empty `outputsToAdmit`. Keep "broadcast" if overlay rejects. Add overlay lookup verification.

### P1 — Needed for Publish to Work — PUBLISH COMPLETE, UNPUBLISH BLOCKED (2026-03-17)

**Status:** Broadcast to miners works (`SEEN_ON_NETWORK`). Overlay rejects with `outputsToAdmit: []`.
The on-chain PushDrop output is tracked at `20abe28b...:0` (basket=identity_certificates).
When overlay issue is resolved, we can either resubmit the same BEEF or unpublish and create a corrected tx.

- [ ] **#3 Understand overlay BEEF format requirements** — Our BEEF V1 with ancestry is submitted but overlay returns `outputsToAdmit: []`. Need to determine:
  - Does the overlay expect BEEF V1 (`0100beef`) or V2 (`0200beef`)? (We send V1, confirmed in logs: `BEEF starts with: 0100beef`)
  - Does it need full ancestry or just the tx?
  - Is our PushDrop script format exactly what the `tm_identity` topic manager expects?
  - **Next step:** Compare our BEEF/PushDrop byte-for-byte with a known-working SDK publish
  - **Next step:** Check `tm_identity` topic manager source in overlay-services repo for validation rules
  - **Next step:** Try submitting the on-chain `7d39a34d...` tx (old publish, also rejected) to see if the issue is consistent

- [x] **#13 Why overlay rejects our BEEF** — ROOT CAUSE FOUND AND FIXED (2026-03-17): PushDrop was missing the data signature field. The `tm_identity` topic manager (`bsv-blockchain/identity-services`) validates:
  1. PushDrop decode → `fields[last]` is signature, remaining fields are data
  2. `anyoneWallet.verifySignature({ data, signature, counterparty: subject, protocolID: [1, 'identity'], keyID: '1' })`
  3. `certificate.verify()` — BRC-52 cert signature
  4. `certificate.decryptFields(anyoneWallet)` — at least one publicly decryptable field
  Our PushDrop only had field[0] (cert JSON). Fix: sign cert bytes with BRC-42 child key (`derive_child_private_key(master, anyone, "1-identity-1")`) → append DER signature as field[1]. Now `outputsToAdmit: [0]` on both US and EU.

### P1b — BUMP Merge Bug — ACTIVE BLOCKER (2026-03-17)

**Status:** Unpublish fails with ARC error 468 "Invalid BUMPs" when two parent transactions are confirmed in the same block. The BUMP merge logic in `beef.rs` produces invalid compound merkle proofs.

- [ ] **#14 BUMP merge produces invalid merkle proofs** — When `unpublish_certificate_core` builds BEEF for the spending tx, it calls `build_beef_for_txid` for each input's parent. If two parents are in the same block, `beef.rs` merges their BUMPs via `🔀 Merging BUMP for block N into existing BUMP index 0`. ARC validates the merged BUMP and rejects with HTTP 468.
  - **Reproduction:** Publish a cert, wait for confirmation, then unpublish. The PushDrop parent and funding UTXO parent end up in the same block.
  - **First unpublish test worked** because parents were in different blocks (no merge needed).
  - **This is a BEEF infrastructure bug**, not certificate-specific. Could affect any transaction with two inputs whose parents share a block.
  - **File:** `rust-wallet/src/beef.rs` — BUMP merge logic
  - **Next step:** Compare our BUMP merge with the BSV SDK's `MerklePath.combine()` implementation
  - **Next step:** Check if `signAction` in handlers.rs has the same merge logic (publish uses `create_action_internal` → `signAction` which builds BEEF differently and works)
  - **Workaround:** If funding UTXO's parent is in a different block than PushDrop's parent, unpublish works. Could force this by selecting UTXOs from older confirmed blocks.

### P2 — Wallet Health (noSend Lifecycle)

- [ ] **#6 nosend txs may lock UTXOs indefinitely** — `f84e92a1...` is nosend and never broadcast. If it reserved inputs via `mark_multiple_spent`, those UTXOs are stuck. Current query shows no locked outputs for this tx, but the general risk exists for future nosend txs.
  - Verify: Check if `TaskFailAbandoned` handles nosend status
  - File: `rust-wallet/src/monitor/task_fail_abandoned.rs`

- [ ] **#7 TaskFailAbandoned nosend coverage** — Verify this task cleans up nosend txs after a timeout. If it only handles `unprocessed`/`unsigned`, nosend txs are missed.
  - Fix: Add nosend to the cleanup query with appropriate timeout (e.g., 30 minutes)

- [ ] **#8 No visibility into external broadcast of nosend txs** — When an app takes nosend BEEF and broadcasts externally (overlay or direct), our wallet never learns the outcome.
  - Design: Periodic chain query for nosend txs — check if txid appears on WoC. If confirmed, update status to `completed`. If not found after timeout, fail and restore UTXOs.
  - File: Could be a new monitor task or addition to existing TaskUnFail

### P3 — Correctness

- [ ] **#2 No post-publish verification** — After setting publish_status, we should query the overlay with `POST /lookup` (by serialNumber) to confirm the cert is actually findable. This catches STEAK parsing bugs and overlay processing delays.
  - File: `rust-wallet/src/overlay.rs` (already has `lookup_published_certificate`)

- [ ] **#4 Second publish ORPHAN_MEMPOOL** — `0515a300...` rejected by ARC even though parent `7d39a34d...` is confirmed with valid BUMP in BEEF. Root cause unknown. May be a BEEF format issue or double-spend conflict from previous attempts. Lower priority since the first publish (`7d39a34d...`) is on-chain — if we fix overlay submission, this specific issue may not recur.

- [ ] **#10 PushDrop output tracking** — Previous publish/unpublish attempts may have left inconsistent output state. The `7d39a34d...:0` PushDrop output may or may not be tracked correctly in the outputs table. Need to verify before testing unpublish.
  - Query: `SELECT * FROM outputs WHERE txid LIKE '7d39a34d%'`

- [ ] **#11 Stale transactions cleanup** — `f84e92a1...` (nosend), `0515a300...` (failed), and associated outputs/reservations need cleanup before fresh testing.

- [ ] **#12 What SocialCert "make public" actually calls** — We assumed it's `publiclyRevealAttributes` based on the description string, but haven't verified. Could be a payment, a different cert operation, or something custom. Need to monitor BRC-100 calls during a fresh acquisition with "make public" checked.

### DB Cleanup (run before testing)

```sql
-- 1. Delete the X.com certificate
DELETE FROM certificates WHERE type = 'vdDWvftf1H+5+ZprUw123kjHlywH+v20aPQTuXgMpNc=';

-- 2. Check for stuck nosend/failed transactions
SELECT txid, status, description FROM transactions WHERE status IN ('nosend', 'failed');

-- 3. Check for orphaned outputs from failed publishes
SELECT txid, vout, satoshis, spendable, spending_description FROM outputs
WHERE txid IN ('f84e92a1d3310c71c5cfedffd90857d0f6859e4d0f84f6a8f35b1708a9cb3ac5',
               '0515a300577b254f32d8391a1a4f546bb0b101ca321d71215151aff6f3b1b323',
               '7d39a34d4d6f4272627830ada332e1147e40c28f2f040d3ac74f4b20f20b54cc');

-- 4. Check for UTXOs locked by stale pending placeholders
SELECT txid, vout, satoshis, spending_description FROM outputs WHERE spending_description LIKE 'pending%';

-- After reviewing results, cleanup commands will be determined.
```

### Test Plan (after P0 fixes + cleanup)

1. **Acquire fresh cert** from SocialCert WITHOUT "make public" — verify cert stored, no nosend tx created
2. **Check DB state** — cert exists with `publish_status = 'unpublished'`, no extra transactions
3. **Click publish** — watch server logs for:
   - `createAction` serialization lock acquired
   - UTXO selection + fee calculation
   - Signing succeeded
   - ARC broadcast: expect `SEEN_ON_NETWORK` (not ORPHAN_MEMPOOL)
   - Overlay submission: check STEAK response — `outputsToAdmit` should be `[0]` not `[]`
4. **Verify overlay** — use known-working resolver to check if cert is findable by identity key
5. **Verify DB** — `publish_status = 'published'`, `publish_txid` set, PushDrop output in outputs table
6. **Test unpublish** — click unpublish, verify overlay no longer has it
7. **Test delete** — auto-unpublish + delete flow

---

## Key Files

| File | Purpose |
|------|---------|
| `frontend/src/components/wallet/CertificatesTab.tsx` | Frontend display — publish/unpublish buttons |
| `rust-wallet/src/handlers/certificate_handlers.rs` | All certificate endpoints + `create_certificate_transaction()` |
| `rust-wallet/src/certificate/types.rs` | Certificate struct definitions |
| `rust-wallet/src/database/certificate_repo.rs` | DB operations + publish state tracking |
| `rust-wallet/src/database/migrations.rs` | DB schema migrations (V12: publish columns) |
| `rust-wallet/src/identity_resolver.rs` | Overlay certificate lookup (query pattern) |
| `rust-wallet/src/script/pushdrop.rs` | PushDrop encode/decode |
| `rust-wallet/src/certificate/verifier.rs` | Signature verification + revocation check |
| `rust-wallet/src/certificate/selective_disclosure.rs` | Prove certificate (keyring creation) |
| `frontend/src/components/wallet/WalletDashboard.css` | Wallet styling |

## SDK Reference Files

| File | What it shows |
|------|---------------|
| `@bsv/sdk/src/identity/IdentityClient.ts` | `publiclyRevealAttributes()` — full publish flow, `revokeCertificateRevelation()` — unpublish flow |
| `@bsv/sdk/src/identity/types/index.ts` | `protocolID: [1, "identity"]`, `keyID: "1"`, `tokenAmount: 1` |
| `@bsv/sdk/src/overlay-tools/SHIPBroadcaster.ts` | `POST {host}/submit` — overlay submission format (BEEF bytes + X-Topics header) |
| `@bsv/sdk/src/overlay-tools/LookupResolver.ts` | `POST {host}/lookup` — overlay query format |
| `@bsv/sdk/src/script/PushDrop.ts` | `lock()` — PushDrop script construction with signature |
