# TICKET — we pin dependencies well and never re-evaluate them

**Filed:** 2026-08-17, answering "did we update dependencies after CEF 150?"
**Severity:** process gap — no CVE/freshness path on a money-handling binary
**Status:** OPEN — beta.3, cheap
**Effort:** a documented cadence + one review pass

---

## The direct answer to the question that prompted this

**No — the dependency pass predates the engine bump.** DEP-1 landed **2026-08-03**; CEF 150 S0/S1
landed **2026-08-04**. Nothing has been re-reviewed since, and beta.2 shipped on those same pins.

That is *mostly* fine, for a reason worth writing down: **our dependencies and Chromium's are
separate trees.** Chromium vendors its own crypto (BoringSSL), zlib, libpng and so on through its
`DEPS`, resolved by `automate-git.py`. Our vcpkg OpenSSL / sqlite3 / nlohmann-json link into **our**
shell and the Rust binaries — **not** into `libcef`. They do not have to match Chromium's versions
and there is no compatibility relationship to maintain.

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
without ever making a decision to.** There is currently:

- no cadence for re-reviewing the pinned set,
- no step anywhere that checks the pinned versions against security advisories,
- no record of *why* a given version is acceptable beyond "it is what resolved that day".

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
