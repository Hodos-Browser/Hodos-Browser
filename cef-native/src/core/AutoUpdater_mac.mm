// AutoUpdater_mac.mm — macOS implementation using Sparkle 2
//
// Sparkle 2 handles the entire update lifecycle on macOS:
// - Periodic background check against appcast.xml
// - Native macOS update dialog
// - Download + EdDSA signature verification
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

// Sparkle 2 delegate for updater configuration
@interface HodosUpdaterDelegate : NSObject <SPUUpdaterDelegate>
@property (nonatomic, assign) BOOL autoCheckEnabled;
@end

@implementation HodosUpdaterDelegate
- (NSSet<NSString *> *)allowedChannelsForUpdater:(SPUUpdater *)updater {
    // Allow pre-release channels (beta versions)
    return [NSSet setWithObject:@"beta"];
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
        s_delegate.autoCheckEnabled = autoCheck;

        // SPUStandardUpdaterController reads SUFeedURL and SUPublicEDKey from Info.plist
        s_updaterController = [[SPUStandardUpdaterController alloc]
            initWithStartingUpdater:YES
            updaterDelegate:s_delegate
            userDriverDelegate:nil];

        s_updaterController.updater.automaticallyChecksForUpdates = autoCheck;

        LogInfo("Sparkle 2 initialized successfully");
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
    // No explicit call needed — it runs on its own schedule
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

bool AutoUpdater::IsAutoCheckEnabled() const {
    if (!initialized_) return false;

#if SPARKLE_AVAILABLE
    return s_updaterController.updater.automaticallyChecksForUpdates;
#else
    return true;
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
    // Sparkle 2 handles its own cleanup — just nil our references
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
