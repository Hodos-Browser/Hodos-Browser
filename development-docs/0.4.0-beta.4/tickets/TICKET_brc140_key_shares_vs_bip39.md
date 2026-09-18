# 🔬 BRC-140 threshold key shares as an alternative to the BIP39 phrase — research and decide

**Found:** 2026-09-16, reading `Standards/BRCs/reference/key-derivation/0140.md` in the Marston tree
while building the BSVA team profiles.
**Status:** ⬜ UNASSIGNED · **Track:** unassigned · **Filed by:** owner (Matt), 2026-09-16

> ⚠️ **Method note.** This is a **research-and-decide** ticket, not a defect — the template is
> defect-shaped and the sections are bent to fit. What the wallet does today is **code reading**
> (`json_storage.rs`, `backup.rs`, `recovery.rs`). What BRC-140 specifies is **reading the spec text**.
> ⛔ **Not verified:** whether the BSV TypeScript SDK's `toKeyShares` / `toBackupShares` have a Rust
> equivalent we could use, or whether we would implement the scheme ourselves. That is the first
> question the research answers, and it probably decides cost.

> 👤 **Owner framing, 2026-09-16:** *"We might want to do that as a possible alternative for our users
> in Hodos to create a key instead of using the BIP39 method. But that's down the road."* The decision
> asked for here is **do it in beta.4 / scrap it / defer to beta.5 or later** — not a design.

---

## What this is

**BRC-140 — Threshold Key Sharing and Backup via Shamir's Secret Sharing Scheme.** Sole author
**Deggen** (`d.kellenschwiler@bsvassociation.org`), who is BSVA's Distributed Applications seat and the
person who merges into the BRC registry in practice. Profile:
`Marston Enterprises/Hodos/Marketing/Profiles/bsv/deggen.md`.

It splits a secp256k1 private key into `m` shares such that any `n` (`2 ≤ n ≤ m`) reconstruct it and
fewer than `n` reveal nothing. It specifies the field arithmetic over the secp256k1 prime field,
non-deterministic share-coordinate generation, a short integrity tag, and **a canonical textual
"backup" serialization meant to be written down and stored offline.** It codifies `toKeyShares` /
`fromKeyShares` and `toBackupShares` / `fromBackupShares` in the BSV TypeScript SDK — so it is a
description of shipped SDK behaviour, not a paper design.

⚠️ **The spec's own scope limits, quoted from §Motivation — carry these into any discussion:**

- It covers **storage and reconstruction**. *"It is not a threshold-signature scheme: the key is fully
  reassembled in one place at recovery time, and the dealer (the party performing the split) sees the
  whole key."*
- *"It does not provide verifiable secret sharing (a malicious dealer is not provably constrained
  beyond the integrity tag), nor does it protect the key while reassembled."*

## What the wallet does today — code reading

| Where | What |
|---|---|
| `rust-wallet/src/json_storage.rs` | `bip39::Mnemonic::parse_in(Language::English, …)` → `mnemonic.to_seed("")` → BIP32 master key. English wordlist, **empty passphrase**. |
| `rust-wallet/src/recovery.rs` | `recover_wallet_from_mnemonic()` with gap-limit scanning (BIP32 + BRC-42), `derive_key_at_path()` |
| `rust-wallet/src/handlers.rs` | `reveal_mnemonic` — the user can read their phrase back out |
| `rust-wallet/src/backup.rs` | ⭐ **The encrypted backup deliberately excludes the mnemonic** — `payload.mnemonic = String::new()` before encrypt, with a comment giving the reason. |

⭐ **That last row is why this ticket is interesting rather than academic.** Our backup covers
everything *except* the one secret that matters most, by design. So the phrase is the single artifact
the user must protect out-of-band, intact, in one place — and a 12/24-word phrase has exactly one
failure mode in each direction: lose it and the wallet is gone, leak it and the wallet is gone.
BRC-140 breaks that trade-off for storage: three shares in three places, any two recover, one
compromised share reveals nothing.

## Why it matters

**User-observable, and it is the recovery story.** Today's answer to "how do I not lose my wallet" is
"write down twelve words and don't let anyone see them" — which is the answer every wallet gives and
the one users demonstrably fail at. A share-based option is a genuinely different offer: *"split it
between your safe, your lawyer and your brother; any two of them get it back."*

It also lands in a registry-shaped way. Using the BSV standard for this, authored by the registry
maintainer, is worth something on its own when we are already submitting BRCs into that registry.

## How exposed are we — nothing is broken

⛔ **No defect. No urgency.** BIP39 works, ships, and is what every other wallet does. This is an
*option to add*, and the cost of not doing it is zero today.

| If | Then |
|---|---|
| A Rust implementation of the BRC-140 serialization exists or is small | Cheap enough to consider for beta.5 |
| It means writing and testing our own secret-sharing code | ⛔ Defer. **We do not hand-roll key-handling crypto to add a convenience.** |
| It would replace BIP39 rather than sit beside it | ⛔ Scrap that version. Interop with every other BSV wallet is worth more. |

## What already protects us, and how that shapes the decision

The wallet already has BIP39 + BIP32 + BRC-42 derivation and gap-limit recovery working, and beta.4
track 4 (`../track-4-onchain-backup-sync/`) is already the place where backup and recovery get
worked. **So this is an addition to a working path, never a replacement.** Whatever we do, `reveal_mnemonic`
and mnemonic recovery stay — a user who already wrote down twelve words must not be stranded.

## The decision to make

| Option | What it means |
|---|---|
| **A — beta.4** | Fold into track 4, which already owns backup and recovery |
| **B — beta.5 or later** | Research now, decide later, build when the recovery story gets a dedicated pass |
| **C — scrap** | BIP39 is enough; record the reasoning so this is not reopened annually |

**My read, for the owner to overrule:** ⭐ **B.** Track 4's open questions are already about delta
chains and multi-device sync measured against real token rows; adding a key-splitting scheme to it
widens a track that is last in the order and most at risk of being cut. And the interesting version
of this is a *user-facing recovery option* with UI, wording and a support story attached, which is a
product pass rather than a wallet-internals pass. **Research it in beta.4, build it no earlier than
beta.5.**

**Deliberately out of scope:** threshold *signing* (BRC-140 explicitly is not that); social recovery
or guardian schemes; any multi-party protocol where the key is never assembled; changing the
derivation path or the BRC-42 work.

## What the research must answer, before the decision

1. Is there a Rust implementation of BRC-140's serialization and share generation, or is the SDK's
   TypeScript the only one? **This probably decides the whole question.**
2. Does the scheme split the **BIP39 seed/entropy** or the **derived secp256k1 private key**? These
   are different products: splitting the seed preserves the whole HD tree, splitting one key does not.
   *(Reading the abstract, it is the secp256k1 private key — verify against the spec body.)*
3. What does a share look like written down, and is it worse than twelve words to transcribe by hand?
4. What happens to a share-recovered wallet's `reveal_mnemonic`, and to a user who has both?
5. Does any other BSV wallet expose this, and what did they call it in the UI?

## Test and negative control

⛔ **Not applicable at this stage** — no code changes. If it reaches Option A or B, the phase contract
that builds it carries the GREEN/RED/SUBJECT rows, and the RED must be a **round-trip failure**: split
a key, corrupt one share below the integrity tag, and see reconstruction refuse rather than return a
wrong key silently.

**Standing invariant?** Not yet. If built: *"a wallet recovered from shares derives the same addresses
as the same wallet recovered from its phrase"* is the row for `../REGRESSION_ADDITIONS.md`.

## Links

- `Marston Enterprises/Standards/BRCs/reference/key-derivation/0140.md` — the spec, in our tree
- `../track-4-onchain-backup-sync/` — the track that owns backup and recovery
- `rust-wallet/src/json_storage.rs`, `recovery.rs`, `backup.rs` — what exists today
- `Marston Enterprises/Hodos/Marketing/Profiles/bsv/deggen.md` — the author, and why his opinion of our
  backup BRC matters
