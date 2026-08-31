# TICKET — we pin dependencies well and never re-evaluate them

**Filed:** 2026-08-17, answering "did we update dependencies after CEF 150?"
**Severity:** process gap — no CVE/freshness path on a money-handling binary
**Status:** OPEN — beta.3, cheap
**Sprint:** 📌 Phase 9 (release readiness) — bundled 2026-08-31.
**Effort:** a documented cadence + one review pass

---

## The direct answer to the question that prompted this

**No — the dependency pass predates the engine bump.** DEP-1 landed **2026-08-03**; CEF 150 S0/S1
landed **2026-08-04**. Nothing has been re-reviewed since, and beta.2 shipped on those same pins.

That is *mostly* fine, for a reason worth writing down: **our dependencies and Chromium's are
separate trees.** Chromium vendors its own crypto (BoringSSL), zlib, libpng and so on through its
`DEPS`, resolved by `automate-git.py`. Our vcpkg OpenSSL / sqlite3 / nlohmann-json link into **our**
shell and the Rust binaries — **not** into `libcef`. They do not have to match Chromium's versions.

⚠️ **But "separate trees" is not the same as "no overlap", and an earlier phrasing of this overstated
it.** The two copies live in the **same process**, so the question is not version-matching but
**symbol/ABI coexistence**. Measured against the shipped `libcef.dll` (P4f):

```
total exported symbols : 247
cef_* exports          : 240
crypto/sqlite exports  : 1   -> sqlite3_dbdata_init
```

So CEF's export surface is almost entirely its own C API, and Chromium's BoringSSL/SQLite are
statically linked *inside* `libcef.dll` rather than exported. Our OpenSSL and SQLite are statically
linked into `HodosBrowser.dll` and the Rust binaries, and Windows resolves statically-linked symbols
per-module, so the copies coexist without binding to each other.

The honest summary: **the coexistence surface is real but measurably tiny (1 of 247 exports), and the
relationship is ABI coexistence, not version matching.** Two consequences worth carrying:

- ⛔ **Never link our shell against Chromium's crypto or SQLite**, and never assume a symbol resolved
  at runtime came from our copy. `sqlite3_dbdata_init` is the one name where that assumption could
  quietly be wrong.
- **Re-measure this after every engine bump.** It is one script and it turns "no overlap" from an
  assumption into a number. If a future CEF starts exporting more of its vendored libraries, this is
  the check that notices.

### The one place a real version relationship DOES exist: the frontend

The React/Vite bundle runs **inside** the shipped Chromium's V8, so its output must be syntax the
engine supports. Today that relationship is **entirely undeclared**: `frontend/package.json` has no
`browserslist` and no `engines` field, and nothing ties the Vite build target to the CEF version.

It is not biting because Chromium 150 is far newer than anything Vite targets by default — i.e. we
are safe by accident, not by construction. Declaring a `browserslist` pinned to the shipped Chromium
would make it safe by construction and would catch the reverse case (a dependency emitting syntax
newer than our engine) at build time instead of as a blank page.

What genuinely *must* track the engine is a short list, and all of it is already pinned and was
validated green by the beta.2 build:

| Must match the engine | Pin | Where |
|---|---|---|
| MSVC toolset (v143) | `windows-2022` runner | `release.yml` |
| C++ standard 20 | required by CEF 150 headers | `CMakeLists.txt` |
| Wrapper build settings (`/MT`, `USE_SANDBOX=ON`, C++20) | must equal the app's | `CEF_BUILD_RUNBOOK.md` |

## The actual gap: the policy is "freeze", and nothing ever thaws

Read the pins' own comments and the policy is explicit — and it is **neither "minimum required" nor
"most recent stable"**:

> *"1.97.1 is what `@stable` resolved to on 2026-08-03, so this pin records current behaviour rather
> than changing it."* — `rust-toolchain.toml`

> *"6.7.1 is the newest version Chocolatey packages as of 2026-08-03 and is what the floating command
> resolved to for beta.29, so this pin records current behaviour rather than changing it."*
> — `release.yml`

That is a **freeze at the moment we took control**. It is the right call for a binary that handles
money — it converts silent drift into a reviewable diff, which is exactly what DEP-1 was for.

⛔ **But a freeze with no scheduled thaw is how you end up shipping a known-vulnerable OpenSSL
without ever making a decision to.**

> ⚠️ **CORRECTED 2026-08-17, same day.** An earlier revision of this ticket said there was "no step
> anywhere that checks the pinned versions against security advisories." **That was wrong** —
> `.github/workflows/test.yml` runs **`cargo audit`** (both Rust workspaces) and **`npm audit
> --audit-level=high`** (frontend). The corrected finding is narrower and more specific:

| Dependency family | Advisory check | Can it fail the build? |
|---|---|---|
| Rust crates | `cargo audit`, both workspaces | ⛔ **No** — `continue-on-error: true` **and** `\|\| true` |
| npm / frontend | `npm audit --audit-level=high` | ⛔ **No** — same double neutering |
| **vcpkg C++ (OpenSSL, sqlite3, nlohmann-json)** | ⛔ **none** | — |
| **Inno Setup, WinSparkle, Sparkle** | ⛔ **none** | — |
| **Runner images** | ⛔ **none** | — |

The workflow's own comment is honest about it: *"INFORMATIONAL — flip to blocking after a
dependency-advisory triage chunk."* So the checks exist, cover 2 of 5 families, and are
triple-neutered (`continue-on-error` + `|| true` + a `high` threshold). Nothing has ever been
triaged off the back of them.

🚨 **And right now they are not running at all — see the CI-minutes finding below.**

What remains missing is therefore:

- no cadence for re-reviewing the pinned set,
- **no advisory coverage for the C++ / installer / updater / runner families at all**,
- the two checks that exist cannot fail a build, so a critical advisory is a log line nobody reads,
- no record of *why* a given version is acceptable beyond "it is what resolved that day".

## 🚨 The audits have not run since 2026-08-14

`test.yml` on the dev fork shows a clean break: **every run succeeded through 2026-08-14T20:31 and
every run since has failed** — 2026-08-14T22:36 onward, seven consecutive failures at the time of
writing.

They are not test failures. Every job reports **`steps=0`** with `started_at == created_at`: nothing
executed. **No code changed** in `rust-wallet`, `adblock-engine` or `frontend` between the last
success and the first failure. That signature — instant failure, zero steps, on a **fork with
metered Actions minutes** (the dev fork has a 2,000-minute monthly allowance; the org's are free) —
points squarely at **the dev fork's Actions quota being exhausted**.

⚠️ **Consequence: since 2026-08-14 no `cargo test`, no `clippy`, no secret-log gate (F8), no
`cargo audit` and no `npm audit` has run on any commit — including everything that went into
`v0.4.0-beta.2`.** The beta.2 *build* was green because release builds run on the **org** repo,
whose minutes are free; only the dev fork's test lane is dark.

⛔ **Confirm before acting** — billing needs the `user` scope this session does not have. Run
`gh api users/BSVArchie/settings/billing/actions` with a token that has it, or read
Settings → Billing → Actions. If it is quota, the options are: wait for the monthly reset, raise the
spending limit, or move the test lane to the org repo where minutes are free.

The pins are: `openssl 3.6.3`, `sqlite3 3.53.4`, `nlohmann-json 3.12.0` (vcpkg exact overrides),
Rust `1.97.1` (both workspaces), Inno Setup `6.7.1`, Node `20`, WinSparkle `0.8.1` + `0.9.3`,
Sparkle `2.9.3`, runners `windows-2022` / `macos-15`.

## ⚠️ macOS is weaker than Windows, by design and on the record

`Brewfile` **cannot** pin versions — Homebrew has no mechanism for it, and the file says so:

> *"Version-exactness on macOS is therefore still weaker than the Windows vcpkg pin. If macOS
> dependency drift ever breaks a build, the escalation is a `brew extract` into a Hodos tap."*

So the macOS build's OpenSSL/sqlite3/nlohmann-json float to whatever Homebrew ships that day. That
is a known, documented, accepted weakness — but it means "we pin our dependencies" is only true on
one platform, and anyone reasoning about reproducibility should know which.

## Proposed policy (to replace "freeze and forget")

**Pin exactly, review on a cadence, bump deliberately.** Concretely:

1. **Keep the freeze.** Do not float anything. The pins stay exact.
2. **Review at every engine bump and at minimum quarterly** — a Chromium bump is already a
   whole-stack revalidation, so it is the natural checkpoint.
3. **At review, for each pinned dependency record:** current pin, latest stable, whether any advisory
   affects the pinned version, and the decision (hold / bump). A one-line-per-dep table.
4. **Prefer newest-stable-that-builds over minimum-required.** We are not supporting a range of
   dependency versions for third parties — nobody consumes us as a library — so there is no benefit
   to holding a low floor, and staying near current shrinks the eventual jump.
5. **Decide the macOS escalation** — either accept the float explicitly and say so in the release
   notes' reproducibility section, or do the `brew extract` into a Hodos tap the Brewfile already
   proposes. Right now it is neither, which reads as an oversight rather than a choice.

## Acceptance

- [ ] policy written into `DEPENDENCY_VERIFICATION.md` (it currently records *what* is pinned, not
      *when we re-look*)
- [ ] one review pass done against the current pins, with the table from item 3
- [ ] macOS float either accepted in writing or escalated to a tap
- [ ] review checkpoint added to the Chromium-bump checklist in `CEF_VERSION_UPDATE_TRACKER.md`
