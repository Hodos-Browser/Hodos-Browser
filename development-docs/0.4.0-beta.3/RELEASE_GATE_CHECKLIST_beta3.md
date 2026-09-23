# The beta.3 release gate — the owner's sitting, in order

**Written 2026-09-23**, for `v0.4.0-beta.3`. This is **Phase B** of
`DevOps-CICD/BUILD_AND_RELEASE.md` Steps 5–7: the deliberate *test-before-customers* gate
between a verified DRAFT and a public release.

> ⛔ **Nothing here can be done before the draft exists.** A `v*` tag build stops at a draft and
> publishes nothing. ⛔ **And nothing here can be done by an agent** — every row needs a real
> install on real hardware, which is why they were batched into `INSTALL_TEST_BATCH.md` in the
> first place.

---

## ⭐ The one fact that changes what you are testing

📏 `v0.4.0-beta.1` and `v0.4.0-beta.2` were **built and never published**. The public Latest is
still **`v0.3.0-beta.29`** (2026-07-20).

⇒ **beta.3 is the first 0.4.0 anyone outside this team will ever install**, and the update path
that matters is **`0.3.0-beta.29` → `0.4.0-beta.3`**. ⛔ Do **not** test from a beta.2 install —
nobody in the wild has one.

📏 Build numbers are monotonic across that jump (beta.29 = `30029`, beta.3 = `40003`), so the
Windows anti-rollback floor passes.

---

## Order of operations — why this order

The install rows come **first** and the AV seeding **second**, because seeding uses the same
downloaded installer and `promote.yml` now refuses to publish without the evidence.

### 0. Before you install

- [ ] Download from the **draft release** page: the installer, the portable zip, and
      `SHA256SUMS.txt`. ⭐ Keep `SHA256SUMS.txt` — you paste it into `promote.yml` to pin the
      exact bytes you tested.
- [ ] ⭐ **Do NOT uninstall your current build yet.** `I2` and `R-UPDATE` both need the
      *existing* install in place.
- [ ] Check the signing chain (`BUILD_AND_RELEASE.md` Step 8 pre-step). If it reads
      `Microsoft ID Verified CS EOC CA 03`, say so in the Defender submission.

### 1. 🔴 R-UPDATE — the row that cannot be re-tested after shipping

This is the one that has read *"real N−1→N owed at RC"* at **every** boundary since 2026-08-18.
This is the RC.

- [ ] On a machine running **`0.3.0-beta.29`**, let it check for updates (or Settings → About →
      Check for updates).
- [ ] It offers **0.4.0-beta.3**, downloads, verifies and applies.
- [ ] ⛔ **The negative half:** a **corrupted** staged installer must roll back, not brick.
- [ ] 🍎 The macOS half is Mac's, from a `0.3.0-beta.29` install — **not** their beta.2 profile.

### 2. The install batch — `INSTALL_TEST_BATCH.md` I1–I8

All eight need the real install. Grouped so one install covers them:

- [ ] **I1** — production single-profile taskbar reads **"Hodos Browser"**, not
      `HodosBrowser.exe`, and groups with the pinned icon.
- [ ] **I2** — an **existing pinned shortcut** survives the upgrade (or fails the way Q4/Option A
      says it will — record which).
- [ ] **I3** — covered by §1 above.
- [ ] **I4** — nothing new is created inside `{app}` after a run. ⭐ Diff the installed tree
      before and after. A stray file inside `{app}` broke the silent-update backup hash once.
- [ ] **I5** — the stray-log ticket's T2/T3 rows. Same install as I4.
- [ ] **I6** — ✅ **already closed** (macOS fixed `minimumSystemVersion` on 2026-09-19, and CI now
      derives it from the *measured* `minos` and fails closed).
- [ ] **I7** — uninstall removes the per-profile Start Menu sweep and the WinSparkle keys.
      ⚠️ Do this **last** — it destroys the install the other rows need.
- [ ] **I8** — 🚦 **the release-shaped security row.** On the **installed** build:
      `Get-NetTCPConnection -LocalPort 9222 -State Listen` finds **nothing**, and the log says
      `Remote debugging port: 0`. 🚨 This matters more than usual: the installed
      **`0.4.0-beta.2`** was measured binding CDP **9222** with 81 live targets. That build was
      never public, but this row is what proves beta.3 is not the same.

### 3. Smoke it — `CLAUDE.md` Testing Standards, "Standard" tier

- [ ] Auth: `x.com`, `google.com`, `github.com`
- [ ] Video: `youtube.com` + one more
- [ ] News: one
- [ ] The wallet: create/unlock, a real send, the **gold pill** on the originating tab.

### 4. AV seeding — ⛔ `promote.yml` will not publish without this

| Service | What | You need afterwards |
|---|---|---|
| **VirusTotal** | upload the raw installer `.exe` (not zipped) | the **report URL** — its sha256 is checked against the installer being promoted, so last release's URL is rejected |
| **MS Defender** (WDSI) | submit the raw installer; "Software developer" / "Incorrectly detected" | the **submission ID** (UUID, arrives by email) |
| Norton | only if it flags us in the wild | — |

⚠️ The Defender half is **attestation only** — the gate format-checks it and cannot verify it.
The VirusTotal half is hash-checked and therefore real.

### 5. Then, and only then

- [ ] ⭐ **Rehearse first:** run `promote.yml` with **`dry_run: true`**. It runs every check
      against the real draft bytes and stops before the flip — nothing published, website
      untouched.
- [ ] Then the real run, with: the tag, the `SHA256SUMS.txt` you tested, the VirusTotal URL, the
      detection count, the Defender submission ID, and the `FARBLING-ROTATION-v1` token.
- [ ] ⚠️ A green rehearsal proves **the gates work**, not that the promote path works end to end
      — the flip, the website push and the served-signature verify are all *skipped* on a dry run.

---

## What blocks promotion, and who owns it

| | owner | state |
|---|---|---|
| `C1` Sparkle accepts the real archive, rejects a tampered one | 🍎 macOS | ✅ **GREEN 2026-09-23 (relay 23f)** — on Sparkle **2.9.3** (the field client: beta.29/beta.2 ship it) **and** 2.9.6; DMG pinned `4512c9af…`; positive `OK: EdDSA signature is correct`; negatives = 1-bit-flipped signature **and** a valid swapped DMG, both `EdDSA signature does not match`. ⚠️ a raw byte-flip is rejected by DMG corruption, not the signature — see 23f §3 |
| `C1b` the feed item carries `sparkle:channel` | 🍎 macOS | 🚦 **DOWNGRADED from blocker — not a defect.** ⛔ An earlier note here said *"nothing implements `allowedChannelsForUpdater:`"* as though it never had; 👤 the owner corrected that. The client subscribed **2026-03-30 → 2026-06-24** and the feed labelled items for 7 weeks of it. Both halves were then removed in sequence, each correctly given the other's state, leaving a **coherent** system: no labels, no subscription, every user gets every update (📏 confirmed against the **live** feed). ⭐ And subscribing is **additive**, never a filter — Sparkle always includes no-channel items. 👤 **Decision 2026-09-23: ship beta.3 UNLABELLED**, because `beta.29` was built *after* the subscription came out, so the machines in the field cannot see a labelled item. Re-adding the subscription is free and lands whenever Mac wants it. See relay round 23b |
| `C2` `minimumSystemVersion` in the feed | 🍎 macOS | ✅ closed |
| AV seeding evidence | 👤 owner | §4 above |
| Farbling rotation token | 🪟 Windows | agent-run on the build host |
