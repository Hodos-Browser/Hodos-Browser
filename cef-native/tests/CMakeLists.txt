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
    ../src/core/Logger.cpp
)

# SyncHttpClient pulls platform HTTP libs; UpdateStager pulls Windows trust libs.
if(WIN32)
    target_link_libraries(hodos_tests PRIVATE winhttp wintrust crypt32)
elseif(APPLE)
    find_package(CURL REQUIRED)
    target_link_libraries(hodos_tests PRIVATE CURL::libcurl)
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

target_include_directories(hodos_tests PRIVATE
    ../include
)

target_link_libraries(hodos_tests PRIVATE
    GTest::gtest_main
)

# Register with CTest so `ctest --test-dir build` works.
include(GoogleTest)
gtest_discover_tests(hodos_tests)
