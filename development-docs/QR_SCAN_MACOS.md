# QR Code Scanning — macOS Implementation

> Platform-specific details for macOS. See [QR_SCAN_OVERVIEW.md](./QR_SCAN_OVERVIEW.md) for architecture and Phase 1 (DOM scanning, cross-platform).

## Status: COMPLETE

Phase 1 (DOM scan) is cross-platform and already works on macOS. Phase 2 (screen capture) implemented 2026-04-27.

### Implementation Notes
- Uses `CGWindowListCreateImage` via `dlsym` (marked unavailable in macOS 15 SDK but still functions at runtime)
- QR decoding via native `CIDetector` (no third-party library needed, unlike Windows quirc)
- Permission check is non-blocking: requests Screen Recording access but proceeds with capture attempt regardless. Denied capture returns null/blank image, handled as "No QR found"
- `HideWalletOverlay()` / `ShowWalletOverlay()` added to macOS — hide/show without destroying the CEF browser

---

## Implementation Plan (Detailed)

Investigated 2026-04-27 against branch `feature/qr-scan-phase1`. All file:line references verified.

### Prerequisites: Two platform gaps to fix first

#### 1. HideWalletOverlay / ShowWalletOverlay do not exist on macOS

Windows has `HideWalletOverlay()` (simple_app.cpp:914) and `ShowWalletOverlay()` (simple_app.cpp:812) which hide/show the HWND without destroying the CEF browser. macOS only has `CreateWalletOverlayWithSeparateProcess()` (cef_browser_shell_mac.mm:2659) and `CloseWalletOverlay()` (cef_browser_shell_mac.mm:2640) which create/destroy.

**The `extern void HideWalletOverlay()` call in simple_handler.cpp:3400 will cause a linker error on macOS** unless we provide an implementation.

**Add to `cef_browser_shell_mac.mm`:**

```objc
void HideWalletOverlay() {
    if (!g_wallet_overlay_window) return;
    LOG_INFO("Hiding wallet overlay (macOS)");
    RemoveClickOutsideMonitor(g_wallet_overlay_window);
    [g_wallet_overlay_window orderOut:nil];

    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    if (wallet_browser) {
        wallet_browser->GetMainFrame()->ExecuteJavaScript(
            "window.postMessage({type:'wallet_hidden'},'*');", "", 0);
        wallet_browser->GetHost()->SetFocus(false);
    }
}

void ShowWalletOverlay() {
    if (!g_wallet_overlay_window) return;
    LOG_INFO("Showing wallet overlay (macOS)");
    [g_wallet_overlay_window makeKeyAndOrderFront:nil];
    InstallClickOutsideMonitor(g_wallet_overlay_window);

    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    if (wallet_browser) {
        wallet_browser->GetHost()->SetFocus(true);
    }
}
```

Note: Windows `ShowWalletOverlay` takes `(int iconRightOffset, BrowserWindow* targetWin)`. The macOS QR flow doesn't need repositioning (wallet reopens at same position), so a no-arg version is fine. The `extern` in simple_handler.cpp:86 has default args so the no-arg call compiles.

#### 2. CoreImage framework not linked in CMakeLists.txt

CIDetector (native QR decoding) requires CoreImage. Currently only CoreGraphics and QuartzCore are linked (CMakeLists.txt:321-322).

**Add to CMakeLists.txt macOS section (near line 322):**

```cmake
find_library(COREIMAGE_LIBRARY CoreImage)
```

And add `${COREIMAGE_LIBRARY}` to the `target_link_libraries` list (around line 356).

---

### Step 1: Add `#elif defined(__APPLE__)` in qr_found handler

**File:** `cef-native/src/handlers/simple_handler.cpp`
**Location:** After the `#ifdef _WIN32` block at line 3393, before `#endif` at line 3405

```cpp
#elif defined(__APPLE__)
    if (json == "[]" && g_qr_scan_requester && g_qr_scan_requester->GetMainFrame()) {
        LOG_INFO_BROWSER("📷 DOM scan empty — falling through to screen capture (macOS)");
        CefRefPtr<CefProcessMessage> notify = CefProcessMessage::Create("qr_screen_capture_starting");
        g_qr_scan_requester->GetMainFrame()->SendProcessMessage(PID_RENDERER, notify);
        extern void HideWalletOverlay();
        HideWalletOverlay();
        extern void StartQRScreenCaptureMacOS();
        StartQRScreenCaptureMacOS();
        return true;
    }
```

Pattern matches: navigate handler (simple_handler.cpp:~2430) which has a conditional block inside cross-platform code.

---

### Step 2: Implement QR selection overlay in cef_browser_shell_mac.mm

Add these classes and functions near the other overlay code (after the wallet overlay section, ~line 2760).

#### QRSelectionView (NSView subclass)

Handles mouse events for drag-to-select and drawing the overlay.

```objc
@interface QRSelectionView : NSView
@property (nonatomic) NSPoint dragStart;
@property (nonatomic) NSPoint dragCurrent;
@property (nonatomic) BOOL isDragging;
@end
```

**Mouse handling:**
- `mouseDown:` — record start point, set `isDragging = YES`, `setNeedsDisplay:YES`
- `mouseDragged:` — update current point, `setNeedsDisplay:YES`
- `mouseUp:` — compute selection rect, call `FinishQRScreenCaptureMacOS(false, selectionRect)`
- `keyDown:` — if keyCode == 53 (Escape), call `FinishQRScreenCaptureMacOS(true, NSZeroRect)`
- `rightMouseDown:` — same as Escape (cancel)

**Drawing (`drawRect:`):**
- Fill entire bounds with semi-transparent black (`alpha:0.3`)
- If dragging: clear the selection rect (use `NSCompositingOperationCopy` with clear color)
- Draw 2px gold (#a67c00) border around selection rect (matches Windows implementation)
- If not dragging: draw centered instruction text "Drag to select a QR code. Press ESC to cancel."

**Must accept first responder** (`acceptsFirstResponder` returns YES) for keyboard events.

#### Static globals

```objc
static NSWindow* g_qr_selection_window = nil;
static QRSelectionView* g_qr_selection_view = nil;
```

#### StartQRScreenCaptureMacOS()

```objc
void StartQRScreenCaptureMacOS() {
    LOG_INFO("📷 Starting QR screen capture (macOS)");

    // Check Screen Recording permission (macOS 11+)
    if (@available(macOS 11.0, *)) {
        if (!CGPreflightScreenCaptureAccess()) {
            CGRequestScreenCaptureAccess();
            // Permission prompt shown — deliver "permission_needed" result
            // User must grant in System Preferences and retry
            DeliverQRResultMacOS("{\"status\":\"error\",\"message\":\"Screen Recording permission required\"}");
            return;
        }
    }

    // Clean up any existing selection window
    if (g_qr_selection_window) {
        [g_qr_selection_window close];
        g_qr_selection_window = nil;
    }

    // Cover entire screen (all monitors via NSScreen.screens)
    NSRect screenFrame = NSScreen.mainScreen.frame;
    for (NSScreen* screen in NSScreen.screens) {
        screenFrame = NSUnionRect(screenFrame, screen.frame);
    }

    g_qr_selection_window = [[NSWindow alloc]
        initWithContentRect:screenFrame
        styleMask:NSWindowStyleMaskBorderless
        backing:NSBackingStoreBuffered
        defer:NO];

    [g_qr_selection_window setLevel:NSScreenSaverWindowLevel]; // Above everything
    [g_qr_selection_window setOpaque:NO];
    [g_qr_selection_window setBackgroundColor:[NSColor clearColor]];
    [g_qr_selection_window setIgnoresMouseEvents:NO];
    [g_qr_selection_window setAcceptsMouseMovedEvents:YES];
    [g_qr_selection_window setReleasedWhenClosed:NO];

    g_qr_selection_view = [[QRSelectionView alloc] initWithFrame:screenFrame];
    [g_qr_selection_window setContentView:g_qr_selection_view];
    [g_qr_selection_window makeKeyAndOrderFront:nil];
    [g_qr_selection_window makeFirstResponder:g_qr_selection_view];

    // Set crosshair cursor
    [[NSCursor crosshairCursor] set];
}
```

#### FinishQRScreenCaptureMacOS(bool cancelled, NSRect selection)

```objc
void FinishQRScreenCaptureMacOS(bool cancelled, NSRect selection) {
    LOG_INFO("📷 Finishing QR screen capture (macOS) cancelled=" +
             std::string(cancelled ? "true" : "false"));

    // Destroy selection window
    if (g_qr_selection_window) {
        [g_qr_selection_window orderOut:nil];
        [g_qr_selection_window close];
        g_qr_selection_window = nil;
        g_qr_selection_view = nil;
    }

    // Reset cursor
    [[NSCursor arrowCursor] set];

    if (cancelled) {
        ShowWalletOverlay();
        DeliverQRResultMacOS("{\"status\":\"cancelled\"}");
        return;
    }

    // Convert NSView coords (bottom-left origin) to CG coords (top-left origin)
    CGFloat screenHeight = NSScreen.mainScreen.frame.size.height;
    CGRect captureRect = CGRectMake(
        selection.origin.x,
        screenHeight - selection.origin.y - selection.size.height,
        selection.size.width,
        selection.size.height
    );

    // Capture screen region (excludes our overlay since it's already closed)
    CGImageRef image = CGWindowListCreateImage(
        captureRect,
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        kCGWindowImageDefault
    );

    if (!image) {
        ShowWalletOverlay();
        DeliverQRResultMacOS("{\"status\":\"not_found\"}");
        return;
    }

    // Decode QR via native CIDetector
    CIImage* ciImage = [CIImage imageWithCGImage:image];
    CGImageRelease(image);

    CIDetector* detector = [CIDetector detectorOfType:CIDetectorTypeQRCode
                                              context:nil
                                              options:@{CIDetectorAccuracy: CIDetectorAccuracyHigh}];
    NSArray* features = [detector featuresInImage:ciImage];

    NSMutableArray<NSString*>* decoded = [NSMutableArray array];
    for (CIFeature* feature in features) {
        if ([feature isKindOfClass:[CIQRCodeFeature class]]) {
            CIQRCodeFeature* qr = (CIQRCodeFeature*)feature;
            if (qr.messageString) {
                [decoded addObject:qr.messageString];
            }
        }
    }

    ShowWalletOverlay();

    if (decoded.count == 0) {
        DeliverQRResultMacOS("{\"status\":\"not_found\"}");
        return;
    }

    // Classify first BSV-matching result
    for (NSString* text in decoded) {
        std::string json = ClassifyBSVContent([text UTF8String]);
        if (!json.empty()) {
            DeliverQRResultMacOS("{\"status\":\"found\",\"result\":" + json + "}");
            return;
        }
    }

    DeliverQRResultMacOS("{\"status\":\"not_found\"}");
}
```

#### ClassifyBSVContent (reuse from Windows)

Port the BSV classification logic from QRScreenCapture.cpp:91-143. Same 4 regexes:

```cpp
static const std::regex RE_BSV_ADDRESS(R"(^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$)");
static const std::regex RE_IDENTITY_KEY(R"(^(02|03)[0-9a-fA-F]{64}$)");
static const std::regex RE_PAYMAIL(R"(^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,})$)");
static const std::regex RE_BIP21(R"(^bitcoin:)", std::regex_constants::icase);
```

Returns JSON string: `{"type":"address","value":"1A1z...","address":"1A1z...","source":"screen"}` etc.

Can be placed in cef_browser_shell_mac.mm (local to macOS) or extracted to a shared header. Recommend local for now since it's ~50 lines.

#### DeliverQRResultMacOS

```cpp
void DeliverQRResultMacOS(const std::string& json) {
    extern CefRefPtr<CefBrowser> g_qr_scan_requester;
    if (g_qr_scan_requester && g_qr_scan_requester->GetMainFrame()) {
        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("qr_screen_capture_result");
        msg->GetArgumentList()->SetString(0, json);
        g_qr_scan_requester->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
    }
    g_qr_scan_requester = nullptr;
}
```

---

### Step 3: Wire up CMakeLists.txt

Add CoreImage to the macOS framework search (near line 322):

```cmake
find_library(COREIMAGE_LIBRARY CoreImage)
```

Add to link list (near line 356):

```cmake
${COREIMAGE_LIBRARY}
```

---

### IPC flow (already complete — no changes needed)

The render process handler (simple_render_process_handler.cpp:972-999) already forwards all three QR messages cross-platform:
- `qr_scan_result` (line 972) — Phase 1 DOM results → React
- `qr_screen_capture_starting` (line 984) — "selecting..." notification → React
- `qr_screen_capture_result` (line 992) — Phase 2 result → React

The React listener (WalletPanel.tsx:371-426) already handles all three message types including `cancelled`, `not_found`, and `found` statuses.

---

### Screen Recording Permission

`CGWindowListCreateImage` requires Screen Recording permission on macOS 10.15+.

**Check:** `CGPreflightScreenCaptureAccess()` (macOS 11+)
**Prompt:** `CGRequestScreenCaptureAccess()` — opens System Preferences

If permission is denied, `CGWindowListCreateImage` returns a blank/null image. Handle gracefully by delivering `{"status":"error","message":"Screen Recording permission required"}`.

The React side should display this message in the wallet panel's scan status area. Currently `WalletPanel.tsx` doesn't handle `status:"error"` — add a case:

```tsx
if (result.status === 'error') {
    setScanMessage(result.message || 'Screen capture failed');
    setTimeout(() => setScanMessage(null), 5000);
    return;
}
```

---

### Retina / Multi-Monitor

- `CGWindowListCreateImage` captures at native display resolution automatically when given screen coordinates — no manual DPI scaling needed.
- Multi-monitor: The selection window covers `NSUnionRect` of all `NSScreen.screens`. `CGWindowListCreateImage` uses global display coordinates that span all monitors.

---

### Build & Test

```bash
cd ~/Hodos-Browser/frontend && npm run build
cd ~/Hodos-Browser/cef-native && cmake --build build --config Release
cd build/bin && cp -R "HodosBrowser Helper"*.app HodosBrowser.app/Contents/Frameworks/
codesign --force --deep --sign - HodosBrowser.app
export HODOS_DEV=1 HODOS_MAC_DEV_FLAGS=1
./HodosBrowser.app/Contents/MacOS/HodosBrowser
```

**Test matrix:**
1. Open wallet panel → click "Scan QR" → DOM scan runs (Phase 1, should already work)
2. If no QR in DOM → screen capture overlay appears with crosshair
3. Drag to select a QR code on screen → wallet reopens with result populated
4. Press ESC during selection → wallet reopens with "cancelled" message
5. Select area with no QR → wallet reopens with "no QR found" message
6. Test with: BSV address QR, BIP21 URI QR, paymail QR, identity key QR, non-BSV QR (should be ignored)

---

### Files to modify (summary)

| File | Change |
|------|--------|
| `cef-native/cef_browser_shell_mac.mm` | Add QRSelectionView, QRSelectionWindow, StartQRScreenCaptureMacOS(), FinishQRScreenCaptureMacOS(), ClassifyBSVContent(), DeliverQRResultMacOS(), HideWalletOverlay(), ShowWalletOverlay() |
| `cef-native/src/handlers/simple_handler.cpp` | Add `#elif defined(__APPLE__)` in qr_found handler (~line 3405) |
| `cef-native/CMakeLists.txt` | Add CoreImage framework |
| `frontend/src/components/WalletPanel.tsx` | Add `status:"error"` case in QR result handler (~line 415) |

### Reference files (read, don't modify)

| File | Why |
|------|-----|
| `cef-native/src/core/QRScreenCapture.cpp` | Windows reference: BSV regexes (line 47-51), ClassifyAndBuildJson (line 91-143), user flow |
| `cef-native/src/core/WindowManager_mac.mm:270-341` | Ghost tab NSWindow pattern for borderless floating window |
| `cef-native/src/handlers/simple_render_process_handler.cpp:972-999` | IPC forwarding (already done) |
| `frontend/src/components/WalletPanel.tsx:371-426` | React message listener (already done) |
