# @xanaverse/contracts

BSV smart contracts for trustless social media. Covenant-enforced posts, upvotes, and replies where payment rules are validated by the Bitcoin network, not platforms.

## Install

```bash
npm install @xanaverse/contracts
```

**Peer dependency:** Requires `scrypt-ts` ^1.4.0

## Quick Start

```typescript
import {
  PostAnchor,
  PostAnchorArtifact,
  parsePostAnchor,
  PROTOCOL_ID
} from '@xanaverse/contracts'

// Load artifact before using contract
PostAnchor.loadArtifact(PostAnchorArtifact)

// Parse a post from on-chain data
const post = parsePostAnchor(lockingScriptHex)
console.log(post.content, post.creatorIdentityKey)
```

## Exports

### Contract Classes

| Contract | Purpose |
|----------|---------|
| `PostAnchor` | Immutable posts with content hash verification |
| `PostAnchorV0` | Legacy post format (v0 compatibility) |
| `UpvoteProof` | Upvote proofs with covenant-enforced creator payments |
| `ReplyProof` | Reply proofs with covenant-enforced parent payments |
| `DownvoteProof` | Downvote proofs |

### Artifacts

Compiled Bitcoin Script for each contract. Load before instantiating:

```typescript
import {
  PostAnchorArtifact,
  PostAnchorV0Artifact,
  UpvoteProofArtifact,
  ReplyProofArtifact,
  DownvoteProofArtifact
} from '@xanaverse/contracts'

PostAnchor.loadArtifact(PostAnchorArtifact)
UpvoteProof.loadArtifact(UpvoteProofArtifact)
```

### Protocol Constants

```typescript
import {
  PROTOCOL_ID,
  PROTOCOL_KEY_ID,
  PROTOCOL_COUNTERPARTY,
  PROTOCOL_BASKETS,
  PROTOCOL_SHARED_CONFIG
} from '@xanaverse/contracts'

// BRC-42 key derivation
PROTOCOL_ID           // [0, 'xanaverse']
PROTOCOL_KEY_ID       // '1'
PROTOCOL_COUNTERPARTY // 'self'

// UTXO basket names (BRC-100)
PROTOCOL_BASKETS.posts     // 'xanaverse-posts'
PROTOCOL_BASKETS.upvotes   // 'xanaverse-upvotes'
PROTOCOL_BASKETS.replies   // 'xanaverse-replies'
PROTOCOL_BASKETS.downvotes // 'xanaverse-downvotes'
```

## Parsing Posts

Version-aware parsing handles both v0 (legacy) and v1 posts:

```typescript
import {
  parsePostAnchor,
  isPostAnchor,
  getPostAnchorInstance
} from '@xanaverse/contracts'
import type { ParsedPostAnchor } from '@xanaverse/contracts'

// Check if script is a PostAnchor
if (isPostAnchor(lockingScript)) {
  const post: ParsedPostAnchor = parsePostAnchor(lockingScript)

  console.log(post.version)               // 0 | 1
  console.log(post.content)               // Post text
  console.log(post.creatorProtocolPubKey) // BRC-42 derived key
  console.log(post.creatorIdentityKey)    // Root identity
  console.log(post.contentHash)           // SHA-256 hash
  console.log(post.createdAt)             // Timestamp
  console.log(post.parentHash)            // Parent post (for threads)

  // v1 only
  if (post.version === 1) {
    console.log(post.tags)                // Comma-separated tags
  }
}

// Get raw contract instance (for advanced operations like reclaim)
const { version, contract } = getPostAnchorInstance(lockingScript)
```

## Contract Properties

### PostAnchor (v1)

```typescript
interface PostAnchorState {
  creatorProtocolPubKey: PubKey    // BRC-42 derived identity
  creatorIdentityKey: ByteString   // Root identity (display name)
  content: ByteString              // Post text (max 280 chars)
  contentHash: ByteString          // SHA-256 of content
  nonce: ByteString                // Privacy nonce
  createdAt: bigint                // Unix timestamp
  parentHash: ByteString           // Parent post hash (threads)
  tags: ByteString                 // Comma-separated tags (v1 only)
  targetDifficulty: bigint         // PoW target (immutable)
  version: bigint                  // Contract version
}
```

### UpvoteProof

```typescript
interface UpvoteProofState {
  upvoterProtocolPubKey: PubKey    // Who upvoted (BRC-42)
  upvoterIdentityKey: ByteString   // Upvoter display name
  contentHash: ByteString          // Post being upvoted
  creatorProtocolPubKey: PubKey    // Post creator (payment recipient)
  creatorIdentityKey: ByteString   // Creator display name
  paymentAmount: bigint            // Sats paid to creator (min 100)
  nonce: ByteString                // Privacy nonce
}
```

### ReplyProof

```typescript
interface ReplyProofState {
  replierPubKey: PubKey            // Who replied (BRC-42)
  replier: ByteString              // Replier display name
  replyContent: ByteString         // Reply text
  replyContentHash: ByteString     // SHA-256 of reply
  parentContentHash: ByteString    // Parent being replied to
  parentProtocolPubKey: PubKey     // Parent creator (payment recipient)
  parentIdentityKey: ByteString    // Parent creator display name
  paymentAmount: bigint            // Sats paid to parent (min 50)
  createdAt: bigint                // Unix timestamp
  nonce: ByteString                // Privacy nonce
}
```

## Usage Examples

### Parse Contract from Transaction Output

```typescript
import { PostAnchor, PostAnchorArtifact } from '@xanaverse/contracts'

// Load artifact once at startup
PostAnchor.loadArtifact(PostAnchorArtifact)

// Parse from locking script
const post = PostAnchor.fromLockingScript(lockingScriptHex)
console.log(post.content.toString())
console.log(post.creatorProtocolPubKey.toString())
```

### Derive Earnings from Proof UTXOs

```typescript
import { UpvoteProof, ReplyProof } from '@xanaverse/contracts'

// Parse actual payment amounts (don't estimate!)
let totalEarnings = 0n

for (const utxo of upvoteUTXOs) {
  const proof = UpvoteProof.fromLockingScript(utxo.script)
  totalEarnings += proof.paymentAmount  // Could be 100, 150, 200+ sats
}

for (const utxo of replyUTXOs) {
  const proof = ReplyProof.fromLockingScript(utxo.script)
  totalEarnings += proof.paymentAmount  // Could be 50, 75, 100+ sats
}
```

### Use Protocol Constants for Wallet Integration

```typescript
import { PROTOCOL_SHARED_CONFIG } from '@xanaverse/contracts'

// Get XanaVerse protocol key (BRC-42)
const { publicKey } = await wallet.getPublicKey({
  protocolID: PROTOCOL_SHARED_CONFIG.protocolID,
  keyID: PROTOCOL_SHARED_CONFIG.keyID,
  counterparty: PROTOCOL_SHARED_CONFIG.counterparty
})
```

## TypeScript Types

```typescript
import type { ParsedPostAnchor } from '@xanaverse/contracts'

// Full type definitions included for all exports
```

## Architecture

These contracts implement the XanaVerse trustless social media protocol:

- **Posts** create immutable content anchors with cryptographic authorship
- **Upvotes** create proof UTXOs with covenant-enforced creator payments
- **Replies** create proof UTXOs with covenant-enforced parent payments
- **Counts** are derived by querying proof UTXOs (indexers can't fake them)

All payment rules are enforced by Bitcoin Script at the network level.

## Related

- [XanaVerse Protocol](https://github.com/xanaverse/xanaverse) - Full protocol implementation
- [sCrypt Documentation](https://docs.scrypt.io/) - Smart contract framework

## License

MIT
