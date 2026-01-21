# Mac Platform Support Plan

## Files that need Mac equivalents or cross-platform updates

### 1. BRC100Bridge (needs Mac version)
- **Current**: `cef-native/src/core/BRC100Bridge.cpp` (Windows-only, uses WinHTTP)
- **Needed**: `cef-native/src/core/BRC100Bridge_mac.cpp` (Mac version using libcurl)
- **Header**: `cef-native/include/core/BRC100Bridge.h` (needs #ifdef cleanup)
- **Difficulty**: Medium (similar to WalletService_mac pattern)
- **Estimated time**: 2-3 hours

### 2. BRC100Handler (can be cross-platform)
- **Current**: `cef-native/src/core/BRC100Handler.cpp` (uses BRC100Bridge)
- **Needed**: Make cross-platform (already mostly is, just depends on BRC100Bridge)
- **Header**: `cef-native/include/core/BRC100Handler.h` (already cross-platform)
- **Difficulty**: Easy (works once BRC100Bridge is cross-platform)
- **Estimated time**: 30 minutes

### 3. HttpRequestInterceptor (needs cross-platform paths)
- **Current**: `cef-native/src/core/HttpRequestInterceptor.cpp` (Windows paths: USERPROFILE, \\AppData\\Roaming)
- **Needed**: Add #ifdef for Mac paths (HOME, ~/Library/Application Support)
- **Difficulty**: Easy (just path fixes)
- **Estimated time**: 30 minutes
