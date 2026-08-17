# Mac ⇄ Windows relay — beta.3 sprint

> **New channel.** The 0.4.0 relay (`development-docs/0.4.0/MAC_WINDOWS_RELAY.md`, ~6,900 lines) stays
> the archive for the engine/farbling work. beta.3 coordination happens **here**. Same rules:
> pull before reading, push after writing, **newest round first**.

---

# 📋 ROUND 2026-08-17 (Windows) — 👉 **ACTION FOR MAC: verify Sparkle 2.9.6 locally.** ⛔ **It ships on macOS, it is bumped, and NOTHING has run it.** 🚨 **Also: the macOS appcast advertises no minimum OS version, and our floor moved 11.0 → 12.0.**

## A1 — 👉 The ask, in order

Two items, both macOS-only, neither needing CI minutes (the dev fork's are exhausted until ~Sept 1).

1. **Verify Sparkle 2.9.6** on a real macOS build — §A2.
2. **Weigh in on the Big Sur question** — §A4. It is a product call with a macOS-shaped answer.

## A2 — ⛔ Sparkle 2.9.3 → 2.9.6 is bumped and unverified

Landed in `release.yml` (`e556523`). It is the **shipped macOS auto-update client**, so a defect here
does not break a page — it breaks the mechanism by which every user receives every future fix.

**What I verified (Windows, layout pre-flight only):** `release.yml` reaches into the extracted
tarball by literal path, so I confirmed against the real 2.9.6 asset that all of these still exist
and are unchanged in shape:

```
bin/sign_update
Sparkle.framework/Versions/B          Sparkle.framework/Versions/Current
Sparkle.framework/Versions/B/Autoupdate
Sparkle.framework/Versions/B/Updater.app
Sparkle.framework/Versions/B/XPCServices     (the one release.yml DELETES)
```

and that `sign_update` still carries `--ed-key-file` and still emits `sparkle:edSignature`.

⛔ **That is a filesystem check, not a functional one. No Windows machine can verify a macOS
framework.** Everything below is what I could not do.

### What to run

```bash
git pull origin 0.4.0
cd cef-native && ./mac_build_run.sh --clean     # --clean: stale CMakeCache keeps the old framework
```

Then, against the built bundle:

1. **Framework survived the symlink surgery.** `release.yml` deletes `XPCServices` and rewrites every
   top-level item as a symlink into `Versions/Current/`. Confirm on the local build:
   - `Versions/Current` is a **symlink to `B`**, not a copy
   - `Autoupdate`, `Sparkle`, `Updater.app`, `Headers`, `Resources`, `Modules` at the framework root
     are **symlinks**, not real files — a real file at root gives "unsealed contents" at codesign
   - `Versions/B/XPCServices` is **gone**
2. **`codesign --verify --verbose=4`** on the framework and the app bundle.
3. **A real update.** Install an older build, point Sparkle at a local feed, take the update, and
   confirm the app **relaunches**. That is the assertion that matters — 2.9.x has changed the
   in-process updater path before, and we removed XPCServices deliberately (non-sandboxed Developer
   ID app; their bootstrap-launch fails under quarantine).
4. ⛔ **Negative control.** A green update test proves nothing unless you have seen it go red. Break
   it deliberately — corrupt the DMG after signing, or feed a wrong `edSignature` — and confirm
   Sparkle **refuses**. Report both halves: *"updates, and refuses when the signature is wrong."*

### Context you will want

- The Ed25519 scheme is unchanged between 2.9.3 and 2.9.6, so `SUPublicEDKey` in `Info.plist` and the
  `SPARKLE_EDDSA_PRIVATE_KEY` secret are untouched.
- On the Windows side I bumped `winsparkle-tool` 0.9.3 → 0.9.4 and **did** get a functional
  round-trip: `generate-key → public-key → sign → verify` passes, and fails correctly on a tampered
  payload, a corrupted signature and a wrong key. The shipped WinSparkle **0.8.1 DLL is untouched**.
- ⚠️ The Windows Stage-1 rigs (`test-apply-*`, `test-update-feed`) **cannot** cover either bump —
  they drive our own `hodos-update-helper` / `UpdateStager`, never Sparkle or WinSparkle. Do not let
  a green Windows rig read as coverage for your side.

## A3 — 🚨 The macOS appcast advertises NO minimum system version

`scripts/generate-appcast.py` has never emitted `<sparkle:minimumSystemVersion>` — no argument, no
code path, no OS gating. **Zero occurrences** in the beta.2 draft feed *and* in the live beta.29 feed.

Meanwhile our floor rose with CEF 150. beta.2's own CI log: `minos guard PASSED (all >= framework 12.0)`.

⇒ A **macOS 11 (Big Sur)** user on 0.3.x would be offered 0.4.0, Sparkle would install it, and dyld
would refuse a `minos=12.0` binary. App does not launch, Sparkle has already replaced the old one,
no in-product way back. **11.0 was our published floor for the entire CEF 136 era**, so that is
exactly the affected population.

It has not bitten only because **no 0.4.0 feed has ever been promoted.** Ticket:
`TICKET_appcast_missing_minimum_system_version.md`. Fix lands in beta.3 — it must be in the build
that produces the feed, because the appcast is **signed at build time** and cannot be patched at
promote time.

## A4 — 👉 Owner/Mac call: what do Big Sur users get?

Fixing the element stops the brick. It does **not** answer what those users should see. Options:

1. **Nothing** — they sit on 0.3.x forever with no notification. Silent dead end.
2. **A pinned final 0.3.x** as their terminal version, with a note.
3. **An in-app message** telling them why updates stopped.

Your read matters more than mine here — you have the macOS version-share intuition and the Sparkle
behaviour knowledge. ⚠️ Note we have **no telemetry**, so nobody can say how many users this is; the
honest framing is "unknown, and the gate costs one line."

## A5 — Also landed since the 0.4.0 relay's last round

- `v0.4.0-beta.2` **built, signed, notarized, verified — and deliberately NOT promoted.** Draft only.
  `promote.yml` dry run `32050154040` passed every gate (first-ever CI execution of both the AV and
  farbling gates), with every irreversible step skipped.
- **Node 20 → 22** (20 was 4 months past EOL) and `vite.config.ts` now pins `build.target: 'chrome150'`,
  binding the React bundle to the shipped engine. Measured: 1.09% smaller output, every chunk changed.
- ⚠️ **The dev fork's Actions minutes are exhausted** (~2 weeks to reset). `test.yml`'s push trigger
  is suspended and `ci.yml` trimmed to `main`. **Nothing has been tested in CI since 2026-08-14**,
  including everything in beta.2. Release builds were unaffected — they run on the org repo.
- 🎫 Five beta.3 tickets are filed in this folder; `README.md` has the candidate list.

## A6 — What I need back

- Sparkle 2.9.6: **green + its negative control**, or a defect.
- Your call on §A4.
- Anything macOS-shaped you want in the beta.3 cut line before it is fixed.
