# 🔬 We cannot say how many people use Hodos, and the only number we have counts machines updating

**Found:** 2026-09-17, planning the Q4 marketing push. Download counts are the only user signal we
have, and `Hodos/marketing/Metrics/DOWNLOAD_METRICS.md` says itself that they do not answer the
question.
**Status:** ⬜ UNASSIGNED · **Sprint:** unassigned · **Filed by:** owner (Matt), 2026-09-17

> ⚠️ **Method note.** The measurement below is **read from our own metrics doc**, not re-pulled today.
> The design in *Proposed fix* is **a design, nothing built and nothing tested** — no endpoint exists,
> no code was written, and the linkage claims are reasoning about the scheme rather than results.
> ⛔ **Not verified:** the prior art named below. Those are names recalled from memory and **every one
> of them must be read at its own source before it is cited anywhere**, including in a post.

---

## What happens

Nothing in the browser reports that it is running. The only numbers we have come from GitHub Releases
installer downloads, and they count the wrong thing:

- ≈654 cumulative installer downloads across 37 beta releases as of 2026-07-18 *(read: `DOWNLOAD_METRICS.md`)*
- The same doc's own note: the rise from ≈442 on 2026-06-09 is **mostly auto-update churn**, because
  every prior install re-fetches each new build, and **genuine unique users are "low tens"**
- The count also carries VirusTotal scans, SmartScreen validation, AV vendors, indexers and scrapers

So the question *"did anyone open the browser today"* has no answer, and *"did a post bring anyone
in"* has no answer either.

## Why it matters

Q4 2026 is the first marketing quarter with a schedule behind it (October to December). Without a
user number that means something, we cannot tell a post that worked from a post that did nothing, and
the quarter ends with the same "low tens" estimate it started with.

👤 **Owner decision, 2026-09-17: skip the Cloudflare Worker in front of `appcast.xml`.** Counting
update checks needs no browser change, and it still cannot separate one machine checking ten times
from ten machines. The effort buys a trend line and no user count.

## How exposed are we — answer this first

The exposure here is **ours, not the user's**: this ticket proposes *adding* a network call to a
browser sold on privacy. The severity question is what that call can ever be used to prove.

| If | Then |
|---|---|
| The report carries a stable, permanent install ID | ⛔ We have built exactly the tracking we tell people we refuse. Not acceptable at any count |
| The report carries a rotating derived ID | Counting works, and linkage is bounded by the rotation window — the cost named below |
| The report carries any URL, page, balance, key or wallet field | ⛔ Out of the question, in any form, at any aggregation |
| The report is on by default and undisclosed | 🚨 The worst case. One person reading the network trace writes the story, and they would be right |
| The report is disclosed, documented and switchable | It becomes something we can publish and defend |

## What already protects us, and how that shapes the fix

Nothing protects us, because nothing exists yet. That is the advantage: the scheme is chosen before
any data has been collected, so there is no legacy identifier to migrate off.

## Proposed fix

**The scheme.** A local secret and a rotating derived ID:

- Generate a random secret at install. **It never leaves the machine.**
- Send, once a day: `HMAC(secret, "2026-10-04")` and `HMAC(secret, "2026-10")`, plus version and OS.
- The daily hash gives exact **DAU** with no linkage across days. The monthly hash gives exact **MAU**
  with no linkage across months.
- The server keeps the current day and month sets, counts distinct, and discards them.

⛔ **The honest cost, stated because it is real:** within a single month, the monthly hash links one
install's days together. That is inherent to computing MAU at all — anything that can count a person
twice can tell it is the same person. We say so in our own telemetry document rather than let someone
else find it.

**What makes it safe for a privacy browser:**

- No IP stored. Cloudflare gives country; drop the rest at the edge
- Never a URL, a page, a balance, a key or any wallet field
- An off switch in settings
- A first-run notice
- A public page listing exactly what is sent, field by field

⭐ **Done that way it is a post:** *"here is every byte our browser sends us, and the switch that stops
it."* Done quietly it is the one story that would actually hurt us.

**Prior art to read before designing** — ⚠️ **all unverified recall, every one needs checking at
source:** Brave's P3A and its STAR / Nested STAR k-anonymity work, Mozilla's Prio and the Firefox
telemetry categories, and Tor's approach of estimating users from relay-side request counts without
any per-client identifier. Per `CLAUDE.md` §1.1 the first task is to read these and say what we are
taking and what we are leaving.

**Deliberately out of scope:** feature-usage analytics of any kind, funnel or conversion tracking,
crash reporting, anything that runs per navigation rather than per day, and any attempt to attribute
an install to a post.

## Open questions the design must answer

| Question | Why it is not answered here |
|---|---|
| Opt-in or opt-out | Opt-in is the honest default and undercounts badly at our size; opt-out counts properly and asks the user to trust the notice. This is the owner's call, and it changes what the number means |
| Where the endpoint lives | Cloudflare Worker plus KV, or our own small service. Decides who holds the raw hashes and for how long |
| What "active" counts | Browser launch, or a daily heartbeat while running. These give different numbers and the published definition must match the code |
| Retention | How long the day and month sets survive before deletion, and who can read them |
| Whether the telemetry page ships before the telemetry | ⭐ Recommendation: yes. The page is the reason the feature is defensible |

## Test and negative control

⛔ Per `../../0.4.0-beta.3/HARNESS.md`: **a fix is not done until the check has been *seen* to fail.**

| | |
|---|---|
| **GREEN** | Two installs on two machines, same day, produce two distinct daily hashes and a count of 2. One install reporting twice in a day counts once |
| **RED** | Set the clock forward one day on one machine and re-run: the daily hash must change and the monthly hash must not. Then roll the month: the monthly hash must change. A hash that survives either boundary is the defect this ticket exists to prevent |
| **SUBJECT** | The bytes on the wire, captured at the network layer, not the values logged inside the process. What we ship is what a user can see with a proxy |
| **Tier** | To be set when the ticket is assigned |

**Standing invariant?** ⭐ **Yes, and it is the load-bearing part.** Propose for
`../REGRESSION_ADDITIONS.md`: *the browser sends no identifier that survives its rotation window, and
no request body carries a URL, key, balance or wallet field.* This can regress silently in a way no
user would notice and every critic would.

## Links

- `Hodos/marketing/Metrics/DOWNLOAD_METRICS.md` — the current numbers and why they cannot answer this
- `Hodos/marketing/Strategy/POSTING_STRATEGY.md` — the Q4 schedule this number exists to measure
- `../../0.4.0-beta.3/HARNESS.md` — the standard for the checks above
