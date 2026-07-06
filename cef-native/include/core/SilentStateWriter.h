// SilentStateWriter.h — mirror the user's (global) autoUpdateMode into the silent
// apply-eligibility gate, plus the pure helpers for the one-time global-mode collapse.
//
// See development-docs/DevOps-CICD/AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md.
//
// The pure helpers (rank / most-conservative / compute-eligibility) are cross-platform
// and unit-tested. MirrorSilentEligibility (the FS read-modify-write of the GLOBAL
// update-state.json) is Windows-only + only meaningful under HODOS_SILENT_AUTOUPDATE
// (the apply gate it feeds is compiled out otherwise). Callers MUST invoke it only on a
// NORMAL launch (`!g_picker_mode && !g_post_update_probe`) — that guarantees mutual
// exclusion with the helper's update-state writes (a launch with update.lock held already
// deferred at the honor-probe), so no lock is needed here.

#pragma once

#include <string>
#include "UpdateApply.h"  // hodos::UpdateState (pure, cross-platform)

namespace hodos {

// Rank an update mode by permissiveness: off (safest) < notify < silent (most
// permissive). Unknown/invalid -> treated as the safest (off). Pure.
int UpdateModeRank(const std::string& mode);

// Return whichever of a/b is MORE conservative (lower rank). Ties return `a`.
// Used by the one-time global-mode collapse so an explicit notify/off in ANY profile
// is never silently promoted to silent. Pure.
std::string MoreConservativeMode(const std::string& a, const std::string& b);

// Set `state.silent = (mode=="silent")`, preserving every other field. Returns true iff
// the value CHANGED (a write is warranted). Fail-safe: any non-"silent" mode (including
// unknown/empty) yields silent=false. Pure — testable without the filesystem.
bool ComputeSilentEligibility(UpdateState& state, const std::string& mode);

#ifdef _WIN32
// Read-modify-write the GLOBAL update-state.json via ComputeSilentEligibility (preserves
// paused / highWaterBuild / lastFailure / rescan). No-op if the update paths are
// unavailable or an apply owner holds update.lock (defense-in-depth PROBE — never mutates
// the lock). Call ONLY from a normal launch (see header note).
void MirrorSilentEligibility(const std::string& mode);
#endif

}  // namespace hodos
