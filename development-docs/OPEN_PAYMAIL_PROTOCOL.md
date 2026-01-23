# Vendor-Neutral Paymail Architecture

## Abstract

A decentralized, confederated payment addressing system that enables human-readable identifiers (`alice@paymail`) **without domain dependencies**. The system uses BitcoinSV blockchain for registration and overlay networks for resolution, eliminating the need for DNS and domain registration. **Confederation enables vendor-neutrality** - multiple independent operators run overlay service nodes, ensuring no single vendor controls the system. Payments authenticate operations through micropayments, eliminating API keys and vendor lock-in.

**Core Innovation**: Protocol identifiers (`@paymail`) replace domain names, and blockchain-based overlay resolution replaces DNS lookup. This removes centralization points (registrars, ICANN, DNS) while maintaining human-readable addresses.

**Note**: This is an architectural proposal for a standardization process. It describes a grass-roots, free-market system that will require communication, coordination, and conventional acceptance of non-perfect solutions among participants. Feedback and collaboration are actively sought.

---

## Index

1. [Vision](#vision)
2. [Architecture Overview](#architecture-overview)
3. [How BitcoinSV Removes Domain Dependency](#how-bitcoinsv-removes-domain-dependency)
4. [Migration & Coexistence with Traditional Paymail](#migration--coexistence-with-traditional-paymail)
5. [Registration Process](#registration-process)
6. [Address Resolution](#address-resolution)
7. [Payment Delivery Flow](#payment-delivery-flow)
8. [Overlay Network Architecture](#overlay-network-architecture)
9. [API Standardization](#api-standardization)
10. [Micropayment Economics](#micropayment-economics)
11. [Name Marketplace & Transfer](#name-marketplace--transfer)
12. [Security Model](#security-model)
13. [Threat Model](#threat-model)
14. [Key Recovery & Rotation](#key-recovery--rotation)
15. [BRC Standards Integration](#brc-standards-integration)
16. [Comparison: Traditional vs Vendor-Neutral Paymail](#comparison-traditional-vs-vendor-neutral-paymail)
17. [Design Rationale](#design-rationale)
18. [Questions & Open Discussion](#questions--open-discussion)
19. [References](#references)

---

## Vision

**Core Principles:**
- **No domains required** - Protocol identifiers replace DNS
- **No API keys** - Micropayment-based authentication
- **No vendor lock-in** - Any wallet can implement the protocol
- **Blockchain-based** - BitcoinSV provides permanent, censorship-resistant registry
- **Standards-based** - Built on established BRC protocols

**Example**: `alice@paymail` - The `@paymail` suffix signals overlay resolution, not DNS lookup. No domain registration needed.

**Note on suffix flexibility**: Because no domain needs to be available or registered, the suffix can be anything the community agrees on - `@paymail`, `@Bitcoin`, `@BSV`, or even no suffix at all (just the alias). The only requirement is that the wallet recognizes the input as a vendor-neutral paymail address and routes it to overlay resolution. The suffix is purely a UX convention for human readability, not a technical constraint.

---

## Architecture Overview

The system consists of three layers:

1. **Wallet Layer** - Recognizes paymail addresses and queries overlay services
2. **Overlay Network** - BRC-22 Topic Managers validate transactions, BRC-24 Lookup Services index aliases
3. **Blockchain** - Permanent, censorship-resistant registry of paymail registrations

Wallets query BRC-24 Lookup Services to resolve aliases to public keys. The overlay network uses blockchain as the source of truth, eliminating DNS dependencies. BRC-33 message relay handles offline delivery.

---

## How BitcoinSV Removes Domain Dependency

### Traditional Paymail (Domain-Based)

**Traditional approach**: `alice@example.com`
- Requires domain registration (`example.com`)
- Uses DNS to resolve domain to server IP address
- Centralized: DNS controlled by ICANN/registrars
- Censorship risk: Domains can be seized

### Vendor-Neutral Paymail (Blockchain-Based)

**New approach**: `alice@paymail`
- **No domain registration** - `@paymail` is a protocol identifier, not a domain
- **No DNS lookup** - Overlay network resolves aliases using blockchain data
- **Decentralized** - Blockchain provides permanent, censorship-resistant registry
- **No centralization** - No ICANN, no registrars, no DNS dependency

### The Key Difference

| Aspect | Traditional Paymail | Vendor-Neutral Paymail |
|--------|--------------------|--------------------|
| Address format | `alice@domain.com` | `alice@paymail` |
| Resolution | DNS lookup | Overlay network lookup |
| Registry | Domain registrar | BitcoinSV blockchain |
| Dependency | DNS, ICANN, registrars | Blockchain only |
| Censorship | Domain seizure possible | Very difficult |

**Why this matters**: By using blockchain as the registry instead of DNS, we eliminate all centralization points. The `@paymail` suffix is just a signal to wallets: "resolve via overlay, not DNS."

---

## Migration & Coexistence with Traditional Paymail

Traditional domain-based paymail and vendor-neutral paymail coexist as parallel resolution paths. Wallet developers are responsible for correctly handling both types for their users.

**Wallet Resolution Logic:**

1. User enters a paymail address into the recipient field
2. Wallet parses the suffix:
   - If the suffix matches a recognized vendor-neutral protocol identifier (`@paymail`, `@bitcoin`, `@bsv`): execute overlay resolution path
   - If the suffix contains a domain indicator (`.com`, `.io`, etc.): execute traditional DNS-based paymail resolution
3. Each path resolves to a public key and payment destination independently

**Wallet Responsibility**: It is the wallet developer's responsibility to implement correct detection and routing for both paymail types. The two systems do not conflict - they use different resolution mechanisms triggered by the address format.

**Suffix Registry**: A registry mechanism for recognized vendor-neutral suffixes needs to be designed and implemented as part of the standardization process. This determines which suffixes trigger overlay resolution vs. DNS resolution.

---

## Registration Process

### Transaction Format

Users register paymail aliases by creating on-chain transactions using PushDrop format to embed registration data in transaction outputs. The PushDrop script contains:
- Protocol identifier: `"paymail"`
- Version: `0x01`
- Action: `"register"`
- Alias: The desired identifier (e.g., `"alice"`)
- Public key: 33-byte compressed secp256k1 key (derived via BRC-42/43)
- Optional relay preferences: Fallback nodes for offline delivery

PushDrop allows structured data to be embedded in spendable outputs without affecting spending rules. Unlike OP_RETURN (which creates unspendable outputs), PushDrop tokens can be transferred, traded, or revoked by spending them - giving users full control over their registration via their private key.

**Transaction structure:**
- **Output 0**: Registration fee paid to overlay operator (see [Micropayment Economics](#micropayment-economics))
- **Output 1**: PushDrop token (1 sat) containing registration data - this IS the paymail registration

The fee payment and registration are atomic - both occur in the same transaction.

**Note**: The exact byte-level PushDrop script specification (opcode ordering, field encoding) will be defined in the next planning phase before implementation. At this architectural stage, the logical structure is what matters.

### Key Derivation

Paymail identities use BRC-42/43 key derivation for secure key management. Keys are derived from the wallet's master key using standardized protocol IDs.

**Optional: BRC-84 Linked Key Derivation** - Enables deriving child public keys from only the master public key, without requiring private key access. Useful for non-custodial scenarios where a service needs to generate payment destinations but should never hold keys. Derived keys remain cryptographically linked to the master for auditability.

### Topic Manager Validation

Wallets submit registration transactions to BRC-22 Topic Managers, which validate and admit them. Topic Managers enforce these rules:

- **Alias format**: Alphanumeric characters, underscores, and hyphens only
- **Uniqueness**: First valid registration seen on-chain (in a block) wins - blockchain provides canonical ordering for race conditions
- **Signature verification**: Transaction must be signed by the key embedded in the registration data
- **Fee validation**: Sufficient registration fee must be included in Output 0
- **Multi-sig support**: Registrations using multi-sig locking scripts must be recognized and handled by Topic Managers (see [Key Recovery & Rotation](#key-recovery--rotation))

Topic Managers reject invalid or duplicate registrations. Once a registration is mined into a block, that alias is permanently claimed - the blockchain's ordering resolves any race conditions.

### Lookup Service Indexing

BRC-24 Lookup Services index admitted transactions by alias for fast resolution. Services synchronize with peer nodes using BRC-88 overlay synchronization protocols. The lookup service maintains a queryable index of all valid paymail registrations, using blockchain as the authoritative source.

---

## Address Resolution

### Suffix Recognition

Wallets recognize standardized suffixes that signal overlay resolution:
- `@paymail` - Generic paymail protocol (recommended default)
- `@bitcoin` - Broader appeal, same protocol
- `@bsv` - Explicit BSV network identifier

**Important**: These suffixes are **protocol identifiers**, not domain names. They require **no domain registration, no DNS setup, and no payment to registrars**. The suffix signals "use overlay lookup, not DNS lookup" - the wallet queries overlay service nodes instead of performing DNS resolution. All suffixes resolve through the same overlay network.


### Lookup Process

1. Wallet parses address into alias and suffix
2. Wallet queries BRC-24 Lookup Service via overlay service nodes
3. Lookup service returns public key and optional server address
4. Wallet uses public key to create payment or connects via BRC-33 relay if recipient is offline

### Overlay Service Node Discovery

Wallets need to discover overlay service nodes (Topic Managers and Lookup Services) to begin querying. These are standard BRC-22/BRC-24 overlay nodes.

**Node Discovery Approaches**:

**Hardcoded List**: Simple fallback list of overlay service nodes embedded in wallet software. Each node entry includes its server address (IP address or domain name). Requires wallet updates to add nodes.

**On-Chain Registry (BRC-23 CHIP)**: Overlay service nodes advertise themselves in blockchain transactions using BRC-23 (Confederacy Host Interconnect Protocol). Nodes create CHIP tokens that contain their service addresses, allowing wallets to discover available Topic Managers and Lookup Services dynamically. This is fully decentralized and self-updating.

**Hybrid**: Hardcoded fallback with periodic on-chain updates. Wallets start with hardcoded nodes, then query blockchain for updated CHIP advertisements. Provides resilience with graceful degradation.

**How CHIP Works**: Nodes create on-chain transactions containing their service information (IP addresses, ports, capabilities). Wallets scan the blockchain for CHIP tokens to discover available overlay nodes. This eliminates the need for centralized node directories while maintaining discoverability.

**Trust & Reputation**: Overlay service nodes are public service providers - like storefronts, they should be discoverable and transparent. Users and wallets are responsible for choosing reputable servers. Trust derives from costly signals: operators investing in long-term viable businesses, transparent operations, consistent uptime, and on-chain node advertisements via CHIP. The decentralized nature of the overlay network reduces reliance on any single node operator, and users can switch providers freely.

---

## Payment Delivery Flow

When a wallet resolves a paymail address and wants to send a payment, the following handshake occurs between the sending wallet and the overlay service node:

### Handshake Protocol

1. **Wallet initiates**: Wallet connects to overlay service node, indicates user wants to send a paymail payment
2. **Server responds with terms**: Server derives an ECC public key and responds with:
   - Current fee schedule (in satoshis)
   - Fee payment destination (server's derived public key)
   - Recipient's payment destination (BRC-84 linked ECC-derived public key from the registered paymail master public key)
3. **Wallet sends payment**: Wallet constructs transaction including:
   - Service fee output (to server's key)
   - Payment output (to recipient's BRC-84 derived key)
   - Transaction data and any memo
4. **Server confirms**: Server validates payment, broadcasts transaction, returns txid receipt
5. **Acknowledgment (optional)**: Server asks if wallet wants notification when recipient acknowledges receipt
   - Wallet can decline ("I have the txid, claiming is on them")
   - Wallet can accept (server places message in sender's relay box once recipient acknowledges)

### Offline Recipients

When recipients are offline, the overlay service uses BRC-33 PeerServ message relay. The relay stores transaction notifications until recipients poll for messages. This provides store-and-forward capability without custom relay protocols.

### Transaction Format

All paymail transactions use BEEF (BRC-58) format for SPV validation. Recipients can verify transactions before broadcasting to the network. BRC-70 specifies the paymail BEEF transaction protocol.

---

## Overlay Network Architecture

### SHIP Pattern

The overlay follows the SHIP (Submit, Host, Index, Prove) architecture:

- **Submit**: Wallets submit registration transactions to BRC-22 Topic Managers
- **Host**: BRC-24 Lookup Services host indexed alias data
- **Index**: Lookup services maintain queryable indexes by alias
- **Prove**: On-chain transactions provide cryptographic proof of ownership

### Node Synchronization

Overlay nodes synchronize using BRC-88 protocols. New nodes can:
- **Peer sync**: Fast synchronization from existing nodes
- **Chain scan**: Trustless validation by scanning blockchain

Best practice combines both: peer sync for speed, optional chain scan for verification.

### Node Discovery

BRC-23 (CHIP) enables node discovery through on-chain advertisements. Nodes advertise their services via CHIP tokens, allowing wallets to discover available Topic Managers and Lookup Services dynamically.

---

## API Standardization

Overlay service nodes must implement standardized endpoints with defined JSON request/response formats. The exact schemas require collaborative standardization, but the following endpoint structure is proposed:

### Core Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /paymail/resolve` | Resolve alias to public key and payment destination |
| `POST /paymail/register` | Submit registration transaction |
| `POST /paymail/send` | Initiate payment delivery handshake |
| `POST /paymail/transfer` | Transfer name ownership to new public key |
| `GET /paymail/fees` | Get current fee schedule for all operations |
| `POST /paymail/message` | Send message to a paymail address via relay |
| `GET /paymail/status` | Node health and capability advertisement |

### Dynamic Fee Discovery

Overlay nodes expose a fee endpoint (`GET /paymail/fees`) so wallets can query current rates before initiating operations. This enables:
- Wallets to compare fees across multiple overlay providers
- Dynamic fee adjustment as demand changes
- Transparent pricing for users

**Fee Schedule Response** (example structure):
```json
{
  "registration": { "sats": 5000, "currency_peg": "USD", "peg_rate": 0.03 },
  "lookup": { "sats": 100 },
  "send": { "sats": 500 },
  "relay_message": { "sats": 200 },
  "transfer": { "sats": 1000 },
  "updated_at": "2026-01-23T00:00:00Z"
}
```

### Fee Philosophy

Overlay operators are encouraged to start with fees on the **cooperation** side of the coopetition scale during the bootstrap phase - prioritizing ecosystem growth over revenue maximization. As adoption grows and network effects develop, operators can move toward more competitive pricing that reflects actual demand and service quality.

The standardized JSON formats for each endpoint will be defined collaboratively as part of the standardization process.

---

## Micropayment Economics

### Operation Costs

Fees are set by individual overlay operators and should reflect actual operational costs plus sustainable margins. Fees should be denominated in satoshis but pegged to a stable reference (e.g., USD equivalent) and adjusted dynamically as exchange rates and demand change.

**Example fee ranges** (operators set their own):

| Operation | Suggested Range (sats) | Who Pays |
|-----------|----------------------|----------|
| Register alias | 2,000 - 50,000+ | User (on-chain) |
| Lookup | 50 - 500 | Requesting wallet |
| Payment delivery | 200 - 2,000 | Sender wallet |
| Relay message | 100 - 1,000 | Sender wallet |
| Name transfer | 500 - 5,000 | Seller/buyer (negotiable) |

Vanity names and high-demand aliases may command significantly higher registration fees, set by marketplace dynamics.

### Payment Validation

Wallets include micropayments in request headers using BRC-41 HTTP Service Monetization patterns. Overlay nodes validate payments before processing requests, ensuring sustainable operation through micropayment funding.

### Economic Viability

This is a viable business model based on micro-fees at scale. Key economics:

- **Operator costs**: Primarily VPS hosting + bandwidth. Once the initial design, coding, and implementation costs are covered, ongoing operational costs for processing requests are low.
- **Revenue**: Scales with usage volume across all operations (registrations, lookups, payments, relay)
- **Bootstrap phase**: Early operators invest in building the system with cooperative pricing, establishing trust and user base
- **Growth phase**: As demand increases, fees adjust dynamically, creating competitive marketplace among operators
- **Sustainability**: High volume of low-fee operations creates predictable revenue streams

The bootstrap investment itself serves as a costly trust signal - operators who invest significant resources in building and maintaining infrastructure demonstrate long-term commitment to the ecosystem.

---

## Name Marketplace & Transfer

### Name Ownership as Tradeable Assets

Paymail registrations are PushDrop tokens (UTXOs) - they can be transferred by spending the output to a new owner's key. This creates a natural marketplace for desirable names.

### Overlay Operator Incentives

Overlay operators who bootstrap the system are positioned to understand name value early. They are incentivized to register and tokenize valuable names, but also incentivized to sell at reasonable prices because they want the ecosystem to grow and support their overlay business. Unreasonable hoarding undermines the system they depend on.

### Name Solicitation via Paymail

The paymail and message relay system itself can be used to solicit offers for registered names:

1. **Buyer sends offer**: Buyer sends a message to the desired name via the relay system, including a payment to get the owner's attention (signals serious interest)
2. **Tokenized offers with nLockTime**: Offers can be structured as transactions with `nLockTime` - if not read or accepted within a specified timeframe, the offer transaction becomes invalid and funds return to the buyer automatically
3. **Owner responds**: Owner can accept (complete the transfer transaction), reject, or counter-offer through the same relay channel
4. **Transfer execution**: Agreed transfers are executed on-chain by spending the registration UTXO to the buyer's key

### Marketplace Development

Dedicated marketplaces can and should be built around this system:
- Browse available/registered names
- List names for sale with asking prices
- Auction mechanisms for premium names
- Price discovery through transparent offer history

**We welcome feedback on marketplace design, pricing mechanisms, and anti-squatting measures.**

---

## Security Model

### Identity Verification

Registration transactions are signed by registrant's keys using BRC-42/43 derivation. Key updates require signatures from current keys. On-chain records provide tamper-proof proof of ownership.

**Certificates Optional**: BRC-52 certificates can enhance identity verification but are not required for basic paymail functionality. The system prioritizes simplicity while allowing optional certificate integration.

### Squatting Considerations

Topic Manager validation enforces first-come-first-served registration. Registration fees (in the thousands of sats, pegged to stable value) discourage casual mass squatting. On-chain proof prevents disputes. Alias format restrictions limit abuse. Marketplace dynamics provide a release valve - squatted names that aren't used don't generate value for the squatter, while active names generate relay and lookup fees for the ecosystem.

### Spam Prevention

Micropayments required for every operation. Rate limiting per IP address. Higher fees for suspicious patterns. Double-spend protection through immediate transaction broadcasting and TXID tracking.

---

## Threat Model

### Attack Vectors & Mitigations

| Threat | Description | Mitigation |
|--------|-------------|------------|
| **Malicious overlay node** | Node returns wrong public key, redirecting payments | Wallets query multiple nodes, verify against on-chain data; users choose reputable providers |
| **Name squatting** | Mass registration of common names | Registration fees, marketplace dynamics, community norms (see [Name Marketplace](#name-marketplace--transfer)) |
| **Eclipse attack** | Wallet only connects to attacker-controlled nodes | Multiple hardcoded fallback nodes, BRC-23 CHIP discovery, user-configurable node lists |
| **Key compromise** | Attacker obtains registrant's private key | Multi-sig registrations, immediate re-registration to new key if detected (see [Key Recovery](#key-recovery--rotation)) |
| **Reorg attack** | Blockchain reorganization changes registration ownership | Deeper confirmations for high-value names, overlay nodes wait for sufficient block depth |
| **Relay interception** | Man-in-middle on message relay | End-to-end encryption via BRC-42 derived shared secrets, BEEF format for transaction integrity |
| **Fee manipulation** | Node advertises low fees then demands more | Wallet pre-validates fee schedule, atomic payment-and-service in single transaction |
| **Sybil nodes** | Attacker runs many fake overlay nodes | Costly signals (CHIP advertisements, operational history), wallet reputation tracking |

### Trust Assumptions

- Blockchain ordering is final (standard Bitcoin assumption)
- At least one honest overlay node is reachable by the wallet
- Users take responsibility for choosing reputable service providers
- Key security is the user's responsibility (standard Bitcoin assumption)

---

## Key Recovery & Rotation

### The Problem

Paymail registrations are locked to specific keys via PushDrop tokens. Lost keys mean lost identity. This is not unique to this paymail system - it is a fundamental Bitcoin key responsibility challenge that needs comprehensive solutions across the ecosystem.

### Multi-Sig Registrations

**Overlay system requirement**: Topic Managers must recognize and handle multi-sig registration formats. This enables:

- **2-of-3 registrations**: User holds 2 keys, backup service holds 1. Any 2 can transfer the registration.
- **Recovery threshold**: User can recover their paymail even if one key is compromised or lost.
- **Corporate use**: Multiple signers for business paymail addresses.

The overlay system's Topic Managers and Lookup Services must be designed to validate and process multi-sig PushDrop tokens from the start.

### Other Recovery Approaches

These are handled at the user/wallet level, not by the overlay system:

- **Social recovery**: Trusted contacts hold key shares (Shamir's Secret Sharing)
- **Time-locked backup keys**: Alternative keys that activate after a delay period
- **Cloud key backup**: Encrypted key backups with specialized providers
- **Hardware wallet integration**: Keys stored on dedicated signing devices

These solutions exist in the broader Bitcoin wallet ecosystem and apply equally to vendor-neutral paymail. Wallet developers should integrate appropriate recovery options for their users.

---

## BRC Standards Integration

### Core Standards

| BRC | Purpose | Usage |
|-----|---------|-------|
| [BRC-22](https://bsv.brc.dev/overlays/0022) | Topic Manager | Validates and admits registration transactions |
| [BRC-23](https://bsv.brc.dev/overlays/0023) | CHIP | Node discovery and advertisement |
| [BRC-24](https://bsv.brc.dev/overlays/0024) | Lookup Service | Indexes aliases, responds to queries |
| [BRC-25](https://bsv.brc.dev/overlays/0025) | CLAP | Lookup service availability |
| [BRC-33](https://bsv.brc.dev/peer-to-peer/0033) | PeerServ | Message relay for offline delivery |
| [BRC-42/43](https://bsv.brc.dev/key-derivation/0042) | Key Derivation | Secure key management |
| [BRC-58](https://bsv.brc.dev/transactions/0058) | BEEF | Transaction format for SPV |
| [BRC-70](https://bsv.brc.dev/payments/0070) | Paymail BEEF | BEEF transactions via paymail |
| [BRC-84](https://bsv.brc.dev/key-derivation/0084) | Linked Keys | Derive payment destinations from master public key |
| [BRC-100](https://bsv.brc.dev/wallet/0100) | Wallet Interface | Standard wallet API |

### Supporting Standards

| BRC | Purpose | Usage |
|-----|---------|-------|
| [BRC-28](https://bsv.brc.dev/payments/0028) | Paymail Destinations | Payment destination protocol |
| [BRC-29](https://bsv.brc.dev/payments/0029) | Simple P2PKH | Payment script format |
| [BRC-41](https://bsv.brc.dev/payments/0041) | HTTP Monetization | Micropayment validation |
| [BRC-83](https://bsv.brc.dev/transactions/0083) | Scalable TX Processing | High-volume optimization |
| [BRC-87](https://bsv.brc.dev/overlays/0087) | Naming Conventions | Service naming standards |
| [BRC-88](https://bsv.brc.dev/overlays/0088) | Overlay Sync | Node synchronization |

### Optional Standards

| BRC | Purpose | When to Use |
|-----|---------|-------------|
| [BRC-26](https://bsv.brc.dev/overlays/0026) | UHRP | Content hosting for profiles |
| [BRC-52](https://bsv.brc.dev/peer-to-peer/0052) | Certificates | Enhanced identity verification |

**Note on BRC-31**: BRC-31 (Authrite) may be deprecated in favor of BRC-103/104 for authentication. Topic Managers use BRC-31 for transaction submission authentication, but future implementations should consider BRC-103/104 alternatives.

---

## Comparison: Traditional vs Vendor-Neutral Paymail

| Aspect | Traditional Paymail | Vendor-Neutral Paymail |
|--------|--------------------|--------------------|
| Identity format | `alice@domain.com` | `alice@paymail` |
| Resolution | DNS lookup | Overlay lookup |
| Registry | Domain registrar | BitcoinSV blockchain |
| Dependency | DNS, ICANN, registrars | Blockchain only |
| Censorship | Domain seizure possible | Very difficult |
| Cost (operator) | $119-179/year | Dynamic micro-fees |
| Cost (user) | Vendor-dependent | Micro-fee per operation |
| Offline support | Always online server | BRC-33 relay fallback |
| Name tradability | Domain transfer process | Native UTXO transfer |
| BSV-native | No | Yes |

---

## Design Rationale

### Why Overlay Instead of DNS

DNS requires domain registration, creating centralization points and censorship risks. Overlay networks use blockchain as the source of truth, eliminating registrar dependencies. Protocol identifiers (`@paymail`) signal resolution method without requiring domain ownership.

### Why Blockchain Registry

BitcoinSV blockchain provides a permanent, censorship-resistant registry. Once a registration is mined, it cannot be altered or removed by any central authority. The blockchain's ordering resolves race conditions and provides cryptographic proof of ownership.

### Why Micropayments

BSV's low transaction fees make micropayment-funded services viable. Payment-as-authentication eliminates API keys and simplifies wallet integration. Micropayments scale with usage, creating sustainable economic models.

### Why BRC Standards

Leveraging existing BRC standards ensures interoperability and reduces implementation complexity. Standards provide proven patterns for overlay networks, message relay, and key management. Vendor-neutral standards prevent lock-in.

---

## Questions & Open Discussion

This section captures open questions and areas where community feedback is actively sought. This is a standardization process - conventional acceptance of practical solutions is more valuable than theoretical perfection.

### Q1: Name Squatting & Fair Distribution

**Challenge**: How do we prevent mass registration of valuable names while keeping the system accessible?

**Current thinking**: Registration fees in the thousands of sats (pegged to stable value) create a cost barrier. Overlay operators who bootstrap the system have early access to register names - they're incentivized to price reasonably because ecosystem growth supports their business. Marketplace dynamics (see [Name Marketplace](#name-marketplace--transfer)) provide price discovery and transfer mechanisms.

**Feedback requested**: What fee levels balance accessibility vs. anti-squatting? Should there be name expiry/renewal? Are there other mechanisms we haven't considered?

### Q2: Race Conditions During Registration

**Challenge**: What happens when two registrations for the same alias appear in the same block?

**Current approach**: Topic Managers use transaction ordering within the block (first seen wins). But the UX during the confirmation period (~10 minutes) needs design - is the name "pending"? Should wallets warn users that registration isn't final until confirmed?

**Feedback requested**: How should wallets handle the pending state? Should Topic Managers enforce a minimum confirmation depth before advertising a name?

### Q3: Conflict Resolution Between Nodes

**Challenge**: If two Lookup Services temporarily disagree about name ownership (due to sync delay or network partition), how does a wallet resolve this?

**Current thinking**: Wallets use reputable servers and look for costly trust signals. Users choose providers who invest in long-term viable businesses. Ultimately, the blockchain is the authoritative source and nodes can verify against it.

**Feedback requested**: Should wallets query multiple nodes and require consensus? What's the acceptable sync delay tolerance?

### Q4: Suffix Governance

**Challenge**: Who controls which suffixes (`@paymail`, `@bitcoin`, `@bsv`) are recognized as vendor-neutral protocol identifiers?

**Current approach**: A registry mechanism needs to be designed and implemented as part of the standardization process.

**Feedback requested**: Should this be a fixed list in the protocol spec? An on-chain registry? A community governance process?

### Q5: Economic Bootstrap

**Challenge**: The system needs initial operators and users to reach viability. This is a chicken-and-egg problem.

**Current thinking**: The bootstrap investment itself is a costly trust signal. Early operators who invest in infrastructure demonstrate commitment. Cooperative fee structures during bootstrap phase encourage adoption. As the network grows, competitive dynamics emerge naturally.

**Feedback requested**: What incentives would attract early operators? Are there grant/subsidy models that don't compromise decentralization?

### Q6: Protocol Versioning

**Challenge**: How do we handle protocol upgrades when the PushDrop format or validation rules need to change?

**Current approach**: Version field (`0x01`) is included in registration data. Upgrade path and backwards compatibility strategy needs definition.

**Feedback requested**: How should Topic Managers handle unknown versions? Should there be a deprecation timeline for old versions?

---

## References

### BSV Standards (BRC)
- [BRC-22: Overlay Network Data Synchronization](https://bsv.brc.dev/overlays/0022)
- [BRC-23: Confederacy Host Interconnect Protocol (CHIP)](https://bsv.brc.dev/overlays/0023)
- [BRC-24: Overlay Network Lookup Services](https://bsv.brc.dev/overlays/0024)
- [BRC-25: Confederacy Lookup Availability Protocol (CLAP)](https://bsv.brc.dev/overlays/0025)
- [BRC-26: Universal Hash Resolution Protocol](https://bsv.brc.dev/overlays/0026)
- [BRC-28: Paymail Payment Destinations](https://bsv.brc.dev/payments/0028)
- [BRC-29: Simple Authenticated BSV P2PKH Payment Protocol](https://bsv.brc.dev/payments/0029)
- [BRC-33: PeerServ Message Relay Interface](https://bsv.brc.dev/peer-to-peer/0033)
- [BRC-41: HTTP Service Monetization Framework](https://bsv.brc.dev/payments/0041)
- [BRC-42: BSV Key Derivation Scheme](https://bsv.brc.dev/key-derivation/0042)
- [BRC-43: Security Levels, Protocol IDs, Key IDs and Counterparties](https://bsv.brc.dev/key-derivation/0043)
- [BRC-58: Background Evaluation Extended Format (BEEF) Transactions](https://bsv.brc.dev/transactions/0058)
- [BRC-70: Paymail BEEF Transaction](https://bsv.brc.dev/payments/0070)
- [BRC-83: Scalable Transaction Processing in the BSV Network](https://bsv.brc.dev/transactions/0083)
- [BRC-84: Linked Key Derivation Scheme](https://bsv.brc.dev/key-derivation/0084)
- [BRC-87: Standardized Naming Conventions for BRC-22 Topic Managers and BRC-24 Lookup Services](https://bsv.brc.dev/overlays/0087)
- [BRC-88: Overlay Services Synchronization Architecture](https://bsv.brc.dev/overlays/0088)
- [BRC-100: Unified Abstract Wallet-to-Application Messaging Layer](https://bsv.brc.dev/wallet/0100)

### General References
- [SPV Wallet (BSV Association)](https://github.com/bitcoin-sv/spv-wallet)
- [BSV Overlay Networks](https://docs.bsvblockchain.org/network-topology/overlay-services)

---

**Document Status**: Architecture Proposal for Peer Review & Feedback
**Last Updated**: January 2026
