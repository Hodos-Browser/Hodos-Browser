# macOS Dev Environment Setup — Step by Step

**For:** First-time Mac user setting up Hodos Browser development
**Prerequisite:** Refurbished MacBook Pro (256GB), internet connection
**Time:** ~2-3 hours total

---

## Part 0: Mac Basics (if you've never used a Mac)

**Terminal** is the Mac equivalent of Command Prompt/PowerShell. It's where you'll run all commands.

- **Open Terminal:** Press `Cmd + Space` (Spotlight search), type `Terminal`, press Enter
- **Cmd key** = the key with `⌘` next to the spacebar (where Alt/Windows key would be on PC)
- **Copy/Paste in Terminal:** `Cmd + C` / `Cmd + V` (not Ctrl)
- **File manager** is called **Finder** (blue face icon in the dock)
- **System Settings:** Click Apple menu (top-left) → System Settings
- **Installing apps:** Drag .app to Applications folder, or use Homebrew (command-line package manager)

**Key differences from Windows:**
- No `C:\` drive — paths start with `/` (e.g., `/Users/yourname/`)
- Home directory: `~` = `/Users/yourname/`
- Backslash `\` is NOT used in paths — always forward slash `/`
- No .exe files — apps are `.app` bundles (folders that look like files)
- `Cmd` replaces `Ctrl` for most shortcuts (Cmd+C, Cmd+V, Cmd+S, etc.)

---

## Part 1: Initial Mac Setup (~15 min)

### 1.1 Accept Xcode License and Install Command Line Tools

Open Terminal and run:

```bash
xcode-select --install
```

A popup will appear asking to install developer tools. Click **Install** and wait (may take 10-15 minutes). This gives you Git, clang (C++ compiler), and other essential dev tools.

Verify:
```bash
git --version
clang --version
```

### 1.2 Install Homebrew (Mac's Package Manager)

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

It will ask for your password (the Mac login password). Type it — characters won't show, that's normal.

**Important:** After install, Homebrew will print instructions to add it to your PATH. It will say something like:

```
==> Next steps:
  echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
  eval "$(/opt/homebrew/bin/brew shellenv)"
```

**Run both of those commands exactly as printed.** Then verify:

```bash
brew --version
```

---

## Part 2: Install Development Tools (~20 min)

Run each of these in Terminal:

### 2.1 CMake (C++ build tool)

```bash
brew install cmake
cmake --version    # Should show 3.20+
```

### 2.2 C++ Dependencies

```bash
brew install openssl nlohmann-json sqlite3
```

### 2.3 Rust Toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When prompted, press **1** for default installation. Then:

```bash
source $HOME/.cargo/env
rustc --version    # Should show latest stable
cargo --version
```

### 2.4 Node.js (for frontend)

```bash
brew install node
node --version     # Should be 18+
npm --version
```

### 2.5 GitHub CLI

```bash
brew install gh
```

---

## Part 3: GitHub Authentication (~10 min)

### 3.1 Authenticate with GitHub

```bash
gh auth login
```

Follow the prompts:
- **GitHub.com** (not Enterprise)
- **HTTPS** (recommended)
- **Login with a web browser** — it will give you a code, open a browser, paste the code
- Use the **same GitHub account** as on your Windows machine (BSVArchie or whichever you use)

Verify:
```bash
gh auth status
```

### 3.2 Configure Git Identity

```bash
git config --global user.name "Your Name"
git config --global user.email "your-github-email@example.com"
```

Use the same name/email as your Windows Git config.

---

## Part 4: Clone the Repository (~5 min)

### 4.1 Choose a location

The Mac home directory is fine. In Terminal:

```bash
cd ~
gh repo clone Hodos-Browser/Hodos-Browser Hodos-Browser
cd Hodos-Browser
```

If it's a private repo, `gh` handles auth automatically (from step 3.1).

### 4.2 Check out the right branch

```bash
git checkout post-beta3-cleanup
git pull origin post-beta3-cleanup
```

### 4.3 Verify

```bash
git log --oneline -5
ls
```

You should see the same recent commits as on your Windows machine.

---

## Part 5: Download CEF Binaries (~15 min)

### 5.1 Download

1. Open Safari and go to: `https://cef-builds.spotifycdn.com/index.html`
2. Find **macOS ARM64** (Apple Silicon — your MBP is ARM)
3. Download the **Standard Distribution** for version **136** (latest stable)
4. The file will be ~300MB, downloads to `~/Downloads/`

### 5.2 Extract and Setup

```bash
cd ~/Hodos-Browser

# Extract (replace filename with actual downloaded file)
tar -xjf ~/Downloads/cef_binary_136*.tar.bz2

# Rename to cef-binaries
mv cef_binary_136* cef-binaries
```

### 5.3 Build CEF Wrapper Library

```bash
cd cef-binaries
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --target libcef_dll_wrapper --config Release
```

Verify:
```bash
ls -lh libcef_dll_wrapper/libcef_dll_wrapper.a
# Should show ~5MB file
```

---

## Part 6: Build Everything (~15 min)

### 6.1 Rust Wallet

```bash
cd ~/Hodos-Browser/rust-wallet
cargo build --release
```

First build takes 5-10 minutes (compiling all dependencies).

### 6.2 Adblock Engine

```bash
cd ~/Hodos-Browser/adblock-engine
cargo build --release
```

### 6.3 Frontend

```bash
cd ~/Hodos-Browser/frontend
npm install
npm run build
```

### 6.4 CEF Native Shell (C++ Browser)

```bash
cd ~/Hodos-Browser/cef-native
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
```

### 6.5 Copy Helper Bundles (CRITICAL)

```bash
cd build/bin
cp -r "HodosBrowser Helper"*.app HodosBrowserShell.app/Contents/Frameworks/
```

Verify:
```bash
ls HodosBrowserShell.app/Contents/Frameworks/ | grep Helper
# Should show 5 helper .app entries
```

---

## Part 7: Test Run (~5 min)

You need **3-4 Terminal windows** open simultaneously.

**Open new Terminal tabs:** `Cmd + T` in Terminal app.

### Terminal 1: Rust Wallet
```bash
cd ~/Hodos-Browser/rust-wallet
cargo run --release
# Wait for "Listening on: http://127.0.0.1:31301"
```

### Terminal 2: Adblock Engine
```bash
cd ~/Hodos-Browser/adblock-engine
cargo run --release
# Wait for "Listening on: http://127.0.0.1:31302"
```

### Terminal 3: Frontend Dev Server
```bash
cd ~/Hodos-Browser/frontend
npm run dev
# Wait for "Local: http://127.0.0.1:5137"
```

### Terminal 4: Launch Browser
```bash
cd ~/Hodos-Browser/cef-native/build/bin
./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell
```

**Note:** macOS may ask about network access — click **Allow**.

If a window appears with tab bar and content area, the build is working.

---

## Part 8: Install Claude Code (~5 min)

```bash
npm install -g @anthropic-ai/claude-code
```

Then launch it:
```bash
cd ~/Hodos-Browser
claude
```

It will ask you to log in — use **the same Anthropic account/subscription** as on Windows. Claude Code will read the project's CLAUDE.md files automatically.

---

## Part 9: Verify Disk Space

```bash
df -h /
```

Check "Avail" column. After all builds, you should have 150GB+ free. CEF binaries (~1.5GB) + Rust build cache (~2GB) + Node modules (~500MB) are the biggest consumers.

---

## Troubleshooting

### "command not found: brew"
Run the PATH setup commands from Homebrew's install output, or:
```bash
eval "$(/opt/homebrew/bin/brew shellenv)"
```

### "CEF framework not found" during cmake
```bash
ls ~/Hodos-Browser/cef-binaries/Release/Chromium\ Embedded\ Framework.framework/
```
If missing, the CEF extraction didn't work — re-download and extract.

### "permission denied" on first launch
```bash
chmod +x HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell
```

### macOS "unidentified developer" warning
Since the app isn't code-signed for dev builds:
```bash
# Remove quarantine attribute
xattr -cr HodosBrowserShell.app
```

### App crashes immediately
Check helper bundles were copied (Step 6.5). Also check:
```bash
cat ~/Hodos-Browser/cef-native/build/bin/debug_output.log | tail -50
```

---

## Quick Reference Card

| Task | Command |
|------|---------|
| Open Terminal | `Cmd + Space`, type "Terminal" |
| New Terminal tab | `Cmd + T` |
| Build Rust wallet | `cd ~/Hodos-Browser/rust-wallet && cargo build --release` |
| Build frontend | `cd ~/Hodos-Browser/frontend && npm run build` |
| Build C++ | `cd ~/Hodos-Browser/cef-native && cmake --build build --config Release` |
| Copy helpers | `cd build/bin && cp -r "HodosBrowser Helper"*.app HodosBrowserShell.app/Contents/Frameworks/` |
| Run browser | `./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell` |
| Pull latest code | `cd ~/Hodos-Browser && git pull` |
| Check branch | `git branch` |
| Launch Claude Code | `cd ~/Hodos-Browser && claude` |
