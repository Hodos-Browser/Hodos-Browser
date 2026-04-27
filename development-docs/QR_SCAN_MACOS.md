# QR Code Scanning — macOS Implementation

> Platform-specific details for macOS. See [QR_SCAN_OVERVIEW.md](./QR_SCAN_OVERVIEW.md) for architecture and Phase 1 (DOM scanning, cross-platform).

## Phase 1: DOM Scanning (No macOS-Specific Work)

Phase 1 is pure JavaScript + C++ IPC. All work is cross-platform. See overview doc.

Same consideration as Windows: ensure the scanner script's `getImageData()` calls don't get intercepted by the fingerprint farbling hook.

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
