# Vendor-Neutral Paymail Architecture

## Abstract

A decentralized, confederated payment addressing system that enables human-readable identifiers (`alice@paymail`) without domain dependencies. The system uses IPv6-native overlay networks, BSV blockchain for registration, and established BRC standards for interoperability. **Confederation enables vendor-neutrality** - multiple independent operators run overlay service nodes, ensuring no single vendor controls the system. Payments authenticate operations through micropayments, eliminating API keys and vendor lock-in. **The system works with both IPv4 and IPv6 wallets** - IPv4-only wallets use overlay service nodes as intermediaries, while IPv6-capable wallets can connect directly when both parties have IPv6 connectivity.

---

## Vision

**Core Principles:**
- **No domains required** - Protocol identifiers replace DNS
- **No API keys** - Micropayment-based authentication
- **No vendor lock-in** - Any wallet can implement the protocol
- **True peer-to-peer** - Direct IPv6 connections when available
- **Standards-based** - Built on established BRC protocols

**Example**: `alice@paymail` - The `@paymail` suffix signals overlay resolution, not DNS lookup.

---

## Architecture Overview

The system consists of three layers:

1. **Wallet Layer** - Recognizes paymail addresses and queries overlay services (works with IPv4 or IPv6)
2. **Overlay Network** - BRC-22 Topic Managers validate transactions, BRC-24 Lookup Services index aliases
3. **Blockchain** - Permanent, censorship-resistant registry of paymail registrations

Wallets query BRC-24 Lookup Services to resolve aliases to public keys and IPv6 addresses. IPv4-only wallets use overlay service nodes as intermediaries, while direct peer-to-peer connections occur when both parties have IPv6 connectivity. BRC-33 message relay handles offline delivery.

---

## IPv6 Connectivity Model

### Why IPv6 Addresses

The system uses IPv6 addresses as destination identifiers rather than domain names. This aligns with Satoshi's IP-to-IP transaction vision while maintaining practical compatibility. Benefits include:
- **Abundant addresses** - Every device can have a unique global address
- **No NAT traversal** - Direct peer-to-peer connections when both parties have IPv6 (note: some ISPs use Carrier-Grade NAT even with IPv6; true P2P depends on ISP configuration)
- **Free on cloud** - No additional cost for IPv6 addresses
- **True P2P potential** - Direct connections eliminate intermediaries when available

### IPv4/IPv6 Compatibility

**Important**: IPv4 and IPv6 are separate protocols and cannot directly communicate. However, wallets do not require IPv6 connectivity to use the system, and **no NAT is needed** for wallet-to-overlay-node communication.

**How it works**:
- **IPv4-only wallets**: Send HTTP requests to overlay service nodes using IPv4 addresses
- **Overlay service nodes**: Have dual-stack connectivity (both IPv4 and IPv6), allowing them to:
  - Receive IPv4 requests from wallets (no NAT needed - standard HTTP)
  - Make IPv6 requests to recipient IPv6 addresses on behalf of wallets
  - Handle all IPv6 communication as intermediaries
- **Direct IP-to-IP**: Only occurs when both parties have IPv6 and are online

**Communication Flow**:
1. IPv4-only wallet → IPv4 request → Overlay node (dual-stack) → IPv6 request → Recipient IPv6 address
2. Response flows back: Recipient → Overlay node → Wallet (all via standard HTTP, no NAT)

**Practical implications**:
- **Sending wallet (IPv4-only)**: Sends standard HTTP requests to overlay nodes via IPv4, receives responses via IPv4. No NAT required.
- **Receiving wallet (IPv4-only)**: Receives payments via BRC-33 relay, which overlay nodes handle
- **Overlay nodes**: Must have dual-stack (IPv4 + IPv6) to serve both IPv4 wallets and enable IPv6 P2P
- **True IP-to-IP**: Only occurs when both parties have IPv6 connectivity and are online

**No NAT required**: Wallets communicate with overlay nodes using standard HTTP over IPv4 or IPv6. The overlay nodes handle protocol translation transparently. This design optimizes for IPv6 while maintaining full functionality for IPv4-only wallets.

---

## Registration Process

### Transaction Format

Users register paymail aliases by creating on-chain transactions using PushDrop format to embed registration data in transaction outputs. The PushDrop script contains:
- Protocol identifier: `"paymail"`
- Version: `0x01`
- Action: `"register"`
- Alias: The desired identifier (e.g., `"alice"`)
- Public key: 33-byte compressed secp256k1 key (derived via BRC-42/43)
- Optional IPv6 address: For direct P2P connectivity
- Optional relay preferences: Fallback nodes for offline delivery

PushDrop allows structured data to be embedded in spendable outputs without affecting spending rules. Unlike OP_RETURN (which creates unspendable outputs), PushDrop tokens can be transferred, traded, or revoked by spending them - giving users full control over their registration via their private key.

**Transaction structure:**
- **Output 0**: Registration fee (1000 sats) paid to overlay operator
- **Output 1**: PushDrop token (1 sat) containing registration data - this IS the paymail registration

The fee payment and registration are atomic - both occur in the same transaction.

### Key Derivation

Paymail identities use BRC-42/43 key derivation for secure key management. Keys are derived from the wallet's master key using standardized protocol IDs.

**Optional: BRC-84 Linked Key Derivation** - Enables deriving child public keys from only the master public key, without requiring private key access. Useful for non-custodial scenarios where a service needs to generate addresses but should never hold keys. Derived keys remain cryptographically linked to the master for auditability.

### Topic Manager Validation

Wallets submit registration transactions to BRC-22 Topic Managers, which validate and admit them. Topic Managers enforce these rules:

- **Alias format**: Alphanumeric characters, underscores, and hyphens only
- **Uniqueness**: First valid registration seen on-chain (in a block) wins - blockchain provides canonical ordering for race conditions
- **Signature verification**: Transaction must be signed by the key embedded in the registration data
- **Fee validation**: Sufficient registration fee must be included in Output 0

Topic Managers reject invalid or duplicate registrations. Once a registration is mined into a block, that alias is permanently claimed - the blockchain's ordering resolves any race conditions.

### Lookup Service Indexing

BRC-24 Lookup Services index admitted transactions by alias for fast resolution. Services synchronize with peer nodes using BRC-88 overlay synchronization protocols. The lookup service maintains a queryable index of all valid paymail registrations.

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
3. Lookup service returns public key and optional IPv6 address
4. Wallet connects directly via IPv6 (if both parties have IPv6) or uses BRC-33 relay

### Overlay Service Nodes

Wallets require initial overlay service nodes (Topic Managers and Lookup Services) to begin querying. These are standard BRC-22/BRC-24 overlay nodes.

**Node Discovery Approaches**:

**Hardcoded List**: Simple fallback list of overlay service nodes embedded in wallet software. Requires wallet updates to add nodes.

**On-Chain Registry**: Overlay service nodes advertised in well-known blockchain transactions via BRC-23 (CHIP). Fully decentralized and self-updating.

**Hybrid**: Hardcoded fallback with periodic on-chain updates. Provides resilience with graceful degradation.

**Note on Trust**: Overlay service nodes are standard overlay infrastructure. The low entry cost (~$50/month for VPS) does not provide significant costly signaling. Trust derives from operational transparency, on-chain node advertisements, and the ability for users to choose among multiple service providers. The decentralized nature of the overlay network reduces reliance on any single node operator.

---

## Payment Delivery

### Direct Peer-to-Peer

When both parties have IPv6 connectivity and are online, wallets can connect directly without intermediaries. This enables true IP-to-IP transactions as envisioned by Satoshi. The payment flow follows standard paymail protocols (BRC-28) but uses direct IPv6 connections instead of HTTP servers.

**Note**: Direct IP-to-IP requires both parties to have IPv6 connectivity. Wallets without IPv6 use overlay service nodes as intermediaries, which still provides the benefit of IPv6 address-based routing without requiring IPv6 from the wallet itself.

### BRC-33 Message Relay

When recipients are offline or lack IPv6 connectivity, senders use BRC-33 PeerServ message relay. The relay stores transactions until recipients poll for messages. This provides store-and-forward capability without custom relay protocols.

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

## Micropayment Economics

### Operation Costs

| Operation | Cost (sats) | Who Pays |
|-----------|------------|----------|
| Register alias | 1000 | User (on-chain) |
| Lookup | 1 | Requesting wallet |
| Payment destination | 5 | Sender wallet |
| Relay message | 10 | Sender wallet |
| Direct P2P | 0 | Free |

### Payment Validation

Wallets include micropayments in request headers using BRC-41 HTTP Service Monetization patterns. Overlay nodes validate payments before processing requests, ensuring sustainable operation through micropayment funding.

### Economic Sustainability

Operator costs: $5-30/month per node (VPS + bandwidth). Revenue scales with usage volume. Break-even point approximately 30,000 active users per node. Micropayments enable sustainable operation without subscription fees.

---

## Security Model

### Identity Verification

Registration transactions are signed by registrant's keys using BRC-42/43 derivation. Key updates require signatures from current keys. On-chain records provide tamper-proof proof of ownership.

**Certificates Optional**: BRC-52 certificates can enhance identity verification but are not required for basic paymail functionality. The system prioritizes simplicity while allowing optional certificate integration.

### Squatting Prevention

Topic Manager validation enforces first-come-first-served registration. Registration fees (1000 sats) discourage mass squatting. On-chain proof prevents disputes. Alias format restrictions limit abuse.

### Spam Prevention

Micropayments required for every operation. Rate limiting per IPv6 address. Higher fees for suspicious patterns. Double-spend protection through immediate transaction broadcasting and TXID tracking.

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
| [BRC-84](https://bsv.brc.dev/key-derivation/0084) | Linked Keys | Enhanced privacy |

**Note on BRC-31**: BRC-31 (Authrite) may be deprecated in favor of BRC-103/104 for authentication. Topic Managers use BRC-31 for transaction submission authentication, but future implementations should consider BRC-103/104 alternatives.

---

## Comparison: Traditional vs IPv6-Native Paymail

| Aspect | Traditional Paymail | IPv6-Native Paymail |
|--------|--------------------|--------------------|
| Identity format | `alice@domain.com` | `alice@paymail` |
| Resolution | DNS lookup | Overlay lookup |
| Dependency | Registrars, ICANN | Blockchain only |
| Censorship | Domain seizure possible | Very difficult |
| Cost (operator) | $119-179/year | $60/year |
| Cost (user) | Vendor-dependent | ~$0.01/year |
| Direct P2P | No (always via server) | Yes (when both online) |
| Offline support | Always online server | BRC-33 relay fallback |
| BSV-native | No | Yes |

---

## Design Rationale

### Why Overlay Instead of DNS

DNS requires domain registration, creating centralization points and censorship risks. Overlay networks use blockchain as the source of truth, eliminating registrar dependencies. Protocol identifiers (`@paymail`) signal resolution method without requiring domain ownership.

### Why IPv6 Addresses

The system uses IPv6 addresses as destination identifiers to enable direct peer-to-peer connections when both parties have IPv6 connectivity. This aligns with Satoshi's IP-to-IP transaction vision. Wallets themselves do not require IPv6 - they can send to IPv6 addresses via overlay service nodes. Direct IP-to-IP connections occur only when both parties have IPv6 and are online, providing optimal performance while maintaining full functionality for IPv4-only wallets through relay services.

### Why Micropayments

BSV's low transaction fees make micropayment-funded services viable. Payment-as-authentication eliminates API keys and simplifies wallet integration. Micropayments scale with usage, creating sustainable economic models.

### Why BRC Standards

Leveraging existing BRC standards ensures interoperability and reduces implementation complexity. Standards provide proven patterns for overlay networks, message relay, and key management. Vendor-neutral standards prevent lock-in.

---

## Open Questions for Peer Review

The following topics would benefit from community input:

### Alias Expiration

**Current proposal**: Registrations last forever. Abandoned aliases remain claimed indefinitely.

**Question**: Should registrations expire if not renewed? This prevents alias hoarding but adds complexity. Alternatively, the registration fee could be set high enough to discourage mass squatting while keeping the system simple.

*Seeking opinions on expiration policy.*

### Trademark and Impersonation Disputes

**Current proposal**: First-come-first-served with no dispute mechanism.

**Question**: How should trademark conflicts be handled? This is a legal question as much as a technical one. Options include:
- Pure first-come-first-served (simple, but invites squatting)
- Dispute resolution process (complex, who decides?)
- Higher fees for "premium" short aliases (economic deterrent)
- Accept that this is a social/legal problem, not a technical one

*Seeking opinions on dispute handling - or whether it should be explicitly out of scope.*

### Suffix Standardization

**Current proposal**: Wallets recognize `@paymail`, `@bitcoin`, and `@bsv` as protocol identifiers.

**Question**: Should there be a single canonical suffix, or multiple? If multiple, are they aliases for the same namespace (so `alice@paymail` = `alice@bitcoin`) or separate namespaces?

The suffix must signal to wallets: "This is not a DNS domain - resolve via overlay network." Standardization ensures interoperability.

*Seeking opinions on preferred suffix(es) and whether they should be aliased or distinct.*

### Multiple Registrations Per User

**Current proposal**: Users can register multiple aliases. Each is an independent on-chain token.

**Implication**: Early adopters may claim common names speculatively. This mirrors domain name speculation - some will profit, some will lose money on names that never gain value.

*Note: The authors lost money on MoneyButton paymails that became worthless when the service shut down. This illustrates the risk of identity systems dependent on single vendors - and why vendor-neutral, on-chain registration matters.*

### Revocation Mechanism

**Current proposal**: Users revoke registrations by spending the PushDrop token. Spending the token removes it from the UTXO set, effectively "deleting" the registration.

**Question**: Should spent aliases become available for re-registration, or remain permanently claimed? Allowing re-registration could enable "alias recycling" but might cause confusion if someone else claims a previously-used identity.

*Seeking opinions on alias lifecycle after revocation.*

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
- [BRC-87: Standardized Naming Conventions for BRC-22 Topic Managers and BRC-24 Lookup Services](https://bsv.brc.dev/overlays/0087)
- [BRC-88: Overlay Services Synchronization Architecture](https://bsv.brc.dev/overlays/0088)
- [BRC-100: Unified Abstract Wallet-to-Application Messaging Layer](https://bsv.brc.dev/wallet/0100)

### General References
- [SPV Wallet (BSV Association)](https://github.com/bitcoin-sv/spv-wallet)
- [IPv6 and Bitcoin - CoinGeek](https://coingeek.com/ipv6-and-bitcoin-were-made-for-each-other-while-btc-misses-out/)
- [Satoshi's P2P Vision - BSV Blockchain](https://bsvblockchain.org/realising-finally-satoshi-peer-to-peer-vision-for-bitcoin/)
- [BSV Overlay Networks](https://docs.bsvblockchain.org/network-topology/overlay-networks)

---

**Document Status**: Architecture Proposal for Peer Review
**Last Updated**: January 2025
