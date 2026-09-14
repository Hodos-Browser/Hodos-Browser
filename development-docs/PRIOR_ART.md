# Prior art — the sources, and the ledger of what we actually learned

**Opened:** 2026-08-30. **The rule lives in** the root `CLAUDE.md`, working rule 5. This file carries
the **stack-by-stack source list** and a **running log of lookups**.

> **Why the log exists** *(owner, 2026-08-30)*: to learn **which projects are worth the trip, and for
> what.** A source that has never taught us anything is costing reading time; a source that keeps
> paying out should be consulted earlier. ⛔ **Keep it to one row per lookup.** The moment this
> becomes a chore it stops being filled in, and a half-filled ledger is worse than none.

---

## 1. Sources by stack

⛔ **Do not read the whole list for one question.** Find your layer, read the one or two that fit.

### `rust-wallet/` — BSV protocol and wallet behaviour

| Source | Good for | Trust |
|---|---|---|
| **BRC documentation** | What the spec requires vs leaves open | 🟢 Always first |
| **BSV Association SDKs + `wallet-toolbox`** (TypeScript, Go) | How a conforming wallet behaves | 🟢 **Authoritative.** Outranks everything below |
| **Bitcoin BIPs** | Lineage only — BRC-42/43 descend from BIP32 | 🟡 Read for argument, not for behaviour |
| **BDK / `rust-bitcoin`** | ⛔ One narrow thing only — see §2 | 🔴 **BTC, not BSV** |

### `cef-native/` — engine, privacy, security

| Source | Good for | Trust |
|---|---|---|
| **Chromium upstream** | What we diverge *from*. Every engine bump re-litigates it | 🟢 |
| **Brave** | ⭐ Closest to us in intent, and the origin of our approach | 🟢 |
| **Tor Browser** | ⭐ The threat model — and the **opposing** strategy | 🟢 |
| **Mullvad Browser** | Tor's hardening without the Tor network | 🟢 |
| **Firefox / Gecko** | Independent engine, different answers; conformance reference | 🟢 |
| **Safari / WebKit** | ITP; most aggressive tracking prevention at scale; macOS platform reference | 🟢 |
| **LibreWolf** | Which Firefox defaults a privacy project changes | 🟡 Config-level, not architectural |
| **ungoogled-chromium** | De-Googling patch sets; fork-maintenance burden | 🟡 Patch discipline, not design |

### `frontend/` — browser UI and interaction

| Source | Good for |
|---|---|
| **Vivaldi** | Chrome-level UI over Chromium — closest to what we do |
| **Brave / Firefox** | Permission and consent surfaces — security UI as much as UI |

### BRC drafting (lives in the Marston repo)

Sources are listed in `Marston Enterprises/Standards/BRCs/README.md` § "Prior art before drafting" —
BRCs in `reference/`, the SDKs, BIPs for structure and process, **RFC 2119** for normative language.

---

## 2. ⛔ Two corrections on record — do not repeat them

### 2.1 Farbling is **Brave's**, not Tor's

⚠️ **Corrected 2026-08-30 after the owner queried it. He was right.**

**Brave coined the term "farbling" and shipped fingerprint randomisation first.** Prior academic work
existed (**PriVaricator**, **FPRandom**) but Brave was the first mainstream browser to deploy it.
Firefox and Safari adopted the approach later.

⭐ **Tor Browser is still prior art — for a better reason than the one first given.** Its design
document is the canonical statement of the *threat model*, and its own answer is the **opposite
strategy**:

| Strategy | Approach | Who |
|---|---|---|
| **Randomisation** | Everyone looks different, and different again each session | **Brave** (and us) |
| **Uniformity** | Everyone looks *identical*, so there is nothing to distinguish | **Tor Browser** |

**Why this matters for us specifically:** our open farbling residuals — unfarbled workers, fenced
frames, the 37-host `IsAuthDomain` allowlist — are all questions of *"does this gap actually let
someone re-identify a user?"* Under uniformity a gap is a straightforward break. Under randomisation
it depends on whether the unfarbled surface is stable and high-entropy. ⛔ **We have been arguing the
residuals without a written threat model.** That is what Tor's is for.

### 2.2 BDK / `rust-bitcoin` is a **BTC** library

⚠️ **The owner flagged this and was right to.** It was over-recommended on 2026-08-30 as if it were a
general Rust prior-art source. It is not.

**BSV divergences that would produce confident, wrong assumptions:**

- **No SegWit, no Taproot/Schnorr.** BDK's entire model is descriptor-based (`wpkh`, `wsh`, `tr`) — none of it applies.
- **Key derivation differs in kind.** BSV uses BRC-42/43 invoice-number derivation, **not** output descriptors.
- **No RBF** (first-seen rule), different dust and standardness rules, no practical transaction-size cap, restored opcodes.
- **Different SPV model** — BSV uses merkle proofs / BEEF (BRC-62/74).

⭐ **Use it for exactly one thing:** the *data-model* pattern of **"a UTXO that exists but is
deliberately not selectable"** — BDK's separation of an unspendable set from coin selection. That is
wallet architecture, not chain semantics, and it is the closest Rust prior art for sprint 1's
classification seam.

⛔ **Everything else: assume it does not transfer until proven.** Where BDK and `wallet-toolbox`
disagree, **BSV-native wins, every time.**

---

## 3. The ledger

One row per lookup. Fill it when you look, not later.

**Verdict values:** 🟢 **paid off** — changed what we built · 🟡 **context only** — useful background,
no decision changed · 🔴 **dead end** — say so plainly, it is the most useful row in the table.

| Date | Question | Source(s) read | What we learned | Verdict | Landed in |
|---|---|---|---|---|---|
| 2026-08-30 | Who originated farbling, and is Tor the right reference for it? | Brave privacy-updates 3 & 4; Brave fingerprinting wiki | **Brave coined it and shipped it first** (prior work: PriVaricator, FPRandom); Firefox and Safari followed. **Tor uses uniformity, the opposite strategy.** Tor remains the reference for the *threat model* | 🟢 | §2.1; `CLAUDE.md` rule 5 |
| 2026-08-29 | Is there a canonical "Karpathy method" to adopt? | `multica-ai/andrej-karpathy-skills`; Karpathy's repos, blog, `autoresearch` | The viral file is **not his**. 3 of 4 principles adopted; the 4th declined as our false-green mechanism | 🟢 | `SCOPING_PROCESS.md` §6–7; `CLAUDE.md` working rules |
| 2026-08-29 | What do PM skill files offer sprint scoping? | `pm-*` upstream sources | ~6 adoptable items out of ~26 skills; **do not install** | 🟡 | `SCOPING_PROCESS.md` §7a, §8 |
| 2026-09-01 | Does any BRC specify identity-key rotation? | Registry sweep: BRC-42/43/44/52/77/98/100/103/104/31/138/137/169/174/190(ex-146)/369; live `gh` code search | **No rotation mechanism exists.** But two near-misses do: **BRC-169 §4.3** forwarding record (old subject key signs `from`/`toIdentityKey`/`toHandle`/`created` — a continuity statement in all but name, coupled to a handle) and **BRC-190 §8.2**, which names three rotation-evidence classes in prose without a format. BRC-52's revocation section answers key compromise with "reissue under a new certifier identity key" — start over, no continuity | 🟢 | rotation design Q1, Q3 |
| 2026-09-01 | Is there a BRC-43 convention for "the login key for site X"? | BRC-31 (Bob's signing), BRC-103 §6.3, BRC-104 §3, BRC-138 abstract + §4, BRC-43, BRC-44, BRC-98 | **No — and all three auth BRCs do the opposite**, authenticating by *disclosing* the identity key (`x-bsv-auth-identity-key`; BRC-138's payload literally carries `identityKey`). Nearest patterns are per-protocol, not per-site: `[2,'authrite message signature']` keyID `<nonceA> <nonceB>`, `[2,'certificate signature']`, BRC-174's `[0,'p 1sat']` | 🟢 | rotation design Q4 |
| 2026-09-01 | How do mature systems rotate a root key? | KERI (draft-ssmith-keri-00, DIF KID0005), did:webvh/did:tdw v0.3, RFC 5011, OpenPGP transition statements, BAP (`@sigma-auth` v0.0.6 terminology matrix) | Two families: **anchored indirection** (BAP, DNSSEC KSK/DS) where a permanent key above the rotating one never rotates — **BAP's member key never changes, so BAP never rotates a root at all**; and **continuity chains** (PGP, KERI, did:webvh) which chain-sign successors. Both modern chain designs add **pre-rotation** (commit to a digest of the next key) so a stolen key cannot rotate; did:webvh adds entry-hash chaining plus monotonic version and time. **PGP transition statements are signed by BOTH keys** | 🟢 | rotation design Q1, prior art |
| 2026-09-01 | Can BRC-52 carry a continuity statement as-is? | BRC-52 primitive types, core certificate JSON, field encryption, keyring encryption, revocation | **Mostly yes** — `certifier`=old key, `subject`=new key, `serialNumber`=replay uniqueness, `revocationOutpoint`=revocation, all plaintext signed members. ⚠️ `fields` values are **always encrypted**, so any extra datum (a pre-rotation commitment) needs a keyring entry with counterparty `anyone` — legal but clunky, and **not yet tested against the SDK** | 🟡 | rotation design Q1 option B |
| 2026-09-01 | Is identity-key rotation worth building at all? | **Nostr** NIPs PRs #158 / #1032 / #1452 + NIP-41 draft; NIST SP 800-57; `@sigma-auth` package (full grep) | ⭐ **The row that killed the project.** Nostr is the same problem with a bigger user base — pubkey *is* the account — and has failed to ship rotation since **Jan 2023**: 3 attempts, 83 comments, NIP-41 still not on master. Their draft: *"best-effort, not guaranteed"*, reduces damage *"from catastrophic to just very bad"*, and fails because it needs pre-commitment users never do. Sigma advertises rotation with **no API, no flow, no use case** — a comparison-table row. **Rotation parked.** | 🟢 | Video 3 §6d; `DESIGN_rotation_2026-09-01.md` PARKED banner |
| 2026-09-04 | How do browsers store favicons locally, so a consent screen never asks Google for one? | CEF headers in-tree (**measured**); Firefox / Chromium / Brave / ungoogled-chromium favicon architecture (**from knowledge — schemas not re-read this session**) | ⭐ **CEF already gives us the whole mechanism and we have never used it**: `CefBrowserHost::DownloadImage(url, is_favicon=true, …)` → `CefImage::GetAsPNG()`, and `is_favicon` makes it send and accept **no cookies**. No patch needed. Every major browser keeps a **local** icon store keyed by page URL, normalised as *icon → bitmaps* plus a separate *page → icon* mapping (many pages share one icon) — our bookmarks table's per-row `favicon_url` string is the denormalised shape and would duplicate. ⚠️ **Chrome is not a clean example**: it has a Google-hosted fallback (`t0.gstatic.com/faviconV2`) for surfaces with no local icon — and **Brave and ungoogled-chromium strip exactly that**, which is precisely the change we are making | 🟢 | Phase 7b favicon row |
| 2026-09-14 | Is closing the CDP port enough, and what does dropping `--remote-allow-origins=*` break? | **Chromium** `content/browser/devtools/devtools_http_handler.cc :: OnWebSocketRequest` (local 7871 tree); **CEF** `libcef/common/chrome/chrome_main_delegate_cef.cc` (port range `[1024, 65535]`); Chrome's own default (port off, DevTools on); `websocket-client` 1.9.0 `_handshake.py` | Chrome ships with the port **off** and F12 on — the same split our design D1/D2 makes. The `= 0` disable path is safe because CEF never forwards an out-of-range port. ⭐ Chromium 403s any Origin-bearing CDP WebSocket upgrade not allow-listed, and `websocket-client` sends `Origin` by default — so D3's "nothing depends on it" was false for **dev**; the switch stays inside `IsDevEnv()`. Also: CDP `Input.dispatchKeyEvent` never reaches CEF's `OnPreKeyEvent` — a posted `WM_KEYDOWN` does | 🟢 paid off — shaped D3 and the P9-A3 instrument | beta.3 Phase 9 (`67a9ab6`, D4 commit) |
| | ⏳ **RQ-1** — what does "classified" persist as? | BRCs 46/99/147/150/165 → `wallet-toolbox` (TS + Go) → other SDKs → BDK *(narrow, per §2.2)* | | | beta.4 M0 |
| | ⏳ **RQ-2** — restore behaviour for unidentifiable outputs | `wallet-toolbox` recovery path; other BSV wallets; recovery-related BRCs | | | beta.4 M0 |
| | ⏳ Farbling residuals — workers, fenced frames, `IsAuthDomain` allowlist | **Tor Browser design document**; Brave's fingerprinting wiki | | | beta.3 §H backlog |

⭐ **The three ⏳ rows are the open questions this file already knows about.** Filling the last one is
cheap and would settle arguments that have been running since the 0.4.0 farbling work.

---

## 4. What the ledger is for — read this before deciding it is overhead

After ~10 rows, it should be able to answer:

1. **Which sources actually change decisions**, and which we cite out of habit.
2. **Which questions we keep re-asking** — a repeated question is a missing document.
3. ⭐ **Whether "look at prior art" is paying for itself.** If a year of rows is all 🟡, the rule is
   ceremony and should be cut. ⛔ **Recording that honestly is the point.** Same standard as the rest
   of the project: a check that cannot come back negative is not a check.
