// UpdateApply.h — Windows auto-update: the durable apply-transaction model.
//
// WINDOWS_AUTOUPDATE_PLAN.md commit 6b (AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3).
// The PURE, shared data contract used by BOTH the external rollback-supervisor
// (hodos-update-helper.exe, 6b) AND the in-browser bootstrap/watchdog (6c/6e):
//
//   - apply.json      — the durable transaction state (ApplyRecord). Atomic
//                       temp+rename writes; the supervisor reads it cross-process
//                       and the watchdog reads it on the next cold boot to decide
//                       resume vs restore vs clean (M7 / V3-14).
//   - update-state.json — GLOBAL cross-profile state (UpdateState): silent/paused
//                       eligibility, the anti-rollback high-water, the installed
//                       signer thumbprint cache, last-failure + rescan flag (V3-7).
//   - *-manifest.json — a {relative-path -> sha256} content manifest (FileManifest),
//                       used for BOTH the rollback (old tree) backup-verify AND the
//                       build-time signed expected-new tree integrity check (B4/B5).
//
// Design intent (mirrors UpdateStager / ManifestFetcher): PURE LOGIC, no CEF, no
// process spawning, no globals. Serialize/parse are lenient (never throw; bad
// input -> {ok=false} or a default), so corrupt control files degrade safely
// rather than crashing the updater. The filesystem-mutating primitives (backup,
// restore, integrity-verify) live alongside the orchestration in commit 6b.2.

#pragma once

#include <map>
#include <string>

namespace hodos {

// apply.json transaction phase (V3 / M7). Ordered by progression so a watchdog
// can reason about "how far did we get". The string forms are the on-disk values.
enum class ApplyPhase {
    None,            // no/unreadable apply.json
    Preparing,       // bootstrap wrote it BEFORE arming RunOnce (V3-14)
    Armed,           // backup verified complete; installer not yet run
    Installing,      // installer spawned — power-loss here => RESTORE, never "clean" (M7)
    AwaitingHealth,  // installer done + new-tree integrity ok; P3 launched; awaiting healthy
    Healthy,         // the new build confirmed healthy (profile.lock + children + version)
    RolledBack,      // rollback completed
    Aborted          // a precondition / spawn failure; nothing was applied
};

const char* ApplyPhaseToString(ApplyPhase p);
ApplyPhase ApplyPhaseFromString(const std::string& s);  // unknown -> None

// apply.json — the durable transaction record. Paths are absolute (the helper
// runs in a different process with a different CWD, so nothing is relative).
struct ApplyRecord {
    int schema = 1;
    ApplyPhase phase = ApplyPhase::None;
    long fromBuild = 0;
    long toBuild = 0;
    std::string installerPath;            // …\pending\HodosBrowser-<v>-setup.exe
    std::string rollbackDir;              // …\pending\rollback
    std::string rollbackManifestPath;     // …\pending\rollback\manifest.json (old tree + DB)
    std::string expectedNewManifestPath;  // …\pending\expected-new-manifest.json (signed)
    std::string profileId;                // P0's resolved profile — for the health-probe relaunch (H1)
    std::string toVersion;                // human-readable target version (diagnostics/state)
    std::string signerThumbprint;         // the staged build's Authenticode signer (6c reads from the
                                          // marker); the helper writes it to update-state on success (I5)
    std::string stagedAt;                 // ISO-8601 UTC
    std::string failureReason;            // set on Aborted/RolledBack (diagnostics)
};

std::string SerializeApplyRecord(const ApplyRecord& r);
bool ParseApplyRecord(const std::string& json, ApplyRecord& out);  // false on bad JSON

// update-state.json — GLOBAL, cross-profile (V3-7). A MISSING file means "not
// eligible" (fail-safe-off) — callers must treat ReadUpdateState()==false that way.
// `signerThumbprint`/`highWaterBuild` are a CACHE; the authoritative trust root is
// Authenticode-verifying the live {app}\HodosBrowser.exe (H5), so a user editing
// this file can't forge signer-continuity or lower the rollback floor.
struct UpdateState {
    int schema = 1;
    bool silent = false;             // mirror of autoUpdateMode=="silent"
    bool paused = false;             // a failed silent apply sets this
    long highWaterBuild = 0;         // highest build ever confirmed healthy (anti-rollback cache)
    std::string signerThumbprint;    // installed-signer cache
    long lastFailureBuild = 0;
    std::string lastFailureReason;
    bool rescanAfterRollback = false;  // V3-4: old wallet rescans on-chain after a rollback
};

std::string SerializeUpdateState(const UpdateState& s);
bool ParseUpdateState(const std::string& json, UpdateState& out);  // false on bad JSON

// A content manifest: relative path (normalized to forward slashes, lower-cased on
// Windows for case-insensitive match) -> sha256 hex. Backs BOTH the rollback
// (old-tree) verify and the signed expected-new integrity check (B4/B5).
// `buildNumber` (6c.3 / review #2): for the SIGNED expected-new manifest, the target
// build number — bound into the EdDSA-signed bytes, so apply-time anti-rollback can
// trust it instead of the plaintext (attacker-writable) marker. 0 when unused
// (e.g. the local rollback manifest).
struct FileManifest {
    std::map<std::string, std::string> entries;  // relpath -> sha256-hex
    long buildNumber = 0;
};

std::string SerializeManifest(const FileManifest& m);
bool ParseManifest(const std::string& json, FileManifest& out);  // false on bad JSON

// Normalize a relative path for manifest keys: backslashes -> forward slashes,
// strip a leading "./" / "/", lower-case (Windows is case-insensitive). Pure.
std::string NormalizeManifestKey(const std::string& relPath);

}  // namespace hodos
