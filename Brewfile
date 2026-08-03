# Brewfile — macOS build dependencies for the CEF shell (DEP-1c).
#
# Pins the Homebrew formulae that `cmake` resolves on macOS via find_package /
# find_path / find_library in cef-native/CMakeLists.txt. Without this file the
# release workflow ran a bare `brew install openssl nlohmann-json sqlite3`,
# which floats to whatever HEAD Homebrew ships that day — the same silent-drift
# class DEP-1a fixes for vcpkg on Windows.
#
# Homebrew does NOT support pinning a formula to an arbitrary version in a
# Brewfile; `brew bundle` installs the current formula. What this file DOES buy
# us is a single declared dependency set (one home for the fact) plus
# `brew bundle check`, so an added/removed dependency is a reviewable diff
# rather than an inline edit buried in release.yml.
#
# Version-exactness on macOS is therefore still weaker than the Windows vcpkg
# pin. If macOS dependency drift ever breaks a build, the escalation is a
# `brew extract` into a Hodos tap — tracked in DEPENDENCY_VERIFICATION.md.
#
# Usage:  brew bundle --file=Brewfile

brew "openssl@3"
brew "nlohmann-json"
brew "sqlite3"
