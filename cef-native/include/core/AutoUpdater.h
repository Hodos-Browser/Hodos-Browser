// AutoUpdater.h — Cross-platform auto-update singleton
//
// Windows: wraps WinSparkle (https://winsparkle.org)
// macOS:   wraps Sparkle 2 (https://sparkle-project.org)
//
// Both use the Sparkle appcast XML format for update discovery.
// The singleton is initialized after the main window is created
// and cleaned up during ShutdownApplication().

#ifndef AUTO_UPDATER_H
#define AUTO_UPDATER_H

#include <string>
#include <functional>

class AutoUpdater {
public:
    static AutoUpdater& GetInstance();

    // Initialize the updater. Call after main window is visible.
    // version: current app version (e.g. "0.2.0-beta.1")
    // appcastUrl: URL to appcast.xml (e.g. "https://hodosbrowser.com/appcast.xml")
    // autoCheck: whether to check for updates automatically on startup
    void Initialize(const std::string& version, const std::string& appcastUrl, bool autoCheck);

    // Manual check triggered by user ("Check for updates" button).
    // Shows progress UI and "no update found" if up to date.
    void CheckForUpdatesInteractively();

    // Silent background check. No UI if up to date.
    // Shows update dialog only if a new version is found.
    void CheckForUpdatesInBackground();

    // Enable or disable automatic update checking.
    void SetAutoCheckEnabled(bool enabled);

    // Get current auto-check state.
    bool IsAutoCheckEnabled() const;

    // Set the interval between automatic checks (in seconds).
    // Minimum is 3600 (1 hour). Default is 86400 (24 hours).
    void SetCheckInterval(int seconds);

    // Clean up. Call during shutdown before CefShutdown().
    void Cleanup();

    // Callback for shutdown request from the updater.
    // WinSparkle calls this when the user accepts an update and the
    // installer needs to launch — the app must shut down gracefully.
    using ShutdownCallback = std::function<void()>;
    void SetShutdownCallback(ShutdownCallback callback);

private:
    AutoUpdater() = default;
    ~AutoUpdater() = default;
    AutoUpdater(const AutoUpdater&) = delete;
    AutoUpdater& operator=(const AutoUpdater&) = delete;

    bool initialized_ = false;
    ShutdownCallback shutdown_callback_;
};

#endif // AUTO_UPDATER_H
