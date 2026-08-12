# TICKET — a MISSING profile directory is reported as "Profile is already in use by another instance"

**Found:** 2026-08-12, while creating a clean profile for the P6 T1 (auth sign-in) leg.
**Status:** OPEN, not started. Small. **Not a regression** — behaviour is as originally written.
**Severity:** low impact, high confusion. It sends the user hunting for a second browser that
does not exist.

## What happens

`cef_browser_shell.cpp` (~:4832):

```cpp
if (!AcquireProfileLock(profile_cache)) {
    MessageBoxA(nullptr,
        ("Profile \"" + profileId + "\" is already in use by another instance.\n\n"
         "Close the other instance first, or launch with a different profile.").c_str(),
        "Hodos Browser - Profile Locked", MB_OK | MB_ICONERROR);
```

`src/core/ProfileLock.cpp`:

```cpp
std::string lock_file = profile_path + "\\profile.lock";
HANDLE handle = CreateFileA(lock_file.c_str(), GENERIC_WRITE, 0, NULL,
                            CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_DELETE_ON_CLOSE, NULL);
```

**`CreateFileA` does not create parent directories.** If `<profile_path>` is absent it fails with
`ERROR_PATH_NOT_FOUND` (3), and `AcquireProfileLock` treats *every* failure identically: retry 6×
at 500 ms, then return false. The caller has only one message for `false`, so a missing directory
is announced as a lock conflict — after a **3-second** stall from the retry loop.

**Reproduced deterministically:** add a profile entry to `profiles.json` with no directory on disk,
launch with `--profile=<id>`. Dialog appears every time; no `Using profile:` line is ever logged,
because the failure precedes it. Creating the directory — changing nothing else — fixes it
immediately (`Remote debugging port: 9325`, browser up, fresh cookie DB).

## Why it can happen to a real user

The picker's own create-profile flow makes the directory, so the normal path is fine. But the
directory can go missing underneath an existing `profiles.json` entry:

- backup/restore or sync software (OneDrive, Dropbox) pruning or relocating an empty-looking dir
- "clean up temporary files" utilities
- a partially-restored profile after the wallet/profile restore flow
- manual tidying of `%APPDATA%`

In each case the user is told to close a second instance that does not exist, and the real problem
— a missing directory — is never mentioned.

## Suggested fix (not started — needs owner sign-off, this is startup-path production code)

1. **Distinguish the errors.** `CreateFileA` failure is not one condition. `ERROR_PATH_NOT_FOUND` /
   `ERROR_FILE_NOT_FOUND` mean "no directory"; `ERROR_SHARING_VIOLATION` / `ERROR_ACCESS_DENIED`
   mean a genuine conflict. Return the distinction (or an enum) instead of a bare `bool`.
2. **Create the directory and continue** on the not-found case — `std::filesystem::create_directories`
   before the first attempt. This is almost certainly the right behaviour: a profile listed in
   `profiles.json` whose directory is absent should be re-created, not refused.
3. **Do not retry 6× on a not-found error.** Retrying is correct for a shutting-down previous
   instance and pointless for a missing path; today it adds a 3-second stall to the wrong case.
4. Keep the existing message for the *genuine* sharing violation — it is accurate there.

⚠️ Fix (2) has a judgement call in it: silently re-creating a directory could mask a profile whose
data was lost, so the user may deserve a distinct, non-alarming notice ("profile data was missing
and has been re-initialised") rather than a silent recovery. Owner's call.

## Related

- The same startup path also calls `SingleInstance::StartListenerThread(profileId)` **before** the
  lock, and the existing S1 review note there already documents having to stop+join the listener
  on this failure path to avoid `std::terminate()` at static destruction. Any change here must keep
  that ordering.
- Found while setting up `Profile_3` / `t1-clean` for the P6 T1 leg; see
  `development-docs/0.4.0/IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` §7.
