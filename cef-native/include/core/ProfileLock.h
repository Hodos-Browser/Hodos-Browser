#pragma once

#include <string>

// Acquire an exclusive lock on a profile directory to prevent multiple instances
// from using the same profile simultaneously (would corrupt SQLite DBs).
// Returns true on success, false if the profile is already locked.
bool AcquireProfileLock(const std::string& profile_path);

// Release the profile lock. Call before CefShutdown().
void ReleaseProfileLock();
