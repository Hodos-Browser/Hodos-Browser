# HodosBrowser Renaming Plan

## Overview
This document outlines the comprehensive renaming from BitcoinBrowser/BabbageBrowser to HodosBrowser throughout the entire repository.

## Naming Conventions

### Product Names
- **BitcoinBrowser** → **HodosBrowser**
- **BabbageBrowser** → **HodosBrowser**
- **Bitcoin-Browser** → **HodosBrowser**
- **Babbage-Browser** → **HodosBrowser**

### Wallet Name
- **BitcoinBrowserWallet** → **HodosWallet**
- **bitcoin-browser-wallet** (Rust package) → **hodos-wallet**

### JavaScript API
- **window.bitcoinBrowser** → **window.hodosBrowser**
- **bitcoinBrowserReady** (event) → **hodosBrowserReady**

### Function/Type Names
- **useBitcoinBrowser** → **useHodosBrowser**
- **InjectBitcoinBrowserAPI** → **InjectHodosBrowserAPI**
- **BitcoinBrowser API Types** → **HodosBrowser API Types**

### Directory Paths
- **%APPDATA%/BabbageBrowser** → **%APPDATA%/HodosBrowser**

### C++ Project Names
- **BitcoinBrowserShell** → **HodosBrowserShell**
- **BitcoinBrowserWndClass** → **HodosBrowserWndClass**

## Areas to Update

### 1. C++ Native Code
- CMakeLists.txt: Project name and executable name
- cef_browser_shell.cpp: Window class names, window titles
- simple_app.cpp: InjectBitcoinBrowserAPI function name and implementation
- simple_render_process_handler.cpp: V8 API injection (bitcoinBrowser object)
- All handler files: References to bitcoinBrowser API
- HttpRequestInterceptor.cpp: Directory paths and API references
- WalletService.cpp: User agent strings

### 2. TypeScript/JavaScript Frontend
- bitcoinBrowser.d.ts: Type definitions (rename file and content)
- useBitcoinBrowser.ts: Hook name and all references
- All component files: window.bitcoinBrowser references
- bridge files: API references

### 3. Rust Wallet
- Cargo.toml: Package name
- handlers.rs: Version strings
- main.rs: Directory paths
- domain_whitelist.rs: Directory paths
- All test scripts: Directory paths

### 4. Documentation
- README.md
- PROJECT_OVERVIEW.md
- ARCHITECTURE.md
- BUILD_INSTRUCTIONS.md
- All other .md files

### 5. Build Files
- CMakeLists.txt
- Any .vcxproj files (if present)
- Any .sln files (if present)

## Execution Order

1. **C++ Code** (Most critical - affects runtime)
   - Update CMakeLists.txt
   - Update function names
   - Update window classes
   - Update API injection code

2. **TypeScript/JavaScript** (High priority - affects frontend)
   - Rename type definition file
   - Update hook names
   - Update all API references

3. **Rust Wallet** (High priority - affects backend)
   - Update package name
   - Update directory paths
   - Update version strings

4. **Documentation** (Lower priority - informational)
   - Update all markdown files

## Important Notes

- **Backward Compatibility**: We're making breaking changes. Existing wallets in %APPDATA%/BabbageBrowser will need to be migrated or users will need to create new wallets.
- **API Breaking Changes**: Any external code using window.bitcoinBrowser will break. This is intentional for the rebrand.
- **File Paths**: The directory path change means existing wallet data won't be automatically found. Consider migration strategy.

## Testing Checklist

After renaming:
- [ ] C++ project builds successfully
- [ ] Frontend compiles without errors
- [ ] Rust wallet builds successfully
- [ ] Application starts and creates wallet in new directory
- [ ] JavaScript API is accessible as window.hodosBrowser
- [ ] All documentation is updated
- [ ] No references to old names remain
