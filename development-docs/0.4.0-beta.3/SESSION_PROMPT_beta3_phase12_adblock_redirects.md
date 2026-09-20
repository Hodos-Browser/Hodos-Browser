# Session prompt — beta.3 **Phase 12: adblock on redirected arrivals**

Paste this to open the phase in a fresh context.

---

Read `development-docs/0.4.0-beta.3/phase-12-adblock-redirect-arrivals/README.md` and run Phase 12.
Follow the phase-kickoff workflow in the root `CLAUDE.md` before writing any code, and hand back the
kickoff summary before the first commit.

## The report

👤 *"The ad blocker works when the user navigates to YouTube and watches a video. If the user clicks a
YouTube video from inside X, it launches in a new tab or navigates there, but the ad is not blocked."*

## The hypothesis — a code read, NOT a measurement

`OnBeforeBrowse` pre-caches cosmetic scriptlets keyed by the **requested** URL; `OnContextCreated` looks
them up by the frame's **actual** URL and injects only on an **exact string match**. A link from X is a
redirect chain (`t.co` → `youtu.be` → `youtube.com/watch`), so the key is the first URL and the document
is the last. No match, no injection, the ad plays.

⛔ **Reproduce before designing anything.** The phase README says so and it is the phase's whole shape.

⭐ **Sharpest first measurement:** paste a `youtu.be` link straight into the address bar, no X involved.
If that alone reproduces it, X is irrelevant and this is purely about redirects — which changes the fix.
Then compare, in the log, `OnBeforeBrowse: pre-caching scriptlets for <url>` against
`OnContextCreated: injecting scriptlets for <url>` and see the two URLs disagree.

⚠️ **The browser log lives at `%APPDATA%/HodosBrowserDev/logs/debug_output-<pid>.log`** — NOT
`cef_debug.log`, and NOT the copies under `cef-native/build/bin/Release/`. Two sessions have been lost to
reading the wrong file; the build-dir `debug.log` did not contain the tab's activity at all.

## Prior art is MANDATORY here (root `CLAUDE.md` rule 5)

Brave keys cosmetic/scriptlet injection off the frame's **committed** URL at script-context creation,
fetched from the browser process — a redirect cannot desynchronise a lookup that happens at commit time.
Read the real source before citing the shape; MPL-2.0, so port the **pattern**, never the code. Log a row
in `development-docs/PRIOR_ART.md`.

## Candidate fixes — choose AFTER the measurement

1. re-key on every `OnBeforeBrowse` including redirects, and canonicalise the key (cheapest, keeps the
   pre-cache);
2. move the lookup to commit time, Brave's shape — ⚠️ a sync IPC on the render thread's critical path, so
   measure first-paint cost;
3. both: pre-cache as the fast path, commit-time lookup as the miss handler.

## Evidence rows (reserved in the README)

`P12-A1` redirect arrival blocked · `P12-A2` direct navigation still blocked (non-regression) ·
`P12-A3` **both** the new-tab (`OnBeforePopup`) and same-tab arrivals · `P12-A4` minimal site basket
(youtube, x, github) unchanged. ⛔ Every row needs its negative control, and real-site testing is
mandatory. Both platforms; relay row for macOS.

## ⭐ Phase 12 and Phase 13 may share a root cause

The pre-cache comment says it exists so anti-bot scripts like Cloudflare Turnstile observe the mutated
surface. If scriptlets are not injecting on redirect arrivals, that plausibly feeds the CAPTCHA complaint
too. Worth noting when the mechanism is measured — do not assume it.

## State at handover

- Head **`b47faf3`** on `0.4.0`. `preflight -Full` **PASS** (all gates + all T1, nothing skipped);
  `-NegativeControl` **PASS** (every gate seen to fail). Phase 11 = 11/11 closed.
- ⛔ Dev environment is **DOWN**, deliberately — bring it up yourself so you own a **fresh**
  `debug_output-<pid>.log` with no prior session's noise in it, and so the first build cannot hit
  `LNK1104` against a running dev browser. Run order is in the root `CLAUDE.md`: `.\dev-wallet.ps1`,
  `.\dev-adblock.ps1`, `cd frontend && npm run dev`, then `cd cef-native && .\win_build_run.sh`.
- ⭐ **`R-GOLD` is GREEN** — real money, correct tab, confirmed in the log and on the chain. First time
  this sprint. ⛔ Still owed: the stub-the-emit RED (build-side) and the **BRC-121 paid-retry** emit route.
  👤 The owner will run the GREEN direction at **real sites** at sprint end; `R-COUNT` likewise.

### Carried forward — do not lose

| | |
|---|---|
| `TICKET_connect_on_the_IPC_transport_still_resends_an_empty_body.md` | 👤 owner-found; macOS's open arm; NOT fixed |
| `TICKET_createAction_accepts_an_empty_lockingScript_and_spends_into_it.md` | money path; **asked, not changed** (invariant 13) |
| `TICKET_transaction_row_can_sit_at_created_while_its_coin_is_on_chain.md` | LOW; its non-coverage is protective |
| Phase 13 **step 0** | the CAPTCHA report came from **0.3.x injected-JS farbling**; re-test on 0.4.0 before designing |

### Standing rules that bit in the last session

- ⭐ **`G11` was ratcheted 59 → 58 on 2026-09-19** (`b47faf3`) — nothing owed, but the lesson is live: a
  hand count of **occurrences** disagreed with the gate, which counts matching **lines**, and three
  plausible explanations were wrong before the real one. ⛔ Never set a baseline by hand.
- ⛔ **`git fetch && git rebase origin/0.4.0` before starting, and REBUILD after.** macOS pushed **9**
  commits during one session and edited **three** files Windows had also edited — `LayoutHelpers.h`,
  `BRC100AuthOverlayRoot.tsx`, `HttpRequestInterceptor.cpp`. Every rebase was conflict-free, which proves
  nothing; verify your change **behaviourally**, not textually.
- ⛔ **Never kill a Hodos process by image name — use `scripts/stop-dev.ps1`.** It spared 44 of the
  owner's installed-build processes while stopping 19 dev ones.
- ⚠️ **There are uncommitted beta.4 ticket files in the tree that are the OWNER's.** Stage by file, never
  by directory, and never run a destructive hard reset.
- ⛔ **Negative controls.** Two of mine were worthless last session: one passed with the fix reverted (the
  wallet bridge had to be stubbed to reproduce production timing), and one drove a prompt type that does
  not render the element at all. Name the **layer** and the **subject** your instrument reads.
- 📏 **Ask the chain** before believing our own response about money.
