// UpdateStager.h — Windows auto-update: Hodos-owned download → stage → verify.
//
// WINDOWS_AUTOUPDATE_PLAN.md commit 4. Hodos drives its OWN appcast fetch +
// installer download + verification rather than letting WinSparkle install
// mid-session. The verified installer + marker are staged to
// %LOCALAPPDATA%\HodosBrowser\pending\; commit 6 applies them on next cold boot.
//
// Design intent (mirrors ManifestFetcher / the project's pure-logic test style):
//   - PURE LOGIC where possible. ParseWindowsAppcastItem, VerifyEd25519,
//     IsNewerBuild, Sha256/marker (de)serialize are all pure and unit-tested
//     with no network and no filesystem.
//   - FOUR VERIFY GATES before staging (§B.4): Ed25519 signature, Authenticode
//     signer-continuity (Windows), integer-monotonic anti-rollback, and the
//     whole-appcast-document signature (anti-replay, added in commit 4c).
//   - FAIL-CLOSED. Any verification or transport failure → reject, delete the
//     partial stage, write no marker. Never stage unverified bytes.
//   - INERT UNDER HODOS_DEV. The orchestrator no-ops when HODOS_DEV=1 (so dev
//     builds never touch the staging dir) UNLESS HODOS_UPDATE_TEST=1 is set.
//   - TEST SEAMS ARE COMPILE-TIME. The test-key override + Authenticode-advisory
//     relaxations exist ONLY when HODOS_UPDATE_TEST_SEAM is defined (the
//     hodos_tests target). They are compiled OUT of the shipped browser, so no
//     production install can be tricked via the environment into a weakened
//     verify. EdDSA + Authenticode are unconditional in production.
//
// Crypto: OpenSSL (already linked) for Ed25519 verify + SHA-256.
// Authenticode: WinVerifyTrust + WinTrust (Windows only).

#pragma once

#include <string>

namespace hodos {

// The Windows <item> parsed out of a Sparkle appcast. `valid == false` means
// no usable Windows enclosure was found (the common no-update / parse-fail case).
struct AppcastEntry {
    bool valid = false;
    std::string version;        // human-readable (sparkle:version string on Windows)
    long buildNumber = 0;       // Hodos-read monotonic integer (sparkle:hodosBuildNumber).
                                // 0 = absent → treated as "cannot compare" (reject).
    std::string enclosureUrl;   // installer download URL (https)
    long long enclosureSize = 0;
    std::string edSignature;    // base64 Ed25519 signature over the installer bytes
};

// The staged-update marker written to pending\update-info.json (§C.3). Its
// presence + a matching, fully-verified installer IS the arm signal commit 6
// reads. signerThumbprint feeds the §H.6 signer-continuity auto-degrade gate.
struct StagedUpdateMarker {
    long buildNumber = 0;
    std::string version;
    std::string installerFileName;
    std::string sha256;                 // hex of the staged installer bytes
    bool edVerified = false;
    bool authenticodeVerified = false;
    std::string signer;
    std::string signerThumbprint;
    std::string stagedAt;               // ISO-8601 UTC
};

enum class StageResult {
    Staged,         // a newer, fully-verified installer is now staged + markered
    UpToDate,       // feed offers nothing newer than the running build
    NoUpdate,       // feed had no usable Windows item
    Skipped,        // inert (HODOS_DEV without the test seam)
    NetworkFailed,  // appcast fetch or installer download failed
    VerifyFailed    // a verify gate rejected the candidate (fail-closed)
};

class UpdateStager {
public:
    // ---- Orchestrator (network + filesystem) ----------------------------------
    // Fetch the appcast, anti-rollback-check the Windows item against
    // currentBuildNumber, download the installer into pendingDir, run the verify
    // gates, and (on success) write the marker. Idempotent: clears a stale stage,
    // skips re-download if a same-or-newer verified marker already exists.
    // Inert (returns Skipped) under HODOS_DEV unless HODOS_UPDATE_TEST=1.
    // Wired into startup in commit 4d; unused until then.
    static StageResult StagePendingUpdate(const std::string& appcastUrl,
                                          const std::string& pendingDir,
                                          long currentBuildNumber);

    // ---- Pure / unit-testable pieces ------------------------------------------
    // Parse the Windows <item> from a Sparkle appcast XML. Lenient — returns
    // {valid=false} on any malformation. No network, no throw.
    static AppcastEntry ParseWindowsAppcastItem(const std::string& xml);

    // Verify a detached Ed25519 signature (base64) over `data` using the raw
    // 32-byte base64 public key. Pure (OpenSSL). False on ANY failure (bad
    // base64, wrong key length, bad signature). Ed25519 is one-shot, so `data`
    // must be the full message (e.g. the whole installer bytes).
    static bool VerifyEd25519(const std::string& data,
                              const std::string& signatureBase64,
                              const std::string& publicKeyBase64);

    // Integer-monotonic anti-rollback: a candidate must be STRICTLY newer.
    // Refuses equal or lower, and refuses a candidate of 0 (absent build number).
    static bool IsNewerBuild(long candidateBuildNumber, long currentBuildNumber);

    // SHA-256 hex of a file's bytes (OpenSSL). Empty string on read failure.
    static std::string Sha256File(const std::string& path);

    // Marker (update-info.json) (de)serialize. Pure JSON (nlohmann).
    static std::string SerializeMarker(const StagedUpdateMarker& marker);
    static bool ParseMarker(const std::string& json, StagedUpdateMarker& out);

#ifdef _WIN32
    struct AuthenticodeResult {
        bool trusted = false;       // WinVerifyTrust returned trust (+ padding check)
        std::string signer;         // signer certificate common name
        std::string thumbprint;     // signer cert SHA-1 thumbprint (hex)
    };
    // Authenticode signer verification: WinVerifyTrust (chain + policy), then
    // confirm the signer common name contains expectedSigner. NOTE: the
    // CVE-2013-3900 trailing-data hardening is the system EnableCertPaddingCheck
    // registry mitigation (not a per-call flag); it's acceptable here because
    // EdDSA is computed over the FULL file bytes, so appended-data can't pass
    // both gates without the production key. `expectedSigner` is the test seam — the localhost
    // rig passes its self-signed test CN so the gate is exercised end-to-end
    // (production passes "Marston Enterprises"). trusted=false on any failure.
    static AuthenticodeResult VerifyAuthenticode(const std::string& path,
                                                 const std::string& expectedSigner);
#endif

    // The production Ed25519 public key (the SAME key macOS Sparkle uses —
    // SUPublicEDKey). One key signs both platforms.
    static const char* PublicKeyBase64();
    // The production Authenticode signer common name (Azure Trusted Signing).
    static const char* ExpectedSigner();
};

} // namespace hodos
