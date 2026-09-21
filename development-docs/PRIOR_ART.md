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
| 2026-09-15 | Does the bsvalias P2P payment-destination spec require the returned outputs to sum to the requested satoshis (10c rule 4)? | BRFC `2a40af698840` v1.1 text recovered from `moneybutton/paymail-client` `docs/paymail-07-p2p-payment-destination.md` (bsvalias.org itself is behind a Cloudflare JS challenge — HTTP 403 "Just a moment"; `bitcoin-sv-specs/brfc-paymail` does not carry the P2P docs); reference client `bitcoin-sv/go-paymail :: p2p_payment_destination.go` | ⛔ **No.** The spec says only *"a list of outputs to receive the payment"* — its own example requests **1,000,100** sats and returns outputs of **10,000 + 20,000**. HTTPS appears only in the example URL, never as a MUST. The reference client requires `https://` in the P2P URL and checks `reference`/`outputs`/`script` non-empty — **and has no sum check either**, i.e. the canonical client shares CU-2's shape. ⇒ "sum == approved amount" is **Hodos's invariant** (the same one 10b enforces: a signature exists only for an amount the user saw), not the spec's — needs the owner's explicit yes | 🟢 paid off — turned an inferred rule into an owner decision | beta.3 Phase 10c kickoff |
| 2026-09-15 | Which bot-detection vendors have public demo pages, and what signals do they score? | 30 sources — Cloudflare Turnstile docs, reCAPTCHA/hCaptcha/GeeTest demos, GitHub's own connectivity docs, DataDome "Picasso" + CDP-signal posts, castle.io "When canvas lies", `brave/brave-browser#45608` + `#58915`, BotD, CreepJS, puppeteer-extra-stealth, `tls.peet.ws`, AWS WAF JA3/JA4 docs (full table in `0.4.0-beta.3/phase-13-bot-detection/RESEARCH_steps_1_2.md`) | ⭐ **Vendors allow-list known randomising browsers by name** (castle.io names Firefox/Brave/Samsung) and score everything else as "abnormal" — a CEF embedder with its own brand string that farbles like Brave is structurally likelier to be flagged than Brave is. Cloudflare's diagnostics literally log *"WebGL renderer info is spoofed/blocked"* (#58915, fetched). Only 5 of 13 vendors have a public demo (Turnstile test keys, reCAPTCHA, hCaptcha, GeeTest, Arkose; Kasada's 401s a non-browser fetch); DataDome/HUMAN/Akamai/Imperva/Shape have none. TLS/JA4 is a signal family JS cannot see — our own BoringSSL build could diverge from Chrome's ClientHello; `tls.peet.ws` echoes it | 🟢 paid off — shaped the P13 matrix rows/columns and the allow-list hypothesis | beta.3 Phase 13 steps 1–2 |
| 2026-09-16 | `go-wallet-toolbox` (BSV Association) | How a conforming wallet releases a transaction it will never broadcast | `pkg/storage/internal/actions/abort.go :: abortTx` does four things in ONE unit of work — unreserve the inputs, restore the outputs it marked spent, **mark created outputs NOT spendable**, then set the status with a positive CAS against the abortable statuses — and gates the whole abort on the transaction *provably* never having reached a broadcaster. ⭐ **Two things we took**: it MARKS created outputs rather than deleting them (the row survives as a record and simply cannot be selected), and it is one atomic unit rather than a sequence of updates. ⭐ **One thing we already had**: `disable_by_txid` + `restore_by_spending_description` were written for the broadcast-failure path and are exactly these primitives — the refusal path just never called them. ⛔ We did NOT take the CAS on status: our refusal holds the create lock and the broadcast has not happened, so there is no racing transition to lose to. 👤 Owner asked for this lookup by name ("how does the wallet toolbox handle this?") before the code was written | Payment sitting M4; `handlers.rs :: release_unbroadcast_transaction` |
| 2026-09-18 | In the address bar, what do Tab and Enter do — and which key accepts an inline autocompletion? | **Firefox** Bugzilla 1596264 (urlbar owner's own words) + `browser.urlbar.autoFill` in the source docs; **Chromium** `omnibox_view_views.cc` and the escape-ladder commit `60dbc25` ("[omnibox] Implement escape key handling logic to revert user input") + `2de0b86` `kBlurWithEscape`; **Vivaldi** Address Field help page ("Address Auto-Complete") + forum thread 14576 on Tab cycling | ⭐ **All three agree, and all three disagree with what our dead code was trying to do.** Tab is **not** the accept key — it **moves through the results**; the **right arrow** accepts the inline completion. Quoting a Firefox urlbar owner directly: *"in Firefox (and Chrome too) TAB moves through results, that's why we complete with the right arrow"* — and the bug asking them to change it has sat **NEW since 2019**, explicitly to protect muscle memory. ⛔ The real disagreement is narrower than the ticket implied: it is not *what Tab does*, it is whether Tab-cycling is **liked** (Vivaldi users complain it is counter-intuitive; Mozilla defends it). Chromium's Escape is a **ladder**, not one action: revert temporary text → close the popup → restore the page URL → blur. 📏 Measured against our own build the same day: we have **no inline autocomplete at all** — `autocompleteText` is assigned `''` at all 8 sites, so the `Tab \|\| ArrowRight \|\| End` branch is unreachable dead code; Tab blurred to the toolbar and left the dropdown on screen indefinitely; one Escape did all four rungs at once | 🟢 paid off — it inverted the change. The dead code said "Tab accepts"; the prior art says "Tab traverses", which is what shipped | beta.3 Phase 11 item 4 (`P11-I4`) |
| 2026-09-18 | How do other browsers stop Ctrl+wheel zooming their own toolbar? | **Chromium** `chrome/browser/ui/views/toolbar/toolbar_view.h`, `render_widget_host_impl.cc :: OnWheelEventAck`, `web_contents_impl.cc :: HandleWheelEvent`, `web_mouse_wheel_event.cc :: GetPlatformSpecificDefaultEventAction`, `screen_win.cc :: GetScaleFactorForDPI` (all read locally); **CEF** `libcef/browser/browser_host_base.cc`, every public header; **Electron** issue #8793; MDN wheel event | ⛔ **Chrome and Brave never solved this, because they never had it**: their toolbar is `views::AccessiblePaneView`, native C++, so page zoom cannot reach it. Ours IS a web page, so the right prior art is **Electron**, whose answer is `preventDefault()` on a **non-passive** wheel listener (React's own `onWheel` is passive and its preventDefault is silently discarded). ⭐ Verified sufficient from source, not assumed: zoom is only reached `if (ack_result != kConsumed && delegate_->HandleWheelEvent(…))`. ⭐ And the opposite direction is also settled by source — `GetScaleFactorForHWND` is *"including accessibility adjustments"*, so chrome SHOULD grow with the OS text-size setting, which is why the two routes need opposite answers | 🟢 paid off — it turned a claimed CEF-patch-plus-Chromium-rebuild into 4 lines of JS | beta.3 Phase 11 item 7 route 1 |
| 2026-09-21 | How far does Hodos actually diverge from stock Chrome on the signals bot-detection vendors score — and is `--disable-gpu-compositing` the culprit the README suspected? | **Measured, not read**: Hodos dev at head vs **stock Chrome 152.0.7977.78** on the same machine, same day (`p13_signals.py` / `p13_chrome.py`, `phase-13-bot-detection/`); `tls.peet.ws/api/all` for JA3/JA4 + HTTP/2 + the real header block; the 0.3.x script recovered from `v0.3.0-beta.29`. Re-read for framing: castle.io *"When canvas lies"*, `brave/brave-browser#58915` | ⭐ **Most of the sheet is identical to Chrome** — WebGL vendor/renderer/version/extensions **byte-identical**, HTTP/2 fingerprint hash identical, header **set and order** identical, `screen`/plugins/mimeTypes/timezone identical, `webdriver` false with no `cdc_` keys. ⛔ **`--disable-gpu-compositing` is NOT the problem** — proven by direct manipulation, Chrome's WebGL strings are unchanged with and without it. ⭐ **The real residual is the one the literature predicted and the README did not list:** our `sec-ch-ua` carries **no Chrome brand** (`"Not;A=Brand";v="8", "Chromium";v="150"`) while `navigator.userAgent` claims `Chrome/150.0.0.0` — an *internal* inconsistency, and exactly castle.io's "abnormal, not a known randomiser" bucket. Every shipping Chromium derivative (Brave/Edge/Vivaldi) adds a third brand; two brands is what a **bare Chromium** reports. ⭐ JA4 differs from Chrome 152 by exactly **one** extension (`0xca34`, a 151/152 addition) with an identical cipher component — i.e. our TLS is *consistent* with our UA, just two majors old | 🟢 paid off — it killed the README's leading hypothesis and replaced it with a better-sourced one | beta.3 Phase 13 Step 0 + Block B |
| 2026-09-19 | On macOS, how do you make a dropdown overlay belong to the window that opened it — and what breaks when you re-parent an `NSWindow` at runtime? | **Chromium** `components/remote_cocoa/app_shim/` read locally: `native_widget_ns_window_bridge.mm :: SetParent` / `SetVisible` / `OrderChildren`, and `native_widget_mac_nswindow.mm :: -addChildWindow:ordered:` / `-removeChildWindow:` | ⭐ **Four things, three of which we would have got wrong.** (1) `-addChildWindow:` **resets the child's window level** — Chromium saves and restores `childWin.level` around the `super` call, and without it every `NSPopUpMenuWindowLevel` dropdown of ours would have silently dropped to normal level. (2) Re-parenting **removes from the old parent first**; a window has exactly one parent. (3) *"Cocoa's childWindow management breaks down when child windows are hidden"* — Chromium removes the child on hide, which is why our hide path **detaches** instead of handing back to the primary the way the Windows fix does. (4) Adding a child to a parent that is not visible on the active space **switches Spaces** (crbug 783521 / 798792), so the attach is guarded on `isVisible` + `isOnActiveSpace`. ⭐ It also confirmed the *shape*: Chromium keeps ownership following the window, exactly as Windows' Phase 3.5 does | 🟢 paid off — (1) and (3) are defects we would have shipped; the level reset in particular fails **invisibly**, as a dropdown that is occasionally behind something | beta.3 `D-h2` (macOS Phase 3.5 port) |
| 2026-09-19 | Where does a browser resolve the cosmetic-filter / scriptlet key for a navigation, so that a redirect **or a cross-process navigation** cannot desynchronise it? | **Brave** `components/cosmetic_filters/renderer/cosmetic_filters_js_render_frame_observer.cc` + `cosmetic_filters_js_handler.cc` (read from source, MPL-2.0 — pattern only, no code); Brave issue #30062 (relative-URL first-party resolution) | ⭐ **Our phase doc had the shape wrong, and the source corrected it.** Brave does **not** look the rules up at script-context creation — it resolves the URL in `DidStartNavigation`, re-validates it in **`ReadyToCommitNavigation`**, and calls `ProcessURL` there. That callback runs **in the frame that is about to commit**, i.e. in the **destination render process**, which is exactly why neither a redirect nor a cross-site process swap can strand the lookup. Three properties worth porting: (1) resolve **at commit, in the committing frame** — never push from the browser process ahead of the swap; (2) an explicit **fallback key** (`url_` empty/invalid/`about:blank` ⇒ the frame's security origin) so a miss is impossible rather than silent; (3) `RunScriptsAtDocumentStart` **waits** on a `OneShotEvent` when the fetch has not returned, rather than skipping — our code skips silently. ⚠️ Also corrects the sync-IPC worry: Brave's synchronous load is behind the `kCosmeticFilteringSyncLoad` feature flag, async mojo is the other path — so sync-on-the-critical-path is a **tunable Brave itself hedges**, not its settled answer | 🟢 paid off — it disproved our own README's "keyed by the committed URL at context creation" citation, and (2)+(3) are two silent-miss guards our implementation lacks | beta.3 Phase 12 |
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
