// Platform window abstraction layer
// This header provides a cross-platform interface for window management
// Platform-specific implementations are in platform_window_win.cpp and platform_window_mac.mm

#pragma once

#include <memory>
#include <string>
#include <functional>

namespace Platform {

// Forward declaration of native handle types
#ifdef _WIN32
    struct HWND__;
    typedef HWND__* NativeWindowHandle;
#elif defined(__APPLE__)
    // On macOS, we need NSWindow* or NSView*
    typedef void* NativeWindowHandle;  // Will be cast to NSWindow* or NSView*
#else
    typedef void* NativeWindowHandle;
#endif

// Window creation parameters
struct WindowParams {
    int x = 0;
    int y = 0;
    int width = 800;
    int height = 600;
    std::string title = "Window";
    bool visible = true;
    void* parent = nullptr;  // Parent window handle (platform-specific)
};

// Window class - platform-agnostic interface
class Window {
public:
    virtual ~Window() = default;

    // Get native window handle (HWND on Windows, NSWindow*/NSView* on macOS)
    // This is what you pass to CefWindowInfo::SetAsChild()
    virtual NativeWindowHandle GetNativeHandle() const = 0;

    // Window manipulation
    virtual void SetPosition(int x, int y, int width, int height) = 0;
    virtual void GetPosition(int& x, int& y, int& width, int& height) const = 0;
    virtual void SetTitle(const std::string& title) = 0;
    virtual void Show() = 0;
    virtual void Hide() = 0;
    virtual void Close() = 0;
    virtual bool IsVisible() const = 0;

    // Message/event handling (simplified - platform-specific implementations differ)
    // On Windows: WM_* messages
    // On macOS: NSEvent handling
    virtual void ProcessEvents() = 0;  // Process pending window events

    // Factory method - creates platform-specific implementation
    static std::unique_ptr<Window> Create(const WindowParams& params);
};

// Helper function to get work area (usable screen area excluding taskbar/dock)
struct WorkArea {
    int x, y, width, height;
};
WorkArea GetWorkArea();

} // namespace Platform

