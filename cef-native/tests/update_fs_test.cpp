// update_fs_test.cpp — unit tests for the apply-transaction filesystem primitives
// (commit 6b.2a). Temp-dir based. The headline is RestoreWalletDbSet (V3-3a): a
// rollback must restore the WHOLE {wallet.db,-wal,-shm} set and DELETE a stale
// new -wal/-shm at the target, or SQLite replays it onto the old db -> corruption.

// The code under test — hodos::updatefs in UpdateFs.{h,cpp} — is itself entirely
// #ifdef _WIN32 (the apply-transaction auto-updater is Windows-only; macOS updates
// via Sparkle). So there is nothing to exercise on macOS and this whole file is
// scoped to match the code it tests: an empty TU elsewhere. Test-only, HARNESS §6.
#ifdef _WIN32

#include "core/UpdateFs.h"
#include "core/UpdateStager.h"

#include <atomic>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

#include <gtest/gtest.h>
#include <openssl/evp.h>
#include <windows.h>

namespace fs = std::filesystem;
using namespace hodos::updatefs;

namespace {

std::atomic<unsigned> g_counter{0};

// A unique temp dir, auto-removed at end of scope.
struct TempDir {
    fs::path dir;
    TempDir() {
        const unsigned n = ++g_counter;
        dir = fs::temp_directory_path() /
              (L"hodos_updatefs_" + std::to_wstring(GetCurrentProcessId()) + L"_" + std::to_wstring(n));
        std::error_code ec;
        fs::remove_all(dir, ec);
        fs::create_directories(dir, ec);
    }
    ~TempDir() { std::error_code ec; fs::remove_all(dir, ec); }
    fs::path operator/(const wchar_t* sub) const { return dir / sub; }
};

void Write(const fs::path& p, const std::string& content) {
    std::error_code ec;
    fs::create_directories(p.parent_path(), ec);
    std::ofstream f(p, std::ios::binary | std::ios::trunc);
    f.write(content.data(), static_cast<std::streamsize>(content.size()));
}

std::string Read(const fs::path& p) {
    std::ifstream f(p, std::ios::binary);
    return std::string((std::istreambuf_iterator<char>(f)), std::istreambuf_iterator<char>());
}

bool Exists(const fs::path& p) { std::error_code ec; return fs::exists(p, ec); }

std::string B64(const unsigned char* d, size_t n) {
    if (!n) return "";
    std::string out(((n + 2) / 3) * 4 + 1, '\0');
    int w = EVP_EncodeBlock(reinterpret_cast<unsigned char*>(&out[0]), d, static_cast<int>(n));
    out.resize(w);
    return out;
}

}  // namespace

// ---- Sha256FileW ------------------------------------------------------------
TEST(Sha256FileW, KnownVectorAbc) {
    TempDir t;
    const fs::path p = t / L"abc.txt";
    Write(p, "abc");
    // SHA-256("abc")
    EXPECT_EQ(Sha256FileW(p.wstring()),
              "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

TEST(Sha256FileW, EmptyFileVector) {
    TempDir t;
    const fs::path p = t / L"empty";
    Write(p, "");
    EXPECT_EQ(Sha256FileW(p.wstring()),
              "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

TEST(Sha256FileW, MissingFileReturnsEmpty) {
    TempDir t;
    EXPECT_TRUE(Sha256FileW((t / L"nope").wstring()).empty());
}

// ---- RestoreWalletDbSet (V3-3a) — the headline ------------------------------
TEST(RestoreWalletDbSet, HardKillCase_RestoresFullSet_DeletesStaleShm) {
    TempDir snap, tgt;
    Write(snap / L"wallet.db", "OLD_DB");
    Write(snap / L"wallet.db-wal", "OLD_WAL");
    // Target = the unhealthy NEW build's leftovers after a taskkill.
    Write(tgt / L"wallet.db", "NEW_MIGRATED_DB");
    Write(tgt / L"wallet.db-wal", "NEW_DIRTY_WAL");
    Write(tgt / L"wallet.db-shm", "NEW_SHM");

    ASSERT_TRUE(RestoreWalletDbSet(snap.dir.wstring(), tgt.dir.wstring()));

    EXPECT_EQ(Read(tgt / L"wallet.db"), "OLD_DB");
    EXPECT_EQ(Read(tgt / L"wallet.db-wal"), "OLD_WAL");      // snapshot wal restored
    EXPECT_FALSE(Exists(tgt / L"wallet.db-shm"));            // stale shm deleted (regenerated on open)
}

TEST(RestoreWalletDbSet, GracefulCase_NoSnapshotWal_DeletesTargetWalAndShm) {
    // After a graceful wallet exit (checkpoint+TRUNCATE) the snapshot is just
    // wallet.db. The CRITICAL assertion: the new build's dirty -wal MUST be removed,
    // or it would be replayed onto the restored old db (V3-3a corruption).
    TempDir snap, tgt;
    Write(snap / L"wallet.db", "OLD_DB");                    // no snapshot -wal
    Write(tgt / L"wallet.db", "NEW_MIGRATED_DB");
    Write(tgt / L"wallet.db-wal", "NEW_DIRTY_WAL");          // stale, must vanish
    Write(tgt / L"wallet.db-shm", "NEW_SHM");

    ASSERT_TRUE(RestoreWalletDbSet(snap.dir.wstring(), tgt.dir.wstring()));

    EXPECT_EQ(Read(tgt / L"wallet.db"), "OLD_DB");
    EXPECT_FALSE(Exists(tgt / L"wallet.db-wal"));            // <-- the V3-3a fix
    EXPECT_FALSE(Exists(tgt / L"wallet.db-shm"));
}

TEST(RestoreWalletDbSet, MissingSnapshotDbReturnsFalse) {
    TempDir snap, tgt;
    Write(tgt / L"wallet.db", "NEW");
    EXPECT_FALSE(RestoreWalletDbSet(snap.dir.wstring(), tgt.dir.wstring()));
    EXPECT_EQ(Read(tgt / L"wallet.db"), "NEW");              // target untouched on failure
}

TEST(RestoreWalletDbSet, IsIdempotent) {
    TempDir snap, tgt;
    Write(snap / L"wallet.db", "OLD_DB");
    Write(snap / L"wallet.db-wal", "OLD_WAL");
    Write(tgt / L"wallet.db", "NEW");
    Write(tgt / L"wallet.db-wal", "DIRTY");

    ASSERT_TRUE(RestoreWalletDbSet(snap.dir.wstring(), tgt.dir.wstring()));
    ASSERT_TRUE(RestoreWalletDbSet(snap.dir.wstring(), tgt.dir.wstring()));  // again
    EXPECT_EQ(Read(tgt / L"wallet.db"), "OLD_DB");
    EXPECT_EQ(Read(tgt / L"wallet.db-wal"), "OLD_WAL");
}

// ---- SnapshotWalletDbSet (6c.2) — the backup inverse of RestoreWalletDbSet ----
TEST(SnapshotWalletDbSet, CopiesDbAndWal_NeverShm) {
    TempDir live, snap;
    Write(live / L"wallet.db", "LIVE_DB");
    Write(live / L"wallet.db-wal", "LIVE_WAL");
    Write(live / L"wallet.db-shm", "LIVE_SHM");  // must NOT be snapshotted

    ASSERT_TRUE(SnapshotWalletDbSet(live.dir.wstring(), snap.dir.wstring()));
    EXPECT_EQ(Read(snap / L"wallet.db"), "LIVE_DB");
    EXPECT_EQ(Read(snap / L"wallet.db-wal"), "LIVE_WAL");
    EXPECT_FALSE(Exists(snap / L"wallet.db-shm"));
}

TEST(SnapshotWalletDbSet, GracefulNoWal_SnapshotIsJustDb) {
    TempDir live, snap;
    Write(live / L"wallet.db", "LIVE_DB");  // no -wal (graceful checkpoint+truncate)
    ASSERT_TRUE(SnapshotWalletDbSet(live.dir.wstring(), snap.dir.wstring()));
    EXPECT_EQ(Read(snap / L"wallet.db"), "LIVE_DB");
    EXPECT_FALSE(Exists(snap / L"wallet.db-wal"));
}

TEST(SnapshotWalletDbSet, MissingSourceDbReturnsFalse) {
    TempDir live, snap;
    EXPECT_FALSE(SnapshotWalletDbSet(live.dir.wstring(), snap.dir.wstring()));
}

TEST(SnapshotWalletDbSet, StaleSnapshotWalClearedWhenSourceHasNone) {
    TempDir live, snap;
    Write(snap / L"wallet.db-wal", "STALE");   // a leftover from a prior snapshot
    Write(snap / L"wallet.db-shm", "STALE");
    Write(live / L"wallet.db", "NEW_DB");      // source has no -wal
    ASSERT_TRUE(SnapshotWalletDbSet(live.dir.wstring(), snap.dir.wstring()));
    EXPECT_EQ(Read(snap / L"wallet.db"), "NEW_DB");
    EXPECT_FALSE(Exists(snap / L"wallet.db-wal"));  // stale cleared
    EXPECT_FALSE(Exists(snap / L"wallet.db-shm"));
}

TEST(SnapshotWalletDbSet, RoundTripsThroughRestore) {
    // Snapshot a live DB, then restore the snapshot into a "new build" target that
    // has dirty new -wal/-shm — the full backup->restore cycle the apply path uses.
    TempDir live, snap, target;
    Write(live / L"wallet.db", "OLD_DB");
    Write(live / L"wallet.db-wal", "OLD_WAL");
    ASSERT_TRUE(SnapshotWalletDbSet(live.dir.wstring(), snap.dir.wstring()));

    Write(target / L"wallet.db", "MIGRATED");
    Write(target / L"wallet.db-wal", "DIRTY_NEW_WAL");
    Write(target / L"wallet.db-shm", "DIRTY_NEW_SHM");
    ASSERT_TRUE(RestoreWalletDbSet(snap.dir.wstring(), target.dir.wstring()));
    EXPECT_EQ(Read(target / L"wallet.db"), "OLD_DB");
    EXPECT_EQ(Read(target / L"wallet.db-wal"), "OLD_WAL");
    EXPECT_FALSE(Exists(target / L"wallet.db-shm"));  // V3-3a: stale new shm gone
}

// ---- BuildManifestForTree + VerifyTreeAgainstManifest -----------------------
TEST(Manifest, BuildThenVerifyRoundTrips) {
    TempDir t;
    Write(t / L"HodosBrowser.exe", "exe-bytes");
    Write(t / L"libcef.dll", "dll-bytes");
    Write(t / L"locales\\en-US.pak", "pak-bytes");

    hodos::FileManifest m;
    ASSERT_TRUE(BuildManifestForTree(t.dir.wstring(), m));
    EXPECT_EQ(m.entries.size(), 3u);
    EXPECT_EQ(m.entries.count("hodosbrowser.exe"), 1u);
    EXPECT_EQ(m.entries.count("locales/en-us.pak"), 1u);

    const VerifyResult r = VerifyTreeAgainstManifest(t.dir.wstring(), m);
    EXPECT_TRUE(r.ok) << r.reason << " " << r.failedPath;
}

TEST(Manifest, TamperedFileIsShaMismatch) {
    TempDir t;
    Write(t / L"a.bin", "original");
    hodos::FileManifest m;
    ASSERT_TRUE(BuildManifestForTree(t.dir.wstring(), m));
    Write(t / L"a.bin", "tampered!");  // same name, different bytes
    const VerifyResult r = VerifyTreeAgainstManifest(t.dir.wstring(), m);
    EXPECT_FALSE(r.ok);
    EXPECT_EQ(r.reason, "sha-mismatch");
}

TEST(Manifest, MissingFileIsMissing) {
    TempDir t;
    Write(t / L"a.bin", "x");
    Write(t / L"b.bin", "y");
    hodos::FileManifest m;
    ASSERT_TRUE(BuildManifestForTree(t.dir.wstring(), m));
    std::error_code ec; fs::remove(t / L"b.bin", ec);
    const VerifyResult r = VerifyTreeAgainstManifest(t.dir.wstring(), m);
    EXPECT_FALSE(r.ok);
    EXPECT_EQ(r.reason, "missing");
}

TEST(Manifest, ExtraFileStillVerifies) {
    // The manifest defines the REQUIRED set; an extra file on disk is fine.
    TempDir t;
    Write(t / L"a.bin", "x");
    hodos::FileManifest m;
    ASSERT_TRUE(BuildManifestForTree(t.dir.wstring(), m));
    Write(t / L"extra.bin", "z");
    EXPECT_TRUE(VerifyTreeAgainstManifest(t.dir.wstring(), m).ok);
}

TEST(Manifest, ExcludesTopLevelDir) {
    TempDir t;
    Write(t / L"a.bin", "x");
    Write(t / L"update\\pending\\junk", "ignore-me");
    hodos::FileManifest m;
    ASSERT_TRUE(BuildManifestForTree(t.dir.wstring(), m, {L"update"}));
    EXPECT_EQ(m.entries.count("a.bin"), 1u);
    for (const auto& kv : m.entries) {
        EXPECT_EQ(kv.first.rfind("update/", 0), std::string::npos);  // nothing under update/
    }
}

// ---- P0-A1: volatile artifacts must not abort the backup --------------------
//
// The defect these cover: Sha256FileW opens with FILE_SHARE_READ|FILE_SHARE_DELETE and
// NOT FILE_SHARE_WRITE, so hashing a file another process holds open for writing returns
// "" -- and BuildManifestForTree turns that into `return false`, which aborts the entire
// silent apply, silently. Any writer under {app} does it: an AV temp file, a crash dump,
// or (until this phase) our own 52 stray log writes.
//
// Each test observes BOTH halves in one run: the pre-fix call (excludeVolatileArtifacts
// = false) must FAIL, and only then does the post-fix call passing prove anything.

namespace {
// Hold a file open for writing WITHOUT FILE_SHARE_WRITE -- i.e. exactly what
// std::ofstream(path, ios::app) does, which is what the stray writes were.
struct ExclusiveWriter {
    HANDLE h = INVALID_HANDLE_VALUE;
    explicit ExclusiveWriter(const fs::path& p) {
        h = CreateFileW(p.wstring().c_str(), GENERIC_WRITE, FILE_SHARE_READ,
                        nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    }
    bool ok() const { return h != INVALID_HANDLE_VALUE; }
    ~ExclusiveWriter() { if (ok()) CloseHandle(h); }
};
}  // namespace

TEST(VolatileArtifacts, ClassifiesLogsTmpAndCrashpadOnly) {
    EXPECT_TRUE(IsVolatileArtifact(L"debug_output.log"));
    EXPECT_TRUE(IsVolatileArtifact(L"debug.log"));
    EXPECT_TRUE(IsVolatileArtifact(L"DEBUG.LOG"));            // case-insensitive
    EXPECT_TRUE(IsVolatileArtifact(L"startup_log.txt"));      // historical stray name
    EXPECT_TRUE(IsVolatileArtifact(L"something.tmp"));
    EXPECT_TRUE(IsVolatileArtifact(L"crashpad/reports/a.dmp"));
    EXPECT_TRUE(IsVolatileArtifact(std::wstring(L"crashpad") + wchar_t(92) + L"a.dmp"));

    // Everything {app} legitimately ships must NOT be excluded, or the backup manifest
    // would silently stop covering real files -- a far worse failure than the one fixed.
    EXPECT_FALSE(IsVolatileArtifact(L"HodosBrowser.exe"));
    EXPECT_FALSE(IsVolatileArtifact(L"libcef.dll"));
    EXPECT_FALSE(IsVolatileArtifact(L"resources.pak"));
    EXPECT_FALSE(IsVolatileArtifact(L"icudtl.dat"));
    EXPECT_FALSE(IsVolatileArtifact(L"v8_context_snapshot.bin"));
    EXPECT_FALSE(IsVolatileArtifact(L"locales/en-US.pak"));
    EXPECT_FALSE(IsVolatileArtifact(L"frontend/index.html"));
    EXPECT_FALSE(IsVolatileArtifact(L"update-state.json"));
    EXPECT_FALSE(IsVolatileArtifact(L"catalog.txt"));         // .txt is NOT blanket-excluded
}

TEST(Manifest, OpenLogAbortsWholeWalkUnlessVolatileExcluded) {
    TempDir t;
    Write(t / L"HodosBrowser.exe", "exe");
    Write(t / L"debug_output.log", "held open by the browser");

    ExclusiveWriter writer(t / L"debug_output.log");
    ASSERT_TRUE(writer.ok());

    // RED (the shipped behaviour): one open log aborts the manifest for the WHOLE tree.
    hodos::FileManifest before;
    EXPECT_FALSE(BuildManifestForTree(t.dir.wstring(), before, {L"update"}, false));

    // GREEN: with the volatile exclusion the walk completes and still covers real files.
    hodos::FileManifest after;
    ASSERT_TRUE(BuildManifestForTree(t.dir.wstring(), after, {L"update"}, true));
    EXPECT_EQ(after.entries.count("hodosbrowser.exe"), 1u);   // keys are lower-cased
    EXPECT_EQ(after.entries.count("debug_output.log"), 0u);
}

TEST(CopyTreeRecursive, StaleLogIsNotCarriedIntoTheBackup) {
    TempDir src, dst;
    Write(src / L"HodosBrowser.exe", "exe");
    Write(src / L"debug_output.log", "held open");

    ExclusiveWriter writer(src / L"debug_output.log");
    ASSERT_TRUE(writer.ok());

    // RED: WITHOUT the exclusion the stale log IS copied into the backup, so a rollback
    // restores it. (The copy itself succeeds -- unlike the manifest walk above. Only
    // Sha256FileW omits FILE_SHARE_WRITE; copy_file opens the source permissively.)
    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dst.dir.wstring(), {L"update"}, false));
    EXPECT_TRUE(Exists(dst / L"debug_output.log"));

    TempDir dst2;
    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dst2.dir.wstring(), {L"update"}, true));
    EXPECT_TRUE(Exists(dst2 / L"HodosBrowser.exe"));
    // A stale log must not be carried into the backup -- a rollback would restore it.
    EXPECT_FALSE(Exists(dst2 / L"debug_output.log"));
}

// R-UPDATE: the backup manifest and the rollback copy must agree
//
// THE risk P0-A1 introduced. The silent apply builds a manifest of {app} and copies
// {app} to rollback\ -- two separate walks, now BOTH filtering volatile artifacts. If
// they ever disagree about what to skip, the manifest describes files the backup does
// not contain, and VerifyTreeAgainstManifest fails at exactly the worst moment: during
// a rollback, after the new build has already been judged unhealthy.

TEST(BackupRoundTrip, ManifestAndCopyAgreeWithVolatileExclusion) {
    TempDir src, dst;
    Write(src / L"HodosBrowser.exe", "exe");
    Write(src / L"libcef.dll", "cef");
    Write(src / L"locales/en-US.pak", "pak");
    Write(src / L"frontend/index.html", "html");
    Write(src / L"debug_output.log", "volatile");
    Write(src / L"debug.log", "volatile");
    Write(src / L"startup_log.txt", "volatile");
    Write(src / L"update/pending/big.exe", "excluded-dir");

    hodos::FileManifest m;
    ASSERT_TRUE(BuildManifestForTree(src.dir.wstring(), m, {L"update"}, true));
    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dst.dir.wstring(), {L"update"}, true));

    // Every manifest entry must exist in the COPY with a matching hash. This is the
    // check the real rollback runs against rollback\.
    VerifyResult r = VerifyTreeAgainstManifest(dst.dir.wstring(), m);
    EXPECT_TRUE(r.ok);

    // And the real files are actually covered -- an empty manifest would also "verify".
    EXPECT_EQ(m.entries.count("hodosbrowser.exe"), 1u);
    EXPECT_EQ(m.entries.count("libcef.dll"), 1u);
    EXPECT_EQ(m.entries.count("locales/en-us.pak"), 1u);
    EXPECT_EQ(m.entries.count("frontend/index.html"), 1u);
    EXPECT_EQ(m.entries.size(), 4u);
}

TEST(BackupRoundTrip, RedHalfMismatchedFiltersFailVerification) {
    // RED: filter the manifest but NOT the copy and it still verifies (the copy is a
    // superset -- VerifyTreeAgainstManifest tolerates extras by design). Filter the
    // COPY but not the manifest and verification MUST fail, because the manifest then
    // lists volatile files the backup does not contain. That asymmetry is why both
    // call sites in cef_browser_shell.cpp pass the same flag.
    TempDir src, dstA, dstB;
    Write(src / L"HodosBrowser.exe", "exe");
    Write(src / L"debug_output.log", "volatile");

    hodos::FileManifest filtered, unfiltered;
    ASSERT_TRUE(BuildManifestForTree(src.dir.wstring(), filtered, {}, true));
    ASSERT_TRUE(BuildManifestForTree(src.dir.wstring(), unfiltered, {}, false));
    EXPECT_EQ(filtered.entries.size(), 1u);
    EXPECT_EQ(unfiltered.entries.size(), 2u);

    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dstA.dir.wstring(), {}, false));
    EXPECT_TRUE(VerifyTreeAgainstManifest(dstA.dir.wstring(), filtered).ok);   // superset: fine

    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dstB.dir.wstring(), {}, true));
    EXPECT_FALSE(VerifyTreeAgainstManifest(dstB.dir.wstring(), unfiltered).ok); // subset: FAILS
}

// ---- CopyTreeRecursive ------------------------------------------------------
TEST(CopyTreeRecursive, CopiesAllPreservingStructure) {
    TempDir src, dst;
    Write(src / L"HodosBrowser.exe", "exe");
    Write(src / L"locales\\en-US.pak", "pak");
    Write(src / L"frontend\\index.html", "html");

    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dst.dir.wstring()));
    EXPECT_EQ(Read(dst / L"HodosBrowser.exe"), "exe");
    EXPECT_EQ(Read(dst / L"locales\\en-US.pak"), "pak");
    EXPECT_EQ(Read(dst / L"frontend\\index.html"), "html");
}

TEST(CopyTreeRecursive, SkipsExcludedTopLevelDir) {
    TempDir src, dst;
    Write(src / L"keep.bin", "k");
    Write(src / L"update\\pending\\big.exe", "skip");
    ASSERT_TRUE(CopyTreeRecursive(src.dir.wstring(), dst.dir.wstring(), {L"update"}));
    EXPECT_TRUE(Exists(dst / L"keep.bin"));
    EXPECT_FALSE(Exists(dst / L"update"));
}

// ---- SwapFileReplace --------------------------------------------------------
TEST(SwapFileReplace, ReplacesExistingTarget) {
    TempDir t;
    Write(t / L"src.tmp", "NEWCONTENT");
    Write(t / L"dst.exe", "OLDCONTENT");
    ASSERT_TRUE(SwapFileReplace((t / L"src.tmp").wstring(), (t / L"dst.exe").wstring()));
    EXPECT_EQ(Read(t / L"dst.exe"), "NEWCONTENT");
    EXPECT_FALSE(Exists(t / L"src.tmp"));  // src consumed (moved)
}

TEST(SwapFileReplace, MovesWhenTargetAbsent) {
    TempDir t;
    Write(t / L"src.tmp", "DATA");
    ASSERT_TRUE(SwapFileReplace((t / L"src.tmp").wstring(), (t / L"dst.exe").wstring()));
    EXPECT_EQ(Read(t / L"dst.exe"), "DATA");
}

// ---- DirSizeBytes / EnsureDirExists -----------------------------------------
TEST(DirSizeBytes, SumsRegularFiles) {
    TempDir t;
    Write(t / L"a", "12345");          // 5
    Write(t / L"sub\\b", "678");       // 3
    EXPECT_EQ(DirSizeBytes(t.dir.wstring()), 8u);
}

TEST(RemoveTree, DeletesRecursivelyAndTolueratesAbsent) {
    TempDir t;
    Write(t / L"a\\b\\c.txt", "x");
    Write(t / L"a\\d.txt", "y");
    ASSERT_TRUE(Exists(t / L"a"));
    EXPECT_TRUE(RemoveTree((t / L"a").wstring()));
    EXPECT_FALSE(Exists(t / L"a"));
    EXPECT_TRUE(RemoveTree((t / L"nonexistent").wstring()));  // absent -> true
}

TEST(EnsureDirExists, CreatesNestedAndIsIdempotent) {
    TempDir t;
    const fs::path nested = t / L"a\\b\\c";
    ASSERT_TRUE(EnsureDirExists(nested.wstring()));
    EXPECT_TRUE(Exists(nested));
    EXPECT_TRUE(EnsureDirExists(nested.wstring()));  // idempotent
}

// ---- VerifyEd25519 / VerifyManifestSignature (6b.3) -------------------------
TEST(VerifyEd25519, RoundTripAndTamper) {
    // Generate an Ed25519 keypair, sign a message, and round-trip through the
    // same base64/raw-32 encoding the CI signer uses.
    EVP_PKEY* pkey = nullptr;
    EVP_PKEY_CTX* c = EVP_PKEY_CTX_new_id(EVP_PKEY_ED25519, nullptr);
    ASSERT_TRUE(c);
    ASSERT_EQ(EVP_PKEY_keygen_init(c), 1);
    ASSERT_EQ(EVP_PKEY_keygen(c, &pkey), 1);
    EVP_PKEY_CTX_free(c);

    unsigned char pub[32]; size_t publen = sizeof(pub);
    ASSERT_EQ(EVP_PKEY_get_raw_public_key(pkey, pub, &publen), 1);
    ASSERT_EQ(publen, 32u);
    const std::string pubB64 = B64(pub, 32);

    const std::string msg = std::string(ManifestSignaturePrefix()) + "{\"files\":{\"a\":\"bb\"}}";
    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    ASSERT_EQ(EVP_DigestSignInit(ctx, nullptr, nullptr, nullptr, pkey), 1);
    size_t siglen = 0;
    EVP_DigestSign(ctx, nullptr, &siglen, reinterpret_cast<const unsigned char*>(msg.data()), msg.size());
    std::vector<unsigned char> sig(siglen);
    ASSERT_EQ(EVP_DigestSign(ctx, sig.data(), &siglen,
                             reinterpret_cast<const unsigned char*>(msg.data()), msg.size()), 1);
    EVP_MD_CTX_free(ctx);
    EVP_PKEY_free(pkey);
    const std::string sigB64 = B64(sig.data(), siglen);

    EXPECT_TRUE(VerifyEd25519(msg, sigB64, pubB64));
    EXPECT_FALSE(VerifyEd25519(msg + "x", sigB64, pubB64));   // tampered data
    EXPECT_FALSE(VerifyEd25519(msg, sigB64, "QUJD"));         // wrong/short key
    EXPECT_FALSE(VerifyEd25519(msg, "QUJD", pubB64));         // bad signature
}

TEST(VerifyManifestSignature, FailsClosedWithoutProdKey) {
    // We don't have the production private key, so any signature must fail —
    // proving the gate is fail-closed (no accidental accept).
    EXPECT_FALSE(VerifyManifestSignature("{\"files\":{}}", "QUJD"));
}

TEST(ManifestSignaturePrefix, IsStableAndDomainSeparated) {
    EXPECT_STREQ(ManifestSignaturePrefix(), "hodos-manifest-v1\n");
    EXPECT_STRNE(ManifestSignaturePrefix(), "hodos-appcast-v1\n");  // disjoint from appcast
}

TEST(EmbeddedKey, StagerAndFsPublicKeysMatch) {
    // The embedded Ed25519 pubkey is duplicated in UpdateStager (appcast/installer)
    // and UpdateFs (manifest) to keep the helper free of UpdateStager. They MUST stay
    // identical, or a CI-signed manifest verifies in one place and not the other
    // (review 6c.3 LOW — catches a one-sided key rotation at test time).
    EXPECT_STREQ(hodos::UpdateStager::PublicKeyBase64(), hodos::updatefs::PublicKeyBase64());
}

#endif  // _WIN32
