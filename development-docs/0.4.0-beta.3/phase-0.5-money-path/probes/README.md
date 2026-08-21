# Phase 0.5 Task A probes — the two-phase action lifecycle

Everything here was **run**, on 2026-08-21, against the dev stack (wallet `31401`,
dev browser CDP `9322`). Results are in `PHASE_CONTRACT.md §4o`, evidence row `P0.5-X6`.

## Safety — read before re-running

⛔ Every probe that could move money uses the address
`1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW` — valid base58, valid 25-byte length, mainnet
version byte `0x00`, **checksum deliberately broken** (`mkaddr.py` derives it and
prints `BAD_CHECKSUM` to prove it). Address→script conversion happens inside
`create_action_internal`, i.e. **after** the point where the gate would have fired, so a
gate bypass dies at `Address checksum mismatch` and no transaction is ever built.

⛔ A **format**-invalid address would 400 *before* the gate and measure nothing. Do not
substitute one.

⛔ Never point these at production (`31301` / CDP `9222`).

## Files

| File | What it measures |
|---|---|
| `taskA_m1.py` | **M1** — the paired control. Identical body/headers/domain to `/createAction` (gated: `202 engine Prompt`) and `/processAction` (**no gate line at all**). |
| `probe_process.js` | **M2 subject** — `/processAction` driven from a real external https page via `window.__hodos_walletCall`. Returns `location.href` with the result. |
| `probe_create_control.js` | **M2 control** — same page, same body, via `/createAction`. Fired without awaiting the promise, because an over-cap call opens a modal that does not settle until the owner answers. The evidence is the wallet log line. |
| `taskA_m3.py` | **M3** — (a) `options.noSend:true` does not change the phase-1 decision; (b) `/listActions` disclosure; (c) `/signAction` with a reference that cannot exist → `404`, proving existence is the only check. |
| `ungateable.py` | Cross-references every route in `main.rs` against its handler signature. **107 registered, 34 take `HttpRequest`, 73 do not.** |
| `cdp.py` | Minimal CDP driver. Selects the target **by URL and refuses on ambiguity** — 12 targets all report `type:"page"` and ten are overlays on `:5137`. |
| `mkaddr.py` | Derives the safe probe address and prints its checksum verdict. |

## Running

```bash
python taskA_m1.py                              # M1, no browser needed
python cdp.py nav  teragun.com https://teragun.com/
python cdp.py evalp teragun.com probe_process.js         # subject
python cdp.py eval  teragun.com probe_create_control.js  # control (raises a modal)
python taskA_m3.py
python ungateable.py
```

⚠️ `taskA_m1.py` and `taskA_m3.py` hardcode `PRICE = 17.555` and the derived
`X-Payment-Cents`, read from `/wallet/price` on the day. **Re-read the price** — the cap
comparison is meaningless if the stamped cents no longer match the satoshis.
`sats / 100_000_000 × price` — check it twice.

⚠️ `teragun.com` is used because it is an **approved** domain with a low cap
(`per_tx_limit_cents = 13`), so an over-cap call is unmistakable. A payment-gate probe
against an **unapproved** domain measures nothing: `domain_trust_mw` answers first with
`domain_approval` and the row goes green with the feature disabled.

⚠️ `/listActions` with `labels: []` returns `{"totalActions":0}`. That is the filter, not
a gate. Drop the filter before concluding anything.
