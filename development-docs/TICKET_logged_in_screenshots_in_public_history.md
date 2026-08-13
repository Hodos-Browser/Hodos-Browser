# TICKET — logged-in session screenshots are in the PUBLIC repo's git history

**Filed:** 2026-08-13 · **Severity:** moderate, time-sensitive (exposure grows with time, not severity)
**Status:** OPEN · **Owner decision needed:** rewrite now vs. accept

---

## What happened

| When | What |
|---|---|
| 2026-08-11 | `99e72aa` committed 12 soak screenshots under `development-docs/0.4.0/chromium-rebuild/soak_out2/`. One — `Auth__x_com.png` — shows a **logged-in X session with the owner's real name, handle and profile photo**. |
| 2026-08-11 | `8eeb2b5` deleted them from HEAD and gitignored the soak dirs. **This does not remove them from history.** |
| 2026-08-11 | The Mac session flagged it in `MAC_WINDOWS_RELAY.md` round 11c §K1: *"`origin` is private but `release` is PUBLIC and 0.4.0 flows there. Fixable now, permanent later."* |
| **2026-08-13** | ⛔ **The Windows session pushed `0.4.0` → `release` (`0.4.0`, `main`, `staging`).** The warned-about event happened. |

**Current state, verified 2026-08-13:**

- `Hodos-Browser/Hodos-Browser` (the `release` remote) — `visibility=PUBLIC`
- `BSVArchie/Hodos-Browser` (`origin`) — private
- `99e72aa` is an ancestor of `release/0.4.0`, `release/main` **and** `release/staging`
- The blobs are **not** in HEAD on any branch — reachable only by walking history or by blob SHA
- Tag `v0.4.0-beta.1` also descends from `99e72aa`

## Realistic severity — stated plainly, neither minimised nor inflated

**What is exposed:** a rendered screenshot of a logged-in X timeline — real name, handle, profile
photo, and whatever feed content was on screen. Plus 11 other site screenshots (github, google,
amazon, nytimes, reddit, twitch, youtube, whatsonchain) taken while browsing normally.

**What is NOT exposed:** no credentials, no tokens, no cookies. A screenshot is a picture, not a
session. Nobody can authenticate as anyone from this.

**Why it still matters:** the owner's identity is already public, but this ties a *personal* account
to the project's test machine and publishes whatever was in the feed at that moment. And public-repo
history is mirrored by third parties (code search indexes, the GitHub public event stream, forks,
archives) — so the cost of removal rises with every day it stays up. That is the whole argument for
acting soon: severity is fixed, **recoverability decays**.

## Remediation options

### Option A — rewrite history on `release` (recommended)

~1 hour. Removes the blobs from the public repo.

1. `git filter-repo --path development-docs/0.4.0/chromium-rebuild/soak_out2/ --invert-paths`
   (and `soak_out/` if it carries any) against a fresh clone.
2. Force-push the rewritten `0.4.0`, `main`, `staging` to `release`.
3. **Re-create tag `v0.4.0-beta.1`** on the rewritten commit — it currently descends from the bad
   commit. The draft release's *assets* are attached to the release object, not the commit, so they
   survive; only the tag pointer needs moving.
4. **Ask GitHub Support to purge the unreachable objects.** Force-pushing does NOT delete them —
   GitHub serves dangling commits by SHA until garbage-collected, so anyone with the SHA can still
   fetch the blob. This step is what actually completes the removal, and it is the one people skip.
5. Re-run the same rewrite on `origin` so the two do not diverge, and so a future push does not
   reintroduce the history.
6. Verify: `git log --all --oneline -- '*soak_out2*'` empty on both remotes; try fetching the blob
   SHA directly and confirm 404.

⚠️ **Coordinate with the Mac session before force-pushing** — anyone holding the old history will
need `git fetch --all && git reset --hard <remote>/<branch>`, and a stale local push would restore it.

### Option B — accept and move on

Defensible if the owner judges the content harmless. Then **close this ticket explicitly** as an
accepted risk rather than leaving it open, and delete the `soak_out*` dirs from `.gitignore` limbo so
it cannot recur.

### Not viable

Making the `release` repo private — auto-update and public downloads pull release assets from it.

## Prevention (do regardless of A or B)

- `.gitignore` already covers the soak dirs since `8eeb2b5`.
- Harnesses that screenshot real browsing should write **outside the repo** by default (scratch dir),
  not into a gitignored subdirectory of it — gitignore is one `git add -f` away from failing.
- Prefer a clean throwaway profile for soak runs so no logged-in state is capturable in the first
  place.
