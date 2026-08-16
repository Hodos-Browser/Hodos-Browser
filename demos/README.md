# Demos — runnable example sites

Localhost example sites used to exercise Hodos features, test them end to end, and record the
marketing demo videos. **Code only. No video files ever live in this repo** — see "Where the videos
live" below.

Was `development-docs/Sigma-BRC121-Sprint/phase-4-demos/`, split out 2026-08-15. That folder also
carried open question **Q3** — "demo repo subdir vs separate `hodos-demos/` repo." Answered in
practice: they live here.

## What's here

| Folder | Runs | Exercises |
|---|---|---|
| `brc121-402/` | `npm install && npm start` | BRC-121: 402 → BRC-29 payment → retry. Express server, verifies BEEF output 0. |
| `qr-codes/` | any static server, e.g. `npx serve demos/qr-codes` | QR parsing: BSV address, paymail, handle, identity key, BIP21 with/without amount, and negative cases (segwit, random text, plain URL). |

`qr-codes/` was at `frontend/public/qr-test.html`, `qr-test-bip21.html` and `qr-images/`. It was
being **bundled into the shipped browser** — test pages served to real users. Moved here 2026-08-15
and image paths made relative so the folder is self-contained. Nothing in the app referenced them;
`development-docs/QR_SCAN_OVERVIEW.md` is the only other mention.

## Still to build

Each of these backs one demo video. Build the example site first, verify the flow end to end, then
record.

| Demo | Backs video | Exercises | Blocked on |
|---|---|---|---|
| `brc100-connect-auth/` | Account creation + authentication | `getPublicKey({identityKey:true})`, `domain_approval` modal with identity-key checkbox, dApp receives identity key | — |
| `brc100-permissions/` | Auto-approve engine | scoped prompts per (protocol, counterparty), "always allow for this site", silent thereafter, per-site budget | — |
| `ordinals/` | 1Sat Ordinals | ordinal display, transfer, verified-vs-claimed provenance | `development-docs/1SatOrdinals-BSV21/` sprint |
| `brc29-peerpay/` | (regression only) | PeerPay send/receive | — |

**Existing sites we can point at instead of building:** `teragun.com` (createAction payments),
`now.bsvblockchain.tech` (BRC-121 paid content), `socialcert.net` (X verification in the connect
flow), `1sat.market`. Prefer a real site where one exists — it demos better than a toy.

## What each demo has to prove

Written down because a demo that doesn't fail when the feature is broken isn't a test.

- **Connect / auth** — the identity key reaches the dApp only after explicit approval, and the
  privacy perimeter still forces a prompt on identity-key reveal regardless of any other setting.
- **Auto-approve** — first call of each `(protocol, counterparty)` tuple prompts; subsequent calls
  are silent; the tab badge (green dot) fires on every auto-approved payment. That badge is the
  user's only defense against a site quietly spending under caps — if it stops firing, the demo
  must fail visibly.
- **Payments** — under-cap silent, over-cap prompts with amount / perTx / sessionSpent shown.
- **Ordinals** — a 1-sat output caught by ordinary coin selection is a permanently destroyed asset.
  The demo must show it being refused, not just show a transfer working.

## Also planned: LLM-ready integration guides

One `.md` per demo, copy-pasteable into a developer's Claude / Cursor / Replit session, with code
blocks, error handling, expected user flow, and Hodos as the test wallet. Audience is external BSV
devs. Not started.

## Where the videos live

Recording, editing, publishing and the video slate are marketing, and live outside this repo:

- **`Marston Enterprises/Hodos/Marketing/Videos/`** — source footage, edits, finished videos, and
  `README.md` with the slate, status, and the notify-on-publish list.

`Marketing/Videos/` is gitignored there and must stay that way. Nothing in `Hodos-Browser` should
ever contain a `.mp4`, `.mov`, `.fbr`, or Premiere scratch file.

## Related

- `development-docs/1SatOrdinals-BSV21/` — gates the ordinals demo
- `development-docs/QR_SCAN_OVERVIEW.md` — what the QR pages are testing against
- `archived-docs/Sigma-BRC121-Sprint/` — where these demos were originally specified, including the
  four-pillar comprehensive-flow narrative
