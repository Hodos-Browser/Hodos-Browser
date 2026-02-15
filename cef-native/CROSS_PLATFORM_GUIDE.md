# Cross-Platform Development Guide for Hodos Browser

## Overview

This guide explains how to handle cross-platform differences between Windows and macOS without maintaining separate repositories. We use a **hybrid approach** combining conditional compilation (`#ifdef`) with platform abstraction layers.

## Philosophy: One Codebase, Multiple Platforms

**You don't need separate repositories.** Most successful cross-platform applications (CEF, Chromium, Electron, Brave) use a single codebase with platform-specific implementations.

## Strategies for Cross-Platform Code

### 1. Conditional Compilation (`#ifdef`) - For Small Differences

**Use when:**
- Including platform-specific headers
- Defining platform-specific types/constants
- Simple platform checks

**Example:**
```cpp
#ifdef _WIN32
    #include <windows.h>
    typedef HWND NativeWindowHandle;
#elif defined(__APPLE__)
    #include <Cocoa/Cocoa.h>
    typedef NSWindow* NativeWindowHandle;
#endif

#ifdef _WIN32
    std::string path = std::getenv("APPDATA") + "\\HodosBrowser";
#else
    std::string path = std::string(std::getenv("HOME")) + "/Library/Application Support/HodosBrowser";
#endif
```

**Pros:**
- Simple and direct
- Widely understood pattern
- Used by CEF itself

**Cons:**
- Can clutter code if overused
- Harder to read with many platform checks

### 2. Platform Abstraction Layer - For Complex Subsystems

**Use when:**
- Window creation/management
- File system operations
- Process management
- Any complex platform-specific APIs

**Example Structure:**
```
cef-native/
  include/platform/
    platform_window.h        # Abstract interface
    platform_path.h          # Path utilities
  src/platform/
    platform_window_win.cpp  # Windows implementation
    platform_window_mac.mm   # macOS implementation (Objective-C++)
    platform_path_win.cpp
    platform_path_mac.cpp
```

**Example Usage:**
```cpp
// In your main code (platform-agnostic):
auto window = Platform::Window::Create({.width = 800, .height = 600});
CefWindowInfo window_info;
window_info.SetAsChild(window->GetNativeHandle(), ...);
```

**Pros:**
- Clean separation of concerns
- Easier to test
- More maintainable
- Platform-specific code isolated

**Cons:**
- More upfront design work
- Slight abstraction overhead

### 3. Hybrid Approach (Recommended for Hodos Browser)

**Strategy:**
- **Abstraction layer** for major subsystems (windows, paths, processes)
- **`#ifdef`** for minor differences (header includes, constants, simple conditionals)

**Why This Works:**
- Reduces code duplication
- Keeps platform differences manageable
- Allows incremental migration
- Makes testing easier

## Platform-Specific Code Patterns

### Window Management

**Problem:** Windows uses `HWND`, macOS uses `NSWindow*`/`NSView*`

**Solution:** Abstract window creation through `Platform::Window` interface

**Windows Implementation:**
```cpp
// src/platform/platform_window_win.cpp
#ifdef _WIN32
#include "platform_window.h"
#include <windows.h>

namespace Platform {
    class WindowImpl : public Window {
        HWND hwnd_;
    public:
        NativeWindowHandle GetNativeHandle() const override {
            return hwnd_;
        }
        // ... rest of implementation
    };
}
#endif
```

**macOS Implementation:**
```cpp
// src/platform/platform_window_mac.mm (note .mm extension for Objective-C++)
#ifdef __APPLE__
#include "platform_window.h"
#include <Cocoa/Cocoa.h>

namespace Platform {
    class WindowImpl : public Window {
        NSWindow* window_;
    public:
        NativeWindowHandle GetNativeHandle() const override {
            return (NativeWindowHandle)window_;
        }
        // ... rest of implementation
    };
}
#endif
```

### File System Paths

**Problem:** Different path separators and user data locations

**Solution:** Use `std::filesystem::path` + platform abstraction

```cpp
// include/platform/platform_path.h
namespace Platform {
    std::string GetUserDataPath();  // Returns appropriate path for current platform
    std::string GetExecutablePath();
    std::string GetCachePath();
}

// src/platform/platform_path_win.cpp
#ifdef _WIN32
std::string Platform::GetUserDataPath() {
    const char* appdata = std::getenv("APPDATA");
    return std::string(appdata) + "\\HodosBrowser";
}
#endif

// src/platform/platform_path_mac.cpp
#ifdef __APPLE__
std::string Platform::GetUserDataPath() {
    const char* home = std::getenv("HOME");
    return std::string(home) + "/Library/Application Support/HodosBrowser";
}
#endif
```

### Process Management

**Problem:** Different APIs for spawning processes

**Solution:** CEF handles most of this, but for custom code:

```cpp
// include/platform/platform_process.h
namespace Platform {
    class Process {
    public:
        static bool Spawn(const std::string& executable, const std::vector<std::string>& args);
        // ...
    };
}

// Implementations in separate files per platform
```

## Migration Strategy

### Phase 1: Isolate Windows Code (Quick Win)
Wrap existing Windows-specific code in `#ifdef _WIN32` blocks:

```cpp
#ifdef _WIN32
    HWND hwnd = CreateWindow(...);
    // Windows-specific code
#endif
```

### Phase 2: Create Abstraction Interfaces
Design platform-agnostic interfaces for major subsystems:

```cpp
// include/platform/platform_window.h
namespace Platform {
    class Window { /* abstract interface */ };
}
```

### Phase 3: Move to Platform-Specific Files
Create `src/platform/` directory and move implementations:

```
src/platform/
  platform_window_win.cpp   # All Windows window code
  platform_window_mac.mm    # All macOS window code
```

### Phase 4: Implement macOS Versions
Add macOS implementations alongside Windows code.

## CMake Configuration

```cmake
# CMakeLists.txt
if(WIN32)
    set(PLATFORM_SOURCES
        src/platform/platform_window_win.cpp
        src/platform/platform_path_win.cpp
    )
    target_link_libraries(HodosBrowserShell
        # Windows libraries
    )
elseif(APPLE)
    set(PLATFORM_SOURCES
        src/platform/platform_window_mac.mm  # Note .mm extension
        src/platform/platform_path_mac.cpp
    )
    target_link_libraries(HodosBrowserShell
        "-framework Cocoa"
        "-framework AppKit"
    )
endif()

target_sources(HodosBrowserShell PRIVATE ${PLATFORM_SOURCES})
```

## Common Pitfalls to Avoid

### ❌ Don't: Scatter platform checks everywhere
```cpp
// BAD: Hard to maintain
void CreateWindow() {
    #ifdef _WIN32
        HWND h = CreateWindowEx(...);
    #else
        NSWindow* w = [[NSWindow alloc] init];
    #endif
}
```

### ✅ Do: Centralize platform differences
```cpp
// GOOD: Platform differences isolated
auto window = Platform::Window::Create(params);
```

### ❌ Don't: Use raw platform types in shared code
```cpp
// BAD: HWND leaks into platform-agnostic code
extern HWND g_hwnd;
```

### ✅ Do: Use abstraction types
```cpp
// GOOD: Platform-agnostic
std::unique_ptr<Platform::Window> mainWindow;
```

## How Other Projects Handle This

### CEF (Chromium Embedded Framework)
- Uses `#if defined(OS_WIN)` pattern extensively
- Platform-specific code in separate directories: `cef/`, `chromium/`
- Example: `include/base/cef_platform.h` defines platform macros

### Electron
- Abstraction layers with platform-specific directories
- `shell/browser/` has `win/`, `mac/`, `linux/` subdirectories
- Uses factory patterns for platform-specific implementations

### Brave Browser
- Hybrid approach similar to Chromium
- Platform abstraction with `#ifdef` for compile-time optimizations
- Separate platform-specific implementation files

## Testing Cross-Platform Code

1. **Build on both platforms** - CI/CD should test Windows and macOS builds
2. **Platform-specific unit tests** - Test each platform implementation separately
3. **Integration tests** - Test platform abstraction interfaces work correctly

## Example: Converting Window Creation

**Before (Windows-only):**
```cpp
WNDCLASS wc = {};
wc.lpfnWndProc = ShellWindowProc;
wc.hInstance = hInstance;
wc.lpszClassName = L"HodosBrowserWndClass";
RegisterClass(&wc);

HWND hwnd = CreateWindowEx(0, L"HodosBrowserWndClass", ...);
```

**After (Cross-platform with abstraction):**
```cpp
Platform::WindowParams params;
params.width = 1200;
params.height = 800;
params.title = "Hodos Browser";
auto window = Platform::Window::Create(params);

CefWindowInfo window_info;
window_info.SetAsChild(window->GetNativeHandle(), ...);
```

**Platform-specific implementation (Windows):**
```cpp
// src/platform/platform_window_win.cpp
std::unique_ptr<Platform::Window> Platform::Window::Create(const WindowParams& params) {
    return std::make_unique<WindowImpl>(params);  // Windows-specific implementation
}
```

## Summary

**Answer to "Is `#ifdef` a good plan?":**
- **Yes**, but not for everything
- Use `#ifdef` for: headers, simple types, constants
- Use **abstraction layers** for: windows, file systems, complex APIs
- This hybrid approach is what most professional cross-platform projects use

**One repository, multiple platforms** - This is the standard approach, not an exception!


