# QR Code Scanning — macOS Implementation

> Platform-specific details for macOS. See [QR_SCAN_OVERVIEW.md](./QR_SCAN_OVERVIEW.md) for architecture and Phase 1 (DOM scanning, cross-platform).

## Status: NOT STARTED

Phase 1 (DOM scan) is cross-platform and already works on macOS. Phase 2 (screen capture) needs macOS-native implementation.

---

## Turnover Instructions (for Claude on Mac)

**Start here.** Pull `main` and work on branch `feature/qr-scan-phase1`.

### What's already done (cross-platform + Windows)

1. **Phase 1 DOM scan** — fully cross-platform, works on macOS already. User clicks "Scan QR" in wallet overlay → JS scanner injected into active tab → scans DOM elements → results forwarded to wallet via IPC.

2. **Phase 2 Windows screen capture** — COMPLETE. When DOM scan returns 0 results, C++ auto-triggers screen capture. The trigger logic is in `simple_handler.cpp` in the `qr_found` handler — it checks `if (json == "[]")` and calls `StartQRScreenCapture()`. This is already wrapped in `#ifdef _WIN32`.

3. **BIP21 extension** — BIP21 URIs accept paymail/identity key/$handle in the address position (not just BSV addresses). This is in the JS scanner logic and is cross-platform.

### What you need to build (macOS only)

Add the macOS `#elif defined(__APPLE__)` branch in the `qr_found` handler (simple_handler.cpp ~line 3390) that calls a macOS-native screen capture function. The function should:

1. Hide wallet overlay via `HideWalletOverlay()` (already cross-platform in simple_app.cpp)
2. Show a full-screen transparent NSWindow with crosshair cursor
3. User drags rectangle to select a region
4. Capture pixels via `CGWindowListCreateImage` (excludes the selection overlay)
5. Decode QR via `CIDetector` with `CIDetectorTypeQRCode` (no third-party library needed — unlike Windows which uses quirc)
6. Apply BSV pattern filter (same regexes — can reuse from QRScreenCapture.cpp or reimplement in ObjC)
7. Show wallet overlay via `ShowWalletOverlay()`
8. Deliver result via `SendProcessMessage(PID_RENDERER, "qr_screen_capture_result", json)`

### Key patterns to follow

- **Selection overlay**: Follow the ghost tab pattern in `WindowManager_mac.mm` (`ShowGhostTabMacOS`) — NSWindow with `NSWindowStyleMaskBorderless`, `NSFloatingWindowLevel`, `setOpaque:NO`
- **Wallet hide/show**: `HideWalletOverlay()` / `ShowWalletOverlay()` are already cross-platform
- **Result format**: JSON must match: `{"status":"found","result":{"type":"address","value":"1A1z...","address":"1A1z...","source":"screen"}}` or `{"status":"not_found"}` or `{"status":"cancelled"}`
- **IPC delivery**: Use `g_qr_scan_requester` (extern in simple_handler.cpp) to route the result back to the wallet overlay's render process
- **BSV types**: `address`, `bip21`, `identity_key`, `paymail` — same as Phase 1

### Key files to read first

| File | Why |
|------|-----|
| `cef-native/src/core/QRScreenCapture.cpp` | Windows reference implementation (~300 lines). Copy the BSV classification logic |
| `cef-native/src/handlers/simple_handler.cpp` (~line 3383) | The `qr_found` handler with `#ifdef _WIN32` block — add `#elif defined(__APPLE__)` |
| `cef-native/src/core/WindowManager_mac.mm` (~line 270) | Ghost tab pattern for NSWindow creation |
| `cef-native/cef_browser_shell_mac.mm` | Where overlay creation functions live on macOS |
| `cef-native/src/handlers/simple_render_process_handler.cpp` (~line 982) | IPC forwarders already done (cross-platform) |

### Screen Recording Permission

`CGWindowListCreateImage` requires Screen Recording permission on macOS 10.15+. Handle with:
- `CGPreflightScreenCaptureAccess()` (macOS 11+) to check
- `CGRequestScreenCaptureAccess()` to prompt
- Graceful fallback if denied

### Files to create/modify

| File | Change |
|------|--------|
| `cef_browser_shell_mac.mm` | NEW: `QRSelectionView`, `QRSelectionWindow`, `StartQRScreenCaptureMacOS()`, `FinishQRScreenCaptureMacOS()` |
| `simple_handler.cpp` | Add `#elif defined(__APPLE__)` in `qr_found` handler to call macOS capture |
| No CMake changes needed | CIDetector and Core Graphics are system frameworks already linked |

---

## Phase 1: DOM Scanning (No macOS-Specific Work)

Phase 1 is pure JavaScript + C++ IPC. All work is cross-platform. See overview doc.

The fingerprint farbling concern was a non-issue — jsQR's error correction handles canvas perturbation.

## Phase 2: Screen Region Capture

### Overview

Same flow as Windows but using macOS APIs:
1. Close wallet overlay
2. Show full-screen transparent NSWindow with crosshair cursor
3. User drags to select region
4. Capture pixels from screen within that region
5. Decode QR from captured pixels
6. Reopen wallet overlay with results

### macOS-Specific Advantage: Built-in QR Detection

macOS has **native QR code detection** via Core Image (`CIDetector` with `CIDetectorTypeQRCode`). This eliminates the need for a third-party QR library on macOS.

```objc
CIImage *ciImage = [CIImage imageWithCGImage:capturedCGImage];
CIDetector *detector = [CIDetector detectorOfType:CIDetectorTypeQRCode
                                          context:nil
                                          options:@{CIDetectorAccuracy: CIDetectorAccuracyHigh}];
NSArray<CIFeature *> *features = [detector featuresInImage:ciImage];
for (CIQRCodeFeature *qr in features) {
    NSString *decoded = qr.messageString;
    // Process decoded string...
}
```

### Implementation Plan

#### 1. Selection Overlay Window

Add to `cef_browser_shell_mac.mm`:

```objc
@interface QRSelectionView : NSView
@property (nonatomic) NSPoint dragStart;
@property (nonatomic) NSPoint dragCurrent;
@property (nonatomic) BOOL isDragging;
@end

@interface QRSelectionWindow : NSWindow
@end
```

Window properties:
- `NSBorderlessWindowMask` (or `NSWindowStyleMaskBorderless`)
- `[window setLevel:NSScreenSaverWindowLevel]` (above everything)
- `[window setOpaque:NO]`
- `[window setBackgroundColor:[NSColor colorWithCalibratedWhite:0.0 alpha:0.3]]`
- Covers entire screen frame
- Custom crosshair cursor: `[[NSCursor crosshairCursor] set]`

#### 2. Mouse Handling in QRSelectionView

```objc
- (void)mouseDown:(NSEvent *)event {
    self.dragStart = [self convertPoint:event.locationInWindow fromView:nil];
    self.isDragging = YES;
}

- (void)mouseDragged:(NSEvent *)event {
    self.dragCurrent = [self convertPoint:event.locationInWindow fromView:nil];
    [self setNeedsDisplay:YES]; // Redraw selection rectangle
}

- (void)mouseUp:(NSEvent *)event {
    self.isDragging = NO;
    NSRect selection = [self selectionRect];
    [self captureRegion:selection];
}

- (void)keyDown:(NSEvent *)event {
    if (event.keyCode == 53) { // ESC
        [self cancel];
    }
}
```

#### 3. Screen Capture (CGWindowListCreateImage)

```objc
- (void)captureRegion:(NSRect)selection {
    // Convert from view coordinates to screen coordinates
    NSRect screenRect = [self.window convertRectToScreen:selection];

    // Flip Y axis (NSView uses bottom-left origin, CG uses top-left)
    CGFloat screenHeight = NSScreen.mainScreen.frame.size.height;
    CGRect captureRect = CGRectMake(
        screenRect.origin.x,
        screenHeight - screenRect.origin.y - screenRect.size.height,
        screenRect.size.width,
        screenRect.size.height
    );

    // Capture screen region (excludes our own overlay window)
    CGImageRef image = CGWindowListCreateImage(
        captureRect,
        kCGWindowListOptionOnScreenBelowWindow,
        (CGWindowID)[self.window windowNumber],
        kCGWindowImageDefault
    );

    if (image) {
        [self decodeQRFromImage:image];
        CGImageRelease(image);
    }
}
```

#### 4. QR Decoding (Native CIDetector)

```objc
- (void)decodeQRFromImage:(CGImageRef)cgImage {
    CIImage *ciImage = [CIImage imageWithCGImage:cgImage];
    CIDetector *detector = [CIDetector detectorOfType:CIDetectorTypeQRCode
                                              context:nil
                                              options:@{CIDetectorAccuracy: CIDetectorAccuracyHigh}];
    NSArray *features = [detector featuresInImage:ciImage];

    NSMutableArray<NSString *> *results = [NSMutableArray array];
    for (CIFeature *feature in features) {
        if ([feature isKindOfClass:[CIQRCodeFeature class]]) {
            CIQRCodeFeature *qr = (CIQRCodeFeature *)feature;
            if (qr.messageString) {
                [results addObject:qr.messageString];
            }
        }
    }

    // Close selection window
    [self.window orderOut:nil];

    // Send results back to browser process
    // Reopen wallet overlay with QR data
    if (results.count > 0) {
        [self sendQRResult:results[0]]; // Or show picker for multiple
    } else {
        [self sendQRResult:nil]; // No QR found
    }
}
```

No third-party QR library needed on macOS.

#### 5. Drawing the Selection Rectangle

```objc
- (void)drawRect:(NSRect)dirtyRect {
    // Semi-transparent dark overlay
    [[NSColor colorWithCalibratedWhite:0.0 alpha:0.3] set];
    NSRectFillUsingOperation(self.bounds, NSCompositingOperationSourceOver);

    if (self.isDragging) {
        NSRect selection = [self selectionRect];

        // Clear the selection area (show through to screen)
        [[NSColor clearColor] set];
        NSRectFillUsingOperation(selection, NSCompositingOperationCopy);

        // Draw selection border
        [[NSColor whiteColor] set];
        NSFrameRectWithWidth(selection, 2.0);

        // Optional: draw instruction text
        // "Release to capture"
    } else {
        // Draw instruction text
        // "Drag to select a QR code. Press ESC to cancel."
    }
}

- (NSRect)selectionRect {
    return NSMakeRect(
        MIN(self.dragStart.x, self.dragCurrent.x),
        MIN(self.dragStart.y, self.dragCurrent.y),
        fabs(self.dragCurrent.x - self.dragStart.x),
        fabs(self.dragCurrent.y - self.dragStart.y)
    );
}
```

### Screen Recording Permission (macOS 10.15+)

`CGWindowListCreateImage` requires **Screen Recording permission** on macOS Catalina and later. The system will prompt the user on first use. If denied, the capture returns a blank image.

**Handling:**
- Check permission status via `CGPreflightScreenCaptureAccess()` (macOS 11+)
- If not granted, call `CGRequestScreenCaptureAccess()` to trigger the prompt
- Show a user-friendly message: "Hodos Browser needs Screen Recording permission to scan QR codes"
- Direct user to System Preferences > Privacy & Security > Screen Recording
- Fall back gracefully if permission denied (tell user to use copy/paste instead)

### Retina Display

`CGWindowListCreateImage` captures at physical pixel density. The selection coordinates from NSView are in points (logical pixels). Use `[screen backingScaleFactor]` to convert:

```objc
CGFloat scale = self.window.screen.backingScaleFactor;
CGRect physicalRect = CGRectMake(
    captureRect.origin.x * scale,
    captureRect.origin.y * scale,
    captureRect.size.width * scale,
    captureRect.size.height * scale
);
```

Actually, `CGWindowListCreateImage` handles this automatically when you pass screen coordinates — it captures at the display's native resolution. The `CIDetector` works on the full-resolution image regardless.

### Multi-Monitor

Use `NSScreen.screens` to find the screen containing the selection, then adjust coordinates relative to that screen's frame. `CGWindowListCreateImage` uses global display coordinates which span all monitors.

### Key Files to Modify

| File | Change |
|------|--------|
| `cef_browser_shell_mac.mm` | Add `QRSelectionWindow`, `QRSelectionView`, creation/destruction functions |
| `cef_browser_shell_mac.mm` | Handle `qr_screen_capture_start` notification |
| `simple_handler.cpp` | Platform-conditional IPC handling (same message, different native call) |

### Dependencies

- **None** — `CIDetector`, `CGWindowListCreateImage`, and Core Image are all system frameworks already linked
- No third-party QR library needed (unlike Windows which needs `quirc`)
