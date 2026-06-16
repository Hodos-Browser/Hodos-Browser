# Possible MVP Features

Research and planning documents for features under consideration for Hodos Browser.

## Prioritization

The canonical priority view for everything in this folder lives in **[marketing/intelligence/FEATURE_PRIORITY.md](../../../Marston%20Enterprises/Hodos/marketing/intelligence/FEATURE_PRIORITY.md)** (path: `C:\Users\archb\Marston Enterprises\Hodos\marketing\intelligence\FEATURE_PRIORITY.md`). That file buckets features into NOW / NEXT QUARTER / RESEARCH / BACKLOG / DEFER and ties demand signals + ecosystem signals + effort estimates to each row. Effort scores come from `marketing/intelligence/EFFORT_MATRIX.md`.

Files in this folder are the **research depth** behind each row in the priority matrix. Add a new doc here when a feature graduates from "noticed in passing" to "worth detailed planning."

## Documents

| File | Description |
|------|-------------|
| [CONTENT_SIGNING_AND_TIPPING.md](./CONTENT_SIGNING_AND_TIPPING.md) | AuthSig-style content signing with payments. Includes Brave history, implementation plan. |
| [SOCIALCERT_DEEP_DIVE.md](./SOCIALCERT_DEEP_DIVE.md) | How social certificates work, OAuth flows, Gmail/YouTube expansion, integration plan. |
| [BRC52_CERTIFICATE_RESEARCH_PLAN.md](./BRC52_CERTIFICATE_RESEARCH_PLAN.md) | Social certificates linking social accounts (X, Discord, Gmail, YouTube) to BSV keys. |
| [PRIVILEGED_KEYRING_ANALYSIS.md](./PRIVILEGED_KEYRING_ANALYSIS.md) | Architectural analysis of a privileged-keyring approach to wallet key management. |
| [Decentralized-Naming/](./Decentralized-Naming/) | Federated paymail + on-chain domain naming research — strategic planning, BOOTSTRAP_PROBLEM, NARRATIVES, plus Paymail and Domain-Names subfolders. |
| [LINUX_BUILD.md](./LINUX_BUILD.md) | Linux build candidate — pre-decision stub. 3 user asks logged 2026-05-11; BACKLOG per priority matrix. |
| [Dolphin Milk + Edwin Integration](../Dolphin%20Milk%20%2B%20Edwin%20Integration/DOLPHIN_MILK_INTEGRATION.md) | Bundle John Calhoun's open-source Dolphin Milk AI agent + Jake Jones's Edwin signed-envelope security layer inside Hodos — single-install AI agent with a BSV wallet, no terminal, no API keys, no Claude account, cryptographic prompt-injection resistance. Logged 2026-05-11; RESEARCH per priority matrix. **Promoted out of this bucket 2026-05-29 — active planning under `development-docs/Dolphin Milk + Edwin Integration/`.** |
| [OAUTH_CONNECTED_AGENT.md](./OAUTH_CONNECTED_AGENT.md) | OAuth-Connected Personal Data Agent — Hodos brokers OAuth tokens (YouTube/Gmail/Drive/X/etc.) and lets the bundled agent act on the user's behalf. Triggered by @hbgnostic 2026-05-11 YT-reorg post + @ruthheasman "must try this" reply. Logged 2026-05-11; RESEARCH per priority matrix; depends on Dolphin Milk integration. |

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
