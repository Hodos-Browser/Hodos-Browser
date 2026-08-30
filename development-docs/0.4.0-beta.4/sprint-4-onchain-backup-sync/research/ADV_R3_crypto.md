# ADV_R3 — Crypto & Key-Management Adversary

Lens R3. Target: IMPLEMENTATION_PLAN.md D4/D5 (KDF + address derivation), §4/§8 padding, and
the owner's 2026-08-24 KDF/BRC-42 question. Every code premise below was re-verified against the
cited lines (this review did NOT take the plan's relayed claims on trust — the plan itself says it
re-verified nothing, §7).

## Premises verified against code (not relayed)

- **Encryption key** = `SHA-256(master_privkey || "hodos-wallet-backup-v1")` — `backup.rs:958-967`
  (`derive_onchain_backup_key`), used by `encrypt_compressed`/`deserialize_from_onchain`
  (`backup.rs:1270-1333`). GCM: random 96-bit IV via `thread_rng().fill_bytes` per message
  (`backup.rs:1277-1279`); format `nonce(12) || ct || tag(16)`.
- **`master_privkey`** = the **BIP32 root private key** `m` — `database/helpers.rs:14-32`
  (`XPrv::new(seed).private_key()`), i.e. the same root every HD/BRC-42 spending key descends from.
- **Address / signing keypair** = BRC-42 self, invoice literal `"1-wallet-backup-1"` —
  `handlers.rs:13332`, `:14711`, `connection.rs:390/550/716`. Level `1`, protocol id contains
  hyphens. Illegal under BRC-43 (0043.md format `<level>-<protocolID>-<keyID>`; hyphens are the
  field separator, so `"1-wallet-backup-1"` parses as level=1, protocol=`wallet`, keyID=`backup-1` —
  not the intent, though the derivation just HMACs the whole literal so it is self-consistent).
- **brc42.rs** (`crypto/brc42.rs:52-270`) implements BRC-42 exactly per 0042.md: ECDH →
  `HMAC-SHA256(compressed_shared_secret, invoice_utf8)` → scalar add/tweak. `derive_child_public_key`
  and `derive_child_private_key` are consistent (child_priv*G == child_pub). Verified the self case
  by hand below.
- **Recovery (shipped)**: `handlers.rs:14696-14818` (`fetch_onchain_backup`) queries WoC
  `unspent/all` at the single legacy address, picks the "newest" marker (`height==0`⇒`i64::MAX`,
  else max height — `:14764-14767`), decrypts vout-0 PushDrop. **No header is parsed today**; the
  header + `parent_txid` walk + recency check are *planned* (D6/Phase 4). The findings below target
  that planned design, which is what the review is about.
- **Draft** proposes BRC-42 (level 2, protocol `"wallet-backup"`, keyID `1`, counterparty self) for
  BOTH the encryption key (§3) and the address (§6). The plan's D4/D5 correctly say the draft does
  not match code and rule "draft follows code."

---

## VERDICT (adjudication of the owner's question)

**KEEP-SHIPPED for both the encryption KDF and the address derivation. Reject the main session's
provisional MIGRATE-TO-BRC42-V2.** Update the *draft* to match code (already D4/D5); ship **no code
migration** of either key. Ranked technical reasons:

1. **BRC-42-with-self-counterparty buys zero security over `SHA-256(master || domain)` for the
   encryption key.** The security of BOTH schemes reduces to one sentence: *anyone who has the
   master root can derive the key; nobody else can.* BRC-42's advertised benefit (0042.md
   Motivation) is a *shared* secret with an **external** counterparty, giving privacy from outsiders
   and per-pair key universes. With counterparty = self there is no second party and no external
   entropy: the "shared secret" is `master*(master*G) = master^2*G`, a deterministic function of the
   master alone — computationally equivalent, as a secret, to `SHA-256(master||const)`. Both give
   domain separation (the invoice string vs the `"hodos-wallet-backup-v1"` string). The migration is
   pure churn: it swaps one sound 256-bit-in/256-bit-out KDF for another of identical security and
   adds a **permanent** second decrypt path (H15 burden, forever) for no gain.

2. **Migrating the *address* derivation is net-NEGATIVE for security**, because it moves every
   existing wallet's chain to a new address and manufactures a dual-address recovery surface whose
   downgrade failure mode (R3-1) can silently restore stale state and lose post-migration money. The
   BRC-43 non-conformance of `"1-wallet-backup-1"` is a **spec-hygiene wart with no cryptographic
   consequence**: BRC-42 HMACs the literal invoice bytes and does not parse or validate BRC-43
   structure, so the derived key is exactly as strong as a conformant one. Trading a cosmetic wart
   for a money-loss surface is a bad trade.

3. **The shipped scheme's key *separation* is actually cleaner than the draft's.** Shipped: the
   encryption key (SHA-256 universe) and the signing key (BRC-42 universe) come from **disjoint**
   derivations. The draft proposes the **same** invoice (level 2, `"wallet-backup"`, keyID 1, self)
   for both encryption and the address/signing key. Not a vulnerability (deriveSymmetricKey and the
   signing child key produce different outputs from one invoice), but a weaker separation posture
   than what ships. "Migrate for conformance" makes separation *worse*.

4. **The one real hygiene debt is shared by both options and is not fixed by migrating:** the
   encryption key and every spending key both descend from the raw BIP32 **root** `m` fed directly
   into a primitive (SHA-256 here, ECDH in BRC-42). Cleaner design derives the backup key from a
   *hardened child* of `m`. Neither scheme does this. Flag as LOW (R3-5); do not let it justify the
   migration.

Bottom line for D4/D5: **the plan's "keep, document, migration on the blocker list" is correct and
should be hardened to "keep, document, migration REJECTED on security grounds, not merely
deferred."** Strike the provisional recommendation to "migrate code to conformant BRC-42 as v2 in
Phase 4." The only migration worth doing is one forced by an actual break in SHA-256 or in the
shipped GCM construction — none exists.

---

## Findings

### R3-1 — CRITICAL — Dual-address recovery downgrade: v2 migration can silently restore pre-migration state and lose money

**The finding that kills the migration proposal.** The main session's recommendation is "migrate to
conformant BRC-42 v2 (Phase 4), legacy address fallback forever." A conformant invoice
(`"2-wallet backup-1"`, level 2, space not hyphen) derives a **different key → different pubkey →
different P2PKH address** than legacy `"1-wallet-backup-1"`. A migrated wallet then has TWO backup
addresses: v2 (new live chain) and legacy (frozen at the last pre-migration token).

Starting state: wallet migrated; live tip is token #500 at the **v2** address; legacy holds token
#499 (pre-migration final). Balance grew by $X after migration.

Break:
1. Fresh machine, seed only. Recovery derives both addresses and (per the recommendation) tries v2
   first, legacy as fallback.
2. The trigger for "fall back to legacy" is "no valid v2 token found." Two ways that fires wrongly:
   **(a)** WoC's `unspent/all`/history index lags 30 s–5 min (the window the plan cites,
   `handlers.rs:14730` TODO) and returns empty for the v2 address during the propagation window
   right after a fresh backup; **(b)** an attacker cannot forge a v2 token (no key) but can send junk
   markers to the v2 address so the bootstrap marker query returns only non-decrypting entries, and
   — if "no *decryptable* v2 token" is conflated with "no v2 token" — recovery concludes v2 is empty.
3. Recovery falls back to legacy, finds token #499 (a genuine, GCM-valid, self-authored token that
   passes every check H15 pins), and restores it.
4. Post-migration state (tokens #500…, the $X) is gone. Because #499 authenticates perfectly, no
   error is raised.

**Violates G11** (a version/format change broke an existing user's recovery), **G1/G2** (money older
than the bound unrecoverable), and D6's own "fail closed, never restore stale" principle — which the
plan applies only *within* one address, never *across* the two.

**Harness gap:** H15 pins only *decrypt-compatibility* — "current build recovers every legacy
fixture byte-exact; decode order current-first-then-legacy." It says nothing about **address
selection / recency ordering** between two *live* addresses under a lagging or littered index. H1
recovers a single-address chain; H10 litters ONE address. No H-test builds a wallet with a live v2
chain AND a stale legacy chain and asserts v2 always wins, or that "v2 address unreachable/lagging"
fails closed instead of silently using legacy.

**Fix:** Do not migrate the address (verdict). If ever forced: recovery MUST fail closed when the v2
address cannot be *authoritatively* confirmed empty (indexer error/lag ≠ "no backup"), MUST
cross-validate v2 emptiness against the D12 second oracle before reading legacy, and legacy may be
consulted only when BOTH oracles agree v2 has zero markers ever. Add H15b pinning this ordering
under lag + litter on the v2 address.

### R3-2 — HIGH — Plaintext header trusted before GCM decryption in the D6 parent-walk (partial-parsing / littering escalation)

The Phase-4 token header `version|kind|seq|parent_txid|device_id` lives in the **plaintext** PushDrop
data fields (draft §5), OUTSIDE the AES-GCM envelope (only `payload` is encrypted). D6 makes recovery
"find a recent marker via the address index (bootstrap), then walk `parent_txid` backward," running
the spent-check recency test "on the found marker" — the marker is chosen by index/seq **before**
anything is decrypted.

Starting state: victim's real tip is snapshot S_new (seq 500). A stale real snapshot S_old (seq 100)
is still on-chain (spent history readable forever — D3).

Break:
1. Attacker (no key) mints a fresh token at the backup address: valid P2PKH marker, plaintext header
   `seq = 0xFFFFFFFF`, `parent_txid = <txid of S_old>`, `kind = 0`, payload = 32 random bytes.
2. Bootstrap picks the newest marker. Attacker's token wins on `seq` (or on unconfirmed-height ⇒
   `i64::MAX`, the exact tiebreak shipped at `handlers.rs:14764`).
3. Recency spent-check runs on the attacker's marker: genuinely unspent (just created) ⇒ "fresh" ⇒
   passes.
4. Recovery attempts GCM decrypt ⇒ tag fails ⇒ token discarded. **What happens next is the break.**
   If the design trusts the discarded token's `parent_txid` to seed the walk (or fails to
   distinguish "bootstrap token failed" from "no chain"), recovery either walks to S_old and
   restores **stale state** (HIGH: silent wrong state) or dead-ends and reports "no backup"
   (MEDIUM DoS — the fail-open→"no backup exists" class G5/BS-C2 forbids).

**Violates G10** (chain robustness — attacker at the address must not steer recovery), **G5**
(attacker-controlled input must not read as "no backup"), and the D6 "seq+parent normative, never
trust indexer newest" intent — which the plan applies to *honest* lag but not to an attacker minting
a high-seq plaintext header.

**Harness gap:** H10 sends "50 random PushDrop tokens by a foreign key" and asserts "all junk skipped
by GCM failure; chain walk unaffected" — junk with *random* headers. It does NOT model the
**targeted** case: a junk token crafted to be *selected as bootstrap tip* (max seq / unconfirmed)
with a **valid-looking header pointing at a real stale snapshot**. H6's "stale unspent index pointing
at a superseded token" is the honest-lag case, not the attacker-minted high-seq case. Header fields
of a token that FAILED decryption influencing tip/walk selection is pinned by no H-test.

**Fix:** Normative rule the harness must pin: **no plaintext header field may influence tip
selection, walk seeding, or recency judgement until that token's payload has passed GCM
authentication.** Bootstrap iterates markers newest→older and selects the first that *decrypts*;
`seq`/`parent_txid` are trusted only from decrypted tokens. Extend H10 with the targeted
high-seq/parent-spoof token; assert byte-identical result to the no-litter run.

### R3-3 — MEDIUM — Size-class padding is theater against the on-chain observer the draft implicitly claims to defend; H12 measures the wrong adversary

Draft §8.4 + Phase 6 pad payloads to 1/4/16 KB; H12 gates it with chi-square (size-class ×
action-type independence) and a classifier over (size class, inter-backup interval). The stated
observer "can observe backup frequency and sizes."

Break — padding hides *how much* changed but not *that* a backup fired, *when*, or (via §3.3's new
trigger) *what* changed:
1. §3.3 makes the dirty flag fire on **any new spendable output** (the ≥$3 threshold is dropped), so
   each backup broadcast is ~1:1 with a receive/spend event.
2. Those events are **themselves on-chain**, timestamped, in the same public ledger.
3. An observer joins backup-address broadcast timestamps to the wallet's other on-chain activity by
   time proximity: the backup tx at ~T says "a wallet change happened at ~T"; the payment tx at ~T
   says "*this* payment happened." The padded size was never the leak that mattered.
4. Worse, the backup tx spends the previous backup token **plus a fee UTXO from the wallet** and
   returns change, linking the backup address to the wallet's main UTXO set — so §8's "address
   reuse is fine, contents are encrypted" understates the linkage.

**Violates the draft §8/§Security privacy claim as *stated*** (it implies size padding buys
meaningful privacy against a size/frequency observer). Padding does defend a strictly weaker observer
seeing ONLY the backup address in isolation — but §3.3 + on-chain reality make that observer
unrealistic.

**Harness gap:** H12's chi-square and classifier draw features only from the **backup stream** (size
class, inter-backup interval). Neither joins backup broadcast times to the wallet's **other on-chain
transactions** — the actual correlation channel. H12 can pass green while cross-channel timing
correlation is wide open. H12 measures the wrong adversary.

**Fix:** Either (a) downgrade the draft's privacy claim to exactly what padding delivers ("hides
payload size from an observer of the backup address alone; does NOT hide existence, timing, or
correlation with the wallet's on-chain activity; a chain-graph observer can link backups to
payments"), or (b) add decorrelation (fixed-cadence batched backups independent of event timing) and
give H12 a test correlating backup broadcast times against a synthetic payment graph with a mutual
information threshold. Given cost, (a) is the honest answer; do not sell padding as more than
size-hiding.

### R3-4 — LOW (computed and DISMISSED, recorded so nobody re-opens it) — GCM never-rotating-key limits are not reached at any realistic volume

One fixed 256-bit key forever, random 96-bit IVs, many tokens over decades, multi-device, all
ciphertext readable on-chain forever (D3 no-erase).

Nonce-collision bound (random 96-bit IV): P(≥1 collision after q encryptions) ≈ q^2 / 2^97.
- Realistic: 4/day × 365 × 20 yr × 4 devices ≈ 1.17e5 ≈ 2^16.8 ⇒ q^2/2^97 ≈ 2^-63 ≈ 1.6e-19.
- Pathological: 24/day × 365 × 50 yr × 20 devices ≈ 8.8e6 ≈ 2^23 ⇒ ≈ 2^-51 ≈ 4e-16.
NIST SP 800-38D's 2^32-random-nonce ceiling is 9+ orders above even the pathological case; per-message
block counts are trivially under 2^32 blocks. **IV collision is a non-issue; no key rotation is
required on collision grounds.**

Key-commitment / Invisible-Salamanders relevance to the **origin-proof claim** (draft §5.5, G10):
the claim "the GCM tag proves this wallet wrote the token; a third party cannot produce one that
decrypts" is **sound** for littering — forging a ciphertext that authenticates under the victim's key
K needs K or GHASH subkey H=E_K(0), i.e. 2^-128 per attempt. Non-committing GCM (a ciphertext valid
under two *different* keys) does NOT help the attacker here, because the victim only ever decrypts
under their own K; there is no second-key oracle. GCM's key-commitment weakness is **not exploitable
for this use**. (The real header-trust problem is R3-2, an *ordering* bug, not a GCM-commitment bug.)
Recorded as LOW/no-action so the verdict rests on facts.

### R3-5 — LOW — Backup key derives from the raw BIP32 root `m`, not a hardened child (shared by both KDF options)

`master_privkey` is the BIP32 root `m` (`helpers.rs:14-32`). Both the shipped SHA-256 KDF and any
BRC-42-self scheme feed this root directly into a primitive, and the root is the common ancestor of
every spending key. Cleaner practice derives the backup key from a hardened child (e.g.
`m/backup'`), so the backup subsystem never handles the root scalar in the clear. Hygiene, not
exploitable (SHA-256 and ECDH are one-way; no path leaks `m`), and — critically — the BRC-42
migration does **not** fix it (BRC-42-self also multiplies with the root). Flagged so the migration
is not sold as a fix. If ever addressed, change both encryption and address derivation to a hardened
base at once — but that is the same chain-migration event R3-1 warns against, so only under an actual
forcing weakness.

---

## Self-ECDH degeneracy check (owner question b) — worked, result: SOUND

Address: `derive_child_public_key(m_priv, m_pub, inv)` with `m_pub = m_priv*G`.
- shared = `m_priv * m_pub = m_priv^2 * G`. Point at infinity requires `m_priv ≡ 0 (mod n)` or a
  zero point — impossible for a valid key. No degeneracy.
- `e = HMAC(compress(m_priv^2*G), inv)`; child_pub = `m_pub + e*G = (m_priv + e)*G`.
- `derive_child_private_key(m_priv, m_pub, inv)`: shared = `m_priv * m_pub = m_priv^2*G` (ECDH
  commutes for the same pair), same `e`, child_priv = `m_priv + e`.
- Consistency: `child_priv * G = (m_priv + e)*G = child_pub`. ✓ The self keypair is valid and
  matches. Only failure mode: `e = 0` or `e ≥ n`, rejected by `SecretKey::from_slice` ⇒ backup
  errors out (liveness, ~2^-128), never a silent weak key. **No subtle self-ECDH break exists.**
  BRC-42-self is cryptographically fine — simply *pointless* as added security (verdict reason 1),
  not *broken*.

## What this review did NOT check
- Did not execute brc42.rs test vectors (read them at `crypto/brc42.rs:275+`; math matches 0042.md).
- Did not build/run the wallet; R3-1/R3-2 are read against the *planned* D6 design + shipped
  `fetch_onchain_backup`, not an executed Phase-4 build.
- Did not audit `pushdrop::decode`/`extract_output_script` for the slice-panic class (A1 §5 / G10
  cover it; H6/H10 pin it).
- Did not measure RNG quality (`thread_rng` assumed CSPRNG-backed, which it is).
