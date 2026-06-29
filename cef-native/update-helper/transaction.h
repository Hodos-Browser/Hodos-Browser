// transaction.h — the supervisor's Phase B/C/E + --resume state machine (6b.2b).
//
// AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3. The fragile Win32 process orchestration:
// wait-for-bootstrap, image-unlock poll, child-shutdown, installer spawn, integrity
// gate, health-probe launch+wait, and the DB-first crash-atomic rollback. Wires the
// pure 6b.2a primitives (UpdateFs) + the two-mode lock (UpdateLock). Helper-only
// (the in-browser bootstrap is 6c; this never links CEF).

#pragma once
#ifdef _WIN32

#include <map>
#include <string>

#include "core/UpdateApply.h"
#include "core/UpdateLock.h"

namespace hodos {
namespace helper {

// Logging sink (the helper owns the file logger in main.cpp).
using LogFn = void (*)(const std::string&);
void SetLogger(LogFn fn);

// Phase B/C/E — the normal apply (the bootstrap spawned us with an inherited owner
// lock + bootstrap handle). `lock` is the held owner lock (released before any
// old-build relaunch so the relaunch doesn't self-defer). Returns a process exit code.
int RunApplyTransaction(const std::map<std::string, std::string>& args,
                        UpdateLockOwner& lock, ApplyRecord rec);

// --resume — the browser-independent watchdog (RunOnce / in-browser tripwire). Re-arms
// RunOnce at entry, then resumes/restores/cleans per apply.json phase. Idempotent.
int RunResume(const std::map<std::string, std::string>& args,
              UpdateLockOwner& lock, ApplyRecord rec);

}  // namespace helper
}  // namespace hodos

#endif  // _WIN32
