// Copyright (c) 2013 The Chromium Embedded Framework Authors.
// Portions copyright (c) 2010 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Entry point for CEF helper processes on macOS
// Uses minimal approach - helpers don't need full SimpleApp

#include "include/cef_app.h"
#include "include/wrapper/cef_library_loader.h"
#include "include/cef_render_process_handler.h"
#include "../include/handlers/simple_render_process_handler.h"
#include "../include/core/Logger.h"

// Minimal CefApp for helpers (only provides render process handler)
class HelperApp : public CefApp {
public:
    HelperApp() {}

    CefRefPtr<CefRenderProcessHandler> GetRenderProcessHandler() override {
        if (!render_process_handler_) {
            render_process_handler_ = new SimpleRenderProcessHandler();
        }
        return render_process_handler_;
    }

private:
    CefRefPtr<SimpleRenderProcessHandler> render_process_handler_;
    IMPLEMENT_REFCOUNTING(HelperApp);
};

// Entry point function for helper processes
int main(int argc, char* argv[]) {
  // Load the CEF framework library at runtime
  CefScopedLibraryLoader library_loader;
  if (!library_loader.LoadInHelper()) {
    return 1;
  }

  // Create main args
  CefMainArgs main_args(argc, argv);

  // Use minimal app with only render process handler (for V8 injections)
  CefRefPtr<HelperApp> app(new HelperApp);

  // Execute the helper process
  return CefExecuteProcess(main_args, app.get(), nullptr);
}
