// src/simple_app.cpp
#include "../../include/handlers/simple_app.h"
#include "../../include/handlers/simple_handler.h"
#include "../../include/handlers/simple_render_process_handler.h"
#include "../../include/handlers/my_overlay_render_handler.h"
#include "include/wrapper/cef_helpers.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include "include/cef_process_message.h"
#include "include/cef_request_context.h"
#include <iostream>
#include <fstream>

#include "../../include/core/TabManager.h"
#include "../../include/core/Logger.h"

// Convenience macros for easier logging
#define LOG_DEBUG_APP(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_APP(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_APP(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_APP(msg) Logger::Log(msg, 3, 2)

#ifdef _WIN32
// External global HWND declarations for shutdown cleanup
extern HWND g_settings_overlay_hwnd;
extern HWND g_wallet_overlay_hwnd;
extern HWND g_backup_overlay_hwnd;
extern HWND g_brc100_auth_overlay_hwnd;
extern HWND g_omnibox_overlay_hwnd;
extern HWND g_cookie_panel_overlay_hwnd;
#endif

SimpleApp::SimpleApp()
    : render_process_handler_(new SimpleRenderProcessHandler()) {
    std::cout << "🔧 SimpleApp constructor called!" << std::endl;
    std::cout << "🔧 Render process handler created: " << (render_process_handler_ ? "true" : "false") << std::endl;
}

CefRefPtr<CefBrowserProcessHandler> SimpleApp::GetBrowserProcessHandler() {
    std::cout << "✅ SimpleApp::GetBrowserProcessHandler CALLED" << std::endl;
    return this;
}

CefRefPtr<CefRenderProcessHandler> SimpleApp::GetRenderProcessHandler() {
    std::cout << "🔧 SimpleApp::GetRenderProcessHandler CALLED" << std::endl;
    std::cout << "🔧 Returning render process handler: " << (render_process_handler_ ? "true" : "false") << std::endl;
    return render_process_handler_;
}

void SimpleApp::OnBeforeCommandLineProcessing(const CefString& process_type,
                                               CefRefPtr<CefCommandLine> command_line) {
    std::wcout << L"OnBeforeCommandLineProcessing for type: " << std::wstring(process_type) << std::endl;

    if (!command_line->HasSwitch("lang")) {
        std::wcout << L"Appending --lang=en-US" << std::endl;
        command_line->AppendSwitchWithValue("lang", "en-US");
    } else {
        std::wcout << L"--lang already present" << std::endl;
    }

    command_line->AppendSwitchWithValue("remote-allow-origins", "*");

    // Fix first-render black screen issue - disable GPU compositing for reliable rendering
    command_line->AppendSwitch("disable-gpu-compositing");

    // Prevent WebRTC from leaking real IP address via STUN requests
    command_line->AppendSwitchWithValue("force-webrtc-ip-handling-policy", "default_public_interface_only");

    // macOS: Use in-process GPU instead of separate GPU process
    // This avoids GPU process launch failures during development
#ifdef __APPLE__
    command_line->AppendSwitch("in-process-gpu");
    command_line->AppendSwitch("disable-gpu-sandbox");
    // CRITICAL: Allow localhost connections for frontend dev server
    command_line->AppendSwitch("allow-loopback-in-sandbox");
    command_line->AppendSwitch("disable-web-security");  // Disable for development
    command_line->AppendSwitch("allow-running-insecure-content");
    LOG_INFO_APP("Using in-process GPU on macOS; web security disabled for localhost dev");
#endif

    // Additional GPU flags (keep commented for now):
    // command_line->AppendSwitch("disable-gpu");
    // command_line->AppendSwitch("disable-gpu-shader-disk-cache");
    // command_line->AppendSwitchWithValue("use-gl", "disabled");
    // command_line->AppendSwitchWithValue("use-angle", "none");

    // command_line->AppendSwitch("allow-running-insecure-content");
    // command_line->AppendSwitch("disable-web-security");
    // command_line->AppendSwitch("disable-site-isolation-trials");
    // command_line->AppendSwitch("no-sandbox");
    // command_line->AppendSwitch("disable-features=RendererCodeIntegrity");

}

#ifdef _WIN32
void SimpleApp::SetWindowHandles(HWND hwnd, HWND header, HWND webview) {
    hwnd_ = hwnd;
    header_hwnd_ = header;
    webview_hwnd_ = webview;
}
#elif defined(__APPLE__)
void SimpleApp::SetMacOSWindow(void* main_window, void* header_view, void* webview_view) {
    main_window_ = main_window;
    header_view_ = header_view;
    webview_view_ = webview_view;
}
#endif

void SimpleApp::OnContextInitialized() {
    CEF_REQUIRE_UI_THREAD();

    // Clear any cached SSL certificate exceptions from previous sessions
    // so OnCertificateError fires fresh every browser launch (session-only exceptions)
    CefRefPtr<CefRequestContext> ctx = CefRequestContext::GetGlobalContext();
    if (ctx) {
        ctx->ClearCertificateExceptions(nullptr);
    }

#ifdef _WIN32
    std::cout << "✅ OnContextInitialized CALLED (Windows)" << std::endl;

    std::ofstream log("startup_log.txt", std::ios::app);
    log << "\n========================================\n";
    log << "🚀 OnContextInitialized entered\n";
    log << "→ this pointer: " << this << "\n";
    log << "→ member header_hwnd_: " << header_hwnd_ << "\n";
    log << "→ global g_header_hwnd: " << g_header_hwnd << "\n";
    log << "→ global g_hwnd: " << g_hwnd << "\n";
    log << "→ IsWindow(g_header_hwnd): " << IsWindow(g_header_hwnd) << "\n";
    log << "→ IsWindow(g_hwnd): " << IsWindow(g_hwnd) << "\n";
    log << "→ Proceeding without guard (original 625af25 behavior)\n";
    log << "========================================\n";
    log.close();

    std::ofstream log_trace("startup_log.txt", std::ios::app);
    log_trace << "🔍 Starting header browser setup...\n";
    log_trace.flush();

    // ───── header Browser Setup ─────
    RECT headerRect;
    log_trace << "🔍 About to call GetClientRect on g_header_hwnd: " << g_header_hwnd << "\n";
    log_trace.flush();

    GetClientRect(g_header_hwnd, &headerRect);

    log_trace << "🔍 GetClientRect succeeded\n";
    log_trace.close();
    int headerWidth = headerRect.right - headerRect.left;
    int headerHeight = headerRect.bottom - headerRect.top;

    std::ofstream log2("startup_log.txt", std::ios::app);
    log2 << "📊 Header setup:\n";
    log2 << "→ g_header_hwnd: " << g_header_hwnd << "\n";
    log2 << "→ IsWindow(g_header_hwnd): " << IsWindow(g_header_hwnd) << "\n";
    log2 << "→ IsWindowVisible(g_header_hwnd): " << IsWindowVisible(g_header_hwnd) << "\n";
    log2 << "→ headerRect: " << headerWidth << "x" << headerHeight << "\n";

    CefWindowInfo header_window_info;
    log2 << "🔍 Before SetAsChild - window_info has parent: " << (header_window_info.parent_window != nullptr) << "\n";
    header_window_info.SetAsChild(g_header_hwnd, CefRect(0, 0, headerWidth, headerHeight));
    log2 << "🔍 After SetAsChild - window_info has parent: " << (header_window_info.parent_window != nullptr) << "\n";
    log2 << "🔍 After SetAsChild - parent HWND: " << header_window_info.parent_window << "\n";
    log2.close();

    CefRefPtr<SimpleHandler> header_handler = new SimpleHandler("header");
    CefBrowserSettings header_settings;
    std::string header_url = "http://127.0.0.1:5137";
    std::cout << "Loading React header at: " << header_url << std::endl;

    try {
        bool header_result = CefBrowserHost::CreateBrowser(
            header_window_info,
            header_handler,
            header_url,
            header_settings,
            nullptr,
            CefRequestContext::GetGlobalContext()
        );
        std::cout << "header browser created: " << (header_result ? "true" : "false") << std::endl;

        std::ofstream log3("startup_log.txt", std::ios::app);
        log3 << "✅ Header browser creation result: " << (header_result ? "success" : "failed") << "\n";
        log3.close();
    } catch (...) {
        std::ofstream errLog("startup_log.txt", std::ios::app);
        errLog << "❌ header browser creation threw an exception!\n";
        errLog.close();
    }

    // ───── Initial Tab Creation (replaces single webview) ─────
    RECT mainRect;
    GetClientRect(g_hwnd, &mainRect);
    int width = mainRect.right - mainRect.left;
    int height = mainRect.bottom - mainRect.top;
    int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
    int tabHeight = height - shellHeight;

    LOG_INFO_APP("📑 Creating initial tab with TabManager...");

    std::ofstream log4("startup_log.txt", std::ios::app);
    log4 << "📊 Tab setup:\n";
    log4 << "→ g_hwnd: " << g_hwnd << "\n";
    log4 << "→ IsWindow(g_hwnd): " << IsWindow(g_hwnd) << "\n";
    log4 << "→ tabHeight: " << tabHeight << "\n";
    log4.close();

    try {
        int initial_tab_id = TabManager::GetInstance().CreateTab(
            "https://coingeek.com/",
            g_hwnd,
            0,              // x position
            shellHeight,    // y position (below header)
            width,
            tabHeight
        );

        LOG_INFO_APP("✅ Initial tab created: ID " + std::to_string(initial_tab_id));
        std::cout << "Initial tab created: ID " << initial_tab_id << std::endl;

        std::ofstream log5("startup_log.txt", std::ios::app);
        log5 << "✅ Initial tab creation result: ID = " << initial_tab_id << "\n";
        log5.close();
    } catch (...) {
        std::ofstream errLog("startup_log.txt", std::ios::app);
        errLog << "❌ Initial tab creation threw an exception!\n";
        errLog.close();
        LOG(ERROR) << "Failed to create initial tab";
    }

#elif defined(__APPLE__)
    std::cout << "✅ OnContextInitialized CALLED (macOS)" << std::endl;
    LOG_INFO_APP("✅ OnContextInitialized CALLED (macOS)");

    // On macOS, browsers are created manually in main() after windows are set up
    // This callback runs too early (before windows exist), so we skip it
    LOG_INFO_APP("🔧 Browsers will be created manually after window setup");

    // Tab system not implemented on macOS yet
    LOG_INFO_APP("🔧 Tab system not implemented on macOS yet");
#endif
}



// Chrome-style approach: Inject JavaScript directly into the overlay browser
void InjectHodosBrowserAPI(CefRefPtr<CefBrowser> browser) {
    if (!browser || !browser->GetMainFrame()) {
        std::cout << "❌ Cannot inject API - browser or frame not available" << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "❌ Cannot inject API - browser or frame not available" << std::endl;
        debugLog.close();
        return;
    }

    std::cout << "🔧 Injecting hodosBrowser API into browser ID: " << browser->GetIdentifier() << std::endl;
    std::ofstream debugLog1("debug_output.log", std::ios::app);
    debugLog1 << "🔧 Injecting hodosBrowser API into browser ID: " << browser->GetIdentifier() << std::endl;
    debugLog1.close();

    std::string jsCode = R"(
                 // Create hodosBrowser object using CEF's built-in V8 integration
                 window.hodosBrowser = {
                     address: {
                         generate: function() {
                             console.log('🔑 Address generation requested via injected JavaScript');

                             // Also try to log to a visible element for debugging
                             var debugDiv = document.getElementById('debug-log');
                             if (!debugDiv) {
                                 debugDiv = document.createElement('div');
                                 debugDiv.id = 'debug-log';
                                 debugDiv.style.position = 'fixed';
                                 debugDiv.style.top = '10px';
                                 debugDiv.style.left = '10px';
                                 debugDiv.style.background = 'black';
                                 debugDiv.style.color = 'white';
                                 debugDiv.style.padding = '10px';
                                 debugDiv.style.zIndex = '9999';
                                 debugDiv.style.fontSize = '12px';
                                 document.body.appendChild(debugDiv);
                             }
                             debugDiv.innerHTML += '🔑 Address generation requested<br>';

                             // Return a Promise for async operation
                             return new Promise((resolve, reject) => {
                                 try {
                                     // Use CEF's process message system
                                     if (window.chrome && window.chrome.runtime && window.chrome.runtime.sendMessage) {
                                         debugDiv.innerHTML += '📤 Sending process message<br>';
                                         window.chrome.runtime.sendMessage({
                                             type: 'address_generate'
                                         }, function(response) {
                                             debugDiv.innerHTML += '📥 Response received<br>';
                                             console.log('🔍 Response received:', JSON.stringify(response));
                                             if (response && response.success) {
                                                 debugDiv.innerHTML += '✅ Address generated successfully<br>';
                                                 console.log('✅ Address generated:', response.data);
                                                 console.log('🔍 Address field:', response.data.address);
                                                 console.log('🔍 PublicKey field:', response.data.publicKey);
                                                 console.log('🔍 PrivateKey field:', response.data.privateKey);
                                                 resolve(response.data);
                                             } else {
                                                 debugDiv.innerHTML += '❌ Address generation failed<br>';
                                                 console.error('❌ Address generation failed:', response ? response.error : 'Unknown error');
                                                 reject(new Error(response ? response.error : 'Unknown error'));
                                             }
                                         });
                                     } else {
                                         debugDiv.innerHTML += '❌ CEF runtime not available<br>';
                                         console.error('❌ CEF runtime not available, trying alternative method');
                                         // Fallback: try to call a global function
                                         if (window.generateAddress) {
                                             try {
                                                 var result = window.generateAddress();
                                                 debugDiv.innerHTML += '✅ Address generated via fallback<br>';
                                                 console.log('✅ Address generated via fallback:', result);
                                                 resolve(result);
                                             } catch (e) {
                                                 debugDiv.innerHTML += '❌ Error in fallback<br>';
                                                 console.error('❌ Error in fallback address generation:', e);
                                                 reject(e);
                                             }
                                         } else {
                                             debugDiv.innerHTML += '❌ No address generation method available<br>';
                                             console.error('❌ No address generation method available');
                                             reject(new Error('No address generation method available'));
                                         }
                                     }
                                 } catch (e) {
                                     debugDiv.innerHTML += '❌ Error in address generation<br>';
                                     console.error('❌ Error in address generation:', e);
                                     reject(e);
                                 }
                             });
                         }
                     },
                     overlay: {
                         show: function() {
                             console.log('🧪 Test overlay requested via hodosBrowser API');
                             // Send process message for test overlay
                             if (window.chrome && window.chrome.runtime && window.chrome.runtime.sendMessage) {
                                 window.chrome.runtime.sendMessage({
                                     type: 'test_overlay'
                                 }, function(response) {
                                     console.log('🧪 Test overlay response:', response);
                                 });
                             } else {
                                 console.error('❌ CEF runtime not available for test overlay');
                             }
                         }
                     }
                 };

                // cefMessage is now implemented in the render process handler
                // No need to set it up here as a stub


        console.log('✅ hodosBrowser API injected successfully');
    )";

    browser->GetMainFrame()->ExecuteJavaScript(jsCode, "", 0);
    std::cout << "🔧 Injected hodosBrowser API into browser ID: " << browser->GetIdentifier() << std::endl;

    // Also log to file
    std::ofstream debugLog2("debug_output.log", std::ios::app);
    debugLog2 << "🔧 Injected hodosBrowser API into browser ID: " << browser->GetIdentifier() << std::endl;
    debugLog2.close();
}

#ifdef _WIN32
void CreateSettingsOverlayWithSeparateProcess(HINSTANCE hInstance, int iconRightOffset) {
    LOG_INFO_APP("Creating settings overlay with iconRightOffset=" + std::to_string(iconRightOffset));

    // Store offset globally for WM_SIZE/WM_MOVE repositioning
    extern int g_settings_icon_right_offset;
    g_settings_icon_right_offset = iconRightOffset;

    // Get main window and header dimensions for positioning
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    extern HWND g_header_hwnd;
    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);

    // Right-side popup panel, right edge aligned under the icon
    int panelWidth = 450;
    int panelHeight = 450;
    int overlayX = headerRect.right - iconRightOffset - panelWidth;
    int overlayY = headerRect.top + 104;

    // Clamp so panel doesn't extend below or outside main window
    if (overlayY + panelHeight > mainRect.bottom) {
        panelHeight = mainRect.bottom - overlayY;
        if (panelHeight < 200) panelHeight = 200; // minimum usable height
    }

    LOG_DEBUG_APP("⚙️ Settings panel position: (" + std::to_string(overlayX) + ", " + std::to_string(overlayY)
                  + ") size: " + std::to_string(panelWidth) + "x" + std::to_string(panelHeight));

    // Remove existing mouse hook if present
    extern HHOOK g_settings_mouse_hook;
    if (g_settings_mouse_hook) {
        UnhookWindowsHookEx(g_settings_mouse_hook);
        g_settings_mouse_hook = nullptr;
    }

    // Check if overlay already exists - destroy old one first
    if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd)) {
        LOG_DEBUG_APP("⚙️ Settings overlay already exists, destroying old one");
        DestroyWindow(g_settings_overlay_hwnd);
        g_settings_overlay_hwnd = nullptr;
    }

    // Create new HWND for settings overlay
    HWND settings_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFSettingsOverlayWindow",
        L"Settings Overlay",
        WS_POPUP,
        overlayX, overlayY, panelWidth, panelHeight,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!settings_hwnd) {
        LOG_ERROR_APP("❌ Failed to create settings overlay HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    // Force position and make visible
    SetWindowPos(settings_hwnd, HWND_TOPMOST,
        overlayX, overlayY, panelWidth, panelHeight,
        SWP_NOACTIVATE | SWP_SHOWWINDOW);

    g_settings_overlay_hwnd = settings_hwnd;

    // Create CEF browser with windowless rendering
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(settings_hwnd, "SettingsOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> settings_handler(new SimpleHandler("settings"));
    CefRefPtr<MyOverlayRenderHandler> render_handler = new MyOverlayRenderHandler(settings_hwnd, panelWidth, panelHeight);
    settings_handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        settings_handler,
        "http://127.0.0.1:5137/settings",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (result) {
        LOG_INFO_APP("✅ Settings overlay browser created");
        // Enable mouse input
        LONG exStyle = GetWindowLong(settings_hwnd, GWL_EXSTYLE);
        SetWindowLong(settings_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);

        // Install global mouse hook for click-outside detection
        extern LRESULT CALLBACK SettingsPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam);
        g_settings_mouse_hook = SetWindowsHookEx(WH_MOUSE_LL, SettingsPanelMouseHookProc, nullptr, 0);
        if (g_settings_mouse_hook) {
            LOG_INFO_APP("✅ Settings panel mouse hook installed for click-outside detection");
        } else {
            LOG_WARNING_APP("⚠️ Failed to install settings panel mouse hook. Error: " + std::to_string(GetLastError()));
        }
    } else {
        LOG_ERROR_APP("❌ Failed to create settings overlay browser");
    }
}
#endif // _WIN32

#ifdef _WIN32
void CreateWalletOverlayWithSeparateProcess(HINSTANCE hInstance, int iconRightOffset) {
    LOG_INFO_APP("Creating wallet overlay with iconRightOffset=" + std::to_string(iconRightOffset));

    // Store offset globally for repositioning
    extern int g_wallet_icon_right_offset;
    g_wallet_icon_right_offset = iconRightOffset;

    // Get main window dimensions for positioning
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    int width = mainRect.right - mainRect.left;
    int height = mainRect.bottom - mainRect.top;

    // DEBUG: Log the position we're using
    LOG_INFO_APP("💰 Main window position: (" + std::to_string(mainRect.left) + ", " +
        std::to_string(mainRect.top) + ") size: " + std::to_string(width) + "x" + std::to_string(height));
    LOG_INFO_APP("💰 Creating overlay at these coordinates");

    // Check if overlay already exists
    if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd)) {
        LOG_WARNING_APP("💰 Wallet overlay already exists! Destroying old one first.");
        DestroyWindow(g_wallet_overlay_hwnd);
        g_wallet_overlay_hwnd = nullptr;
    }

    // Create new HWND for wallet overlay
    LOG_INFO_APP("💰 Creating wallet overlay HWND at position: (" +
        std::to_string(mainRect.left) + ", " + std::to_string(mainRect.top) + ")");

    HWND wallet_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFWalletOverlayWindow",
        L"Wallet Overlay",
        WS_POPUP | WS_VISIBLE,
        mainRect.left, mainRect.top, width, height,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!wallet_hwnd) {
        std::cout << "❌ Failed to create wallet overlay HWND. Error: " << GetLastError() << std::endl;
        LOG_ERROR_APP("❌ Failed to create wallet overlay HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    // Verify the created window position
    RECT createdRect;
    GetWindowRect(wallet_hwnd, &createdRect);
    LOG_INFO_APP("✅ Wallet overlay HWND created at actual position: (" +
        std::to_string(createdRect.left) + ", " + std::to_string(createdRect.top) +
        ") size: " + std::to_string(createdRect.right - createdRect.left) + "x" +
        std::to_string(createdRect.bottom - createdRect.top));

    // WORKAROUND: Force position in case Windows cached the old position
    if (createdRect.left != mainRect.left || createdRect.top != mainRect.top) {
        LOG_WARNING_APP("🔧 Window position mismatch! Forcing correct position...");
        LOG_WARNING_APP("🔧 Expected: (" + std::to_string(mainRect.left) + ", " + std::to_string(mainRect.top) + ")");
        LOG_WARNING_APP("🔧 Actual: (" + std::to_string(createdRect.left) + ", " + std::to_string(createdRect.top) + ")");

        SetWindowPos(wallet_hwnd, HWND_TOPMOST,
            mainRect.left, mainRect.top, width, height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW);

        // Verify again
        GetWindowRect(wallet_hwnd, &createdRect);
        LOG_INFO_APP("🔧 After forcing position: (" +
            std::to_string(createdRect.left) + ", " + std::to_string(createdRect.top) + ")");
    }

    // Store HWND for shutdown cleanup
    g_wallet_overlay_hwnd = wallet_hwnd;

    std::ofstream debugLog3("debug_output.log", std::ios::app);
    debugLog3 << "✅ Wallet overlay HWND created: " << wallet_hwnd << std::endl;
    debugLog3.close();

    // Create new CEF browser with subprocess
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(wallet_hwnd, "WalletOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0); // fully transparent
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    // Note: DevTools is enabled through context menu handler, not browser settings

    // Create new handler for wallet overlay
    CefRefPtr<SimpleHandler> wallet_handler(new SimpleHandler("wallet"));

    // Set render handler for wallet overlay (same as settings overlay)
    CefRefPtr<MyOverlayRenderHandler> render_handler = new MyOverlayRenderHandler(wallet_hwnd, width, height);
    wallet_handler->SetRenderHandler(render_handler);

    // Create new browser with subprocess (pass icon offset as URL param for CSS positioning)
    std::string walletUrl = "http://127.0.0.1:5137/wallet-panel?iro=" + std::to_string(iconRightOffset);
    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        wallet_handler,
        walletUrl,
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (result) {
        std::cout << "✅ Wallet overlay browser created with subprocess" << std::endl;
        std::ofstream debugLog4("debug_output.log", std::ios::app);
        debugLog4 << "✅ Wallet overlay browser created with subprocess" << std::endl;
        debugLog4.close();

        // Enable mouse input for wallet overlay
        LONG exStyle = GetWindowLong(wallet_hwnd, GWL_EXSTYLE);
        SetWindowLong(wallet_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
        std::ofstream debugLog6("debug_output.log", std::ios::app);
        debugLog6 << "💰 Mouse input ENABLED for wallet overlay HWND: " << wallet_hwnd << std::endl;
        debugLog6.close();
    } else {
        std::cout << "❌ Failed to create wallet overlay browser" << std::endl;
        std::ofstream debugLog5("debug_output.log", std::ios::app);
        debugLog5 << "❌ Failed to create wallet overlay browser" << std::endl;
        debugLog5.close();
    }
}
#endif // _WIN32

#ifdef _WIN32
void CreateBackupOverlayWithSeparateProcess(HINSTANCE hInstance) {
    std::cout << "💾 Creating backup overlay with separate process" << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "💾 Creating backup overlay with separate process" << std::endl;
    debugLog.close();

    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    int width = mainRect.right - mainRect.left;
    int height = mainRect.bottom - mainRect.top;

    HWND backup_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFBackupOverlayWindow",
        L"Backup Overlay",
        WS_POPUP | WS_VISIBLE,
        mainRect.left, mainRect.top, width, height,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!backup_hwnd) {
        std::cout << "❌ Failed to create backup overlay HWND. Error: " << GetLastError() << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "❌ Failed to create backup overlay HWND. Error: " << GetLastError() << std::endl;
        debugLog2.close();
        return;
    }

    std::cout << "✅ Backup overlay HWND created: " << backup_hwnd << std::endl;

    // Store HWND for shutdown cleanup
    g_backup_overlay_hwnd = backup_hwnd;

    std::ofstream debugLog3("debug_output.log", std::ios::app);
    debugLog3 << "✅ Backup overlay HWND created: " << backup_hwnd << std::endl;
    debugLog3.close();

    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(backup_hwnd, "BackupOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> backup_handler(new SimpleHandler("backup"));
    CefRefPtr<MyOverlayRenderHandler> render_handler = new MyOverlayRenderHandler(backup_hwnd, width, height);
    backup_handler->SetRenderHandler(render_handler);

    std::ofstream debugLog4("debug_output.log", std::ios::app);
    debugLog4 << "💾 Backup overlay render handler set for HWND: " << backup_hwnd << std::endl;
    debugLog4.close();

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        backup_handler,
        "http://127.0.0.1:5137/backup",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (result) {
        std::cout << "✅ Backup overlay browser created with subprocess" << std::endl;
        std::ofstream debugLog4("debug_output.log", std::ios::app);
        debugLog4 << "✅ Backup overlay browser created with subprocess" << std::endl;
        debugLog4.close();

        LONG exStyle = GetWindowLong(backup_hwnd, GWL_EXSTYLE);
        SetWindowLong(backup_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
        std::ofstream debugLog6("debug_output.log", std::ios::app);
        debugLog6 << "💾 Mouse input ENABLED for backup overlay HWND: " << backup_hwnd << std::endl;
        debugLog6.close();

    } else {
        std::cout << "❌ Failed to create backup overlay browser" << std::endl;
        std::ofstream debugLog5("debug_output.log", std::ios::app);
        debugLog5 << "❌ Failed to create backup overlay browser" << std::endl;
        debugLog5.close();
    }
}
#endif // _WIN32

#ifdef _WIN32
void CreateBRC100AuthOverlayWithSeparateProcess(HINSTANCE hInstance) {
    std::cout << "🔐 Creating BRC-100 auth overlay with separate process" << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🔐 Creating BRC-100 auth overlay with separate process" << std::endl;
    debugLog.close();

    // Get main window dimensions for positioning
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    int width = mainRect.right - mainRect.left;
    int height = mainRect.bottom - mainRect.top;

    // Create new HWND for BRC-100 auth overlay
    HWND auth_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFBRC100AuthOverlayWindow",
        L"BRC-100 Auth Overlay",
        WS_POPUP | WS_VISIBLE,
        mainRect.left, mainRect.top, width, height,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!auth_hwnd) {
        std::cout << "❌ Failed to create BRC-100 auth overlay HWND. Error: " << GetLastError() << std::endl;
        std::ofstream debugLog2("debug_output.log", std::ios::app);
        debugLog2 << "❌ Failed to create BRC-100 auth overlay HWND. Error: " << GetLastError() << std::endl;
        debugLog2.close();
        return;
    }

    std::cout << "✅ BRC-100 auth overlay HWND created: " << auth_hwnd << std::endl;

    // Store HWND for shutdown cleanup
    g_brc100_auth_overlay_hwnd = auth_hwnd;

    std::ofstream debugLog3("debug_output.log", std::ios::app);
    debugLog3 << "✅ BRC-100 auth overlay HWND created: " << auth_hwnd << std::endl;
    debugLog3.close();

    // Create new CEF browser with subprocess
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(auth_hwnd, "BRC100AuthOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0); // fully transparent
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    // Create new handler for BRC-100 auth overlay
    CefRefPtr<SimpleHandler> auth_handler(new SimpleHandler("brc100auth"));

    // Set render handler for BRC-100 auth overlay
    CefRefPtr<MyOverlayRenderHandler> render_handler = new MyOverlayRenderHandler(auth_hwnd, width, height);
    auth_handler->SetRenderHandler(render_handler);

    // Create new browser with subprocess
    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        auth_handler,
        "http://127.0.0.1:5137/brc100-auth",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (result) {
        std::cout << "✅ BRC-100 auth overlay browser created with subprocess" << std::endl;
        std::ofstream debugLog4("debug_output.log", std::ios::app);
        debugLog4 << "✅ BRC-100 auth overlay browser created with subprocess" << std::endl;
        debugLog4.close();

        // Enable mouse input for BRC-100 auth overlay
        LONG exStyle = GetWindowLong(auth_hwnd, GWL_EXSTYLE);
        SetWindowLong(auth_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
        std::ofstream debugLog6("debug_output.log", std::ios::app);
        debugLog6 << "🔐 Mouse input ENABLED for BRC-100 auth overlay HWND: " << auth_hwnd << std::endl;
        debugLog6.close();

        // Force a repaint to ensure the overlay is visible
        InvalidateRect(auth_hwnd, nullptr, TRUE);
        UpdateWindow(auth_hwnd);
        std::ofstream debugLog7("debug_output.log", std::ios::app);
        debugLog7 << "🔐 Forced repaint for BRC-100 auth overlay HWND: " << auth_hwnd << std::endl;
        debugLog7.close();
    } else {
        std::cout << "❌ Failed to create BRC-100 auth overlay browser" << std::endl;
        std::ofstream debugLog5("debug_output.log", std::ios::app);
        debugLog5 << "❌ Failed to create BRC-100 auth overlay browser" << std::endl;
        debugLog5.close();
    }
}

void CreateNotificationOverlay(HINSTANCE hInstance, const std::string& type, const std::string& domain, const std::string& extraParams) {
    LOG_INFO_APP("🔔 Creating notification overlay (type: " + type + ", domain: " + domain + ")");

    extern HWND g_notification_overlay_hwnd;

    // Build URL — preload loads the actual React app (idle state) so JS bundle is warm
    std::string url = "http://127.0.0.1:5137/brc100-auth?type=idle";
    std::string queryString = "type=" + type + "&domain=" + domain;
    if (!extraParams.empty()) queryString += extraParams;
    if (type != "preload") {
        url = "http://127.0.0.1:5137/brc100-auth?" + queryString;
    }

    // Keep-alive: if HWND and browser already exist, use JS injection (instant, no page navigation)
    CefRefPtr<CefBrowser> existing = SimpleHandler::GetNotificationBrowser();
    if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd) && existing) {
        LOG_INFO_APP("🔔 Reusing existing notification overlay (keep-alive, JS injection)");

        // Resize to match current main window
        RECT mainRect;
        GetWindowRect(g_hwnd, &mainRect);
        int width = mainRect.right - mainRect.left;
        int height = mainRect.bottom - mainRect.top;

        SetWindowPos(g_notification_overlay_hwnd, HWND_TOPMOST,
            mainRect.left, mainRect.top, width, height,
            SWP_SHOWWINDOW);

        if (type != "preload") {
            // Escape single quotes in the query string for JS
            std::string safeQuery = queryString;
            size_t pos = 0;
            while ((pos = safeQuery.find('\'', pos)) != std::string::npos) {
                safeQuery.replace(pos, 1, "\\'");
                pos += 2;
            }

            // Call window.showNotification() — instant React state update, no page load
            std::string js = "if(window.showNotification){window.showNotification('" + safeQuery + "')}else{window.location.search='?" + safeQuery + "'}";
            existing->GetMainFrame()->ExecuteJavaScript(js, "", 0);
        }

        // Notify CEF of potential resize
        existing->GetHost()->WasResized();

        // Ensure mouse input enabled
        LONG exStyle = GetWindowLong(g_notification_overlay_hwnd, GWL_EXSTYLE);
        SetWindowLong(g_notification_overlay_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);

        InvalidateRect(g_notification_overlay_hwnd, nullptr, TRUE);
        UpdateWindow(g_notification_overlay_hwnd);
        return;
    }

    // First time or stale HWND: clean up and create fresh
    if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd)) {
        CefRefPtr<CefBrowser> old_browser = SimpleHandler::GetNotificationBrowser();
        if (old_browser) {
            old_browser->GetHost()->CloseBrowser(false);
        }
        DestroyWindow(g_notification_overlay_hwnd);
        g_notification_overlay_hwnd = nullptr;
    }

    // Full-screen overlay matching main window
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    int width = mainRect.right - mainRect.left;
    int height = mainRect.bottom - mainRect.top;

    DWORD windowStyle = WS_POPUP;
    if (type != "preload") {
        windowStyle |= WS_VISIBLE;
    }

    HWND notif_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFNotificationOverlayWindow",
        L"Notification Overlay",
        windowStyle,
        mainRect.left, mainRect.top, width, height,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!notif_hwnd) {
        LOG_ERROR_APP("Failed to create notification overlay HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    g_notification_overlay_hwnd = notif_hwnd;

    // Windowless CEF browser with render handler
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(notif_hwnd, "NotificationOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);
    settings.javascript = STATE_ENABLED;
    settings.javascript_access_clipboard = STATE_ENABLED;
    settings.javascript_dom_paste = STATE_ENABLED;

    CefRefPtr<SimpleHandler> notif_handler(new SimpleHandler("notification"));
    CefRefPtr<MyOverlayRenderHandler> render_handler = new MyOverlayRenderHandler(notif_hwnd, width, height);
    notif_handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        notif_handler,
        url,
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext());

    if (result) {
        LOG_INFO_APP("🔔 Notification overlay browser created (first time)");
        LONG exStyle = GetWindowLong(notif_hwnd, GWL_EXSTYLE);
        SetWindowLong(notif_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);

        if (type == "preload") {
            // Pre-created: hide immediately, browser warms up in background
            ShowWindow(notif_hwnd, SW_HIDE);
            LOG_INFO_APP("🔔 Notification overlay pre-created (hidden)");
        } else {
            InvalidateRect(notif_hwnd, nullptr, TRUE);
            UpdateWindow(notif_hwnd);
        }
    } else {
        LOG_ERROR_APP("Failed to create notification overlay browser");
    }
}
#endif // _WIN32

#ifdef _WIN32
void CreateSettingsMenuOverlay(HINSTANCE hInstance) {
    LOG_INFO_APP("📋 Creating settings menu dropdown overlay");

    // Check if overlay already exists
    if (g_settings_menu_overlay_hwnd && IsWindow(g_settings_menu_overlay_hwnd)) {
        LOG_INFO_APP("📋 Settings menu overlay already exists, closing it");
        DestroyWindow(g_settings_menu_overlay_hwnd);
        g_settings_menu_overlay_hwnd = nullptr;
        return; // Toggle behavior - click again to close
    }

    // Get main window dimensions
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    int mainWidth = mainRect.right - mainRect.left;

    // Small dropdown dimensions
    int menuWidth = 200;
    int menuHeight = 120;

    // Position below toolbar (below both wallet and settings icons)
    // Tab bar = 40px, Toolbar = 54px, total header = 94px
    int menuX = mainRect.left + mainWidth - menuWidth - 10; // 10px from right edge
    int menuY = mainRect.top + 100; // Below header + small gap (94px header + 6px gap)

    LOG_INFO_APP("📋 Creating menu at position: (" + std::to_string(menuX) + ", " + std::to_string(menuY) + ")");

    // Create small popup window
    HWND menu_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFSettingsMenuOverlayWindow",
        L"Settings Menu",
        WS_POPUP,
        menuX, menuY, menuWidth, menuHeight,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!menu_hwnd) {
        LOG_ERROR_APP("❌ Failed to create settings menu HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    // Force position and show
    SetWindowPos(menu_hwnd, HWND_TOPMOST,
        menuX, menuY, menuWidth, menuHeight,
        SWP_NOACTIVATE | SWP_SHOWWINDOW);

    g_settings_menu_overlay_hwnd = menu_hwnd;
    LOG_INFO_APP("✅ Settings menu HWND created: " + std::to_string(reinterpret_cast<intptr_t>(menu_hwnd)));

    // Create CEF browser for the menu
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(menu_hwnd, "SettingsMenu");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 255, 255, 255); // white background
    settings.javascript = STATE_ENABLED;

    CefRefPtr<SimpleHandler> menu_handler(new SimpleHandler("settings_menu"));
    CefRefPtr<MyOverlayRenderHandler> render_handler = new MyOverlayRenderHandler(menu_hwnd, menuWidth, menuHeight);
    menu_handler->SetRenderHandler(render_handler);

    bool result = CefBrowserHost::CreateBrowser(
        window_info,
        menu_handler,
        "http://127.0.0.1:5137/settings-menu",
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    if (result) {
        LOG_INFO_APP("✅ Settings menu browser created");

        // Enable mouse input
        LONG exStyle = GetWindowLong(menu_hwnd, GWL_EXSTYLE);
        SetWindowLong(menu_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
    } else {
        LOG_ERROR_APP("❌ Failed to create settings menu browser");
    }
}

void CreateOmniboxOverlay(HINSTANCE hInstance, bool showImmediately) {
    LOG_INFO_APP("🔍 Creating omnibox overlay with keep-alive pattern (showImmediately=" +
                 std::string(showImmediately ? "true" : "false") + ")");

    // Keep-alive check: if HWND already exists, conditionally show it
    if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd)) {
        LOG_INFO_APP("🔍 Omnibox overlay already exists");
        if (showImmediately) {
            ShowOmniboxOverlay();
        }
        return;
    }

    // Get main window dimensions
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);

    // Get header dimensions
    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);

    // Calculate position from header geometry
    // Tab bar height: 40px, Toolbar height: 54px (total 94px)
    // Address bar left offset: 8px padding + (3 buttons * 34px) + (3 gaps * 6px) = 128px
    // Address bar right offset: similar for 3 right buttons = ~128px from right edge
    int overlayX = mainRect.left + 160;
    int overlayY = headerRect.top + 104;  // Flush below toolbar
    int overlayWidth = (headerRect.right - headerRect.left) - 152 - 152;
    int overlayHeight = 350;  // Max height, will be dynamically adjusted by content later

    LOG_INFO_APP("🔍 Creating omnibox overlay at position: (" + std::to_string(overlayX) + ", " +
                 std::to_string(overlayY) + ") size: " + std::to_string(overlayWidth) + "x" +
                 std::to_string(overlayHeight));

    // Create HWND for omnibox overlay
    HWND omnibox_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFOmniboxOverlayWindow",
        L"Omnibox Overlay",
        WS_POPUP,
        overlayX, overlayY, overlayWidth, overlayHeight,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!omnibox_hwnd) {
        LOG_ERROR_APP("❌ Failed to create omnibox overlay HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    // Force position with SWP_NOACTIVATE (conditionally show)
    UINT flags = SWP_NOACTIVATE;
    if (showImmediately) {
        flags |= SWP_SHOWWINDOW;
    } else {
        flags |= SWP_HIDEWINDOW;
    }
    SetWindowPos(omnibox_hwnd, HWND_TOPMOST,
        overlayX, overlayY, overlayWidth, overlayHeight,
        flags);

    // Store HWND globally
    g_omnibox_overlay_hwnd = omnibox_hwnd;
    LOG_INFO_APP("✅ Omnibox overlay HWND created: " + std::to_string(reinterpret_cast<intptr_t>(omnibox_hwnd)) +
                 " (visible=" + std::string(showImmediately ? "true" : "false") + ")");

    // Create CEF browser subprocess for omnibox overlay
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(omnibox_hwnd, "OmniboxOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);  // transparent background
    settings.javascript = STATE_ENABLED;

    CefRefPtr<SimpleHandler> omnibox_handler(new SimpleHandler("omnibox"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler(omnibox_hwnd, overlayWidth, overlayHeight);
    omnibox_handler->SetRenderHandler(render_handler);

    // Minimal isolation: use global context (shared cache/cookies as per CONTEXT.md decision)
    bool result = CefBrowserHost::CreateBrowser(
        window_info, omnibox_handler,
        "http://127.0.0.1:5137/omnibox",
        settings, nullptr,
        CefRequestContext::GetGlobalContext());

    if (result) {
        LOG_INFO_APP("✅ Omnibox overlay browser created with subprocess");
    } else {
        LOG_ERROR_APP("❌ Failed to create omnibox overlay browser");
    }
}

void ShowOmniboxOverlay() {
    // Guard: verify HWND exists
    if (!g_omnibox_overlay_hwnd || !IsWindow(g_omnibox_overlay_hwnd)) {
        LOG_WARNING_APP("⚠️ Cannot show omnibox overlay - HWND does not exist");
        return;
    }

    LOG_INFO_APP("🔍 Showing omnibox overlay");

    // Install global mouse hook for click-outside detection
    extern HHOOK g_omnibox_mouse_hook;
    extern LRESULT CALLBACK OmniboxMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam);
    if (!g_omnibox_mouse_hook) {
        g_omnibox_mouse_hook = SetWindowsHookEx(WH_MOUSE_LL, OmniboxMouseHookProc, nullptr, 0);
        if (g_omnibox_mouse_hook) {
            LOG_INFO_APP("✅ Omnibox mouse hook installed for click-outside detection");
        } else {
            LOG_WARNING_APP("⚠️ Failed to install omnibox mouse hook. Error: " + std::to_string(GetLastError()));
        }
    }

    // Recalculate position in case address bar moved
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);

    int overlayX = mainRect.left + 160;
    int overlayY = headerRect.top + 104;
    int overlayWidth = (headerRect.right - headerRect.left) - 152 - 152;
    int overlayHeight = 350;

    // Force position and show with SWP_NOACTIVATE
    SetWindowPos(g_omnibox_overlay_hwnd, HWND_TOPMOST,
        overlayX, overlayY, overlayWidth, overlayHeight,
        SWP_NOACTIVATE | SWP_SHOWWINDOW);

    // Remove WS_EX_TRANSPARENT to enable mouse input
    LONG exStyle = GetWindowLong(g_omnibox_overlay_hwnd, GWL_EXSTYLE);
    SetWindowLong(g_omnibox_overlay_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);

    // Clear any persistent focus states by executing blur on active element
    CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
    if (omnibox_browser && omnibox_browser->GetMainFrame()) {
        omnibox_browser->GetHost()->WasResized();
        omnibox_browser->GetHost()->Invalidate(PET_VIEW);

        // Execute JavaScript to blur any focused elements
        std::string blurJs = "if (document.activeElement) { document.activeElement.blur(); }";
        omnibox_browser->GetMainFrame()->ExecuteJavaScript(blurJs, "about:blank", 0);

        LOG_INFO_APP("🔍 Cleared focus states in omnibox browser");
    }

    LOG_INFO_APP("✅ Omnibox overlay shown");
}

void HideOmniboxOverlay() {
    // Guard: verify HWND exists
    if (!g_omnibox_overlay_hwnd || !IsWindow(g_omnibox_overlay_hwnd)) {
        LOG_WARNING_APP("⚠️ Cannot hide omnibox overlay - HWND does not exist");
        return;
    }

    LOG_INFO_APP("🔍 Hiding omnibox overlay");

    // Remove global mouse hook
    extern HHOOK g_omnibox_mouse_hook;
    if (g_omnibox_mouse_hook) {
        UnhookWindowsHookEx(g_omnibox_mouse_hook);
        g_omnibox_mouse_hook = nullptr;
        LOG_INFO_APP("✅ Omnibox mouse hook removed");
    }

    // Clear focus from omnibox browser before hiding
    CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
    if (omnibox_browser) {
        omnibox_browser->GetHost()->SetFocus(false);
        LOG_INFO_APP("✅ Cleared focus from omnibox browser");
    }

    // Hide window (keep-alive - don't destroy)
    ShowWindow(g_omnibox_overlay_hwnd, SW_HIDE);

    // Return focus to header browser
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    if (header_browser) {
        header_browser->GetHost()->SetFocus(true);
        LOG_INFO_APP("✅ Returned focus to header browser");
    }

    LOG_INFO_APP("✅ Omnibox overlay hidden");
}

// Forward declarations for cookie panel overlay functions
void ShowCookiePanelOverlay(int iconRightOffset = 0);
void HideCookiePanelOverlay();

void CreateCookiePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset) {
    LOG_INFO_APP("Creating cookie panel overlay (showImmediately=" +
                 std::string(showImmediately ? "true" : "false") + ", iconRightOffset=" +
                 std::to_string(iconRightOffset) + ")");

    // Store offset globally for WM_SIZE/WM_MOVE repositioning
    extern int g_cookie_icon_right_offset;
    if (iconRightOffset > 0) {
        g_cookie_icon_right_offset = iconRightOffset;
    }

    // Keep-alive check: if HWND already exists, conditionally show it
    if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd)) {
        LOG_INFO_APP("Cookie panel overlay already exists");
        if (showImmediately) {
            ShowCookiePanelOverlay(iconRightOffset);
        }
        return;
    }

    // Get main window dimensions
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);

    // Calculate position - right side panel, right edge aligned under icon
    int panelWidth = 450;
    int panelHeight = 520;
    int overlayX = headerRect.right - iconRightOffset - panelWidth;
    int overlayY = headerRect.top + 104;
    // Clamp to main window bottom
    if (overlayY + panelHeight > mainRect.bottom) {
        panelHeight = mainRect.bottom - overlayY;
        if (panelHeight < 200) panelHeight = 200;
    }

    LOG_INFO_APP("🍪 Creating cookie panel overlay at position: (" + std::to_string(overlayX) + ", " +
                 std::to_string(overlayY) + ") size: " + std::to_string(panelWidth) + "x" +
                 std::to_string(panelHeight));

    // Create HWND for cookie panel overlay
    HWND cookie_panel_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFCookiePanelOverlayWindow",
        L"Cookie Panel Overlay",
        WS_POPUP,
        overlayX, overlayY, panelWidth, panelHeight,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!cookie_panel_hwnd) {
        LOG_ERROR_APP("❌ Failed to create cookie panel overlay HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    // Force position with SWP_NOACTIVATE (conditionally show)
    UINT flags = SWP_NOACTIVATE;
    if (showImmediately) {
        flags |= SWP_SHOWWINDOW;
    } else {
        flags |= SWP_HIDEWINDOW;
    }
    SetWindowPos(cookie_panel_hwnd, HWND_TOPMOST,
        overlayX, overlayY, panelWidth, panelHeight,
        flags);

    // Store HWND globally
    g_cookie_panel_overlay_hwnd = cookie_panel_hwnd;
    LOG_INFO_APP("✅ Cookie panel overlay HWND created: " + std::to_string(reinterpret_cast<intptr_t>(cookie_panel_hwnd)) +
                 " (visible=" + std::string(showImmediately ? "true" : "false") + ")");

    // Create CEF browser subprocess for cookie panel overlay
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(cookie_panel_hwnd, "CookiePanelOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);  // transparent background
    settings.javascript = STATE_ENABLED;

    CefRefPtr<SimpleHandler> cookie_panel_handler(new SimpleHandler("cookiepanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler(cookie_panel_hwnd, panelWidth, panelHeight);
    cookie_panel_handler->SetRenderHandler(render_handler);

    // Use global context (shared cache/cookies)
    bool result = CefBrowserHost::CreateBrowser(
        window_info, cookie_panel_handler,
        "http://127.0.0.1:5137/privacy-shield",
        settings, nullptr,
        CefRequestContext::GetGlobalContext());

    if (result) {
        LOG_INFO_APP("✅ Cookie panel overlay browser created with subprocess");

        // If showing immediately, install mouse hook and enable mouse input
        if (showImmediately) {
            extern HHOOK g_cookie_panel_mouse_hook;
            extern LRESULT CALLBACK CookiePanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam);

            if (!g_cookie_panel_mouse_hook) {
                g_cookie_panel_mouse_hook = SetWindowsHookEx(WH_MOUSE_LL, CookiePanelMouseHookProc, nullptr, 0);
                if (g_cookie_panel_mouse_hook) {
                    LOG_INFO_APP("✅ Cookie panel mouse hook installed for click-outside detection");
                } else {
                    LOG_WARNING_APP("⚠️ Failed to install cookie panel mouse hook. Error: " + std::to_string(GetLastError()));
                }
            }

            // Enable mouse input
            LONG exStyle = GetWindowLong(cookie_panel_hwnd, GWL_EXSTYLE);
            SetWindowLong(cookie_panel_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
            LOG_INFO_APP("✅ Mouse input enabled for cookie panel overlay");
        }
    } else {
        LOG_ERROR_APP("❌ Failed to create cookie panel overlay browser");
    }
}

void ShowCookiePanelOverlay(int iconRightOffset) {
    // Guard: verify HWND exists
    if (!g_cookie_panel_overlay_hwnd || !IsWindow(g_cookie_panel_overlay_hwnd)) {
        LOG_WARNING_APP("Cannot show cookie panel overlay - HWND does not exist");
        return;
    }

    // Update stored offset if provided
    extern int g_cookie_icon_right_offset;
    if (iconRightOffset > 0) {
        g_cookie_icon_right_offset = iconRightOffset;
    }

    LOG_INFO_APP("Showing cookie panel overlay with iconRightOffset=" + std::to_string(g_cookie_icon_right_offset));

    // Install global mouse hook for click-outside detection
    extern HHOOK g_cookie_panel_mouse_hook;
    extern LRESULT CALLBACK CookiePanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam);
    if (!g_cookie_panel_mouse_hook) {
        g_cookie_panel_mouse_hook = SetWindowsHookEx(WH_MOUSE_LL, CookiePanelMouseHookProc, nullptr, 0);
        if (g_cookie_panel_mouse_hook) {
            LOG_INFO_APP("Cookie panel mouse hook installed");
        } else {
            LOG_WARNING_APP("Failed to install cookie panel mouse hook. Error: " + std::to_string(GetLastError()));
        }
    }

    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);

    // Calculate position - right edge aligned under icon
    int panelWidth = 450;
    int panelHeight = 520;
    int overlayX = headerRect.right - g_cookie_icon_right_offset - panelWidth;
    int overlayY = headerRect.top + 104;
    // Clamp to main window bottom
    if (overlayY + panelHeight > mainRect.bottom) {
        panelHeight = mainRect.bottom - overlayY;
        if (panelHeight < 200) panelHeight = 200;
    }

    // Force position and show with SWP_NOACTIVATE
    SetWindowPos(g_cookie_panel_overlay_hwnd, HWND_TOPMOST,
        overlayX, overlayY, panelWidth, panelHeight,
        SWP_NOACTIVATE | SWP_SHOWWINDOW);

    // Remove WS_EX_TRANSPARENT to enable mouse input
    LONG exStyle = GetWindowLong(g_cookie_panel_overlay_hwnd, GWL_EXSTYLE);
    SetWindowLong(g_cookie_panel_overlay_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);

    // Trigger render update
    CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
    if (cookie_browser && cookie_browser->GetHost()) {
        cookie_browser->GetHost()->WasResized();
        cookie_browser->GetHost()->Invalidate(PET_VIEW);
        LOG_INFO_APP("🍪 Triggered render update for cookie panel browser");
    }

    LOG_INFO_APP("✅ Cookie panel overlay shown");
}

void HideCookiePanelOverlay() {
    // Guard: verify HWND exists
    if (!g_cookie_panel_overlay_hwnd || !IsWindow(g_cookie_panel_overlay_hwnd)) {
        LOG_WARNING_APP("⚠️ Cannot hide cookie panel overlay - HWND does not exist");
        return;
    }

    LOG_INFO_APP("🍪 Hiding cookie panel overlay");

    // Remove global mouse hook
    extern HHOOK g_cookie_panel_mouse_hook;
    if (g_cookie_panel_mouse_hook) {
        UnhookWindowsHookEx(g_cookie_panel_mouse_hook);
        g_cookie_panel_mouse_hook = nullptr;
        LOG_INFO_APP("✅ Cookie panel mouse hook removed");
    }

    // Clear focus from cookie panel browser before hiding
    CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
    if (cookie_browser) {
        cookie_browser->GetHost()->SetFocus(false);
        LOG_INFO_APP("✅ Cleared focus from cookie panel browser");
    }

    // Hide window (keep-alive - don't destroy)
    ShowWindow(g_cookie_panel_overlay_hwnd, SW_HIDE);

    // Return focus to header browser
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    if (header_browser) {
        header_browser->GetHost()->SetFocus(true);
        LOG_INFO_APP("✅ Returned focus to header browser");
    }

    LOG_INFO_APP("✅ Cookie panel overlay hidden");
}

// ========== DOWNLOAD PANEL OVERLAY ==========

// Forward declaration
void ShowDownloadPanelOverlay(int iconRightOffset);

void CreateDownloadPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset) {
    LOG_INFO_APP("Creating download panel overlay (showImmediately=" +
                 std::string(showImmediately ? "true" : "false") + ", iconRightOffset=" +
                 std::to_string(iconRightOffset) + ")");

    // Store offset globally for WM_SIZE/WM_MOVE repositioning
    extern int g_download_icon_right_offset;
    if (iconRightOffset > 0) {
        g_download_icon_right_offset = iconRightOffset;
    }

    // Keep-alive check: if HWND already exists, conditionally show it
    extern HWND g_download_panel_overlay_hwnd;
    if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd)) {
        LOG_INFO_APP("Download panel overlay already exists");
        if (showImmediately) {
            ShowDownloadPanelOverlay(iconRightOffset);
        }
        return;
    }

    // Get main window dimensions
    extern HWND g_hwnd;
    extern HWND g_header_hwnd;
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);
    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);

    // Calculate position - right side panel, right edge aligned under icon
    int panelWidth = 380;
    int panelHeight = 400;
    int overlayX = headerRect.right - iconRightOffset - panelWidth;
    int overlayY = headerRect.top + 104;
    // Clamp to main window bottom
    if (overlayY + panelHeight > mainRect.bottom) {
        panelHeight = mainRect.bottom - overlayY;
        if (panelHeight < 200) panelHeight = 200;
    }

    LOG_INFO_APP("Creating download panel overlay at position: (" + std::to_string(overlayX) + ", " +
                 std::to_string(overlayY) + ") size: " + std::to_string(panelWidth) + "x" +
                 std::to_string(panelHeight));

    // Create HWND for download panel overlay
    HWND download_panel_hwnd = CreateWindowEx(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        L"CEFDownloadPanelOverlayWindow",
        L"Download Panel Overlay",
        WS_POPUP,
        overlayX, overlayY, panelWidth, panelHeight,
        g_hwnd, nullptr, hInstance, nullptr);

    if (!download_panel_hwnd) {
        LOG_ERROR_APP("Failed to create download panel overlay HWND. Error: " + std::to_string(GetLastError()));
        return;
    }

    // Force position with SWP_NOACTIVATE (conditionally show)
    UINT flags = SWP_NOACTIVATE;
    if (showImmediately) {
        flags |= SWP_SHOWWINDOW;
    } else {
        flags |= SWP_HIDEWINDOW;
    }
    SetWindowPos(download_panel_hwnd, HWND_TOPMOST,
        overlayX, overlayY, panelWidth, panelHeight,
        flags);

    // Store HWND globally
    g_download_panel_overlay_hwnd = download_panel_hwnd;
    LOG_INFO_APP("Download panel overlay HWND created: " + std::to_string(reinterpret_cast<intptr_t>(download_panel_hwnd)) +
                 " (visible=" + std::string(showImmediately ? "true" : "false") + ")");

    // Create CEF browser subprocess for download panel overlay
    CefWindowInfo window_info;
    window_info.windowless_rendering_enabled = true;
    window_info.SetAsPopup(download_panel_hwnd, "DownloadPanelOverlay");

    CefBrowserSettings settings;
    settings.windowless_frame_rate = 30;
    settings.background_color = CefColorSetARGB(0, 0, 0, 0);  // transparent background
    settings.javascript = STATE_ENABLED;

    CefRefPtr<SimpleHandler> download_panel_handler(new SimpleHandler("downloadpanel"));
    CefRefPtr<MyOverlayRenderHandler> render_handler =
        new MyOverlayRenderHandler(download_panel_hwnd, panelWidth, panelHeight);
    download_panel_handler->SetRenderHandler(render_handler);

    // Use global context (shared cache/cookies)
    bool result = CefBrowserHost::CreateBrowser(
        window_info, download_panel_handler,
        "http://127.0.0.1:5137/downloads",
        settings, nullptr,
        CefRequestContext::GetGlobalContext());

    if (result) {
        LOG_INFO_APP("Download panel overlay browser created with subprocess");

        // If showing immediately, install mouse hook and enable mouse input
        if (showImmediately) {
            extern HHOOK g_download_panel_mouse_hook;
            extern LRESULT CALLBACK DownloadPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam);

            if (!g_download_panel_mouse_hook) {
                g_download_panel_mouse_hook = SetWindowsHookEx(WH_MOUSE_LL, DownloadPanelMouseHookProc, nullptr, 0);
                if (g_download_panel_mouse_hook) {
                    LOG_INFO_APP("Download panel mouse hook installed for click-outside detection");
                } else {
                    LOG_WARNING_APP("Failed to install download panel mouse hook. Error: " + std::to_string(GetLastError()));
                }
            }

            // Enable mouse input
            LONG exStyle = GetWindowLong(download_panel_hwnd, GWL_EXSTYLE);
            SetWindowLong(download_panel_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
        }
    } else {
        LOG_ERROR_APP("Failed to create download panel overlay browser");
    }
}

void ShowDownloadPanelOverlay(int iconRightOffset) {
    // Guard: verify HWND exists
    extern HWND g_download_panel_overlay_hwnd;
    if (!g_download_panel_overlay_hwnd || !IsWindow(g_download_panel_overlay_hwnd)) {
        LOG_WARNING_APP("Cannot show download panel overlay - HWND does not exist");
        return;
    }

    // Update stored offset if provided
    extern int g_download_icon_right_offset;
    if (iconRightOffset > 0) {
        g_download_icon_right_offset = iconRightOffset;
    }

    LOG_INFO_APP("Showing download panel overlay with iconRightOffset=" + std::to_string(g_download_icon_right_offset));

    // Install global mouse hook for click-outside detection
    extern HHOOK g_download_panel_mouse_hook;
    extern LRESULT CALLBACK DownloadPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam);
    if (!g_download_panel_mouse_hook) {
        g_download_panel_mouse_hook = SetWindowsHookEx(WH_MOUSE_LL, DownloadPanelMouseHookProc, nullptr, 0);
        if (g_download_panel_mouse_hook) {
            LOG_INFO_APP("Download panel mouse hook installed");
        } else {
            LOG_WARNING_APP("Failed to install download panel mouse hook. Error: " + std::to_string(GetLastError()));
        }
    }

    extern HWND g_header_hwnd;
    extern HWND g_hwnd;
    RECT headerRect;
    GetWindowRect(g_header_hwnd, &headerRect);
    RECT mainRect;
    GetWindowRect(g_hwnd, &mainRect);

    // Calculate position - right edge aligned under icon
    int panelWidth = 380;
    int panelHeight = 400;
    int overlayX = headerRect.right - g_download_icon_right_offset - panelWidth;
    int overlayY = headerRect.top + 104;
    // Clamp to main window bottom
    if (overlayY + panelHeight > mainRect.bottom) {
        panelHeight = mainRect.bottom - overlayY;
        if (panelHeight < 200) panelHeight = 200;
    }

    // Force position and show with SWP_NOACTIVATE
    SetWindowPos(g_download_panel_overlay_hwnd, HWND_TOPMOST,
        overlayX, overlayY, panelWidth, panelHeight,
        SWP_NOACTIVATE | SWP_SHOWWINDOW);

    // Remove WS_EX_TRANSPARENT to enable mouse input
    LONG exStyle = GetWindowLong(g_download_panel_overlay_hwnd, GWL_EXSTYLE);
    SetWindowLong(g_download_panel_overlay_hwnd, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);

    // Trigger render update
    CefRefPtr<CefBrowser> dl_browser = SimpleHandler::GetDownloadPanelBrowser();
    if (dl_browser && dl_browser->GetHost()) {
        dl_browser->GetHost()->WasResized();
        dl_browser->GetHost()->Invalidate(PET_VIEW);
    }

    LOG_INFO_APP("Download panel overlay shown");
}

void HideDownloadPanelOverlay() {
    // Guard: verify HWND exists
    extern HWND g_download_panel_overlay_hwnd;
    if (!g_download_panel_overlay_hwnd || !IsWindow(g_download_panel_overlay_hwnd)) {
        LOG_WARNING_APP("Cannot hide download panel overlay - HWND does not exist");
        return;
    }

    LOG_INFO_APP("Hiding download panel overlay");

    // Remove global mouse hook
    extern HHOOK g_download_panel_mouse_hook;
    if (g_download_panel_mouse_hook) {
        UnhookWindowsHookEx(g_download_panel_mouse_hook);
        g_download_panel_mouse_hook = nullptr;
        LOG_INFO_APP("Download panel mouse hook removed");
    }

    // Clear focus from download panel browser before hiding
    CefRefPtr<CefBrowser> dl_browser = SimpleHandler::GetDownloadPanelBrowser();
    if (dl_browser) {
        dl_browser->GetHost()->SetFocus(false);
    }

    // Hide window (keep-alive - don't destroy)
    ShowWindow(g_download_panel_overlay_hwnd, SW_HIDE);

    // Return focus to header browser
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    if (header_browser) {
        header_browser->GetHost()->SetFocus(true);
    }

    LOG_INFO_APP("Download panel overlay hidden");
}
#endif // _WIN32
