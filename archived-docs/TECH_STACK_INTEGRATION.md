> **ARCHIVED**: This document predates Phase 2 (Feb 2026) and contains stale integration descriptions. The CEF deep-dive sections remain useful as reference material. For current architecture, see [PROJECT_OVERVIEW.md](./PROJECT_OVERVIEW.md). For CEF-specific research, see `development-docs/browser-core/01-chrome-brave-research.md`.

# Tech Stack Integration

## Frontend (React + TypeScript)

- **React frontend built using TypeScript** with Vite as the build tool and dev server
- **Key entry point**: `main.tsx` initializes React app and loads `initWindowBridge.ts` to set up communication bridge
- **Frontend flow**: React Router handles routing (`/`, `/settings`, `/wallet`, `/backup`, `/brc100-auth`), components use custom hooks (`useHodosBrowser`, `useWallet`, `useBalance`) to interact with native layer
- **Communication with CEF-Native**: Frontend sends messages via `window.cefMessage.send(messageName, args)` (defined in `initWindowBridge.ts`), receives responses through callback functions registered on `window` object (e.g., `window.onAddressGenerated`, `window.onGetBalanceResponse`) that are invoked by CEF-Native when responses arrive
- **Dev server**: Vite dev server runs on **port 5137** (`http://127.0.0.1:5137`) for development

## CEF-Native C++

- **Communication with frontend**: Uses CEF's V8 render process to inject JavaScript APIs (`window.hodosBrowser`, `window.cefMessage`) into browser contexts, receives messages from frontend via `OnProcessMessageReceived` in render process handler, sends responses back using `browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response)` to trigger JavaScript callbacks
- **CEF-binaries integration**: Uses Chromium Embedded Framework (CEF) binaries to create browser windows via `CefBrowserHost::CreateBrowser()`, leverages CEF's multi-process architecture (main process, render process, browser process) for security isolation, handles all Chromium functionality (rendering, networking, JavaScript execution) through CEF API
- **HTTP interceptor**: `HttpRequestInterceptor` intercepts HTTP requests in the browser process, routes wallet API requests (matching `/getBalance`, `/sendTransaction`, `/.well-known/auth`, etc.) to Rust wallet backend on port 3301, forwards original headers (including BRC-31 Authrite authentication headers) to maintain security context, handles domain whitelisting and BRC-100 authentication approval flows
- **Build output**: CEF-Native build creates the executable (`cef_browser_shell.exe`) that launches the browser shell and manages all browser windows

## Rust Wallet Backend

- **Request handling**: Runs an **Actix-web HTTP server on port 3301** (`http://127.0.0.1:3301`), receives HTTP POST requests from CEF-Native HTTP interceptor, responds with JSON payloads containing wallet data, transaction results, or error messages
- **Blockchain integration**: Interacts with the **BitcoinSV blockchain** through SPV (Simplified Payment Verification) queries and transaction broadcasting, implements **BRC100 protocol** for authentication, identity management, and transaction signing, supports BRC-103/104 mutual authentication, BRC-33 message relay, and BRC-42 key derivation for secure wallet operations

---
====================================================================================================

## Chromium Embedded Framework (CEF) Deep Dive

### Why CEF Exists

CEF (Chromium Embedded Framework) is an open-source framework that embeds Chromium-based browsers into native applications. It enables applications to use modern web technologies (HTML5, CSS3, JavaScript) for UI while maintaining native code control. CEF provides the full Chromium browser engine without requiring a full browser application, making it ideal for applications that need web rendering capabilities with custom native integration.

### How to Interact with CEF

**Initialization Flow:**
1. Call `CefInitialize()` with `CefSettings` to start CEF framework
2. Create `CefApp` instance implementing application-level handlers
3. Use `CefExecuteProcess()` to handle subprocess execution
4. Create browser instances via `CefBrowserHost::CreateBrowser()`
5. Implement handler classes (`CefClient`, `CefLifeSpanHandler`, etc.) to customize behavior
6. Call `CefShutdown()` on application exit

**Key CEF Classes and Methods for Core Browser Features:**

**Browser Management:**
- `CefBrowserHost::CreateBrowser()` - Create new browser instances (used for tabs)
- `CefBrowserHost::CloseBrowser()` - Close browser instances
- `CefBrowserHost::GetWindowHandle()` - Get native window handle (HWND on Windows, NSView on macOS)
- `CefBrowser::GetMainFrame()->LoadURL()` - Navigate to URL
- `CefBrowser::GoBack()`, `CefBrowser::GoForward()` - History navigation
- `CefBrowser::Reload()`, `CefBrowser::StopLoad()` - Page control
- `CefBrowser::CanGoBack()`, `CefBrowser::CanGoForward()` - Check navigation state

**Navigation & History:**
- `CefDisplayHandler::OnAddressChange()` - Monitor URL changes for address bar
- `CefNavigationEntryVisitor` - Iterate through history entries
- `CefBrowserHost::GetNavigationEntries()` - Get navigation history
- `CefNavigationEntry::GetURL()`, `GetTitle()`, `GetTimestamp()` - Extract history data

**Request Interception (Ad Blocking, Security, Privacy):**
- `CefRequestHandler::OnBeforeResourceLoad()` - Intercept ALL resource requests (primary method for ad blocking, security checks, privacy protection)
- `CefRequest::GetURL()`, `GetResourceType()`, `GetMethod()` - Get request details
- Return `RV_CANCEL` to block requests, `RV_CONTINUE` to allow
- `CefRequest::SetHeaderMap()` - Modify request headers (for privacy protection)
- `CefRequest::SetReferrer()` - Control referrer policy

**Cookies Management:**
- `CefCookieManager::GetGlobalManager()` - Get cookie manager instance
- `CefCookieManager::VisitAllCookies()` - Iterate all cookies
- `CefCookieVisitor::Visit()` - Process each cookie (access name, value, domain, path, etc.)
- `CefCookieManager::SetCookie()` - Set/modify cookies
- `CefCookieManager::DeleteCookies()` - Delete cookies
- `CefCookieManager::FlushStore()` - Persist cookie changes
- `CefCookieManager::SetSupportedSchemes()` - Control which URL schemes accept cookies

**Downloads:**
- `CefDownloadHandler::OnBeforeDownload()` - Handle download start, set download path
- `CefDownloadHandler::OnDownloadUpdated()` - Monitor download progress
- `CefDownloadItem::GetURL()`, `GetFullPath()`, `GetReceivedBytes()`, `GetTotalBytes()` - Get download info
- `CefDownloadItem::Pause()`, `Resume()`, `Cancel()` - Control downloads
- `CefBrowserHost::SetDownloadPath()` - Set default download directory

**JavaScript Integration:**
- `CefV8Context`, `CefV8Value` - V8 JavaScript bindings
- `CefV8Handler::Execute()` - Expose native C++ functions to JavaScript
- `CefFrame::ExecuteJavaScript()` - Execute JavaScript from C++
- `CefRenderProcessHandler::OnContextCreated()` - Inject JavaScript APIs when V8 context is created

**Security & Privacy:**
- `CefRequestHandler::OnCertificateError()` - Handle SSL/TLS certificate errors
- `CefSSLInfo::GetSubject()`, `GetIssuer()` - Get certificate details
- `CefRequestContext::ClearCertificateExceptions()` - Clear SSL exceptions
- `CefRequestContext::ClearHttpAuthCredentials()` - Clear authentication data
- `CefRequestContext::CreateContext()` - Create isolated context (for private browsing)

**Developer Tools:**
- `CefBrowserHost::ShowDevTools()` - Open DevTools window
- `CefBrowserHost::CloseDevTools()` - Close DevTools
- `CefSettings::remote_debugging_port` - Enable remote debugging

### CEF's Built-in Functionality (What Already Works)

**Database & Storage:**
- **Location**: CEF stores browser data (cookies, cache, localStorage, IndexedDB) in platform-specific locations:
  - **Windows**: `%LOCALAPPDATA%\CEF\User Data\Default\` (or custom path via `CefRequestContextSettings::cache_path`)
  - **macOS**: `~/Library/Application Support/CEF/User Data/Default/`
- **Storage Format**: Uses Chromium's LevelDB for cookies, SQLite for some metadata, and file-based storage for cache
- **Access**: Managed through `CefCookieManager` API and `CefRequestContext` settings - CEF handles all database creation and access automatically
- **Why YouTube Works**: CEF automatically uses Chromium's standard cookie and session storage, so sites like YouTube can store login sessions in cookies/localStorage just like in Chrome. The database is created automatically on first use.

**Downloads (Already Functional):**
- CEF automatically handles downloads through `CefDownloadHandler` - if you don't implement it, CEF uses default behavior (downloads to system default folder)
- Default download location: Windows Downloads folder, macOS Downloads folder
- Downloads work without UI because CEF's default handler manages the file saving process
- To customize: Implement `CefDownloadHandler` in your `CefClient` to intercept and control download behavior

**Cookies (Already Functional):**
- CEF automatically manages cookies using Chromium's cookie database
- Cookies are stored persistently and shared across browser instances using the same `CefRequestContext`
- Sites can set/read cookies normally - no custom code needed
- To customize: Use `CefCookieManager` API to view, modify, or block cookies

### Security, Privacy, and Ad Blocking Implementation Guide

**How These Features Work Together:**
1. **Request Interception** (`CefRequestHandler::OnBeforeResourceLoad()`) is the central mechanism for all three
2. **Ad Blocking**: Parse filter lists (EasyList, EasyPrivacy), match URLs/domains against rules, return `RV_CANCEL` to block
3. **Security**: Check URLs against threat databases, validate certificates, block malicious content
4. **Privacy**: Modify/remove headers (User-Agent, Referer), block tracking requests, prevent fingerprinting

**What CEF Already Provides:**
- **Cookies**: Full cookie management via `CefCookieManager` - storage, retrieval, deletion
- **SSL/TLS**: Certificate validation and error handling via `OnCertificateError()`
- **Request Interception**: `OnBeforeResourceLoad()` hook for all network requests
- **Context Isolation**: `CefRequestContext::CreateContext()` for isolated browsing sessions
- **Cache Management**: Automatic cache via `CefRequestContextSettings::cache_path`

**What Needs to Be Implemented:**
- **Ad Blocking Engine**: Parse filter lists, build efficient matching data structures, implement blocking logic
- **Threat Intelligence**: Integrate malware/phishing databases, implement URL checking
- **Privacy Enhancements**: Header modification, fingerprinting protection, WebRTC leak prevention
- **Cookie Controls**: UI for viewing/editing cookies, per-site cookie policies

**Libraries and Resources:**
- **Filter List Parsing**: Use existing parsers for EasyList format (many open-source implementations)
- **Brave Browser Approach**: Brave uses Chromium's `declarativeNetRequest` API (CEF equivalent: `OnBeforeResourceLoad()`), maintains filter lists, implements cosmetic filtering via JavaScript injection
- **Open Source Resources**:
  - Brave's filter list management (GitHub: brave/adblock-rust, brave/adblock-lists)
  - uBlock Origin's filter matching algorithms (can be adapted for CEF)
  - Chromium's network stack documentation for request interception patterns

**Implementation Strategy:**
1. **Phase 1**: Basic ad blocking - implement `OnBeforeResourceLoad()`, parse EasyList, block matching URLs
2. **Phase 2**: Privacy headers - modify User-Agent, Referer, Accept-Language headers
3. **Phase 3**: Advanced blocking - cosmetic filtering via JavaScript injection, tracker blocking
4. **Phase 4**: Security integration - threat database checking, certificate pinning
5. **Phase 5**: UI integration - settings UI for managing blocklists, privacy preferences

### Cross-Platform Considerations (Windows → macOS)

**CMake Changes:**
- Platform detection: Use `if(APPLE)` and `if(WIN32)` conditionals in CMakeLists.txt
- Library linking: macOS uses frameworks (`.framework`), Windows uses DLLs (`.dll`/`.lib`)
- CEF binaries: Download platform-specific CEF binaries (separate builds for Windows/macOS)
- Resource paths: Adjust paths for macOS bundle structure (`Contents/Resources/`)

**Window Management:**
- **Windows**: Uses `HWND` (window handles) via Win32 API (`CreateWindow`, `SetWindowPos`)
- **macOS**: Uses `NSView` (Cocoa views) via AppKit framework
- **Solution**: Abstract window creation - create platform-specific wrapper classes or use conditional compilation
- **CEF API**: `CefWindowInfo::SetAsChild()` works on both platforms but takes different handle types

**Process Management:**
- **Windows**: Process handles via Win32 API, subprocess creation via `CreateProcess()`
- **macOS**: Process management via Foundation framework, subprocess creation via `NSTask` or `posix_spawn()`
- **CEF Handling**: CEF abstracts most process management - `CefExecuteProcess()` works on both platforms
- **Custom Code**: Any custom process spawning code needs platform-specific implementations

**Function Calls:**
- Most CEF API calls are identical across platforms
- Platform-specific code needed for: window creation, file system paths, process management
- Use `#ifdef _WIN32` / `#ifdef __APPLE__` for platform-specific code sections

**Key Files to Modify:**
- `cef_browser_shell.cpp` - Replace Win32 window creation with Cocoa/AppKit equivalents
- `CMakeLists.txt` - Add macOS-specific library linking and framework inclusion
- Path handling - Replace Windows path separators (`\`) with platform-agnostic or macOS paths (`/`)
- Remove Windows-specific includes (`windows.h`) and add macOS equivalents (`Cocoa/Cocoa.h`)

### Building Installation Executables (Windows & macOS)

**Windows Installation:**
- **Tool Options**: Inno Setup (free, script-based), WiX Toolset (Microsoft, XML-based), NSIS (free, script-based)
- **CEF Considerations**:
  - Include all CEF DLLs (`libcef.dll`, `chrome_elf.dll`, etc.) in installer
  - Include CEF resources directory (`Resources/`, `locales/`, `.pak` files)
  - Set up proper DLL search paths or install to application directory
  - Include Visual C++ redistributables if needed
- **Structure**: Installer should create application folder, copy CEF binaries, create Start Menu shortcuts, optionally add registry entries

**macOS Installation:**
- **App Bundle Structure**: Create `.app` bundle with `Contents/MacOS/` (executable), `Contents/Resources/` (CEF resources), `Contents/Frameworks/` (CEF frameworks)
- **CEF Considerations**:
  - Include CEF frameworks in `Contents/Frameworks/`
  - Include CEF resources in `Contents/Resources/`
  - Set up proper framework search paths (`@rpath`, `@executable_path`)
- **Code Signing**: Required for distribution - sign application bundle and all frameworks
- **Notarization**: Submit to Apple for notarization (required for macOS Catalina+)
- **DMG Creation**: Use `hdiutil` or tools like Create-DMG to create disk images for distribution

**CEF's Role in Distribution:**
- CEF provides pre-built binaries for each platform, simplifying distribution
- CEF binaries are large (~100-200MB) - consider compression or separate download
- CEF version must match between development and distribution builds
- Include CEF's `README.txt` and license files as required by CEF's BSD license

**MVP Considerations:**
- **Windows**: Start with simple folder-based distribution (zip file), add installer later
- **macOS**: Create basic `.app` bundle manually, add code signing/notarization for production
- **Testing**: Test installation on clean systems (VMs recommended) to catch missing dependencies
- **Updates**: Plan update mechanism early - CEF updates may require redistributing entire application
