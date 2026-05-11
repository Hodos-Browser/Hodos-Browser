# CEF Native — C++ Unit Tests

> Pure-logic C++ unit tests for the Hodos browser shell. First infrastructure of its kind in the project — designed as a reference example per `development-docs/UNIT_TESTING.md` §1.3 + §5.

## Overview

This directory holds C++ unit tests using **Google Test (GoogleTest v1.14)**. Tests are **opt-in** at configure time (`-DHODOS_BUILD_TESTS=ON`); the default shell build is unaffected by anything here. GoogleTest is pulled via CMake `FetchContent` so contributors don't need a system / vcpkg install — anyone who can build the main shell can run the tests.

## Files

| File | Purpose |
|------|---------|
| `CMakeLists.txt` | Test target definition. Lists each test source file explicitly + the production sources under test. Wires `FetchContent` for GoogleTest. |
| `permission_engine_test.cpp` | Decision-matrix coverage for `core/PermissionEngine`. First C++ test in the project. |
| `CLAUDE.md` | This file. |

## Build + Run

```powershell
# From repo root (Windows, PowerShell)
cd cef-native
cmake -S . -B build -DHODOS_BUILD_TESTS=ON `
    -G "Visual Studio 17 2022" -A x64 `
    -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake
cmake --build build --config Release --target hodos_tests
build/tests/Release/hodos_tests.exe
```

```bash
# From repo root (macOS / Linux)
cd cef-native
cmake -S . -B build -DHODOS_BUILD_TESTS=ON -G "Unix Makefiles"
cmake --build build --config Release --target hodos_tests
./build/tests/hodos_tests
```

First run pulls GoogleTest from GitHub (a few seconds). Subsequent runs use the cached download.

CTest integration is enabled — `ctest --test-dir build` runs all discovered tests with verbose output on failure.

## Adding a New Test

The PermissionEngine test is the reference template. To add a new test target:

1. **Pick a pure-logic class.** Things that need CEF (a real `CefBrowser*`, the message loop, `CefPostTask` semantics) are NOT in scope. Move that logic to a plain class first, or write the test against the surrounding helpers.
2. **Create `<class_name>_test.cpp`** in this directory. Follow the existing test discipline:
   - One concern per test, present-tense indicative naming (`ApprovedDomainWithinCapsIsSilent`, not `TestPayment1`).
   - Build a fresh fixture-helper output in each test; no shared mutable state.
   - Cover every branch + boundary conditions (cap exact-match, zero values, empty inputs).
3. **Append the source files to `CMakeLists.txt`**:
   ```cmake
   add_executable(hodos_tests
       permission_engine_test.cpp
       my_new_test.cpp                     # <-- add here

       ../src/core/PermissionEngine.cpp
       ../src/core/MyNewClass.cpp          # <-- production source under test
   )
   ```
   Listing each file explicitly is deliberate — it forces a conscious choice on every addition and keeps the build deterministic.
4. **Rebuild + run:** `cmake --build build --config Release --target hodos_tests && ./build/tests/Release/hodos_tests.exe`

## Conventions

| Aspect | Choice | Reason |
|--------|--------|--------|
| Framework | Google Test 1.14 | Industry standard; matches `UNIT_TESTING.md` §5.2 plan |
| Dependency mgmt | CMake `FetchContent` (not vcpkg) | Zero install friction; reproducible by version pin |
| Opt-in | `-DHODOS_BUILD_TESTS=ON` | Default shell build stays fast; CI / contributors opt in |
| Naming | `<class>_test.cpp` | Mirrors `<class>.cpp` for grep-friendliness |
| Test method names | Present-tense indicative | Describes the invariant, not the action |
| Fixtures | Plain helper functions | Simpler than `TEST_F` for stateless decision logic |
| Includes | `#include "core/Foo.h"` works because we add `../include` to `target_include_directories` | Mirrors how production code includes work |

## What's Tested (Today)

| Class | Coverage | Test File |
|-------|----------|-----------|
| `PermissionEngine` | All 6 branches of Matrix C + boundary conditions + branch ordering (blocked > privacy-perimeter, unknown > privacy-perimeter) | `permission_engine_test.cpp` |

## What Should Be Tested Next (Roadmap)

Per `development-docs/UNIT_TESTING.md` §5.1:

- **`SessionManager`** — spending counters, rate-limit windows, per-tab reset on close
- **`DomainPermissionCache`** — cache hit / miss / negative cache TTL, invalidation
- **`PaidContentCache`** — TTL parsing, LRU eviction, hard-reload bypass
- **`AdblockCache`** — URL filter check, cosmetic resource caching

Each of these is mostly pure logic; the small amount of platform-specific HTTP can be injected via a function pointer or hidden behind an interface.

## What Should NOT Be Tested Here

- Anything requiring a live `CefBrowser*`, `CefRequest`, or CEF message loop
- Anything requiring the Rust wallet running at `localhost:31301` (use the Rust integration tests instead)
- UI rendering — covered by Playwright e2e on the React side
- Cross-process IPC — covered by manual smoke checklists

## CI Integration (Future)

When `BUILD_AND_RELEASE.md` §5's CI pipeline lands, this test target should be added to the `cpp-test` job:

```yaml
cpp-test:
  runs-on: windows-latest
  steps:
    - uses: actions/checkout@v4
    - uses: lukka/get-cmake@latest
    - uses: lukka/run-vcpkg@v11
    - run: cmake -S cef-native -B cef-native/build -DHODOS_BUILD_TESTS=ON -G "Visual Studio 17 2022" -A x64 -DCMAKE_TOOLCHAIN_FILE=$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake
    - run: cmake --build cef-native/build --config Release --target hodos_tests
    - run: ctest --test-dir cef-native/build --output-on-failure
```

Adding `cpp-test` to the gate alongside `rust-test` and `frontend-test` would mean a failing `PermissionEngine` test blocks the PR.

## Related

- `cef-native/CLAUDE.md` — build instructions, HWND hierarchy, IPC flow
- `cef-native/src/core/CLAUDE.md` — production source under test
- `development-docs/UNIT_TESTING.md` — overall testing strategy this file implements
- `rust-wallet/tests/CLAUDE.md` — Rust test conventions (different framework, same spirit)
