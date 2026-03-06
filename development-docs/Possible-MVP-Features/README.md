# Possible MVP Features

Research and planning documents for features under consideration for Hodos Browser.

## Documents

| File | Description |
|------|-------------|
| [CONTENT_SIGNING_AND_TIPPING.md](./CONTENT_SIGNING_AND_TIPPING.md) | AuthSig-style content signing with payments. Includes Brave history, implementation plan. |
| [SOCIALCERT_DEEP_DIVE.md](./SOCIALCERT_DEEP_DIVE.md) | How social certificates work, OAuth flows, Gmail/YouTube expansion, integration plan. |

## Feature: Content Signing & Tipping

**Inspiration:** AuthSig extension, Brave Rewards

**Summary:**
- Users sign posts on X.com (and other platforms) with their Hodos key
- Signatures anchored to BSV blockchain
- Other users can verify authorship and send tips
- Uses SocialCert for identity (X.com handle → public key)

**Key Advantages Over Brave:**
- BSV fees: $0.0001 vs BAT/Ethereum $5+
- Decentralized verification vs Brave's central database
- Instant settlement vs monthly payouts
- Native wallet integration (no extension)

## Feature: Social Certificates

**What:** Certificates linking social accounts (X.com, Discord, Gmail, YouTube) to BSV keys

**How Authentication Works:**
1. OAuth 2.0 flow with social platform
2. Platform confirms user controls account
3. Certifier issues signed certificate
4. Certificate stored on-chain (UTXO-based, revocable)

**Key Questions Answered:**
- How does certifier know you own the account? → OAuth flow, platform API confirms
- Can one account have multiple keys? → Yes, each cert is valid at issuance time
- Could we do Gmail/YouTube? → Yes, same OAuth pattern with Google

## Status

- [x] AuthSig/Brave research complete
- [x] SocialCert technical deep dive
- [x] OAuth flow documentation
- [ ] Implementation decision (priority, timing)
- [ ] AuthSig compatibility decision
- [ ] Certifier trust model decision

---

*Created: 2026-03-04*
