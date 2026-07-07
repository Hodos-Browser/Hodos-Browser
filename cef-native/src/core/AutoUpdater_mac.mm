// AutoUpdater_mac.mm — macOS implementation using Sparkle 2
//
// Sparkle 2 handles the entire update lifecycle on macOS:
// - Periodic background check against appcast.xml
// - Native macOS update dialog (Notify mode)
// - Silent download + install-on-quit (Silent mode)
// - EdDSA signature verification
// - Replace app bundle and relaunch
//
// Sparkle 2 reads SUFeedURL and SUPublicEDKey from Info.plist.
// The framework is downloaded by CI and bundled in the .app.

#ifdef __APPLE__

#include "../../include/core/AutoUpdater.h"
#import <Foundation/Foundation.h>

// Sparkle 2 framework import — linked at build time when available
#if __has_include(<Sparkle/Sparkle.h>)
    #import <Sparkle/Sparkle.h>
    #define SPARKLE_AVAILABLE 1
#else
    #define SPARKLE_AVAILABLE 0
#endif

namespace {
    void LogInfo(const std::string& msg) {
        NSLog(@"[AutoUpdater] %s", msg.c_str());
    }
}

#if SPARKLE_AVAILABLE

@interface HodosUpdaterDelegate : NSObject <SPUUpdaterDelegate>
@end

@implementation HodosUpdaterDelegate

- (BOOL)updater:(nonnull SPUUpdater *)updater
    willInstallUpdateOnQuit:(nonnull SUAppcastItem *)item
    immediateInstallationBlock:(nonnull void (^)(void))immediateInstallHandler {
    NSLog(@"[AutoUpdater] Update %@ staged for install on quit", item.displayVersionString);
    // Return NO — lets Sparkle's scheduler continue running future cycles.
    // The update still installs when the app quits regardless.
    return NO;
}

@end

// Static Sparkle objects (retained for app lifetime)
static SPUStandardUpdaterController *s_updaterController = nil;
static HodosUpdaterDelegate *s_delegate = nil;

#endif // SPARKLE_AVAILABLE

AutoUpdater& AutoUpdater::GetInstance() {
    static AutoUpdater instance;
    return instance;
}

void AutoUpdater::Initialize(const std::string& version, const std::string& appcastUrl, bool autoCheck) {
    if (initialized_) return;

    LogInfo("Initializing Sparkle 2 v" + version + " (autoCheck=" + (autoCheck ? "true" : "false") + ")");

#if SPARKLE_AVAILABLE
    @autoreleasepool {
        s_delegate = [[HodosUpdaterDelegate alloc] init];

        // Use deferred start so we can configure all properties before
        // Sparkle begins its first update check cycle.
        s_updaterController = [[SPUStandardUpdaterController alloc]
            initWithStartingUpdater:NO
            updaterDelegate:s_delegate
            userDriverDelegate:nil];

        LogInfo("Sparkle 2 controller created (deferred start)");
    }
#else
    LogInfo("Sparkle framework not available — update checks disabled");
    LogInfo("(Framework will be linked when building via CI with Sparkle.framework bundled)");
#endif

    initialized_ = true;
}

void AutoUpdater::CheckForUpdatesInteractively() {
    if (!initialized_) return;
    LogInfo("Manual update check requested");

#if SPARKLE_AVAILABLE
    @autoreleasepool {
        [s_updaterController checkForUpdates:nil];
    }
#else
    LogInfo("Sparkle not available — cannot check for updates");
#endif
}

void AutoUpdater::CheckForUpdatesInBackground() {
    if (!initialized_) return;

#if SPARKLE_AVAILABLE
    // Sparkle handles background checks automatically via automaticallyChecksForUpdates
#endif
}

void AutoUpdater::SetAutoCheckEnabled(bool enabled) {
    if (!initialized_) return;
    LogInfo("Auto-check " + std::string(enabled ? "enabled" : "disabled"));

#if SPARKLE_AVAILABLE
    @autoreleasepool {
        s_updaterController.updater.automaticallyChecksForUpdates = enabled;
    }
#endif
}

void AutoUpdater::SetUpdateMode(UpdateMode mode) {
    if (!initialized_) return;
    update_mode_ = mode;

    std::string modeStr = mode == UpdateMode::Off ? "off" :
                          mode == UpdateMode::Notify ? "notify" : "silent";
    LogInfo("Update mode set to: " + modeStr);

#if SPARKLE_AVAILABLE
    @autoreleasepool {
        SPUUpdater *updater = s_updaterController.updater;
        switch (mode) {
            case UpdateMode::Off:
                updater.automaticallyChecksForUpdates = NO;
                updater.automaticallyDownloadsUpdates = NO;
                break;
            case UpdateMode::Notify:
                updater.automaticallyChecksForUpdates = YES;
                updater.automaticallyDownloadsUpdates = NO;
                break;
            case UpdateMode::Silent:
                updater.automaticallyChecksForUpdates = YES;
                updater.automaticallyDownloadsUpdates = YES;
                break;
        }

        // Start the updater if this is the first SetUpdateMode call.
        // Properties are now configured, so Sparkle's first check cycle
        // will use the correct mode.
        NSError *error = nil;
        if (![updater startUpdater:&error]) {
            if (error) {
                NSLog(@"[AutoUpdater] Sparkle startUpdater error: %@", error.localizedDescription);
            }
            // startUpdater returns NO if already started — that's fine.
        }

        // Force a background check on every launch. Sparkle's scheduled
        // check only fires when SUScheduledCheckInterval has elapsed since
        // the last check, so a user who quits before the interval expires
        // and relaunches would never see the update. This ensures we
        // always check on startup; the interval still governs subsequent
        // checks while the app is running.
        if (mode != UpdateMode::Off) {
            [updater checkForUpdatesInBackground];
            LogInfo("Forced background update check on launch");
        }
    }
#endif
}

bool AutoUpdater::IsAutoCheckEnabled() const {
    if (!initialized_) return false;

#if SPARKLE_AVAILABLE
    return s_updaterController.updater.automaticallyChecksForUpdates;
#else
    return false;
#endif
}

void AutoUpdater::SetCheckInterval(int seconds) {
    if (!initialized_) return;

#if SPARKLE_AVAILABLE
    @autoreleasepool {
        s_updaterController.updater.updateCheckInterval = (NSTimeInterval)seconds;
    }
#endif
}

void AutoUpdater::Cleanup() {
    if (!initialized_) return;
    LogInfo("Cleaning up Sparkle 2...");

#if SPARKLE_AVAILABLE
    s_updaterController = nil;
    s_delegate = nil;
#endif

    initialized_ = false;
}

void AutoUpdater::SetShutdownCallback(ShutdownCallback callback) {
    shutdown_callback_ = callback;
    // Sparkle 2 handles shutdown internally — it terminates the app
    // and relaunches after update. No explicit callback needed.
}

#endif // __APPLE__
