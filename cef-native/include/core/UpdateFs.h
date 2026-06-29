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

// Snapshot the money DB for the rollback backup (commit 6c.2, V3-1/V3-2). RAW copy
// of <walletDir>\wallet.db (required) + wallet.db-wal (only if present), with NO
// checkpoint (no legitimate opener — C++ opening the money DB violates CLAUDE.md #2)
// and NO -shm (regenerable; a stale one misleads recovery). The CALLER MUST have
// proven the wallet dead first (else a torn copy). Clears any stale snapshot
// -wal/-shm first (idempotent). The exact inverse of RestoreWalletDbSet. False if
// the source wallet.db is missing or a copy fails.
bool SnapshotWalletDbSet(const std::wstring& walletDir, const std::wstring& snapshotDir);

// Atomically replace dstPath with srcPath's content (rename-based; same-volume).
// Uses ReplaceFile when dst exists (atomic swap that preserves nothing we need),
// else MoveFileEx(REPLACE_EXISTING|WRITE_THROUGH). srcPath is consumed (moved).
// The caller orders the swaps so the HodosBrowser.exe+libcef.dll pair goes LAST.
bool SwapFileReplace(const std::wstring& srcPath, const std::wstring& dstPath);

// Free bytes on the volume hosting `anyPathOnVolume`. 0 on error.
unsigned long long FreeBytesOnVolume(const std::wstring& anyPathOnVolume);

// Sum of regular-file sizes under dir (recursive). 0 on error/empty.
unsigned long long DirSizeBytes(const std::wstring& dir);

// Atomically write `content` to `path`: write a sibling temp file, flush, then
// rename over `path` (SwapFileReplace). A reader cross-process never sees a
// half-written file (the M7 requirement for apply.json/update-state.json). The
// parent dir is created if missing. False on any failure.
bool WriteFileAtomic(const std::wstring& path, const std::string& content);

// Read an entire file into `out` (binary). False if it can't be opened.
bool ReadFileAll(const std::wstring& path, std::string& out);

// Recursively delete a directory + its contents (best-effort, non-throwing). True
// if the dir no longer exists afterward (incl. it never existing).
bool RemoveTree(const std::wstring& dir);

// Verify a detached Ed25519 signature (base64) over `data` using a raw-32-byte
// base64 public key. Pure (OpenSSL). False on ANY failure. Encoding is byte-for-byte
// identical to UpdateStager::VerifyEd25519 (raw-32 key, 64-byte sig, one-shot) so the
// same CI-produced key/signature verifies here; duplicated (not shared) to keep the
// helper free of UpdateStager's SyncHttpClient/Logger dependencies.
bool VerifyEd25519(const std::string& data, const std::string& signatureBase64,
                   const std::string& publicKeyBase64);

// Domain-separation prefix for the expected-new-manifest signature (commit 6b.3,
// V3-8). DISJOINT from the appcast ("hodos-appcast-v1\n") and installer signing
// domains. scripts/generate-tree-manifest.py MUST prepend these EXACT bytes before
// signing; the integrity gate verifies over (prefix || manifest-bytes).
const char* ManifestSignaturePrefix();  // "hodos-manifest-v1\n"

// The production Ed25519 public key (base64 raw-32) — the SAME key macOS Sparkle +
// the Windows appcast use (one key signs everything).
const char* PublicKeyBase64();

// Verify the expected-new-manifest's detached signature: VerifyEd25519 over
// (ManifestSignaturePrefix() || manifestBytes) with PublicKeyBase64(). Call this
// BEFORE trusting/parsing the manifest (fail-closed).
bool VerifyManifestSignature(const std::string& manifestBytes, const std::string& signatureBase64);

}  // namespace updatefs
}  // namespace hodos

#endif  // _WIN32
