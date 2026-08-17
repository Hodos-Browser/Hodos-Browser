# TICKET — the shipping engine's "pin" is a mutable branch, not a tag

**Filed:** 2026-08-17, during 0.4.0 archiving
**Severity:** reproducibility — the source ref for the engine we ship can move
**Status:** OPEN — beta.3, cheap
**Effort:** minutes (push two tags), plus a convention note

---

## What is on the fork right now

Queried live on `Hodos-Browser/cef`:

```
refs/tags/pin-c636546/7871       c63654654948db230ac9bbbac70dde6bfab59bab   ← a TAG
refs/heads/pin-7dd0357/7871      7dd0357392446e62662d5a7d1c642c23dc8de6a5   ← a BRANCH
refs/heads/pin-9ccef04/7871      9ccef044fe31bb057525e218949eb81e2ec5929c   ← a BRANCH
refs/heads/hodos/7871            9ccef044fe31bb057525e218949eb81e2ec5929c   ← working branch
```

**The engine `v0.4.0-beta.2` ships (`g9ccef04`, "P4f") is pinned by a branch.** So is P4e. Only the
oldest of the three — `c636546`, which we no longer ship — got an actual immutable tag.

The convention was established correctly and then not followed for the two engines that matter.

## Why it matters

A branch is mutable by anyone with write access. `pin-9ccef04/7871` and `hodos/7871` currently point
at the *same commit*, so the ordinary act of continuing development on `hodos/7871` sits one
`git push --force`, one mis-typed branch name, or one branch deletion away from the "pin" no longer
identifying the engine in the shipped browser.

A tag is the right instrument for "this exact source built the binary users are running." That is
precisely the claim a pin is making.

⚠️ **Not urgent, and not currently broken** — the commits exist, are reachable, and won't be
garbage-collected while the branches point at them. The SHAs are also written down in
`cef-native/CLAUDE.md`, the relay, and `CEF_VERSION_UPDATE_TRACKER.md`. This is about removing a
silent-failure mode, not repairing damage.

## Related: one engine's binary is already unrecoverable

While surveying, the `cef-binaries` release assets were checked:

| Engine | Built binary on the release? |
|---|---|
| `g9ccef04` (P4f, **shipping**) | ✅ both platforms, versioned names |
| `g7dd0357` (P4e) | ✅ both platforms, versioned names |
| `c636546` (C1–C6, pre-P4e) | ⛔ **no versioned asset** |

`cef-binaries-windows-150.zip` is **byte-size-identical** to the P4e asset (`239437626`) and was
uploaded three hours *before* it — i.e. the unversioned name carries P4e, and `c636546`'s Windows
binary was clobbered away. That is the exact damage the never-`--clobber` rule now written into
`CEF_BUILD_RUNBOOK.md` exists to prevent; it is recorded here as the concrete instance.

⭐ **Consequence:** the only built copy of `c636546` is the local `cef-binaries-backup-gc636546/`
directory on the Windows build host. The *source* is safe (it is the one properly tagged engine), so
it is rebuildable — at roughly a five-hour build.

## Fix

1. Push real tags for both current engines, pointing at the same commits:
   ```
   pin-9ccef04/7871  -> 9ccef044fe31bb057525e218949eb81e2ec5929c
   pin-7dd0357/7871  -> 7dd0357392446e62662d5a7d1c642c23dc8de6a5
   ```
2. Delete the two `pin-*` **branches** once the tags exist, so there is exactly one ref per pin and
   no ambiguity about which kind it is.
3. Write the convention where the next person will look — `CEF_BUILD_RUNBOOK.md`, beside the new
   "Publishing the built distribution as a CI asset" section: **an engine pin is a tag; the working
   branch is `hodos/<branch>`; never a `pin-*` branch.**
4. ⛔ Verify by re-querying `git ls-remote --tags`, not by assuming the push worked. A pin that
   silently failed to push is worse than a branch, because it reads as done.

## Acceptance

- [ ] `git ls-remote --tags` shows tags for all three engines
- [ ] `pin-*` branches removed
- [ ] convention recorded in `CEF_BUILD_RUNBOOK.md`
- [ ] decide the fate of `cef-binaries-backup-gc636546/` — it is the only built copy of that engine
