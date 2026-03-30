// AutoUpdater_mac.mm — macOS implementation using Sparkle 2
//
// Sparkle 2 handles the entire update lifecycle on macOS:
// - Periodic background check against appcast.xml
// - Native macOS update dialog
// - Download + EdDSA signature verification
// - Replace app bundle and relaunch
//
// CURRENT STATUS: Stub implementation. Sparkle 2 framework integration
// will be completed when macOS build testing is available.
// The AutoUpdater singleton API is fully implemented — just needs
// Sparkle 2 calls wired in.

#ifdef __APPLE__

#include "core/AutoUpdater.h"
#import <Foundation/Foundation.h>

// TODO: When Sparkle 2 is integrated, uncomment:
// #import <Sparkle/Sparkle.h>

namespace {
    void LogInfo(const std::string& msg) {
        NSLog(@"[AutoUpdater] %s", msg.c_str());
    }
}

AutoUpdater& AutoUpdater::GetInstance() {
    static AutoUpdater instance;
    return instance;
}

void AutoUpdater::Initialize(const std::string& version, const std::string& appcastUrl, bool autoCheck) {
    if (initialized_) return;

    LogInfo("Initializing Sparkle 2 (stub) v" + version + " (autoCheck=" + (autoCheck ? "true" : "false") + ")");
    LogInfo("Appcast URL: " + appcastUrl);

    // TODO: Sparkle 2 integration
    // SPUUpdater *updater = [[SPUUpdater alloc] initWithHostBundle:[NSBundle mainBundle]
    //                                           applicationBundle:[NSBundle mainBundle]
    //                                              userDriverDelegate:nil
    //                                                        delegate:nil];
    // updater.automaticallyChecksForUpdates = autoCheck;
    // [updater startUpdater:nil];

    initialized_ = true;
    LogInfo("Sparkle 2 stub initialized (update checks are no-ops until framework is linked)");
}

void AutoUpdater::CheckForUpdatesInteractively() {
    if (!initialized_) return;
    LogInfo("Manual update check requested (stub — no-op)");
    // TODO: [updater checkForUpdates];
}

void AutoUpdater::CheckForUpdatesInBackground() {
    if (!initialized_) return;
    // TODO: [updater checkForUpdatesInBackground];
}

void AutoUpdater::SetAutoCheckEnabled(bool enabled) {
    if (!initialized_) return;
    LogInfo("Auto-check " + std::string(enabled ? "enabled" : "disabled") + " (stub)");
    // TODO: updater.automaticallyChecksForUpdates = enabled;
}

bool AutoUpdater::IsAutoCheckEnabled() const {
    if (!initialized_) return false;
    // TODO: return updater.automaticallyChecksForUpdates;
    return true;
}

void AutoUpdater::SetCheckInterval(int seconds) {
    if (!initialized_) return;
    // TODO: updater.updateCheckInterval = seconds;
}

void AutoUpdater::Cleanup() {
    if (!initialized_) return;
    LogInfo("Cleaning up Sparkle 2 (stub)...");
    initialized_ = false;
}

void AutoUpdater::SetShutdownCallback(ShutdownCallback callback) {
    shutdown_callback_ = callback;
}

#endif // __APPLE__
