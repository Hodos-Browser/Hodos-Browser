#pragma once

#include <string>

// Acquire an exclusive lock on a profile directory to prevent multiple instances
// from using the same profile simultaneously (would corrupt SQLite DBs).
// Returns true on success, false if the profile is already locked.
bool AcquireProfileLock(const std::string& profile_path);

// Release the profile lock. Call before CefShutdown().
void ReleaseProfileLock();

// Is some OTHER running instance holding this profile's lock?
//
// ⛔ Needed because ProfileManager::DeleteProfile's "can't delete the profile this window
// is running on" guard compares against currentProfileId_, which only knows about THIS
// process. A second Hodos instance running a different profile is invisible to it — so
// without this probe you can delete a profile another window is actively using, and since
// 2026-08-26 that also deletes its files out from under it.
//
// Does not acquire or hold anything: probes and releases immediately. A missing lock file
// means "not in use" — on Windows the lock is FILE_FLAG_DELETE_ON_CLOSE, so it exists only
// while held. Fails CLOSED: if the answer cannot be determined, reports true (in use),
// because refusing a delete is recoverable and deleting a live profile is not.
bool IsProfileLockedByAnotherInstance(const std::string& profile_path);
