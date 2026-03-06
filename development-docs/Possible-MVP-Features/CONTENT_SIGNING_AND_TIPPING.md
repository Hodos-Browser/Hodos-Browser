# Content Signing & Tipping: Native Browser Feature

> **Inspiration:** AuthSig extension, Brave Rewards Twitter tipping  
> **Goal:** Let users sign content and receive payments, natively in Hodos

---

## Table of Contents

1. [Feature Overview](#feature-overview)
2. [Prior Art: Brave Browser](#prior-art-brave-browser)
3. [Prior Art: AuthSig Extension](#prior-art-authsig-extension)
4. [Proposed Hodos Implementation](#proposed-hodos-implementation)
5. [Technical Architecture](#technical-architecture)
6. [Social Certificate Integration](#social-certificate-integration)
7. [Platform-Specific Considerations](#platform-specific-considerations)
8. [Implementation Phases](#implementation-phases)
9. [Open Questions](#open-questions)

---

## Feature Overview

### What It Does

1. **Content Signing:** Users cryptographically sign posts/content on social platforms
2. **Verification:** Other users can verify the author and timestamp
3. **Payments:** Verified users can send BSV payments directly to content creators

### Why It Matters

- **Authenticity:** Prove human authorship (vs. AI-generated content)
- **Monetization:** Creators earn directly from fans, no platform cut
- **Decentralization:** Identity tied to keys, not platform accounts
- **BSV Showcase:** Demonstrates micropayments + identity in one feature

---

## Prior Art: Brave Browser

### History

| Date | Event |
|------|-------|
| **May 2019** | Twitter tipping launched in Nightly build |
| **Aug 2019** | Rolled out to desktop Brave Rewards |
| **2019-2020** | Expanded to YouTube, Twitch, Reddit, GitHub, Vimeo |
| **Ongoing** | Still functional, 59,000+ verified creators |

### How Brave Did It

**Technical Implementation:**
```
┌─────────────────────────────────────────────────────────┐
│                    BRAVE BROWSER                        │
├─────────────────────────────────────────────────────────┤
│  Content Script (DOM Injection)                         │
│  ├─ Scans for tweet containers                          │
│  ├─ Injects "Tip" button between each tweet             │
│  ├─ Monitors DOM changes (MutationObserver)             │
│  └─ Handles click → opens Brave Rewards panel           │
├─────────────────────────────────────────────────────────┤
│  Brave Rewards System                                   │
│  ├─ BAT token (ERC-20 on Ethereum)                      │
│  ├─ Creator verification database (centralized)         │
│  ├─ 90-day hold for unverified creators                 │
│  └─ Monthly payouts to verified creators                │
└─────────────────────────────────────────────────────────┘
```

**Creator Verification Flow:**
1. Creator registers at creators.brave.com
2. Verifies ownership via DNS TXT record or file upload
3. Brave adds creator to verified database
4. Blue checkmark appears in browser when visiting their content

**User Funding:**
- Earn BAT by viewing privacy-preserving ads
- Purchase BAT and add to Brave Wallet
- Set monthly auto-contribute budgets

### Brave's Limitations

| Limitation | Impact |
|------------|--------|
| Centralized verification | Single point of failure |
| BAT on Ethereum | High fees for small tips |
| 90-day hold | Friction for new creators |
| Platform-specific | Each platform needs custom integration |
| API dependencies | Twitter API changes could break it |

---

## Prior Art: AuthSig Extension

### What AuthSig Does

- Chrome extension for BSV blockchain
- Signs content with cryptographic signatures
- Stores proofs on-chain (immutable timestamp)
- Uses SocialCert for X.com identity verification
- Enables payments to verified content creators

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│                   AUTHSIG FLOW                          │
├─────────────────────────────────────────────────────────┤
│  1. Creator has SocialCert (X.com handle → BSV key)     │
│  2. Creator composes post on X.com                      │
│  3. Extension signs content hash with private key       │
│  4. Signature + hash anchored to BSV (OP_RETURN)        │
│  5. Extension injects "signed" badge + pay button       │
├─────────────────────────────────────────────────────────┤
│  Verification:                                          │
│  1. Reader's extension detects signed content           │
│  2. Retrieves signature from BSV                        │
│  3. Verifies against SocialCert public key              │
│  4. Shows verification badge + payment option           │
└─────────────────────────────────────────────────────────┘
```

### AuthSig Advantages Over Brave

| Aspect | Brave | AuthSig/BSV |
|--------|-------|-------------|
| Verification | Centralized database | On-chain certificates |
| Payments | BAT (Ethereum, $5+ fees) | BSV ($0.0001 fees) |
| Immutability | Database records | Blockchain anchored |
| Speed | Settlement takes time | Instant confirmation |
| Independence | Brave controls | User controls keys |

---

## Proposed Hodos Implementation

### Feature Name Candidates

#### Hodos-Branded (if we want to own it)

| Name | Vibe |
|------|------|
| **Hodos Sign** | Simple, clear, tied to browser |
| **ContentSeal** | Emphasizes authenticity |
| **TruthMark** | BSV "truth machine" tie-in |
| **VerifyPost** | Functional description |
| **AuthorProof** | Emphasizes authorship |

#### Generic/Standard (if we want ecosystem adoption)

| Name | Vibe |
|------|------|
| **AuthSig** | Already exists — instant compatibility with existing users |
| **ContentSig** | Clear, professional, not branded to anyone |
| **PostProof** | Simple, "proof" implies verification |
| **SignPost** | Clever double meaning (sign a post / signpost) |
| **TruthSig** | BSV-flavored, ties to "truth machine" narrative |
| **ChainSign** | Blockchain-y, generic but clear |
| **OnChainAuth** | Technical, appeals to devs |
| **VerifyMe** | User-focused, personal, approachable |

#### Protocol-Style (for BRC proposal)

| Name | Format |
|------|--------|
| **SigChain** | "SigChain Protocol" |
| **ContentAuth** | "ContentAuth-compatible wallet" |
| **OpenSig** | Implies interoperability |

**Recommendations:**

1. **If adopting existing standard:** Use **AuthSig** — already exists, has format defined, Hodos becomes "AuthSig-compatible"

2. **If creating new standard:** Use **ContentSig** — clean, professional, could submit as BRC proposal. Hodos implements first = standard-setter advantage without branding baggage.

3. **If Hodos-branded:** Use **Hodos Sign** — simple, memorable, tied to product

### Core Capabilities

1. **Sign Content**
   - Right-click any text → "Sign with Hodos"
   - Or click Hodos toolbar icon on supported sites
   - Signs content hash + timestamp to BSV

2. **Verify Content**
   - Automatic badge on signed content
   - Click to see signer identity, timestamp, certificate details
   - Warning if signature doesn't match current content

3. **Pay Creator**
   - One-click tip from verification panel
   - Uses native Hodos wallet (no extension needed)
   - Instant BSV transfer via existing paymail/BRC-100

4. **Identity Layer**
   - Integrates with SocialCert or similar certifier
   - Shows verified social handles (X, YouTube, etc.)
   - Selective disclosure — user controls what's revealed

---

## Technical Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────┐
│                    HODOS BROWSER                        │
├─────────────────────────────────────────────────────────┤
│  Native Signing Engine (Rust)                           │
│  ├─ Content hash computation (SHA-256)                  │
│  ├─ ECDSA signature generation                          │
│  ├─ Transaction creation (OP_RETURN)                    │
│  └─ Broadcast to BSV network                            │
├─────────────────────────────────────────────────────────┤
│  DOM Integration (C++/JS Bridge)                        │
│  ├─ Content script injection (platform-specific)        │
│  ├─ Sign/Verify button injection                        │
│  ├─ Verification badge rendering                        │
│  └─ Payment panel UI                                    │
├─────────────────────────────────────────────────────────┤
│  Certificate Manager                                    │
│  ├─ SocialCert integration (BRC-52)                     │
│  ├─ Local certificate storage                           │
│  ├─ Selective disclosure proofs                         │
│  └─ Certifier trust configuration                       │
├─────────────────────────────────────────────────────────┤
│  Payment Engine (existing wallet)                       │
│  ├─ createAction for tips                               │
│  ├─ Paymail resolution                                  │
│  └─ Transaction broadcast                               │
└─────────────────────────────────────────────────────────┘
```

### Signing Flow

```
User clicks "Sign" on X.com post
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  1. Extract Content                                     │
│     • Get post text, author handle, post ID             │
│     • Normalize (remove dynamic elements)               │
│     • Compute SHA-256 hash                              │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  2. Create Signature                                    │
│     • Sign hash with user's Hodos private key           │
│     • Include timestamp, platform, content type         │
│     • Format as AuthSig-compatible structure            │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  3. Anchor to BSV                                       │
│     • Create transaction with OP_RETURN                 │
│     • Include: hash, signature, certificate ref         │
│     • Broadcast via existing wallet infrastructure      │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  4. Update UI                                           │
│     • Show "Signed ✓" badge on post                     │
│     • Store TXID for verification lookups               │
│     • Enable payment button for others                  │
└─────────────────────────────────────────────────────────┘
```

### Verification Flow

```
Page loads / content detected
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  1. Scan for Signatures                                 │
│     • Query overlay service / lookup service            │
│     • Match content hashes to known signatures          │
│     • Retrieve certificate data                         │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  2. Verify Signature                                    │
│     • Compute hash of current content                   │
│     • Verify signature against public key               │
│     • Check certificate validity (not revoked)          │
│     • Verify certifier is trusted                       │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│  3. Display Results                                     │
│     • ✅ Verified: Show badge + identity                │
│     • ⚠️ Modified: Content changed since signing        │
│     • ❌ Invalid: Signature doesn't verify              │
│     • Enable "Tip" button for verified content          │
└─────────────────────────────────────────────────────────┘
```

### DOM Injection Strategy

For X.com (Twitter):

```javascript
// Pseudo-code for content script
const TWEET_SELECTOR = '[data-testid="tweet"]';
const ACTION_BAR_SELECTOR = '[role="group"]';

// Watch for new tweets
const observer = new MutationObserver((mutations) => {
  mutations.forEach((mutation) => {
    mutation.addedNodes.forEach((node) => {
      if (node.nodeType === Node.ELEMENT_NODE) {
        const tweets = node.querySelectorAll(TWEET_SELECTOR);
        tweets.forEach(injectHodosControls);
      }
    });
  });
});

function injectHodosControls(tweetElement) {
  const actionBar = tweetElement.querySelector(ACTION_BAR_SELECTOR);
  if (!actionBar || actionBar.querySelector('.hodos-controls')) return;
  
  const hodosButton = createHodosButton(tweetElement);
  actionBar.appendChild(hodosButton);
  
  // Check for existing signature
  checkSignature(tweetElement).then(updateBadge);
}
```

### Data Structures

**Signed Content Record (OP_RETURN):**
```
┌────────────────────────────────────────────────────────┐
│  Protocol Prefix: "HODOS_SIGN" (or existing standard)  │
│  Version: 1                                            │
│  Content Hash: SHA-256 (32 bytes)                      │
│  Timestamp: Unix timestamp (4 bytes)                   │
│  Platform: "x.com" | "youtube.com" | etc.              │
│  Content ID: Platform-specific identifier              │
│  Signature: ECDSA signature (71-73 bytes)              │
│  Certificate Ref: TXID of SocialCert (32 bytes)        │
└────────────────────────────────────────────────────────┘
```

---

## Social Certificate Integration

### How SocialCert Works

See [SOCIALCERT_DEEP_DIVE.md](./SOCIALCERT_DEEP_DIVE.md) for full details.

**Summary:**
1. User authenticates with social platform (OAuth)
2. Certifier verifies user controls the account
3. Certificate issued linking social handle → BSV public key
4. Certificate stored on-chain (UTXO-based, revocable)

### Required for Hodos Sign

- **Acquire certificate:** User gets SocialCert for their X.com handle
- **Store locally:** Certificate cached in Hodos wallet
- **Include in signatures:** Reference certificate when signing
- **Verify trust:** Check certifier is trusted by verifier

---

## Platform-Specific Considerations

### X.com (Twitter)

| Aspect | Details |
|--------|---------|
| **DOM Structure** | Complex React app, frequent changes |
| **Selectors** | Use `data-testid` attributes (more stable) |
| **Content Extraction** | Tweet text in specific divs |
| **Author Extraction** | Handle in URL or data attributes |
| **API Access** | Not needed — we inject, don't call APIs |

**Risks:**
- DOM changes could break injection
- X could block content modifications
- Dynamic loading requires MutationObserver

### YouTube

| Aspect | Details |
|--------|---------|
| **Target Content** | Video descriptions, comments |
| **DOM Structure** | Polymer components, complex |
| **Selectors** | `ytd-*` custom elements |
| **Author Extraction** | Channel name/ID from page |

### Gmail (Potential)

| Aspect | Details |
|--------|---------|
| **Target Content** | Email bodies |
| **Value Prop** | Sign important emails |
| **Complexity** | Gmail DOM is notoriously complex |
| **Alternative** | Could sign attachments instead |

### Generic Web Content

| Aspect | Details |
|--------|---------|
| **Target** | Any selected text |
| **Method** | Right-click context menu |
| **Storage** | Link URL + content hash |
| **Verification** | Requires user to have same page |

---

## Implementation Phases

### Phase 1: Core Signing (MVP)

**Scope:**
- [ ] Signing engine (hash + sign + broadcast)
- [ ] Basic X.com integration (inject sign button)
- [ ] Verification badge display
- [ ] Manual certificate input (no SocialCert yet)

**Effort:** 2-3 weeks

### Phase 2: Payments

**Scope:**
- [ ] Tip button on verified content
- [ ] Payment panel UI
- [ ] Paymail resolution from certificate
- [ ] Transaction creation via wallet

**Effort:** 1-2 weeks

### Phase 3: SocialCert Integration

**Scope:**
- [ ] SocialCert acquisition flow
- [ ] Certificate storage in wallet
- [ ] Automatic certificate inclusion
- [ ] Certifier trust settings

**Effort:** 2-3 weeks

### Phase 4: Multi-Platform

**Scope:**
- [ ] YouTube integration
- [ ] Generic right-click signing
- [ ] Platform detection logic
- [ ] Unified settings UI

**Effort:** 2-3 weeks

### Phase 5: Polish

**Scope:**
- [ ] Verification caching
- [ ] Offline verification (cached certs)
- [ ] Analytics / usage tracking
- [ ] User education / onboarding

**Effort:** 1-2 weeks

**Total:** ~10-13 weeks for full feature

---

## Open Questions

1. **AuthSig Compatibility:** Should we be 100% compatible with AuthSig format, or define our own? Compatibility means instant network effect with existing AuthSig users.

2. **Certifier Trust:** Who decides which certifiers are trusted? User config? Hodos default list? Both?

3. **Content Normalization:** How do we handle platform-specific formatting (mentions, links, etc.) when hashing content?

4. **Modification Detection:** If content is edited after signing, do we show a warning or fail verification entirely?

5. **Privacy:** Should signatures be public (anyone can see you signed something) or private (only verifiable with intent)?

6. **Overlay Service:** Do we run our own lookup service for Hodos signatures, or rely on existing BSV infrastructure?

7. **Fee Handling:** Who pays the ~$0.0001 signing fee? User? Subsidized? Batched?

---

## References

- Brave Rewards: [brave.com/brave-rewards](https://brave.com/brave-rewards/)
- AuthSig Extension: Chrome Web Store
- BRC-52 (Identity Certificates): BSV Academy
- SocialCert: [socialcert.net](https://socialcert.net)

---

*Last updated: 2026-03-04*
