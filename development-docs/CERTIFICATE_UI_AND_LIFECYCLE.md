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

## Phase 3: Public/Private Visibility

### 3.1 Research Questions

These need investigation before implementation:

1. **How does a certificate become public?**
   - A PushDrop transaction embeds the certificate on-chain
   - The transaction output becomes discoverable via overlay `ls_identity` service
   - Helper function `build_certificate_publishing_transaction()` exists in certificate_handlers.rs but isn't exposed

2. **Who pays for the transaction?**
   - The wallet owner (subject) pays the mining fee
   - Estimated cost: ~600 satoshis (output) + ~5000 satoshis (fee) = ~$0.001

3. **How do we check if a cert is public?**
   - Query the overlay: `POST /lookup` with `{ service: "ls_identity", query: { identityKey, certifiers } }`
   - If our cert appears in the response, it's public
   - We already do this in `identity_resolver.rs`

4. **How do we unpublish (make private)?**
   - Spend the PushDrop UTXO (the one created during publishing)
   - This removes it from the overlay's UTXO set
   - Requires the wallet to track which UTXO published the cert

5. **Do certificates expire?**
   - The BRC-52 spec doesn't define expiration
   - Revocation (spending the revocation UTXO) is the mechanism for invalidation
   - Some certifiers may issue time-limited certificates via custom fields

6. **What overlay services exist?**
   - US: `https://overlay-us-1.bsvb.tech/lookup`
   - EU: `https://overlay-eu-1.bsvb.tech/lookup`
   - Service: `ls_identity`
   - Both used in `identity_resolver.rs`

### 3.2 Implementation Plan (After Research)

1. **Add `is_public` field to certificates table** — track publishing state
2. **Expose publishing endpoint** — `POST /wallet/certificate/publish`
3. **Expose unpublish endpoint** — `POST /wallet/certificate/unpublish`
4. **Check overlay on list** — add `public` status to `listCertificates` response
5. **UI: Publish/Unpublish buttons** — with confirmation and cost estimate

---

## Phase 4: Database Changes

### 4.1 New Fields for `certificates` Table

| Field | Type | Purpose |
|-------|------|---------|
| `display_name` | TEXT | Human-readable cert type name (e.g. "X Twitter") |
| `certifier_name` | TEXT | Human-readable certifier name (e.g. "SocialCert") |
| `is_public` | INTEGER | 0=private, 1=published to overlay |
| `publish_txid` | TEXT | Transaction ID of the PushDrop publishing tx |
| `publish_outpoint` | TEXT | "txid.vout" of the publishing output (for unpublishing) |
| `acquired_at` | INTEGER | When the cert was acquired (vs created_at which is DB insert time) |

**Migration:** V12 — add columns with defaults (display_name=NULL, is_public=0, etc.)

### 4.2 Populate on Acquisition

When `acquireCertificate` succeeds:
1. Look up cert type in known types map → set `display_name`
2. Look up certifier pubkey in known certifiers map → set `certifier_name`
3. Set `is_public = 0` (newly acquired certs are private)
4. Set `acquired_at` to current timestamp

---

## Phase 5: Revised `listCertificates` Response

Add to response:
```json
{
  "certificates": [
    {
      "type": "vdDWvftf1H+5+...",
      "type_name": "X (Twitter)",
      "certifier": "02cf6cdf...",
      "certifier_name": "SocialCert",
      "subject": "020b9558...",
      "serial_number": "abc123...",
      "revocation_outpoint": "txid.0",
      "is_public": false,
      "is_revoked": false,
      "decrypted_fields": {
        "userName": "bsvarchie",
        "profilePhoto": "https://..."
      },
      "created_at": 1710000000
    }
  ]
}
```

**Notes:**
- `is_revoked` requires a WoC API call per cert — may want to cache or check on-demand only
- `decrypted_fields` requires the wallet's private key — only works for certs where we are the subject

---

## Sprint Plan

### Sprint 1: Display (1-2 days)
- [ ] Add type/certifier name constants to frontend
- [ ] Add decrypted fields to `listCertificates` backend response
- [ ] Redesign CertificatesTab with new columns
- [ ] Add expandable row for field details
- [ ] Add info tooltips

### Sprint 2: Actions (1-2 days)
- [ ] Delete button with confirmation (calls `relinquishCertificate`)
- [ ] View details modal
- [ ] DB migration V12 for new fields
- [ ] Populate display_name/certifier_name on acquisition

### Sprint 3: Status & Overlay (2-3 days)
- [ ] Research: how to check if cert is published on overlay
- [ ] Research: how to publish/unpublish via PushDrop
- [ ] Add is_public tracking
- [ ] Add revocation status check (cached)
- [ ] Status badges in UI

### Sprint 4: Publish/Unpublish (2-3 days)
- [ ] Expose publish endpoint (uses existing helper function)
- [ ] Expose unpublish endpoint (spend PushDrop UTXO)
- [ ] Publish/Unpublish buttons in UI
- [ ] Cost estimate display

### Sprint 5: Selective Disclosure (1-2 days)
- [ ] Prove Certificate UI (field selection + verifier key input)
- [ ] Display verifier keyring result
- [ ] Copy/share functionality

---

## Open Questions

1. **Should we auto-publish certificates?** Or always require user action?
2. **Can we request revocation from the certifier?** What API would that use?
3. **What happens when the certifier's server goes down?** Can we still prove/verify certs locally?
4. **Should we support multiple certifiers for the same type?** (e.g. multiple email certifiers)
5. **How should we handle cert type versioning?** If SocialCert changes the type ID, old certs become "Unknown"
6. **Should the wallet store decrypted field values?** Or decrypt on-the-fly each time?
7. **Do we need to support cert renewal/reissuance?** (Same identity, new serial number)
8. **What does the frontend need from C++ for overlay queries?** Can we call overlays directly from Rust, or do they need to go through the interceptor?

---

## Key Files

| File | Purpose |
|------|---------|
| `frontend/src/components/wallet/CertificatesTab.tsx` | Frontend display (needs redesign) |
| `rust-wallet/src/handlers/certificate_handlers.rs` | All certificate endpoints |
| `rust-wallet/src/certificate/types.rs` | Certificate struct definitions |
| `rust-wallet/src/database/certificate_repo.rs` | DB operations |
| `rust-wallet/src/database/migrations.rs` | DB schema migrations |
| `rust-wallet/src/identity_resolver.rs` | Overlay certificate lookup |
| `rust-wallet/src/certificate/verifier.rs` | Signature verification + revocation check |
| `rust-wallet/src/certificate/selective_disclosure.rs` | Prove certificate (keyring creation) |
| `frontend/src/components/wallet/WalletDashboard.css` | Wallet styling |
