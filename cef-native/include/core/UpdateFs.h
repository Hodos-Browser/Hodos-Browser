// UpdateFs.h — filesystem primitives for the apply transaction (commit 6b.2a).
//
// AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3. The data-integrity heart of the silent
// apply/rollback: build + verify content manifests, copy the `{app}` tree for the
// rollback backup, restore it crash-atomically, and — the safety-critical one —
// restore the money DB as a FULL `{wallet.db, -wal, -shm}` set (V3-3a / I9).
//
// Pure-ish: filesystem + OpenSSL only; NO process spawning, NO CEF, NO globals.
// All paths are wide (std::wstring) so non-ASCII usernames in %LOCALAPPDATA% /
// %APPDATA% can't desync a hash or a copy. Non-throwing (std::filesystem with
// error_code internally); every function reports failure via its return value.
// Shared by the supervisor (6b) and the in-browser bootstrap (6c Phase A backup).

#pragma once
#ifdef _WIN32

#include <string>
#include <vector>

#include "UpdateApply.h"  // FileManifest

namespace hodos {
namespace updatefs {

struct VerifyResult {
    bool ok = false;
    std::string failedPath;  // first offending relpath (diagnostics)
    std::string reason;      // "" | "missing" | "sha-mismatch" | "read-error"
};

// SHA-256 hex of a file's bytes (OpenSSL, wide-path-safe). "" on open/read error.
std::string Sha256FileW(const std::wstring& path);

// Create dir + all missing parents (the RISK-A precondition: the `update\` subtree
// must exist before the first owner-lock Acquire / any write). True if it exists
// after the call.
bool EnsureDirExists(const std::wstring& dir);

// Build a {normalized-relpath -> sha256} manifest for every regular file under
// rootDir (recursive). Top-level subdirs whose name is in excludeDirNames are
// skipped entirely (e.g. "update", ".restore-tmp"). False on enumeration error.
bool BuildManifestForTree(const std::wstring& rootDir, FileManifest& out,
                          const std::vector<std::wstring>& excludeDirNames = {});

// Verify every manifest entry exists under rootDir with the recorded sha256.
// Does NOT reject EXTRA files (a superset tree passes — the manifest defines the
// REQUIRED set). Used for both the backup-complete check (M3) and the new-tree
// integrity gate against the signed expected-new manifest (B4).
VerifyResult VerifyTreeAgainstManifest(const std::wstring& rootDir, const FileManifest& m);

// Recursively copy every regular file under srcDir into dstDir, preserving the
// relative structure and creating dirs as needed. Overwrites existing files.
// Top-level subdirs in excludeDirNames are skipped. False on any copy failure.
bool CopyTreeRecursive(const std::wstring& srcDir, const std::wstring& dstDir,
                       const std::vector<std::wstring>& excludeDirNames = {});

// THE V3-3a money-DB restore primitive (I9). Restore the wallet DB as a FULL SET:
//   1. DELETE target <walletDir>\wallet.db-wal AND \wallet.db-shm FIRST — a
//      leftover NEW -wal would be replayed onto the restored OLD db by SQLite's
//      checksum-only (no db-identity) WAL recovery => funded-DB corruption.
//   2. Copy <snapshotDir>\wallet.db -> <walletDir>\wallet.db.
//   3. Copy <snapshotDir>\wallet.db-wal -> target IFF present in the snapshot
//      (after a graceful wallet exit the snapshot is just wallet.db, no -wal).
// Never copies -shm (regenerable; a stale one misleads recovery). Idempotent.
// False if the snapshot wallet.db is missing or a copy/delete fails.
bool RestoreWalletDbSet(const std::wstring& snapshotDir, const std::wstring& walletDir);

// Atomically replace dstPath with srcPath's content (rename-based; same-volume).
// Uses ReplaceFile when dst exists (atomic swap that preserves nothing we need),
// else MoveFileEx(REPLACE_EXISTING|WRITE_THROUGH). srcPath is consumed (moved).
// The caller orders the swaps so the HodosBrowser.exe+libcef.dll pair goes LAST.
bool SwapFileReplace(const std::wstring& srcPath, const std::wstring& dstPath);

// Free bytes on the volume hosting `anyPathOnVolume`. 0 on error.
unsigned long long FreeBytesOnVolume(const std::wstring& anyPathOnVolume);

// Sum of regular-file sizes under dir (recursive). 0 on error/empty.
unsigned long long DirSizeBytes(const std::wstring& dir);

}  // namespace updatefs
}  // namespace hodos

#endif  // _WIN32
