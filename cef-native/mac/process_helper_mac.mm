// Copyright (c) 2013 The Chromium Embedded Framework Authors.
// Portions copyright (c) 2010 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Entry point for CEF helper processes on macOS
// Helpers are minimal - they just execute CEF subprocesses

#include "include/cef_app.h"
#include "include/wrapper/cef_library_loader.h"

// Entry point function for helper processes
int main(int argc, char* argv[]) {
  // Load the CEF framework library at runtime
  CefScopedLibraryLoader library_loader;
  if (!library_loader.LoadInHelper()) {
    return 1;
  }

  // Create main args
  CefMainArgs main_args(argc, argv);

  // Execute the helper process
  // Pass nullptr for app - helpers don't need full app implementation
  return CefExecuteProcess(main_args, nullptr, nullptr);
}
