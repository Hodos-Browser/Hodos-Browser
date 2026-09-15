# Dependency Verification — Procedure (run on every CEF bump)

**Created:** 2026-06-16 · **Owner:** DevOps/CI-CD · **Canonical home:** `development-docs/DevOps-CICD/`
**Per root CLAUDE.md Invariant #12** — keep this current; append lessons learned each time.

> **Why this exists.** The hard part of a CEF/Chromium bump is **not** Chromium's *internal* dependencies (`automate-git.py`/gclient resolve those for the pinned branch automatically). It's **Hodos's *own* dependencies** staying compatible with the new CEF's C++ ABI, toolchain, and headers. This procedure makes that a repeatable, auditable checklist instead of tribal knowledge — runnable by a small team or a small team of AI agents, with verification at each step.

## When to run
- Every **milestone jump** (new CEF LTS branch, e.g. M150 → M156) — full pass.
- Every **quarterly security point-release** within the pinned LTS — lighter pass (most deps unchanged; confirm nothing shifted).
- See `CEF_BUILD_RUNBOOK.md` for the surrounding build flow and the LTS cadence rationale.

### ⭐ The policy — pin exactly, review on a cadence, bump deliberately (beta.3 Phase 9, 2026-09-14)

`TICKET_dependency_freshness_review.md` named the gap: the pins were a **freeze at the moment we took
control** (every DEP-1 comment says *"records current behaviour rather than changing it"*), and a freeze
with no scheduled thaw is how a known-vulnerable OpenSSL ships without anyone deciding to. So:

1. **Keep the freeze.** Nothing floats. A pin and a bump never share a commit.
2. **Review at every engine bump and at least quarterly** — the bump is already a whole-stack
   revalidation, so it is the natural checkpoint (`CEF_VERSION_UPDATE_TRACKER.md`, *Process for CEF
   Version Updates*, step 3 names this file). Quarterly means a calendar entry, not "when someone
   remembers": the next one is due **2026-12-14** or the next engine bump, whichever is first.
3. **A review is a table, one row per pinned dependency**, appended below under *Freshness review —
   \<date\>*: current pin · latest stable **on the branch we track** (not the newest major — the OpenSSL
   3.6-vs-4.0 lesson below) · whether any advisory affects the pinned version *as we use it* · the
   decision, **hold** or **bump**, with the reason. For runtimes read the EOL schedule, not the version.
4. **Run the advisory tools, do not just read release pages:** `cargo audit` + `cargo outdated
   --root-deps-only` in both Rust workspaces, `npm audit` in `frontend/`, and
   `scripts/libcef_export_coexistence.ps1` (this folder) for the libcef symbol surface. `test.yml`
   runs the first two but they are `continue-on-error` + `|| true`, and the dev fork's Actions quota has
   been exhausted since 2026-08-14 — a review on this box is the only run that has ever been read.
5. **Prefer newest-stable-that-builds over minimum-required.** Nobody consumes Hodos as a library;
   staying near current shrinks the eventual jump.
6. ⛔ **A review reports. A bump is a separate, owner-approved change** with the build re-run and the
   relevant rigs (`SILENT_UPDATE_TEST_PLAN.md` Stage 1 for the updater libraries) re-executed. A
   money-handling binary is never upgraded as a side effect of a review.
7. **macOS float — accepted in writing (owner, 2026-09-14).** `Brewfile` cannot pin a formula version, so
   the macOS build's OpenSSL / sqlite3 / nlohmann-json are whatever Homebrew ships on build day. For
   0.4.0 this is **accepted**, and the release notes' reproducibility section must say so. The
   escalation the Brewfile already names — a `brew extract` into a Hodos tap — is Mac's call: relayed
   in `0.4.0-beta.3/MAC_RELAY_BETA3.md` (round 2026-09-14b) for the Mac side to recommend for or against.

## The dependency inventory (Hodos-owned)
| Layer | Dependency | Where pinned |
|-------|-----------|--------------|
| CEF binding | `libcef_dll_wrapper` (must match `libcef` version exactly) | CEF binary distrib + `cef-native/CMakeLists.txt` |
| C++ libs (vcpkg) | nlohmann-json, sqlite3, OpenSSL, quirc, + others | `vcpkg.json` / vcpkg baseline |
| Toolchain | MSVC / Windows SDK (Win); Xcode/clang + min macOS (Mac); C++ std version | build env + CMake |
| Frontend | React, react-dom, react-router, Vite, TypeScript, MUI/Emotion | `frontend/package.json` + lockfile |
| Rust | wallet + adblock crates | `Cargo.toml` + `Cargo.lock` |

## Per-dependency checklist — answer IN WRITING for each
For **every** dependency above, record:
1. **What is it + current version?** (and the new/target version if changing.)
2. **Is it compatible with the new CEF/Chromium ABI + toolchain?** (compiler, CRT, Windows SDK, C++ std, min-macOS all match what the new CEF was built against?)
3. **Is this the right version — and *why* this one?** (pinned for a reason? a transitive constraint? matches what CEF expects?)
4. **What else does bumping it affect?** (ripple to other deps, changed APIs, behavior changes, removed/renamed symbols.)
5. **Any conflict?** (two deps wanting different versions of a shared lib; ABI mismatch; duplicate symbols.)
6. **Verification performed** (compiles? links? unit/integration tests pass? smoke test?) — record the result.
7. **Decision + record** so the next bump starts from a known baseline, not from scratch.

## Output
- A short table appended to `CEF_VERSION_UPDATE_TRACKER.md` (the living log): each dep, old→new version, verdict, notes.
- Any surprise/breakage → **document the lesson here** and update the runbook (Invariant #12).

---

## DEP-1a..d — the silent-drift pins (landed 2026-08-03, pre-`7871`-build)

Before this pass, **four of the five inventory layers floated.** The checklist above asks "is this
the right version and why" — but there was no pinned answer to compare against, so a re-run could
silently get different versions with no diff anywhere in the repo. These pins give each layer a
declared version so drift becomes a reviewable change.

**Principle: every pin below records what the floating command already resolved to on 2026-08-03.**
None of them is an upgrade. That is deliberate — a pin and a bump must never land in the same
commit, or a build break is ambiguous between the two.

| ID | Layer | Was | Now | Where |
|---|---|---|---|---|
| **DEP-1a** | C++ / vcpkg | classic mode, whatever the runner image shipped | manifest mode, `builtin-baseline` + exact `overrides` (nlohmann-json 3.12.0#2, sqlite3 3.53.4, openssl 3.6.3) | `cef-native/vcpkg.json` |
| **DEP-1b** | Installer | `choco install innosetup` (floating) | `--version=6.7.1` | `release.yml` |
| **DEP-1c** | macOS libs | `brew install openssl nlohmann-json sqlite3` (floating) | `brew bundle --file=Brewfile` | `/Brewfile` |
| **DEP-1d** | Rust toolchain | `dtolnay/rust-toolchain@stable` ×4 | `channel = "1.97.1"` | `rust-wallet/`, `adblock-engine/rust-toolchain.toml` |
| *(extra)* | Rust crates | `actix-web = "4.9"` caret, held only by `Cargo.lock` | `= "=4.11.0"` | `rust-wallet/Cargo.toml` |
| *(extra)* | CI runners | commented-out `*-latest` in disabled `test.yml` stubs | pinned images | `test.yml` |

### Verification performed
- **Rust:** `cargo build --release` green on 1.97.1 for **both** workspaces (wallet 3m00s). **Both
  `Cargo.lock` files unchanged** — proving the `actix-web` exact pin records the existing resolution
  rather than moving it.
- **vcpkg / Inno / Brew: NOT verifiable locally.** These only execute in the release workflow. Their
  first real exercise is the next release build — treat a failure there as *this* change, not as a
  CEF-bump symptom.
  - ✅ **That exercise happened: `v0.4.0-beta.2`, 2026-08-17.** All three passed — vcpkg resolved the
    manifest baseline, Inno 6.7.1 built the installer, and `brew bundle` provisioned the macOS
    dependencies. Both platform builds went green. *(The run's `publish` job failed later, at the
    draft-release lookup, for an unrelated GitHub-incident reason — see BUILD_AND_RELEASE.md
    "Known CI flake". Nothing dependency-related.)*
  - It was also exercised once more, earlier the same day and with no release attached, by the
    `workflow_dispatch` validation run `31948482218` — which is what that trigger is for.

### Freshness review — 2026-08-17 (the first one)

Queried upstream directly rather than reasoning from the pin dates. **The posture is better than
"we froze and forgot" suggested — most pins are current.** Two real items.

| Dependency | Pinned | Latest upstream | Verdict |
|---|---|---|---|
| **OpenSSL** | `3.6.3` | `4.0.1` | ✅ **CURRENT.** `3.6.3` shipped **2026-06-09, the same day as 4.0.1** — 3.6 is an actively maintained branch getting simultaneous security releases, and we are on its newest patch. "A major behind" is the wrong reading. |
| **SQLite** | `3.53.4` | `3.53.4` | ✅ current |
| **nlohmann-json** | `3.12.0` | `3.12.0` | ✅ current |
| **Rust** | `1.97.1` | `1.97.1` | ✅ current |
| **Node** | ~~`20`~~ → **`22`** | `26.7.0` | ✅ **FIXED 2026-08-17.** Was EOL since 2026-04-30; bumped to 22 (maintained to 2027-04-30). |
| **Sparkle** (macOS updater) | ~~`2.9.3`~~ → **`2.9.6`** | `2.9.6` | ✅ bumped 2026-08-17 — ⛔ **unverified on a real macOS build** |
| **WinSparkle** | tool ~~`0.9.3`~~ → **`0.9.4`**; shipped dll `0.8.1` unchanged | `0.9.4` | ✅ bumped + round-trip verified 2026-08-17 |
| **Inno Setup** | `6.7.1` | `7.1.0` | ⏸️ **deliberate hold** — Chocolatey lags upstream and 7.x is a major compiler change; do not bump without re-validating `hodos-browser.iss` |

#### 🚨 Node 20 is end-of-life

Node 20 reached EOL on **2026-04-30** (`nodejs/Release` schedule: maintenance from 2024-10-22, end
2026-04-30). It has received no security patches since. `release.yml` pins `node-version: '20'` on
both build arms.

⚠️ **Scope it accurately before panicking:** Node is **build-time only**. `npm run build` is
`tsc -b && vite build`, which emits static assets; no Node runtime ships in the installer (nothing
node-shaped appears in `hodos-browser.iss`). So the exposure is **build-toolchain integrity**, not a
vulnerability in the shipped browser. That is a real supply-chain concern for software that handles
money — an EOL toolchain builds the UI that renders wallet state — but it is not a user-facing CVE.

✅ **DONE 2026-08-17 — bumped to Node 22** (maintained to 2027-04-30) on both build arms. Not 24 or
26: 22 is the LTS-track option with the longest runway that is not still moving.

⚠️ **Not yet exercised in CI** — the dev fork's Actions quota is exhausted and `release.yml` only
runs Node on a tag build or a `workflow_dispatch` validation run. The frontend build was verified
**locally** (see below). Treat the first CI frontend build after this as the real confirmation.

#### Sparkle / WinSparkle — bumped 2026-08-17, and what that is and is NOT backed by

⛔ **The Stage-1 update rigs cannot test either of these. Do not read a green rig as covering them.**
Checked before running rather than after:

| Rig | What it actually drives | Covers the bump? |
|---|---|---|
| `test-apply-forward.ps1` / `test-apply-rollback.ps1` | our **custom `hodos-update-helper.exe`** transaction | ❌ never touches WinSparkle |
| `test-update-feed.ps1` | our **custom `UpdateStager`**, with throwaway test-seam keys | ❌ never invokes `winsparkle-tool` |
| any Windows rig | — | ❌ **Sparkle is macOS-only** |

They were also **not runnable** at the time: `hodos_tests.exe` does not exist (needs
`-DHODOS_BUILD_TESTS=ON`), no rig build of the helper exists (needs `-DHODOS_UPDATE_TEST_SEAM=ON`),
and ports **31301/31302/31401 were all listening** — the rigs abort on that by design, because
rollback **POSTs `/shutdown`** and would take down a live wallet.

**What the WinSparkle tool bump IS backed by** — a real functional round-trip against 0.9.4, using
release.yml's exact call shapes:

```
generate-key -f      -> 44-byte key  (32-byte-seed format; SPARKLE_EDDSA_PRIVATE_KEY stays compatible)
public-key   -f      -> derives pubkey   (the call release.yml self-checks against SUPublicEDKey)
sign <file>  -f      -> signature
verify               -> "Valid signature."
```

with negative controls, all three of which correctly **FAILED**: tampered payload, corrupted
signature, wrong public key. Plus the archive-shape check that matters most — release.yml's EdDSA
step **soft-skips** on a missing tool, so a wrong path would silently ship a **DSA-only feed** rather
than fail the build.

**What the Sparkle bump IS backed by** — a layout pre-flight only: `bin/sign_update`,
`Sparkle.framework/Versions/{B,Current}` and `Versions/B/{Autoupdate,Updater.app,XPCServices}` all
present and unchanged in 2.9.6, and `sign_update` still carries `--ed-key-file` and still emits
`sparkle:edSignature`. ⛔ **It has not run on macOS.** The first macOS tag build is the real
confirmation — watch the "EdDSA sign DMG" step, and do a real N−1 → N update on a Mac before
promoting.

#### ⚠️ Why these two matter more per-patch than their small deltas suggest

These are the **auto-update** libraries. A defect there does not break a page — it breaks the
mechanism by which every user receives every future fix, and this project's standing principle is
that auto-update must never brick an install. Being 3 patches behind on Sparkle is a bigger deal than
being a minor behind on a JSON header. Check each release note before bumping, and re-run
`SILENT_UPDATE_TEST_PLAN.md`'s Stage 1 rigs afterwards.

#### The one place a version relationship with the engine really exists — ✅ CLOSED 2026-08-17

The React bundle runs **inside** the shipped Chromium's V8, so its output must be syntax that engine
supports. Nothing tied the Vite build target to the CEF version; it was safe only because Chromium
150 is far newer than anything Vite targets by default — safe **by accident, not by construction**.

Now bound in `frontend/vite.config.ts`:

```ts
build: { target: 'chrome150', ... }
```

⛔ **`build.target` is the load-bearing setting — NOT the `browserslist` field.** Verified before
adding it: this project has **no** postcss, autoprefixer, lightningcss, babel or `plugin-legacy`, so
**nothing currently reads `browserslist`**. It is added to `package.json` as declaration-of-intent
(and so any future tool inherits the right target), but on its own it would have been decorative —
the exact "a doc example is not evidence the code does it" trap this file warns about elsewhere.

**Measured, not assumed** — full frontend build, same tree, target off vs on:

| | total JS emitted |
|---|---|
| Vite default target | 1,052,119 bytes |
| `target: 'chrome150'` | **1,040,689 bytes** |

11,430 bytes smaller (1.09%), **every chunk shrank and every content hash changed** — esbuild stopped
down-levelling syntax Chromium 150 supports natively. The setting is demonstrably live.

⛔ **Bump `target` in lockstep with the CEF pin.** It now also catches the reverse case: a dependency
emitting syntax *newer* than our engine fails at build time instead of surfacing as a blank overlay.

#### Method — repeat this at every engine bump and quarterly

```bash
gh api repos/openssl/openssl/releases/latest      --jq .tag_name
gh api repos/nlohmann/json/releases/latest        --jq .tag_name
gh api repos/rust-lang/rust/releases/latest       --jq .tag_name
gh api repos/sqlite/sqlite/tags                   --jq '.[0].name'
gh api repos/sparkle-project/Sparkle/releases/latest --jq .tag_name
gh api repos/vslavik/winsparkle/releases/latest   --jq .tag_name
gh api repos/jrsoftware/issrc/releases/latest     --jq .tag_name
curl -sS https://raw.githubusercontent.com/nodejs/Release/main/schedule.json   # EOL, not "latest"
```

⛔ **For runtimes, read the EOL schedule, not the latest version number.** Node 20 looked fine by
"is it still widely used?" and was four months past end-of-life. `latest` tells you how far behind
you are; **EOL tells you whether you are getting security fixes at all.**

⛔ **And read the branch, not just the number.** OpenSSL `3.6.3` vs `4.0.1` looks alarming and is
fine. A raw latest-vs-pinned diff will generate false alarms on any project with parallel maintained
branches.


### Freshness review — 2026-09-14 (the second; beta.3 Phase 9, `P9-D1`)

Run on the Windows build host against the pins as of `43e4b90`. ⛔ **Report only — nothing was bumped.**
Every "bump" below is a recommendation for the owner; the rule (policy item 6) is that a money-handling
binary is never upgraded as a side effect of a review. Tools: `cargo audit` 0.22.2, `cargo outdated`
0.19.0 (both installed on this box 2026-09-14 — they were not before, and CI's copies have not run since
2026-08-14), `npm audit` (npm on Node v23.10.0), `scripts/libcef_export_coexistence.ps1`.

#### Pinned C++ / toolchain / installer / updater set

| Dependency | Pinned | Latest on the branch we track | Advisory affecting the pin *as we use it*? | Decision |
|---|---|---|---|---|
| **OpenSSL** (vcpkg) | `3.6.3` | **`3.6.4`** (2026-08-25, security patch release — CVE-2026-18798 QUIC, -63072 CMS, -63076 CMP, -14456/-14457, -54874 DTLS, -54876 OCSP; worst *Moderate*). 4.0.2 is the other branch | **Not in our call surface.** The shell uses OpenSSL only for `EVP` (Ed25519 verify in `UpdateStager`/`UpdateFs`), `SHA` and `HMAC` (`FarblingPolicy`) — no TLS, QUIC, CMS, CMP, DTLS or OCSP. Both Rust binaries use `rustls` / schannel, not OpenSSL | ✅ **BUMPED 2026-09-15** (owner) — `overrides` → `3.6.4#0`, `builtin-baseline` → `9e44ec0e…` (registry master, 2026-09-15; verified to carry openssl 3.6.4#0, sqlite3 3.53.4#1, nlohmann-json 3.12.0#2). ⛔ **Not locally verifiable** (the local build dir runs vcpkg with manifest mode OFF) — the proof is the next `workflow_dispatch` validation build on the org repo (`BUILD_AND_RELEASE.md` Step 3b); a failure there is *this* change, not a CEF symptom. `release.yml` reads the baseline out of `vcpkg.json`, so nothing else moves |
| **SQLite** (vcpkg) | `3.53.4` | `3.53.4` (sqlite.org amalgamation 3530400) | none | ✅ hold |
| **nlohmann-json** (vcpkg) | `3.12.0#2` | `3.12.0` (registry `#2`) | none | ✅ hold |
| **Rust toolchain** | `1.97.1` | `1.98.1` (2026-09-03) | none (compiler) | 🟡 hold; take at the next engine bump per policy item 5 |
| **Node** (build only) | `22` | 22 maintenance to **2027-04-30**; 24 is the current LTS (maint. 2026-10-20 → 2028-04-30) | none — build-time only | ✅ hold (EOL far enough); reconsider 24 in 2027 Q1 |
| **Inno Setup** | `6.7.1` (Chocolatey) | upstream `7.1.0`; Chocolatey's list was not re-queried today (its OData filter returned nothing) — 2026-08-17 recorded `6.7.1` as Chocolatey's newest | none known | ⏸️ hold (major compiler change; `hodos-browser.iss` untested on 7.x) |
| **WinSparkle** | dll `0.8.1` shipped, tool `0.9.4` | `0.9.4` | none | ✅ hold |
| **Sparkle** (macOS) | `2.9.6` | **`2.10.0`** (2026-09-13 — bumps *its own* minimum deployment target to macOS 12.0, matches our floor; updater fixes) | none | 🟡 hold for 0.4.0 — 2.9.6 is still unverified on a real macOS build (C1 in `HUMAN_TEST_QUEUE.md`); do not stack a second updater bump on an unverified one. Relayed to Mac |
| **CI runners** | `windows-2022` / `macos-15` | — | — | ✅ hold |
| **vcpkg baseline** | `fbb17a16…` (2026-08-03) | registry HEAD | see OpenSSL | 🟡 moves with the OpenSSL bump, not alone |

#### Rust crates — `cargo audit` (both workspaces)

| Workspace | Advisory | Crate @ pinned | What it is | Reaches us? | Decision |
|---|---|---|---|---|---|
| wallet | **RUSTSEC-2026-0098 / -0099 / -0104** | `rustls-webpki 0.101.7` | X.509 **name constraints incorrectly accepted** (×2) + reachable panic in CRL parsing | 🚨 **Yes.** This is the certificate validator behind `reqwest 0.11` (`rustls-tls`) — every outbound HTTPS the wallet makes (WhatsOnChain, GorillaPool, MessageBox, price APIs). A mis-accepted name constraint is a server-authentication weakness on the path that feeds balances and broadcasts | 🔴 **bump recommended — owner decision.** Patched only in `0.103.12+`, i.e. `rustls 0.23` ⇒ **`reqwest 0.11 → 0.12+`** (latest 0.13.5; the `rustls-tls` feature is renamed there). A real change to the HTTP client of a money-handling binary: its own phase, full wallet test suite, `authfetch`/MessageBox smoke |
| wallet + adblock | **RUSTSEC-2026-0258** | `h2 0.3.27` | HTTP/2 unbounded empty DATA frames (DoS, client side) | partially — only against a malicious/compromised HTTPS server we connect to | 🟡 same bump path (`h2 0.4` comes with `reqwest 0.12`) |
| wallet + adblock | **RUSTSEC-2026-0009** | `time 0.3.44` / `0.3.41` | DoS via stack exhaustion parsing untrusted time strings (CVSS 6.8) | low — no untrusted time-string parsing found on our paths, but it is transitive and cheap | ✅ **BUMPED 2026-09-15** (owner): `cargo update -p time -p bytes` — `time` 0.3.44/0.3.41 → **0.3.55** (+ `time-core`, `time-macros`, `deranged`, `num-conv` with it), both workspaces; `cargo build --release` green, `cargo audit` no longer lists RUSTSEC-2026-0009 |
| wallet | **RUSTSEC-2026-0007** | `bytes 1.10.1` | integer overflow in `BytesMut::reserve` | low | ✅ **BUMPED 2026-09-15** (owner): `bytes` 1.10.1/1.11.1 → **1.12.1**, both workspaces; RUSTSEC-2026-0007 gone. Wallet now 4 advisories (all the `reqwest`/`rustls-webpki`/`h2` family = beta.4 sprint 0), adblock 1 (`h2`) |
| both | warnings | `rustls-pemfile 1.0.4` unmaintained · `rand 0.8/0.9` unsound with a custom logger · `anyhow 1.0.102` unsound `downcast_mut` · `rmp-serde 0.15.5` unsound `Raw`/`RawRef` · `js-sys`/`wasm-bindgen` yanked (adblock, transitive) | recorded; none affect a code path we use (`rand::rng()` with a custom logger is not our shape; `rmp-serde` `Raw` types unused) | 📝 hold, re-check next review |

`cargo outdated --root-deps-only`: wallet has 30 root deps behind (majors: `reqwest` 0.11→0.13, `rusqlite`
0.30→0.40, `secp256k1` 0.28→0.33, `thiserror` 1→2, `dirs` 5→7, `flexi_logger` 0.29→0.31, `rand` 0.8→0.10,
the RustCrypto set `aes`/`aes-gcm`/`hmac`/`pbkdf2`/`sha2`/`cbc`/`ripemd` one minor each); adblock has 10
(`adblock` 0.10.3→0.13.3 — held deliberately for the MSRV graph, see Lessons). ⛔ `secp256k1` and the
RustCrypto crates are **crypto/signing** — invariant #3, never bumped without the owner and the full
signing test set.

#### Frontend — `npm audit` (16: 11 high, 3 moderate, 2 low; all have an in-range `npm audit fix`)

| Package (runtime or build?) | Range | Advisories | Decision |
|---|---|---|---|
| **`react-router` / `react-router-dom`** 7.6.1 (**runtime**, in the shipped bundle) | ≤7.17.0 | 13 — SSR XSS, CSRF in server actions, open redirect via `//` and backslash in `<Link>`/`useNavigate`, DoS via path expansion | ✅ **BUMPED 2026-09-15** (owner) via `npm audit fix` → `react-router(-dom)` **7.18.3**; `package.json` unchanged (in-range), lockfile only. 📏 Smoke on the dev rig after a hard reload: wallet overlay renders live state (balance, price, the three Received payments, RECEIVE/SEND/SCAN), `getBalance`/`getStatus` through the bridge answer; `npm run build` green |
| `vite` 6.3.5, `rollup`, `postcss`, `@babel/core` (**build / dev server**) | vite ≤6.4.2 | dev-server file read / path traversal / `server.fs.deny` bypass on Windows; rollup path-traversal write; postcss XSS in stringify | ✅ **BUMPED 2026-09-15** — `vite` **6.4.3**, `rollup` 4.63.3, `postcss` 8.5.28 (same `npm audit fix`); `npm audit` = **0 vulnerabilities** |
| `browserslist`, `minimatch`, `brace-expansion`, `flatted`, `js-yaml`, `nanoid`, `ajv`, `yaml`, `@humanfs/node`, `@eslint/plugin-kit` (build tooling) | various | ReDoS / prototype pollution / memory growth | ✅ **BUMPED 2026-09-15** — same `npm audit fix`; 0 remaining. ⚠️ Local-only warning: `eslint-visitor-keys@5.0.1` declares `node ^20.19 || ^22.13 || >=24` and this box runs v23 (non-LTS); CI builds on 22, unaffected |

`npm outdated`: React 19.1→19.3, MUI 7.3.9→9.4 (major), TypeScript 5.8→7.0 (major), Vite 6→8 (major),
`@vitejs/plugin-react` 4→6 (major). ⛔ The Vite/TS majors are build-target changes — `vite.config.ts`
`build.target: 'chrome150'` must survive any Vite bump (it is the engine binding).

#### Symbol coexistence — re-measured, now scripted

`scripts/libcef_export_coexistence.ps1` (this folder) against the shipped `cef-binaries/Release/libcef.dll`
(292,293,632 B, md5 `8c761ddc87d3461dabb7e03087975d8f`, the P4f `g9ccef04` engine): **247 exports, 240
`cef_*`, 1 crypto/sqlite (`sqlite3_dbdata_init`)** — identical to 2026-08-17. Negative control: the same
script on `HodosBrowser.dll` (which *does* link our OpenSSL/SQLite, statically) reports 1 export, 0 `cef_*`,
0 crypto — a different shape, so the script measures the file it is pointed at. Conclusion unchanged: ABI
coexistence, not version matching; never link the shell against Chromium's copies.

#### What this review changes

- **Two owner decisions surfaced:** the `reqwest 0.11 → 0.12+` bump (closes the three `rustls-webpki`
  advisories on the wallet's TLS path — the one item here that is a server-authentication weakness rather
  than a DoS), and the cheap `cargo update -p time -p bytes`. Neither done in the review itself.
  👤 **Decided 2026-09-15:** `reqwest` becomes **beta.4 sprint 0** (`0.4.0-beta.4/sprint-0-reqwest-tls-bump/`);
  `time`/`bytes`, OpenSSL 3.6.4 and the npm set were **bumped the next day as their own commits** — see the ✅ cells.
- OpenSSL 3.6.4 is a recommended, non-urgent bump (no affected API in our use) — ✅ done 2026-09-15, proof owed to the next validation build.
- The CI audit lane is still triple-neutered and dark; this review on the build host is the only advisory
  run anyone has read. Flipping `test.yml`'s audits to blocking is an instrument change and belongs to the
  Actions-quota decision, not to this review.

### Lessons
- **A crate pin without a compiler pin is half a pin.** `adblock-engine` already had exact crate
  pins (`adblock = "=0.10.3"`, `rmp = "=0.8.14"`) chosen to hold an MSRV-sensitive graph together,
  while the toolchain that had to satisfy that MSRV floated. Pin both or neither.
- **Chocolatey lags upstream — check the packaging source, not the project.** Inno Setup upstream is
  on 7.0.x (7.0.0 released 2026-05-18), but Chocolatey's `innosetup` package tops out at **6.7.1**.
  Pinning to the upstream-latest would have produced an uninstallable version string. Always read
  the version list of the *channel you install through*.
- **Commented-out YAML still drifts.** Disabled job stubs carrying `*-latest` are a copy-paste
  source that re-introduces floating images the moment someone enables the job. Pin dead code too.
- **Adding `vcpkg.json` changes vcpkg's MODE, not just its versions.** Manifest mode auto-activates
  from the file's mere presence next to `CMakeLists.txt`, so the CI step that ran a classic
  `vcpkg install` had to be replaced in the same commit — otherwise classic and manifest installs
  fight over the same `find_package` resolution. Also note `builtin-baseline` must exist in the
  runner's vcpkg git history; the workflow now fetches that exact commit before configure.

## Automation goal (0.4.0 target)
This checklist should become **scripted + test-gated** so it runs the same way every time:
- a script that enumerates the pinned versions across all 5 layers and diffs against the new CEF's expected toolchain,
- compile + link + `cargo test` + `ctest` + frontend tests as the pass/fail gate,
- a generated report that drops into the version tracker.
Until automated, run it by hand against this checklist and record results.
