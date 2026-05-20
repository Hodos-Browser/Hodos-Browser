# OAuth-Connected Personal Data Agent — Research Doc

**Status:** RESEARCH (per `marketing/intelligence/FEATURE_PRIORITY.md`)
**Effort:** L (per `marketing/intelligence/EFFORT_MATRIX.md#oauth-connected-personal-data-agent`)
**First logged:** 2026-05-11
**Parent direction:** Builds on `DOLPHIN_MILK_INTEGRATION.md` — same agent runtime, different capability surface.

---

## Trigger artifact (preserved for future reference)

This feature direction is pinned to a specific public moment. Keep this section intact when revising the doc.

**Bridget Doran (@hbgnostic) — 2026-05-11, two tweets in one thread:**

Main post — https://x.com/hbgnostic/status/2053916754469068827 (snapshot at fetch time: 9 likes, 2 replies, 0 retweets, 3 bookmarks, 144 views):

> Reorged my whole YT channel today using Claude Code calling the YT Data API. 7 new topical playlists, old draft playlists deleted, even pulled YouTube auto-captions on orphan videos so Claude could help me categorize them. Way easier than doing it by hand.
> https://youtube.com/@bridgetdoran

Stack disclosure — https://x.com/hbgnostic/status/2053916756016877820 (snapshot at fetch time: 1 like, 1 reply, 0 retweets, 47 views) — **this is the URL Matt asked to pin**:

> Stack: YouTube Data API v3 (OAuth Desktop credentials), Python client lib, plus yt-dlp for pulling auto-captions. Whole reorg cost ~3,000 quota units out of the daily 10K free tier.

Ruth Heasman (@ruthheasman) reply — https://x.com/ruthheasman/status/2053920003628707879, posted ~13 minutes after Bridget's main post:

> Nice, must try this!

**Provenance & cache:**
- Both Bridget's tweets are captured verbatim in `marketing/social/x/research/broad_hbgnostic.json` (x402 fetch on 2026-05-11, txid `fbd72b9eabce0d5e570b685fe7bfec9a4d0ccad25e27ce816ebbaaec86a779ac`, 360,855 sats).
- Ruth's reply is captured in `marketing/social/x/research/broad_ruthheasman.json` (txid `c73b93870d6cc8ae818e2b5fa8ce97038b99c8d87a6e47ea7965f079102f309a`, 360,855 sats).
- Engagement counts above are frozen at fetch time. To get current values without re-fetching, re-run `marketing/social/x/research/fetch_broad_dolphinmilk_x402agency.py` pattern with `hbgnostic` / `ruthheasman` handles.

**Why this is the trigger:** Bridget is a real BSV protocol-deep builder (Teranode mesh explorer, BSV Intel Report) — not a marketing voice. When she describes a workflow as "way easier than doing it by hand," and another credible builder (Ruth) immediately replies "must try this," that's a two-builder demand signal for the underlying pattern within 13 minutes. Not for YouTube specifically — for **the pattern** of an AI agent acting on a user's behalf across an OAuth-gated API. Hodos is uniquely positioned to make that pattern one-click.

**Revisit prompt** (use when revisiting this doc 6+ months out): "Did we ship OAuth-Connected Personal Data Agent? Has @hbgnostic's workflow gotten easier in the broader ecosystem, or is the gap Bridget described still there? Has anyone else closed it?"

---

## ⚠️ Critical path — Google OAuth verification (read first, plan the cost)

**TL;DR for future-us:** Hodos must register as a Google OAuth client to ship this feature. Sensitive/restricted scopes (YouTube management, Gmail mutations, Drive full access) require Google verification — weeks to months of process — and **restricted** scopes additionally require an annual third-party security assessment (CASA) that costs real money. Start the verification submission **the moment a working demo exists**, not at launch.

### What "OAuth client" means here

When a user clicks "Connect YouTube" in Hodos and Google shows them a consent screen, the screen reads *"Hodos Browser wants to manage your YouTube channel."* That registered identity — "Hodos Browser" — is the OAuth client. It is registered by Marston Enterprises in Google Cloud Console, gets a client ID shipped inside the binary, and is the thing Google reviews. Hodos being a Chromium browser is irrelevant to Google's process; what matters is that Hodos is the third-party application requesting scopes on the user's behalf.

This is the same shape as Notion (when it imports your Calendar), Slack (when it pulls Drive previews), or Claude's Gmail MCP server (which Anthropic registered + verified). Chrome is exempt only because Google made Chrome. Brave / Vivaldi don't hit this because they don't call Google APIs on the user's behalf.

**Distinction worth keeping straight:** YouTube web UI login (cookie-based session) — every Chromium browser including Hodos already does this with zero registration. YouTube Data API (OAuth-token-based) — a separate surface a *program* uses to act on the user's behalf, and the program must be a registered OAuth client. The agent walks through Door 2, the user walks through Door 1.

### Scope tiers and what each costs

| Scope tier | Examples | Verification | Annual cost | Timeline (first submit) |
|---|---|---|---|---|
| Non-sensitive | Basic profile, public YouTube read-only | None required | Free | Immediate |
| Sensitive | YouTube channel management (`youtube`, `youtube.force-ssl`), Calendar mutations | Google review only (no CASA) | Free | 4–8 weeks typical |
| Restricted | Gmail read/modify/send, Drive full access | Google review **+ annual CASA assessment** | **~$3K–$15K+/year** (CASA) | 8–16+ weeks |

CASA = Cloud Application Security Assessment, performed by a Google-approved third-party assessor (Bishop Fox, NCC Group, and others). Required annually for as long as Hodos uses restricted scopes. Cost varies by assessor and app complexity. Not negotiable for restricted scopes once Hodos wants more than 100 test users.

Until verification ships, Hodos can operate in **test mode** with up to 100 explicitly added test users — fine for closed alpha, kills public GA.

### Application timing — start when you have a demo, not when you launch

The verification submission requires:
- A registered OAuth client with the actual production scopes
- A privacy policy URL on a real domain (Hodos has one — hodosbrowser.com)
- A homepage describing the integration
- A demo video walking through the OAuth flow and what the agent does with the scopes
- A scope justification document explaining *why* a desktop browser needs each scope

You can't submit these from a research doc — you need a working integration to film the demo. The real timing rule: **submit the moment the integration spike produces a recordable demo**, then build the rest of the feature while verification runs in parallel. Submitting only after launch readiness = 4–16+ week wall in front of public availability.

### Two ways to dodge or reduce the cost

1. **Ship Stage 1 with non-sensitive / read scopes only.** "Agent reads your channel and recommends a reorg" — no mutations, no verification needed, all users get it. Stage 2 with mutation scopes then has its own ship gate. This is the smallest-risk first step and gets a credible demo on the public record fast.
2. **Let Dolphin Milk be the OAuth client.** John Calhoun registers Dolphin Milk as the Google OAuth client and eats the verification cost. Downsides: shared rate limits across all Dolphin Milk users (Hodos + standalone), shared liability for misuse, less Hodos branding on the consent screen. Upside: Marston Enterprises doesn't pay CASA. This is a coordination conversation, not an engineering one — worth raising with John before either side starts the verification clock.

### What to track in marketing/intelligence

Strategic/cost dimensions of this — CASA assessor selection, scope-tier decisions, Calhoun coordination — belong in `marketing/intelligence/features/oauth-connected-agent/` once that folder lands. This engineering doc stays the technical reference; the marketing folder holds the business decisions and external-relationship notes.

---

## The idea in one sentence

Let Hodos users say "agent, reorganize my YouTube channel into topical playlists" (or "clean up my inbox", "organize my Drive", "consolidate my X bookmarks") — and have it actually happen, with the user's own OAuth token transparently brokered by the browser, the agent's reasoning paid for via x402, and a verifiable on-chain proof of what was done.

## Demand signal — 2026-05-11

@hbgnostic (Bridget Doran, BSV protocol builder, runs `explorer.utxoengineer.com` + BSV Intel Report) posted:

> "Reorged my whole YT channel today using Claude Code calling the YT Data API. 7 new topical playlists, old draft playlists deleted, even pulled YouTube auto-captions on orphan videos so Claude could help me categorize them. Way easier than doing it by hand."
>
> Stack: YouTube Data API v3 (OAuth Desktop credentials), Python client lib, plus yt-dlp for pulling auto-captions. Whole reorg cost ~3,000 quota units out of the daily 10K free tier.

@ruthheasman replied: "Nice, must try this!"

That's two visible BSV-ecosystem builders within 24 hours describing the same workflow as a desired pattern. Both Bridget and Ruth are now profiled in `marketing/profiles/bsv/`.

## The YouTube Data API v3 — what it actually is

Google's official API for YouTube. Two authentication surfaces with very different capabilities:

| Operation | Auth required |
|---|---|
| Search public videos; read public channel/video metadata | **API key only** (no user OAuth) |
| Manage your own channel — list drafts, create/delete playlists, move videos, edit titles, manage captions | **OAuth 2.0** with the user's Google account, `youtube` or `youtube.force-ssl` scope |
| Auto-captions on someone else's videos | **Not exposed via official API** — yt-dlp scrapes them off the watch page |

Free-tier quota: 10,000 units/day. Bridget's reorg burned ~3,000 units (~7 `playlists.insert` × 50 + ~50 `playlistItems.insert` × 50 + some deletes/updates).

Key gotchas:
- OAuth Desktop credentials require a Google Cloud Console project with the YouTube Data API enabled and an OAuth client configured.
- The first run does the consent screen + redirect-with-code dance.
- After that, refresh-token-based silent re-auth works until the user revokes access.
- Captions on other people's videos: yt-dlp is the workaround (browser-emulation scrape).

## Is this just "another x402 endpoint"?

**Half yes, half no.** Two distinct workstreams:

**(a) Public read operations** — yes, this could be a Calhoun-runs-it x402 endpoint. "yt-research.x402agency.com/search" or similar. Pay BSV per call, get public YouTube data without an API key. Same model as `x-research.x402agency.com`. Worth requesting from Calhoun — see the "Reply considerations" section below.

**(b) Personal-account operations** — no. Each user's YouTube account is OAuth-gated. Calhoun cannot centrally proxy these because the API checks the token belongs to the channel owner. No amount of BSV solves this; it's a deliberate Google security boundary.

The interesting Hodos opportunity is (b) — the part x402 *cannot* solve centrally.

## The Hodos path — why "use the YouTube login cookie" does NOT work

YouTube's web UI authenticates with session cookies. The YouTube Data API authenticates with OAuth 2.0 bearer tokens. They are different surfaces — Google deliberately separates them so API access goes through a permission-gated dance with explicit scopes. A logged-in YouTube tab does not authenticate API requests.

What Hodos can do instead is **handle the OAuth flow seamlessly because Hodos IS the browser**:

1. **First-time setup:** User clicks "Connect YouTube" (or generic "Connect Google Account"). Hodos pops up the OAuth consent screen in a Hodos tab. User clicks "Allow." Redirect URL (e.g., `http://localhost:31301/oauth/callback` or a custom Hodos URI scheme) is intercepted by Hodos. Code is exchanged for access + refresh tokens.
2. **Token storage:** The refresh token is encrypted with BRC-2 (using the user's wallet keys — `rust-wallet/src/crypto/brc2.rs`) and stored in the Hodos wallet DB. Backup via the existing wallet backup flow.
3. **Agent invocation:** User chats with the bundled Dolphin Milk: "Reorganize my YouTube channel into topical playlists like @hbgnostic did." 
4. **Reasoning paid via x402:** The agent's LLM calls (decide what playlists to create, what video goes where) cost ~$0.05–$0.20 for a channel reorg's worth of thinking, paid from Hodos's wallet via Dolphin Milk's existing x402 flow.
5. **API calls free:** Actual YouTube API calls use the user's cached OAuth token and the user's own 10K-units/day free quota. Zero BSV cost for the calls themselves.
6. **Auto-captions:** Replace yt-dlp side channel with either (a) a small Rust caption extractor bundled in Hodos, or (b) treat yt-dlp itself as another bundled binary (it's GPL-licensed — check licensing implications). The mechanism is straightforward scraping; the auth is the same logged-in YouTube session Hodos already has.
7. **On-chain proof:** Every Dolphin Milk action (each playlist created, each video moved) becomes a BRC-18 OP_RETURN proof. Surfaced in Hodos's transaction history as "agent activity."

## Why this is bigger than YouTube

OAuth-Connected-Agent is a **general pattern**, not a single feature. Gmail, Google Drive, Google Calendar, X/Twitter (with Bearer Token), GitHub, Notion — every major user-data API uses OAuth 2.0. The same Hodos infrastructure (one-click OAuth in-browser → encrypted refresh token in wallet → agent uses it via Dolphin Milk) extends to all of them.

The unit of value is **"AI agent that can act on your behalf because it's already inside the browser session where your OAuth tokens live."** That's a story no Chromium fork can tell.

**The set of demand signals at launch:**
- YouTube channel reorg (Bridget)
- "Must try this" (Ruth)
- Likely Gmail inbox cleanup (universal pain)
- Likely Google Drive organization (universal pain)
- Possibly Twitter bookmark consolidation
- BSV-specific: "Build me a paymail" / "Move my UTXOs" (no OAuth needed but same agent pattern)

## Hard questions

### 1. OAuth client registration and per-app credentials

See the **"Critical path — Google OAuth verification"** section at the top of this doc for the full breakdown of scope tiers, verification timeline, CASA cost, and the application-timing rule. Outstanding questions specific to client setup (not covered in the top block):

- Per-platform client ID — does Google require separate clients per OS (Windows / macOS / Linux), or does one Desktop client cover all? Lean toward one client; confirm before submission.
- Rate limits and quota are tied to the OAuth client — if all Hodos users share one client ID, per-client quota becomes a scaling constraint worth modeling before public launch.
- Refresh-token expiry behavior — Google rotates refresh tokens; what happens if a Hodos user goes 6 months without using the agent? Need a graceful re-consent flow, not an opaque failure.
- Brand / consent-screen UX — the consent screen displays the OAuth client name + logo + verified status. "Hodos Browser" should be the displayed name; logo must be uploaded with the verification submission.

### 2. Token storage and security

Storing OAuth refresh tokens encrypted in the Hodos wallet DB is the right pattern but raises questions:

- Encryption at rest — BRC-2 with wallet keys is correct; reuse `crypto/brc2.rs` pattern.
- Backup compatibility — token must round-trip through the existing wallet backup flow. `paid_url` precedent (local-only fields, per memory `project_phase15_brc121_activity_url_recording`) is relevant here.
- Revocation — UI to disconnect / revoke a connected account. Must remove local token AND optionally call Google's token-revocation endpoint.

### 3. Privacy and consent

Asking a user to grant their AI agent full access to YouTube/Gmail/Drive is a major trust ask. UX must:

- Show scopes plainly ("This will let the agent read and modify your YouTube channel and playlists.")
- Surface what the agent did (BRC-18 proofs help here)
- Make revocation one click

The "AI does things to your accounts" pattern can go wrong fast if it's not transparent.

### 4. Wallet API compatibility (carried over from Dolphin Milk doc)

This feature depends on `DOLPHIN_MILK_INTEGRATION.md` actually shipping — the agent runtime needs to be in-browser before OAuth tools can be added to it. The wallet-API-compat audit between Hodos's BRC-100 wallet and Dolphin Milk's expected `bsv-wallet-cli` surface remains the blocking dependency.

### 5. Reply considerations — should we reply to Bridget's post?

Two possible replies, both honest:

**To Bridget (the YT reorg post):** Acknowledge the workflow, signal that Hodos is heading toward making it one-click. Don't be a feature flex; be a peer who's working on the same problem. Hold until OAuth-Connected-Agent is closer to a real shippable feature; replying with a "we're building this" hook without a working demo is a credibility loss.

**To Calhoun (@x402agency):** Ask if a public YouTube research endpoint on x402agency would fit the marketplace. This is a legitimate ask (we're an existing paying customer), and Calhoun has been broadly receptive to roster expansion. Worth doing.

Both replies should be drafted, not posted, until the feature direction is committed. See `marketing/social/strategy/POSTING_STRATEGY.md` for the standard pre-publish litmus.

### 6. What if Dolphin Milk doesn't have YouTube tools yet?

Dolphin Milk has 38 tools today, including `browse`, `read_file`, `search`, etc. It does NOT have native YouTube / Gmail / Drive tools — would need to:

- Contribute them upstream (Calhoun coordination needed)
- Or wrap Dolphin Milk's `wallet_call` / `x402_call` primitives with Hodos-specific tools registered at runtime
- Or write a Hodos-native tool layer that lives alongside Dolphin Milk in the bundled stack

This is a design question to resolve before commitment.

## Cross-feature dependencies

| Depends on | Why |
|---|---|
| `DOLPHIN_MILK_INTEGRATION.md` | Agent runtime must be in-browser before we can plug OAuth tools into it |
| Wallet backup compatibility | OAuth refresh tokens need to round-trip through `backup.rs` (similar to local-only `paid_url` field pattern) |
| Existing `crypto/brc2.rs` BRC-2 encryption | Reused for refresh-token encryption at rest |
| Existing CEF browser auth UI patterns | OAuth consent screen handling, redirect URL interception |

## Open questions

- [ ] Google OAuth client registration — what scopes, what verification, what per-platform requirements?
- [ ] Where do OAuth refresh tokens live in the DB schema? New table or extend existing?
- [ ] Does the existing `backup.rs` flow correctly treat encrypted refresh tokens as local-only (do not export)?
- [ ] Should Dolphin Milk get YouTube/Gmail/Drive tools upstream, or should Hodos register tools at runtime?
- [ ] What does the "Connect Account" UI look like in Hodos's overlay system? Wallet panel? Settings? New overlay?
- [ ] How does the agent surface what it did before doing it? Dry-run mode? Pre-approval prompt? Just-do-it-with-undo?
- [ ] Reply strategy on Bridget's post (and possibly Ruth's): hold or send. See section 5.

## Related

- `marketing/profiles/bsv/hbgnostic.md` — Bridget Doran profile (the demand signal)
- `marketing/profiles/bsv/ruthheasman.md` — Ruth Heasman profile (the "must try this" reply)
- `development-docs/Possible-MVP-Features/DOLPHIN_MILK_INTEGRATION.md` — parent feature; this depends on it
- `marketing/intelligence/FEATURE_PRIORITY.md` — bucket assignment
- `marketing/intelligence/EFFORT_MATRIX.md#oauth-connected-personal-data-agent` — full effort scoring
- `marketing/intelligence/ECOSYSTEM_PULSE.md` — week-of-2026-05-11 entry with the YT reorg signal
