# C3 — go-private-backup-cache (deggen's encrypted delta-chunk cache)

**Task:** work item 0b(b) — determine whether the blob is toolbox-native or opaque; pin down log,
generation, encryption and auth semantics; map overlap with our on-chain token design.

> **LICENSE WARNING — restated per task instructions.** The repository **root has no LICENSE
> file** (verified by directory listing at HEAD `ca2136e`, 2026-08-21). Treat everything in it as
> all-rights-reserved: read for **format and semantics only**. Never vendor, copy, or adapt code
> from it into anything of ours. One nuance the task brief didn't have: the `ts-client/`
> subdirectory *does* carry a license — **Open BSV License Version 6, (c) 2026 BSV Association**
> (`ts-client/LICENSE`). That license is itself restrictive (BSV-chain-only terms) and does not
> cover the Go code. The read-only rule stands for the whole repo.

Repo state examined: `https://github.com/bsv-blockchain/go-private-backup-cache`, shallow clone,
HEAD `ca2136e3be093c806692e91abf82975a54ea269e` (2026-08-21, "feat: OpenTelemetry traces...").
All line references below are to this commit. **Re-verified 2026-08-22 by a second independent
read** of the same clone: every load-bearing claim below (opaque blob contract, server-enforced
seq contiguity, prevSha256 stored-not-verified, retention guard arithmetic, auth mechanics,
ts-client license nuance, nonce ON CONFLICT, ciphertext-hash chaining convention in
`client_test.go`) was re-checked against the code and held.

---

## 1. THE question — is the blob toolbox-native or opaque?

**VERIFIED: the blob is OPAQUE under the client contract.** It is client-defined bytes the server
never interprets. There is no BRC-38 row form, no BRC-40 SyncChunk shape, no wallet-toolbox type
anywhere in the protocol surface.

Evidence, in order of authority:

**The client API signatures take raw bytes/streams, nothing structured.**

Go client (`client/client.go:172`):

```go
func (c *Client) Append(ctx context.Context, deviceID string, generation, seq int,
    prevSha256 string, body io.Reader, bodySha256 string, size int64) (AppendResult, error)
```

and the convenience form (`client/client.go:197`): `AppendBytes(..., data []byte)`.
Download side (`client/client.go:205`): `Blob(...) (io.ReadCloser, int64, error)` — a byte stream.

TypeScript client (`ts-client/src/client.ts:116-122`):

```ts
async append (deviceId: string, generation: number, seq: number,
  prevSha256: string | undefined, body: Uint8Array | Blob)
```

and `blob(): Promise<Uint8Array>` / `blobStream(): Promise<ReadableStream<Uint8Array>>`
(`ts-client/src/client.ts:134, 144`).

**The store is documented as never interpreting contents** (`internal/blobstore/blobstore.go:1-7`):

> "Package blobstore persists opaque encrypted blobs. Nothing in this package interprets blob
> contents. The bytes arrive encrypted under a key derived from the client's wallet seed with
> counterparty "self", so the server is structurally incapable of reading them — there is no
> decrypt path here, and there must never be one."

**The wire is raw octet-stream** (`internal/server/handlers/append.go:14-16, 31-35`): the only
accepted Content-Type is `application/octet-stream`; "Raw binary rather than base64-in-JSON —
there is no base64 fallback by design." The server hashes the stream and stores 1 MiB chunk rows
(`internal/blobstore/postgres.go:20, 131-152`); at no point does it parse the bytes.

**No toolbox references exist.** `grep -rni "syncchunk|brc-38|brc-40|toolbox"` over all `.go`,
`.ts`, `.md` files (excluding lockfiles) returns nothing. The only content hint in the entire repo
is prose in `README.md:12`: "Wallets push their database to it as encrypted delta chunks; a
reinstalled wallet replays them and is whole again" — an intended use, not a contract. The
ts-client README says the same from the client side (`ts-client/README.md:10`): "The server only
ever sees ciphertext and a pseudonymous identity key; **encrypt before you append**."

**Consequence for 0b(b):** the payload format question is entirely open on their side — whatever
a wallet chooses to put in a chunk is compatible with this rail. If our delta format (item 3) and
the BRC's snapshot payload (BRC-38-shaped) are what a wallet produces, the *same encrypted chunks*
can feed both rails with zero changes to their protocol. Nothing in this service constrains us;
equally, nothing in it gives us a delta *format* to harmonize with — only delta *log semantics*
(section 2).

## 2. Log semantics, precisely

The unit of organization is `(pseudonym, deviceId, generation)` → an append-only log of blobs at
1-based sequence numbers.

- **Pseudonym** = the authenticated caller's compressed public key, DER hex. It is *always* taken
  from the verified auth proof, never from any request field (`internal/blobstore/blobstore.go:43-45`,
  `internal/server/handlers/handlers.go:25-32`, `internal/server/middlewares/identity.go:156-164`).
  There is no identity parameter anywhere in the API.
- **deviceId** = client-generated opaque token matching `^[a-f0-9]{32}$` — "16 random bytes as
  lowercase hex" (`internal/server/handlers/handlers.go:18`). Path segment on every log route.
  So yes: **per-device logs**, keyed inside one account.
- **seq** = positive integer, 1-based, contiguous within a `(pseudonym, deviceId, generation)`
  (README API section; enforced as below).

### Contiguity — enforced by the SERVER (VERIFIED)

`internal/blobstore/postgres.go:104-166`: Append runs in one transaction — `SELECT MAX(seq)` for
the tuple, require `k.Seq == head+1` (or `1` on an empty log), else
`ErrSeqConflict` → HTTP `409 ERR_SEQ_CONFLICT` (`append.go:60-61`). Because the head check runs
under READ COMMITTED, a race between two appends is settled by the primary key
`(pseudonym, device_id, generation, seq)`; a unique violation (`23505`) is mapped back onto the
same `ErrSeqConflict` (`postgres.go:168-182`), so racing writers always see 409, never a 500. The
memory store enforces identically with a double check around the lock (`memory.go:47-73`). Client
guidance on 409 is "resynchronise from the index, don't retry" (`ts-client/src/client.ts:47-48`,
README example).

Rationale comment worth keeping (`blobstore.go:17-20`): "Appends must be contiguous: a gap would
leave a silent hole in a restore, and an overwrite would destroy a backup entry."

### prevSha256 chaining — of CIPHERTEXT, stored by the server, verified only by the client

- What is chained: the previous entry's **sha256 of the stored blob bytes** — i.e. the
  **ciphertext** hash, since the client encrypts before appending and the server computes sha256
  over the streamed body (`postgres.go:131-156`). Convention confirmed by the Go client's own test
  (`client/client_test.go:140-147`): each append passes `prev = previous AppendResult.Sha256`,
  where `Sha256` is "the server's own hash of what it stored" (`client.go:64-66, 170-171`).
- First entry of a generation: `prevSha256` empty (`client/client.go:167-168`: "prevSha256 chains
  the entry to its predecessor and is empty for the first entry of a generation").
- **The server does NOT validate it** (VERIFIED by reading both store implementations): Postgres
  `Append` inserts the client-supplied `prev` into `blob_log.prev_sha256` without comparing it to
  the head row's `sha256` (`postgres.go:158-161`); the memory store likewise (`memory.go:71`). It
  is not even shape-checked (any string the query param carries goes in). It is stored metadata,
  reported back verbatim in the index (`Entry.PrevSha256`), for the **client** to verify chain
  integrity at restore time. So the tamper-evidence model is: server enforces *ordering*
  (contiguous seq), client enforces *content linkage* (hash chain) — INFERRED as to intent, but
  the code division is VERIFIED.

### Generations

- A generation is just a positive-integer coordinate. There is **no "create generation" call and
  no server rule that a new generation be `newest+1`** — Append accepts any
  `1 ≤ generation ≤ 2^30` (`handlers.go:50-56`). A generation "starts" implicitly when its first
  blob is appended at `seq=1` with empty `prevSha256`.
- **Snapshot-at-generation-start is a client convention, not a server rule** — the server cannot
  check it, the blobs being opaque. The documented compaction protocol (README "Retention";
  `internal/server/handlers/prune.go:160-162`): "Compaction is client-driven: write a full
  snapshot as generation N+1, then delete N-2."
- **Retention guard, keep 2:** `RetainedGenerations = 2` (`blobstore.go:33-39`) — "the current one
  and the previous one... Two rather than one, so that a compaction which fails partway never
  leaves a user with zero recoverable backups." `DeleteGeneration` refuses any
  `generation > MAX(generation) - 2` for that `(pseudonym, device)` with `409
  ERR_RETENTION_GUARD` (`postgres.go:337-348`). Note the guard is computed per device from
  `MAX(generation)`, not from a count of existing generations — with generations 1 and 5 present,
  1..3 are deletable, 4 and 5 are protected (INFERRED from the arithmetic; no test read for this
  exact case).
- **No time-based expiry, deliberately** (README; `prune.go:164-166`): "A pseudonym that has not
  been written to for years belongs to precisely the user this service exists for."

### The 200 MiB cap

`DefaultMaxBlobBytes = 200 << 20` (`internal/config/config.go:14-21`), overridable via
`MAX_BLOB_BYTES`, published unauthenticated at `GET /v1/limits` together with the server identity
key (`handlers/limits.go`) so clients read it instead of hardcoding. Oversize uploads get
`413 ERR_BLOB_TOO_LARGE` from a size guard that sits **before** auth (README Operational notes;
`server.go` sizeGuard). Blobs stream both directions through a 1 MiB buffer into/out of
`blob_chunks` rows — server memory does not scale with blob size (`postgres.go:20, 131-152,
199-266`). Empty blobs are refused (`ErrEmptyBlob`, `400 ERR_EMPTY_BLOB`).

### Erase semantics

Two distinct destructive routes, deliberately separate (`handlers/erase.go:206-227` comment):

- `DELETE /v1/generation/{deviceId}/{generation}` — compaction path, retention guard applies,
  `204` on success, `404 ERR_GENERATION_NOT_FOUND` if empty.
- `DELETE /v1/account` — GDPR Art. 17 path: every blob for the pseudonym across all devices and
  generations, **retention guard does NOT apply**, single transaction all-or-nothing
  (`postgres.go:378-406`), idempotent (`200 {"deleted": n}`, zero on an already-empty account so a
  client that lost the response can retry).

### Restore flow

Manifest is the entry point (`client/client.go:240-243`): "a fresh install derives its key from
the recovered seed, calls this, and picks a device and generation to replay." Manifest returns
per-`(device, generation)` heads: `deviceId, generation, headSeq, headSha256, totalBytes,
updatedAt` (`blobstore.go:62-70`). Then `Index` (pages of ≤500 entries, oldest first) and `Blob`
per seq. Note: restore *picks* a device log to replay — the server offers no cross-device merge;
combining logs from several devices is entirely the client's problem (VERIFIED absence: no such
endpoint or code).

## 3. Encryption

**VERIFIED (as an absence): there is no encryption code in this repository.** Neither client
encrypts; both READMEs put encryption on the caller ("encrypt before you append",
`ts-client/README.md:10`). Grep for `encrypt` across `client/`, `ts-client/src/`, `internal/`
finds only comments/prose.

What the docs *claim* about key derivation (prose only, no code to verify against):

- `README.md` ("What the server can and cannot see"): "Blobs are encrypted before they arrive,
  with a key derived from the client's wallet seed using **counterparty `self`**. Nobody but that
  seed holder can derive it." Echoed in `blobstore.go:3-5`.
- **No BRC-42/43 protocol string for encryption is specified anywhere** — the only protocol
  string in the repo is the *auth* one, `[2, "backup cache auth"]`. So "BRC-2-style encryption to
  self" is the stated intent, but protocol ID, key ID and ciphertext framing for the blob
  encryption are unspecified. If we ever wanted chunk-level compatibility, there is nothing here
  to be compatible *with* yet — a real gap in their spec, and a place a common form could be
  proposed.

What the server sees vs not: sees pseudonym (one stable public key per account), deviceIds,
generation/seq structure, ciphertext sizes and sha256s, timestamps, plus transport metadata (the
README is unusually honest: source IP "is the real residual and it is not small"). Cannot see:
plaintext, wallet identity key (the pseudonym key is a *separate* key derived from the seed — the
ts README example calls it `backupPseudonymKeyHex`). Note the pseudonym is stable across all of a
user's requests and devices — this is pseudonymity toward the *on-chain identity*, not
unlinkability of requests from each other (README says exactly this).

## 4. Auth — the per-request proof, mechanically

This replaced BRC-103/104 mutual-auth middleware: "no handshake, no session state, no signed
response envelope, no `/.well-known/auth` route" (`docs/authproof-protocol.md:3-5`).

**One header per request:** `X-Bsv-Auth: base64(JSON)` with fields
`{action, identityKey, expiresAt, nonce, signature}` (`internal/authproof/authproof.go:59-65`).

**The action string** (`authproof.go:71-76`):

```
<METHOD> <request-URI>                      bodyless
<METHOD> <request-URI> sha256=<body hex>    uploads
```

`request-URI` is the literal path-plus-query as sent on the wire — no re-encoding, no parameter
sorting; the server rebuilds the expected string from `r.Method` + `r.URL.RequestURI()` verbatim
(`middlewares/authproof.go:67`). Binding method+URI means a proof for one route/seq/generation
cannot be replayed against another.

**Signed bytes** (`authproof.go:87-93`, matching `@bsv/auth`'s `serializeAuthSigData`):
UTF-8 of `action \n identityKey \n expiresAt \n nonce`.

**Key derivation for the signature** — this is the task's "(BRC-42/43 protocol
[2,"backup cache auth"], keyID=nonce)" and it is exactly that (`authproof.go:144-151` signing,
`:194-202` verifying; `ts-client/src/proof.ts:16, 63-69`):

- `protocolID`: security level 2, protocol `"backup cache auth"`
- `keyID`: the proof's nonce (base64 of 32 random bytes) — "so every proof signs under a fresh
  key" (`proof.ts:66`)
- `counterparty`: the other side's identity key (server's key when signing; the claimed
  `identityKey` when verifying)
- signature: ECDSA, DER, base64 in the JSON.

**Freshness:** `expiresAt` is Unix **milliseconds**; window 2 min, skew 30 s (`authproof.go:39-42`).
Verification refuses expired proofs and ones whose expiry exceeds `now + window + skew`
("a fabricated expiry", `authproof.go:169-179`).

**Replay:** nonce single-use, consumed only *after* the signature verifies (so unauthenticated
requests cannot burn nonces, `middlewares/authproof.go:77-90`). Production store is a Postgres
`INSERT ... ON CONFLICT (nonce) DO NOTHING` — atomic across replicas, zero rows = replay
(`internal/nonce/postgres.go:49-59`); expired rows swept opportunistically (1-in-64 requests).
The Go client has a subtle countermeasure worth remembering (`client/client.go:311-330`): Go's
net/http transparently re-sends a bodyless request on a dead keep-alive connection with the *same*
proof header, which trips the replay guard; the client detects "proof already used" and re-signs a
fresh proof exactly once — never for uploads, since a replayed Append may have really stored the
entry.

**The sha256-in-action-string streaming trick** (`docs/authproof-protocol.md:62-81`) — this is why
they abandoned BRC-103/104: signing the body itself forces the verifier to buffer the whole body
before trusting a byte. Instead the proof signs the body's sha256 inside the action string.
Server order:

1. Size guard — oversize answered `413` before auth is attempted.
2. Verify proof from the header (shape → action match → freshness → signature), consume nonce.
   Sender authenticated; zero body bytes read. A POST whose action lacks `sha256=` is refused 401
   (`middlewares/authproof.go:62-66`).
3. Auth middleware wraps `r.Body` in a digest-verifying reader (`middlewares/authproof.go:92-94,
   118-140`); the handler streams it into the store inside one transaction, hashing as it goes.
4. At EOF, computed hash ≠ signed digest → the reader returns `ErrBodyDigestMismatch`, the store
   transaction aborts: `400 ERR_BODY_DIGEST_MISMATCH`, nothing kept.

Responses are **not** signed — TLS is transport integrity; end-to-end integrity is the client
comparing the server-reported sha256 of the stored blob against its own (`client.go:64-66`).

Go/TS byte compatibility is pinned by shared test vectors generated from the TypeScript package
(`internal/authproof/testdata/vectors.json`, `test-client/genvectors.mjs`) — a pattern worth
copying in spirit for any two-implementation format of ours.

The server wallet is a key-only `CompletedProtoWallet` — no storage, no UTXOs, cannot spend
(`internal/wallet/wallet.go:40-47`), with an explicit warning never to attach payment middleware.

## 5. Overlap map — their log vs our token chain

Our reference points: BRC draft header `version(1) | kind(1) | seq(4) | parent_txid(32) |
device_id(4) | payload` (`wallet-backup-and-sync-onchain.md` § 5, lines 162-171) and the
snapshot/delta rules of § 7; work-plan items 2/3/5.

| Concept | Theirs (server log) | Ours (on-chain token chain) | Verdict |
|---|---|---|---|
| Contiguous seq | 1-based, contiguous within `(pseudonym, device, generation)`; **server-enforced** at append (409 on conflict); the enforcement point doubles as multi-writer conflict detection | `seq` monotonic **per wallet, across all devices**; nothing enforces it — the chain structure (`parent_txid`) is the real ordering; forks detected by two tokens sharing a parent | **Near-miss.** Same integrity goal (no silent holes), enforced by different authorities: their DB transaction ≈ our parent-pointer chain. Harmonized form: keep seq purely as a *monotonic counter for humans/telemetry* and let the hash/parent link be the normative order in both — which is already true of ours; theirs needs seq to be normative because blobs have no parent pointer at the transport level. Not worth forcing identical. |
| Parent hash | `prevSha256` = sha256 of previous **ciphertext blob**, empty at generation start; client-supplied, server-stored, **server does not verify**; client verifies at restore | `parent_txid` = txid of previous **token** (32 zero bytes for first-ever); miners "verify" existence for free, wallet verifies linkage at recovery | **Identical semantics, different hash domain.** Both are client-verified back-links over ciphertext-level artifacts forming one restore chain. A harmonized common form: define the delta *payload* to carry `prev_payload_sha256` (hash of the previous chunk's ciphertext) *inside* our encrypted payload or header — then a chunk uploaded to their cache and inscribed in our token would chain identically on both rails, and their `prevSha256` field would just surface it. Cheap to add to our § 5 header discussion; decide in item 2. |
| Generations as snapshots | Generation N+1 begins (by convention) with a full snapshot at seq 1, `prevSha256` empty; keep newest 2 generations; prune older ones; server guards retention (`RetainedGenerations = 2`) | Snapshots are `kind=0` tokens *inline* in one never-forked chain; snapshot cadence rule (0.5× / 20 deltas / 16 KB / schema change); **no pruning** — the chain is permanent, recovery walks back only to the newest snapshot | **Genuinely different, because chain vs server.** Their generation = our "snapshot epoch", but they need generations as a *deletion unit* (server storage is reclaimable; a failed compaction must never strand the user, hence keep-2). We cannot delete; our equivalent safety rule is "the previous snapshot remains on-chain forever by construction". The keep-2 guard is their answer to the same failure mode our § 7 answers by making snapshots point at their parent too. No harmonization needed — but their compaction protocol (write snapshot N+1 *before* deleting N-2, server refuses newest two) is worth citing in the BRC's § on why our snapshots link into the chain rather than truncating it. |
| Per-device logs | `deviceId` is a **partition**: each device appends to its own independent log; no cross-device ordering, no merge; restore picks one device's log; multi-device reconciliation is out of protocol | `device_id` is a **column** in one shared chain: all devices append to the same chain, read-before-write, fork detect + re-read-re-write (§ 7.2, item 5) | **Near-miss with a real semantic gap.** They dodge the multi-writer problem by partitioning; we solve it because one chain is the whole point (a UTXO chain has a single tip). Their model cannot express "device B applies device A's delta"; ours requires it. Nothing to harmonize at the transport level; but if the same encrypted chunk feeds both rails, our chunk header's `device_id` gives their per-device partition key for free (device_id → deviceId hash). |
| Auth/identity | Stable pseudonym key (derived from seed, not the wallet identity key); per-request BRC-42/43 proofs | Deterministic backup address derived from seed; possession of seed = ability to find and decrypt | **Same recovery bootstrap** ("from the seed alone, find your data"): their `Manifest()` ≈ our address-scan. Different threat surface: their server links all requests under one pseudonym; our chain is public but the address binds to nothing unless the seed holder shows up. |
| Payload | Opaque bytes, format entirely client-defined | Normative: BRC-38-shaped snapshot payload; § 7 delta changeset | **Complementary, not conflicting.** Their rail can carry our exact chunks unchanged. If our BRC defines the chunk format well, it becomes the missing content spec for their rail — the "same encrypted chunks feed both rails" outcome item 0b hoped for is available, and the dependency points *from them to us*, not the reverse. |

**Bottom line for item 0b(b):** OPAQUE. There is no delta format here to align with or defer to —
only log discipline, which we already have in stronger (chain-linked) form. The two genuinely
importable ideas are (1) the compaction safety rule expressed as "server refuses to delete the
newest two generations" — evidence for our design choice that snapshots must not orphan history —
and (2) cross-implementation test vectors pinning byte compatibility. The one concrete
harmonization candidate is carrying `prev_payload_sha256` alongside our header so the identical
chunk chains on both rails; flag for the item 2 header decision. (These are design observations —
per the license warning, no code or text from the repo is to be reused.)

---

## Checked (files actually read, at commit `ca2136e`)

- `README.md` (all 171 lines — API table, retention, privacy model, operational notes)
- `client/client.go` (all 384 lines)
- `ts-client/src/client.ts` (all 236 lines), `ts-client/src/proof.ts` (all 100 lines),
  `ts-client/README.md` (all), `ts-client/LICENSE` (header lines 1-5)
- `docs/authproof-protocol.md` (all 94 lines)
- `internal/authproof/authproof.go` (all 207 lines)
- `internal/blobstore/blobstore.go` (all 111 lines), `internal/blobstore/postgres.go` (all 419
  lines), `internal/blobstore/memory.go` (all 223 lines — second pass read Manifest 115-163,
  DeleteGeneration 166-192, DeleteAccount 195-208 in full; semantics match Postgres)
- `internal/server/handlers/handlers.go`, `append.go`, `read.go`, `prune.go`, `erase.go`,
  `limits.go` (all, in full)
- `internal/server/middlewares/authproof.go` (all 142 lines), `identity.go` (all)
- `internal/nonce/nonce.go` (all), `internal/nonce/postgres.go` (lines 1-60; the
  `INSERT INTO auth_nonces ... ON CONFLICT (nonce) DO NOTHING` + `RowsAffected == 1` consume and
  the 1-in-64 expiry sweep are at lines ~43-60, re-verified)
- `internal/wallet/wallet.go` (all), `internal/config/config.go` (targeted lines 14-54),
  `.env.example` (targeted)
- `internal/server/server.go` lines 100-145 read in full on the second pass: the `maxBody`
  middleware answers 413 before auth, counts streamed bytes rather than trusting the
  client-declared Content-Length, and back-fills the 413 if a handler abandoned the oversize read
- `client/client_test.go` (lines ~135-150, `TestIndexAndManifestAgreeOnTheLogHead`: each append
  passes `prev = res.Sha256`, the server's hash of the stored ciphertext — the chaining
  convention, re-verified), `internal/server/e2e_test.go` (grep for generation/prev usage only)
- `ts-client/LICENSE` lines 1-8 (Open BSV License Version 6, (c) 2026 BSV Association) and
  `ts-client/package.json` (`"name": "@bsv/backup-cache-client"`, `"license": "Open BSV License"`)
  — re-verified
- Repo-wide greps: `syncchunk|brc-38|brc-40|toolbox|delta`, `encrypt`, `prevSha256|prev_sha256`
- Our side: `wallet-backup-and-sync-onchain.md` (grep of header/§5/§7 lines incl. 162-171,
  223-259), work-plan `README.md` (item 0b block, lines 101-153)

## NOT checked

- `internal/server/server.go` outside lines 100-145 (route wiring, the exact middleware chain
  order, sizeGuard/countingBody bodies, timeouts) — the 30-min stream timeout and the
  guard-before-auth *ordering* rest on README plus the maxBody doc comment ("It must answer
  BEFORE the auth layer touches the request"), not on a read of the router assembly itself.
- `internal/server/e2e_test.go`, `security_test.go`, `handlers_test.go`, `authproof_test.go`,
  `maxbody_internal_test.go`, `size_limit_test.go` in full — behavioral claims rest on
  implementation code, not on test corroboration (except client_test.go ~135-150).
- `tracing.go`, `internal/otel/`, `internal/logger/`, `internal/config/config.go` in full,
  `cmd/server/main.go` (header comment only, via grep).
- `test-client/genvectors.mjs`, `interop.mjs`, `testdata/vectors.json` contents.
- The published `@bsv/auth` npm package itself — I verified this repo's *claims* of
  byte-compatibility with it, not the compatibility.
- The retention-guard gap arithmetic (generations 1,5 present → 1..3 deletable) against a test.
- GitHub metadata (whether the hosted repo shows a license the clone lacks) — not fetched;
  clone-at-HEAD is the source of truth used.

## Disagreements / flags

1. **Task brief vs repo:** the brief said the repo has NO license. Root: true (verified twice by
   directory listing). But `ts-client/LICENSE` is Open BSV License v6 ((c) 2026 BSV Association)
   and `ts-client/package.json` declares `"name": "@bsv/backup-cache-client"`,
   `"license": "Open BSV License"` (both verified). Read-only rule unchanged; noting for accuracy
   and because BSV Association copyright on a deggen repo suggests org-level ownership.
2. **README's "zero-knowledge cache" phrasing** (README title line and ts-client README:4) is
   marketing shorthand for "server stores only ciphertext + pseudonym". It is not zero-knowledge
   in the cryptographic sense, and the README itself immediately and honestly enumerates residuals
   (IP, sizes, cadence, device count). Per our house rule (BRC-52 lesson): if we cite this
   service in the BRC or anywhere public, say "the server stores only ciphertext under a
   pseudonym", never "zero-knowledge".
3. **prevSha256 is weaker than it looks in their README's own framing:** the server stores but
   never checks it, and it isn't shape-validated. Anyone describing their log as "hash-chained"
   should say "client-verified hash chain over ciphertext; server enforces only sequence
   contiguity". Our chain differs materially here: `parent_txid` linkage is structural (a txid
   must exist to be pointed at).
4. **Their encryption is unspecified.** "Counterparty self" appears only in prose; no protocol
   string, no ciphertext framing, no code. Anything we build that intends chunk-level
   compatibility with this rail has nothing to target yet — if alignment ever matters, that spec
   would have to be proposed (possibly by us, in the BRC's payload section).
5. **License risk for the ecosystem:** the Go server+client being unlicensed means nobody can
   legally run or fork it as-is; worth remembering in the 1Sat/BSV-org dependency-risk file if
   this service becomes recovery infrastructure others rely on. (Observation only; no action in
   this task.)
