// UpdateFs.cpp — filesystem primitives for the apply transaction (commit 6b.2a).
// See UpdateFs.h. std::filesystem (error_code, non-throwing) + OpenSSL + Win32.

#include "../../include/core/UpdateFs.h"

#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <cstdlib>
#include <filesystem>
#include <vector>

#include <openssl/evp.h>

namespace fs = std::filesystem;

namespace {
// base64 decode (standard alphabet, padding-tolerant) — mirrors UpdateStager's.
bool Base64Decode(const std::string& in, std::vector<unsigned char>& out) {
    auto val = [](unsigned char c) -> int {
        if (c >= 'A' && c <= 'Z') return c - 'A';
        if (c >= 'a' && c <= 'z') return c - 'a' + 26;
        if (c >= '0' && c <= '9') return c - '0' + 52;
        if (c == '+') return 62;
        if (c == '/') return 63;
        return -1;
    };
    out.clear();
    int accum = 0, nbits = 0;
    for (unsigned char c : in) {
        if (c == '\n' || c == '\r' || c == ' ' || c == '\t') continue;
        if (c == '=') break;
        int v = val(c);
        if (v < 0) return false;
        accum = (accum << 6) | v;
        nbits += 6;
        if (nbits >= 8) { nbits -= 8; out.push_back((unsigned char)((accum >> nbits) & 0xFF)); }
    }
    return true;
}
}  // namespace

namespace hodos {
namespace updatefs {

namespace {
// Wide relpath -> UTF-8 narrow (for manifest keys; non-ASCII-safe).
std::string WideToUtf8(const std::wstring& w) {
    if (w.empty()) return "";
    int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, nullptr, 0, nullptr, nullptr);
    std::string s(n > 0 ? n - 1 : 0, '\0');
    if (n > 0) WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, &s[0], n, nullptr, nullptr);
    return s;
}

// First path component of a relative path (for the top-level exclude check).
bool IsExcludedTopLevel(const fs::path& rel, const std::vector<std::wstring>& excl) {
    if (rel.empty()) return false;
    const std::wstring first = rel.begin()->wstring();
    for (const auto& e : excl) if (first == e) return true;
    return false;
}
}  // namespace

std::string Sha256FileW(const std::wstring& path) {
    HANDLE h = CreateFileW(path.c_str(), GENERIC_READ,
                           FILE_SHARE_READ | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return "";

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) { CloseHandle(h); return ""; }
    if (EVP_DigestInit_ex(ctx, EVP_sha256(), nullptr) != 1) {
        EVP_MD_CTX_free(ctx); CloseHandle(h); return "";
    }

    bool ok = true;
    unsigned char buf[64 * 1024];
    for (;;) {
        DWORD got = 0;
        if (!ReadFile(h, buf, sizeof(buf), &got, nullptr)) { ok = false; break; }
        if (got == 0) break;  // EOF
        if (EVP_DigestUpdate(ctx, buf, got) != 1) { ok = false; break; }
    }
    CloseHandle(h);

    std::string hex;
    if (ok) {
        unsigned char md[EVP_MAX_MD_SIZE];
        unsigned int mdLen = 0;
        if (EVP_DigestFinal_ex(ctx, md, &mdLen) == 1) {
            static const char* k = "0123456789abcdef";
            hex.reserve(mdLen * 2);
            for (unsigned int i = 0; i < mdLen; ++i) {
                hex.push_back(k[md[i] >> 4]);
                hex.push_back(k[md[i] & 0xF]);
            }
        }
    }
    EVP_MD_CTX_free(ctx);
    return hex;
}

bool EnsureDirExists(const std::wstring& dir) {
    std::error_code ec;
    if (fs::exists(dir, ec)) return fs::is_directory(dir, ec);
    fs::create_directories(dir, ec);
    return !ec && fs::is_directory(dir, ec);
}

bool BuildManifestForTree(const std::wstring& rootDir, FileManifest& out,
                          const std::vector<std::wstring>& excludeDirNames) {
    out.entries.clear();
    std::error_code ec;
    if (!fs::is_directory(rootDir, ec)) return false;
    const fs::path root(rootDir);

    fs::recursive_directory_iterator it(root, fs::directory_options::skip_permission_denied, ec);
    if (ec) return false;
    for (fs::recursive_directory_iterator end; it != end; it.increment(ec)) {
        if (ec) return false;
        const fs::path& p = it->path();
        const fs::path rel = fs::relative(p, root, ec);
        if (ec) continue;
        if (it->is_directory(ec)) {
            if (IsExcludedTopLevel(rel, excludeDirNames)) it.disable_recursion_pending();
            continue;
        }
        if (!it->is_regular_file(ec)) continue;
        if (IsExcludedTopLevel(rel, excludeDirNames)) continue;
        const std::string sha = Sha256FileW(p.wstring());
        if (sha.empty()) return false;  // unreadable file => fail (don't ship a partial manifest)
        out.entries[NormalizeManifestKey(WideToUtf8(rel.wstring()))] = sha;
    }
    return true;
}

VerifyResult VerifyTreeAgainstManifest(const std::wstring& rootDir, const FileManifest& m) {
    VerifyResult r;
    const fs::path root(rootDir);
    for (const auto& kv : m.entries) {
        // kv.first is a normalized (forward-slash) relpath; rebuild a wide path.
        std::wstring relW;
        {
            int n = MultiByteToWideChar(CP_UTF8, 0, kv.first.c_str(), -1, nullptr, 0);
            relW.assign(n > 0 ? n - 1 : 0, L'\0');
            if (n > 0) MultiByteToWideChar(CP_UTF8, 0, kv.first.c_str(), -1, &relW[0], n);
        }
        const fs::path full = root / fs::path(relW);
        std::error_code ec;
        if (!fs::exists(full, ec) || !fs::is_regular_file(full, ec)) {
            r.ok = false; r.failedPath = kv.first; r.reason = "missing"; return r;
        }
        const std::string sha = Sha256FileW(full.wstring());
        if (sha.empty()) { r.ok = false; r.failedPath = kv.first; r.reason = "read-error"; return r; }
        if (sha != kv.second) { r.ok = false; r.failedPath = kv.first; r.reason = "sha-mismatch"; return r; }
    }
    r.ok = true;
    return r;
}

bool CopyTreeRecursive(const std::wstring& srcDir, const std::wstring& dstDir,
                       const std::vector<std::wstring>& excludeDirNames) {
    std::error_code ec;
    if (!fs::is_directory(srcDir, ec)) return false;
    if (!EnsureDirExists(dstDir)) return false;
    const fs::path src(srcDir), dst(dstDir);

    fs::recursive_directory_iterator it(src, fs::directory_options::skip_permission_denied, ec);
    if (ec) return false;
    for (fs::recursive_directory_iterator end; it != end; it.increment(ec)) {
        if (ec) return false;
        const fs::path& p = it->path();
        const fs::path rel = fs::relative(p, src, ec);
        if (ec) return false;
        if (it->is_directory(ec)) {
            if (IsExcludedTopLevel(rel, excludeDirNames)) { it.disable_recursion_pending(); continue; }
            fs::create_directories(dst / rel, ec);
            if (ec) return false;
            continue;
        }
        if (!it->is_regular_file(ec)) continue;
        if (IsExcludedTopLevel(rel, excludeDirNames)) continue;
        const fs::path target = dst / rel;
        fs::create_directories(target.parent_path(), ec);
        fs::copy_file(p, target, fs::copy_options::overwrite_existing, ec);
        if (ec) return false;
    }
    return true;
}

bool RestoreWalletDbSet(const std::wstring& snapshotDir, const std::wstring& walletDir) {
    std::error_code ec;
    const fs::path snap(snapshotDir), wdir(walletDir);
    const fs::path snapDb = snap / L"wallet.db";
    const fs::path snapWal = snap / L"wallet.db-wal";
    const fs::path tgtDb = wdir / L"wallet.db";
    const fs::path tgtWal = wdir / L"wallet.db-wal";
    const fs::path tgtShm = wdir / L"wallet.db-shm";

    if (!fs::exists(snapDb, ec)) return false;  // nothing to restore from
    if (!EnsureDirExists(walletDir)) return false;

    // 1. DELETE the target -wal and -shm FIRST (V3-3a): a leftover NEW -wal would
    //    be replayed onto the restored OLD db by checksum-only WAL recovery.
    fs::remove(tgtWal, ec);  // ec ignored: absent is fine
    fs::remove(tgtShm, ec);

    // 2. Copy the snapshot db over the target.
    fs::copy_file(snapDb, tgtDb, fs::copy_options::overwrite_existing, ec);
    if (ec) return false;

    // 3. Copy the snapshot -wal ONLY IF present (hard-kill snapshot). After a
    //    graceful wallet exit the snapshot is just wallet.db -> target -wal stays
    //    deleted from step 1. Never restore -shm.
    if (fs::exists(snapWal, ec)) {
        fs::copy_file(snapWal, tgtWal, fs::copy_options::overwrite_existing, ec);
        if (ec) return false;
    }
    return true;
}

bool SnapshotWalletDbSet(const std::wstring& walletDir, const std::wstring& snapshotDir) {
    std::error_code ec;
    const fs::path wdir(walletDir), snap(snapshotDir);
    const fs::path srcDb = wdir / L"wallet.db";
    const fs::path srcWal = wdir / L"wallet.db-wal";
    const fs::path dstDb = snap / L"wallet.db";
    const fs::path dstWal = snap / L"wallet.db-wal";
    const fs::path dstShm = snap / L"wallet.db-shm";

    if (!fs::exists(srcDb, ec)) return false;  // nothing to snapshot
    if (!EnsureDirExists(snapshotDir)) return false;

    // Clear any stale snapshot -wal/-shm so a re-run can't leave a -wal that doesn't
    // belong to the freshly-copied db (idempotent; symmetric with RestoreWalletDbSet).
    fs::remove(dstWal, ec);
    fs::remove(dstShm, ec);

    fs::copy_file(srcDb, dstDb, fs::copy_options::overwrite_existing, ec);
    if (ec) return false;
    if (fs::exists(srcWal, ec)) {
        fs::copy_file(srcWal, dstWal, fs::copy_options::overwrite_existing, ec);
        if (ec) return false;
    }
    return true;
}

bool SwapFileReplace(const std::wstring& srcPath, const std::wstring& dstPath) {
    std::error_code ec;
    if (fs::exists(dstPath, ec)) {
        // Atomic same-volume replace; ignore the backup slot (nullptr).
        if (ReplaceFileW(dstPath.c_str(), srcPath.c_str(), nullptr,
                         REPLACEFILE_IGNORE_MERGE_ERRORS, nullptr, nullptr)) {
            return true;
        }
        // Fall through to MoveFileEx if ReplaceFile failed (e.g. dst on a path
        // ReplaceFile dislikes); MoveFileEx with REPLACE_EXISTING is the fallback.
    }
    return MoveFileExW(srcPath.c_str(), dstPath.c_str(),
                       MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) != 0;
}

unsigned long long FreeBytesOnVolume(const std::wstring& anyPathOnVolume) {
    // GetDiskFreeSpaceExW wants a directory; use the parent if a file path slips in.
    std::error_code ec;
    std::wstring dir = anyPathOnVolume;
    if (fs::exists(anyPathOnVolume, ec) && !fs::is_directory(anyPathOnVolume, ec)) {
        dir = fs::path(anyPathOnVolume).parent_path().wstring();
    }
    ULARGE_INTEGER freeAvail{};
    if (GetDiskFreeSpaceExW(dir.c_str(), &freeAvail, nullptr, nullptr)) {
        return freeAvail.QuadPart;
    }
    return 0;
}

bool WriteFileAtomic(const std::wstring& path, const std::string& content) {
    const fs::path target(path);
    std::error_code ec;
    fs::create_directories(target.parent_path(), ec);  // ec ignored: may already exist
    const fs::path tmp = target.parent_path() / (target.filename().wstring() + L".tmp");

    // Write + flush the temp file via Win32 so we can FlushFileBuffers before rename.
    HANDLE h = CreateFileW(tmp.wstring().c_str(), GENERIC_WRITE, 0, nullptr,
                           CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    bool ok = true;
    size_t off = 0;
    while (off < content.size()) {
        DWORD toWrite = static_cast<DWORD>((content.size() - off > 0x10000000)
                                               ? 0x10000000 : (content.size() - off));
        DWORD wrote = 0;
        if (!WriteFile(h, content.data() + off, toWrite, &wrote, nullptr) || wrote == 0) {
            ok = false; break;
        }
        off += wrote;
    }
    if (ok) FlushFileBuffers(h);
    CloseHandle(h);
    if (!ok) { fs::remove(tmp, ec); return false; }

    if (!SwapFileReplace(tmp.wstring(), path)) { fs::remove(tmp, ec); return false; }
    return true;
}

bool ReadFileAll(const std::wstring& path, std::string& out) {
    HANDLE h = CreateFileW(path.c_str(), GENERIC_READ,
                           FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    out.clear();
    char buf[64 * 1024];
    DWORD got = 0;
    while (ReadFile(h, buf, sizeof(buf), &got, nullptr) && got > 0) out.append(buf, got);
    CloseHandle(h);
    return true;
}

bool RemoveTree(const std::wstring& dir) {
    std::error_code ec;
    fs::remove_all(dir, ec);
    return !fs::exists(dir, ec);
}

bool VerifyEd25519(const std::string& data, const std::string& signatureBase64,
                   const std::string& publicKeyBase64) {
    std::vector<unsigned char> pub, sig;
    if (!Base64Decode(publicKeyBase64, pub) || pub.size() != 32) return false;
    if (!Base64Decode(signatureBase64, sig) || sig.size() != 64) return false;

    EVP_PKEY* pkey = EVP_PKEY_new_raw_public_key(EVP_PKEY_ED25519, nullptr, pub.data(), pub.size());
    if (!pkey) return false;
    bool ok = false;
    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (ctx) {
        if (EVP_DigestVerifyInit(ctx, nullptr, nullptr, nullptr, pkey) == 1) {
            int rc = EVP_DigestVerify(ctx, sig.data(), sig.size(),
                                      reinterpret_cast<const unsigned char*>(data.data()), data.size());
            ok = (rc == 1);
        }
        EVP_MD_CTX_free(ctx);
    }
    EVP_PKEY_free(pkey);
    return ok;
}

const char* ManifestSignaturePrefix() { return "hodos-manifest-v1\n"; }

const char* PublicKeyBase64() {
    // Same Ed25519 key as macOS Sparkle (SUPublicEDKey) + the Windows appcast.
    return "GVq3mpDl8eelsG0A5wqC5FBYZd3fy7U3we9iZ9+Tq3Q=";
}

bool VerifyManifestSignature(const std::string& manifestBytes, const std::string& signatureBase64) {
    std::string pub = PublicKeyBase64();
#ifdef HODOS_UPDATE_TEST_SEAM
    // TEST-BUILD ONLY (compiled OUT of production via the HODOS_UPDATE_TEST_SEAM
    // CMake option): let the localhost rig verify a manifest it signed with its
    // throwaway key. The signature check stays a REAL hard gate — just against the
    // rig key instead of the embedded production key. No env override exists in a
    // shipped build, so a production install can't be tricked into a weaker key.
    {
        const char* t = std::getenv("HODOS_UPDATE_TEST");
        if (t && std::string(t) == "1") {
            const char* tp = std::getenv("HODOS_UPDATE_TEST_PUBKEY");
            if (tp && *tp) pub = tp;
        }
    }
#endif
    return VerifyEd25519(std::string(ManifestSignaturePrefix()) + manifestBytes, signatureBase64, pub);
}

unsigned long long DirSizeBytes(const std::wstring& dir) {
    std::error_code ec;
    if (!fs::is_directory(dir, ec)) return 0;
    unsigned long long total = 0;
    fs::recursive_directory_iterator it(dir, fs::directory_options::skip_permission_denied, ec);
    if (ec) return 0;
    for (fs::recursive_directory_iterator end; it != end; it.increment(ec)) {
        if (ec) break;
        if (it->is_regular_file(ec)) {
            const auto sz = it->file_size(ec);
            if (!ec) total += sz;
        }
    }
    return total;
}

}  // namespace updatefs
}  // namespace hodos

#endif  // _WIN32
