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
    permission_engine_test.cpp

    # Production sources under test (built into the test exe directly).
    # PermissionEngine is pure logic with no CEF deps, so it links cleanly.
    ../src/core/PermissionEngine.cpp
)

target_include_directories(hodos_tests PRIVATE
    ../include
)

target_link_libraries(hodos_tests PRIVATE
    GTest::gtest_main
)

# Register with CTest so `ctest --test-dir build` works.
include(GoogleTest)
gtest_discover_tests(hodos_tests)
