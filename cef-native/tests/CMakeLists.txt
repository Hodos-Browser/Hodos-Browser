# cef-native/tests/CMakeLists.txt
#
# Hodos C++ test target. This is the first C++ test infrastructure in the
# project (per development-docs/UNIT_TESTING.md §1.3 + §5) and is intended
# as a reference example for future C++ tests (SessionManager,
# DomainPermissionCache, etc.).
#
# Design intent:
#   - Self-contained via CMake FetchContent — no system-wide / vcpkg gtest
#     install required. Anyone who can build the main shell can run the tests.
#   - Tests compile ONLY when -DHODOS_BUILD_TESTS=ON is passed to cmake.
#     Default off so the regular shell build is unaffected.
#   - Each test file should target a pure-logic class. Things that need CEF
#     (browser refs, IPC, the message loop) are not in scope here.
#
# Build + run:
#     cmake -S cef-native -B cef-native/build -DHODOS_BUILD_TESTS=ON [...]
#     cmake --build cef-native/build --config Release --target hodos_tests
#     cef-native/build/tests/Release/hodos_tests.exe

include(FetchContent)
FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG        v1.14.0
)
# Tell gtest to use shared CRT to match the rest of the project on Windows.
set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)

# Test executable. Sources listed individually — keeps the build deterministic
# and forces an explicit choice when a new test file is added.
add_executable(hodos_tests
    manifest_fetcher_test.cpp
    sensitive_cert_fields_test.cpp
    js_string_escape_test.cpp           # F6 — JsStringEscape.h is header-only, no production .cpp
    profile_id_test.cpp                 # F5 — IsValidProfileId is inline in ProfileManager.h, no .cpp
    update_stager_test.cpp              # commit 4b — auto-updater verify core
    update_apply_test.cpp               # commit 6b.1 — apply-transaction pure model
    update_fs_test.cpp                  # commit 6b.2a — backup/verify/restore primitives
    silent_state_writer_test.cpp        # #1 — silent-eligibility mirror pure helpers
    farbling_policy_test.cpp            # C2 — eTLD+1 keying, seed persistence, HMAC
    port_config_origin_test.cpp         # P0.5 C1 (row X4) — scheme-anchored origin derivation
    overlay_mouse_test.cpp              # P1-A3 — physical->view mouse conversion for OSR overlays
    payment_cost_test.cpp               # P0.5 finding 6 — body shapes + the sendMax fail-closed rule
    wallet_ssrf_guard_test.cpp          # P0.5 E1 — wallet_call SSRF: URL anchor + method validation
    ipc_role_guard_test.cpp             # P0.5 B1 — self-nav grant gate: family + approval-overlay role (header-only)
    qr_payload_classify_test.cpp        # P0.6 — QR scheme allowlist + amount-injection guard (header-only)
    site_permission_mapping_test.cpp    # P0.9 — CEF-bit -> Hodos-type boundary + frozen stored ids (header-only)
    logger_concurrency_test.cpp         # P2-A7 — Logger write lock; NC disables it on the SAME binary
    logger_policy_test.cpp              # P2-A3/A6 — level gate + rotation + retention, each with its control
    log_safe_url_test.cpp               # P2-A5 — URL redaction (header-only), cases taken from the shipped log
    audit_log_test.cpp                  # P2 — the money/consent trail that survives debug-log retention

    # Production sources under test (built into the test exe directly).
    # (Phase 2.6-H removed permission_engine_test / permission_gate_test along
    # with the C++ PermissionEngine — all permission decisions are now in Rust.)
    # ManifestFetcher's parse path is pure, but it lives in the same TU as
    # Fetch (which references SyncHttpClient::Get + Logger::Log). To keep
    # the linker happy without inflating the test binary, we pull in the
    # SyncHttpClient + Logger implementations too — both are small and
    # have no external deps beyond stdlib + platform HTTP libs.
    # SensitiveCertFields.h is header-only — no production .cpp needed.
    ../src/core/ManifestFetcher.cpp
    ../src/core/SyncHttpClient.cpp
    ../src/core/UpdateStager.cpp          # commit 4b — production source under test
    ../src/core/UpdateApply.cpp           # commit 6b.1 — apply-transaction pure model
    ../src/core/UpdateFs.cpp              # commit 6b.2a — backup/verify/restore primitives
    ../src/core/SilentStateWriter.cpp     # #1 — silent-eligibility mirror
    ../src/core/FarblingPolicy.cpp        # C2 — farbling seed/key derivation
    ../src/core/Logger.cpp
    ../src/core/AuditLog.cpp
)

# SyncHttpClient pulls platform HTTP libs; UpdateStager pulls Windows trust libs.
if(WIN32)
    # bcrypt: FarblingPolicy's BCryptGenRandom (C2 profile-seed generation).
    target_link_libraries(hodos_tests PRIVATE winhttp wintrust crypt32 bcrypt)
elseif(APPLE)
    find_package(CURL REQUIRED)
    target_link_libraries(hodos_tests PRIVATE CURL::libcurl)
    # FarblingPolicy's EnsureProfileSeed uses SecRandomCopyBytes/kSecRandomDefault
    # (Security.framework) — the macOS analogue of the winhttp/bcrypt block above.
    # Without this the suite has never linked on macOS (undefined _SecRandomCopyBytes).
    find_library(SECURITY_LIBRARY Security)
    target_link_libraries(hodos_tests PRIVATE ${SECURITY_LIBRARY})
endif()

# nlohmann::json header is needed by ManifestFetcher + UpdateStager (marker).
find_package(nlohmann_json CONFIG REQUIRED)
target_link_libraries(hodos_tests PRIVATE nlohmann_json::nlohmann_json)

# OpenSSL: UpdateStager's Ed25519 verify + SHA-256, and the test's keygen helper.
find_package(OpenSSL REQUIRED)
target_link_libraries(hodos_tests PRIVATE OpenSSL::SSL OpenSSL::Crypto)

# Enable UpdateStager's test seams (test-key override + Authenticode-advisory)
# ONLY in the test binary. The shipped shell never defines this, so a production
# install has no env-triggerable way to weaken EdDSA / Authenticode verification.
target_compile_definitions(hodos_tests PRIVATE HODOS_UPDATE_TEST_SEAM)

# beta.3 Phase 0.8 — manifest_fetcher_test reads the CANONICAL fixtures in
# demos/manifest-shapes/, the same files rust-wallet/src/manifest.rs compiles in
# with include_str!. One copy, both parsers: a fix in one layer with a test
# driven only by the other proves nothing (see the fixture README). The test
# FAILS (never skips) if a fixture is missing.
target_compile_definitions(hodos_tests PRIVATE
    HODOS_MANIFEST_FIXTURE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../../demos/manifest-shapes")

target_include_directories(hodos_tests PRIVATE
    ../include
)

target_link_libraries(hodos_tests PRIVATE
    GTest::gtest_main
)

# macOS: the project sets `-Wl,-no_adhoc_codesign` globally in the top-level
# CMakeLists (needed for the app bundle's own signing flow), which suppresses the
# linker's automatic ad-hoc signature on EVERY exe target — including this one.
# On Apple Silicon an unsigned Mach-O is SIGKILLed on exec (exit 137, no output),
# so hodos_tests would never run and gtest_discover_tests below would report
# "Subprocess killed". Re-apply an ad-hoc signature after link. (The app bundle
# gets its real signature from mac_build_run.sh; this is test-only.)
if(APPLE)
    add_custom_command(TARGET hodos_tests POST_BUILD
        COMMAND codesign --force --sign - $<TARGET_FILE:hodos_tests>
        COMMENT "Ad-hoc signing hodos_tests (arm64 SIGKILLs unsigned Mach-O)")
endif()

# Register with CTest so `ctest --test-dir build` works.
include(GoogleTest)
gtest_discover_tests(hodos_tests)
