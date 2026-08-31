# A deleted profile's id is reissued over its orphaned data directory

**Status:** ✅ **CLOSED** — Phase 1, `84997eb` + `47d9a06`. Startup sweep renames orphans to `.orphaned-<stamp>`; `DeleteProfile` renames-then-deletes so a freed id can never land on live data. ⚠️ The macOS marker-depth gap from the P1 relay is a SEPARATE item. Labelled 2026-08-31 (had no status line).

**Found:** 2026-08-26, during the Phase 1 (WS1) test session, by deleting a profile and then
reading the log and the disk.
> ## ⚠️ RE-SCOPED 2026-08-26 — the original advice in this ticket was WRONG
>
> It said *"do not fix this by making delete remove the directory"*. That weighed the risk of
> destroying data a user might want back and never weighed the cost of **keeping** it. On a
> privacy browser that is the wrong balance: a user deleting a profile is usually deleting it to
> be rid of the browsing data, and we were silently retaining every cookie and session forever.
> Owner's call, and he was right.
>
> **Shipped 2026-08-26:** `DeleteProfile` now renames the directory (atomic, frees the id
> immediately) and then removes it, gated on a `profile.lock` probe so a profile another window
> is running cannot be deleted out from under it. Verified two-sided: lock held → refused with
> data intact; lock released → 1320 files removed.
>
> **That closes the hazard for every FUTURE deletion.** All three outcomes are safe — delete
> succeeds (folder gone), delete partially fails (folder already renamed, id still free), rename
> fails (refused, nothing touched).
>
> ### 🚨 What remains, and it is a MIGRATION problem, not a code-path problem
>
> **The fix does not reach backwards.** Every user who has ever deleted a profile on an earlier
> build has an orphaned `Profile_N` directory on disk *right now*. `GenerateProfileId` still
> derives the next id from the listed profiles, so the next profile they create after upgrading
> is handed that id and silently adopts the old profile's cookies, sessions and history. This is
> not hypothetical — it is exactly what was found on the owner's machine (173.8 MB, `Profile_4`).
>
> Two things still owed:
> 1. **A startup sweep**: on launch, find directories matching `Profile_<N>` that are not listed
>    in `profiles.json` and rename them to `.orphaned-<ts>` so no id can land on live data.
>    ⛔ Rename, do not delete — this runs unattended against data the user never chose to lose.
> 2. **`CreateProfile` must refuse a non-empty target directory.** `fs::create_directories`
>    silently succeeds on an existing path and nothing checks whether it was populated. Cheap
>    belt-and-braces that would have prevented the whole class regardless of how a stray
>    directory got there (manual copy, restored backup, crash mid-operation).
>
> ⚠️ Without #1, shipping the delete fix **arms** this for existing users rather than fixing it:
> deletion starts working correctly at the same moment their pre-existing orphan becomes
> reachable by a new profile.

**Severity:** 🚨 **Profile separation boundary.** A newly created profile silently adopts a deleted
profile's cookies, logged-in sessions, history and content settings.

⛔ This is **not** the same defect as the 2026-08-24 incident, and the guard added that day does not
prevent it. That guard stops you deleting the profile *this window is running on*. This is about what
happens to the **id and the directory** after any successful delete.

---

## Measured, on the owner's machine

```
2026-08-26 07:34:01  👤 Profile deleted: Profile_4          (accepted, no error)
%APPDATA%\HodosBrowserDev\Profile_4                          still present — 173.8 MB
profiles.json now lists: Default, Profile_1, Profile_2, Profile_3
```

## The three steps that combine

1. **`ProfileManager::DeleteProfile` deliberately does not remove the files.**
   ```cpp
   std::string profilePath = app_data_path_ + "/" + it->path;
   // Note: Not deleting files for safety - user can manually delete
   profiles_.erase(it);
   ```
   Reasonable on its own — data loss is worse than disk use — but it leaves a directory whose name
   is a *reusable key*.

2. **`GenerateProfileId()` derives the next id from the LISTED profiles**, not from what exists on
   disk. With `Profile_4` gone from `profiles.json`, the next generated id is `Profile_4` again.

3. **`CreateProfile` calls `fs::create_directories(profilePath)`**, which **succeeds silently when
   the directory already exists**. Nothing checks whether the path was already populated. The new
   profile then adopts it whole.

⇒ Delete a profile, create a profile, and the new one is the old one wearing a new name and colour.

## Why it matters more here than in a normal browser

Profiles are a **separation boundary**. Inheriting another profile's cookies means inheriting its
logged-in sessions. In a browser that also holds a wallet, "which profile am I in" is a security
question, not a cosmetic one.

⚠️ Compounding: `TICKET_wallet_global_profiles_isolated.md` records that one `wallet.db` already
serves every profile, and
`TICKET_longlived_surfaces_snapshot_state_at_startup.md` records that the profile indicator in the
toolbar is a snapshot from browser start. So the user can be shown the wrong profile identity while
acting on a third profile's session data.

## Shape of the fix

Any ONE of these closes it; the first two are cheap and independent.

1. ⭐ **Never reuse an id.** Generate from a monotonic counter persisted in `profiles.json`, or
   include what is on disk when computing the next number. One-line-ish, no data touched.
2. **Refuse to create into a non-empty directory.** `CreateProfile` should treat an existing,
   populated path as an error rather than adopting it — fail loud instead of silently inheriting.
3. **Rename on delete.** Move `Profile_N` to `Profile_N.deleted-<timestamp>` so the key is freed
   while the data survives for manual recovery. Keeps the "don't destroy user data" intent and
   removes the collision.

~~⛔ Do **not** "fix" this by making delete remove the directory.~~ **Superseded — see the
re-scope note at the top.** Deletion now removes the data, behind a confirmation and a lock probe.
Kept visible rather than deleted, because the reasoning was wrong in an instructive way: it treated
retention as the safe default on a browser whose entire product promise is not retaining things.

⚠️ **A disk-space consequence rides along:** deleted profiles accumulate forever with no UI that
mentions them. 173.8 MB per abandoned profile, invisible to the user.

## Immediate mitigation for any machine that has already deleted a profile

Rename the orphaned directory so the id cannot be matched:

```
%APPDATA%\HodosBrowserDev\Profile_4  ->  Profile_4.orphaned-<date>
```

Reversible, destroys nothing, and frees the id path.

## Test that must be seen to fail

Delete a profile, create a new one, and assert the new profile has **no** history, **no** cookies and
its own settings. ⛔ Do it against a profile with *recognisable* data (a specific logged-in site), or
a green result cannot distinguish "clean profile" from "clean because the old one was empty" — the
vacuous-pass shape this sprint keeps hitting.

## Related

`TICKET_wallet_global_profiles_isolated.md` · `TICKET_longlived_surfaces_snapshot_state_at_startup.md`
· the 2026-08-24 running-profile guard in `ProfileManager::DeleteProfile` (adjacent, different bug)
