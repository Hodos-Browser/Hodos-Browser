# TICKET — connect prompts that arrive AFTER the user already approved are orphaned, and the page hangs

**Found** 2026-09-21 by the **owner**, at the keyboard, on `zanaadu.com` (dev build).
**Status:** ⬜ OPEN — mechanism read from the logs; **no code changed**. **Severity:** HIGH for the
user — the site appears frozen ("just spinning") with nothing on screen to click.

## What the owner saw

Revoked Xanadu, came back, approved the connect. Then a "like" did nothing, and the next "like" just
**spun** — no prompt, no error.

## The timeline (`debug_output-*.log`, 2026-09-21)

```
12:24:47.686  connect prompts opened (req -12, -13, -14)   reason=new_domain_with_manifest
12:24:49.518  three MORE calls forwarded to the wallet      <- sent while the user was still deciding
12:24:49.631  approval written (sync, status 200); "Drained 3 pending request(s)"
12:24:49.708  202 connect prompt  req -15   <- the answers to the 12:24:49.518 calls, arriving
12:24:49.740  202 connect prompt  req -16      AFTER the drain had already run
12:24:49.758  202 connect prompt  req -17
12:24:49.777  "Auth response - Approved: 1" (the user's click)
12:24:59.244  protocol_permission_prompt req -18  (the level-0 "xanaverse" prompt for the like)
12:25:34.524  "Wallet HTTP timeout ignored — request is parked on an approval prompt" (-16, -17)
```

## Mechanism

Calls that are **in flight to the wallet during the approval** — sent before the approval is written,
answered after — come back as **connect** prompts for a domain that is now approved. The drain has
already run, so nothing ever resolves them. They are **orphans**: parked on a connect prompt the user
has already answered, and never shown again.

⚠️ The level-0 prompt for the next "like" (`-18`) then **queues behind the orphans** in the single
keep-alive notification overlay, so it never appears either. ⇒ *"I did a like, it didn't happen."*

## ⚠️ How today's W7 fix changed the symptom — it did not cause the race

The race is older than W7. Before W7 an orphan was answered `Wallet request timeout` at **45 s**, so the
page got an error and could recover. Since W7 (`4b5e750`, correct for a *visible* prompt) the 45 s net
**stands down while a request is parked on a prompt**, so an orphan now waits the full **10-minute**
prompt timeout. ⇒ a 45-second error became a 10-minute freeze. W7 remains right; the orphans are the
defect.

## Candidate fix — for the owner to choose, not implemented

A connect-type 202 arriving for a domain that is **already approved** (the prompt reason is stale by
the time it lands) should be **re-issued**, not opened as a modal — the same thing the drain would have
done had the call arrived a few milliseconds earlier. The narrower alternative is to widen the drain to
catch late arrivals, which is a timing window rather than a fix.

## Evidence rows

- `CO-1` 🔴 before: approve a manifest connect while the page is still firing calls; ≥1 late connect
  prompt is logged after the drain and the page stalls.
- `CO-2` ✅ after: the same sequence logs **no** connect prompt after the approval, every call returns,
  and the page's next action works.
- `CO-3` negative control: a **genuinely new** domain still gets its connect prompt — the fix must not
  silence first contact.

## Also seen, not chased

At `12:24:47.517` the C++ layer logged `trust_level: approved` for `zanaadu.com` while the wallet treated
it as a new domain, after the owner had deleted its permissions. The wallet is authoritative and handled
it correctly, so it caused no harm here — but it looks like the known two-store drift
(`TICKET_site_permission_dual_store.md`).
