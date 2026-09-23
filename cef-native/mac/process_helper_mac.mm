// Copyright (c) 2013 The Chromium Embedded Framework Authors.
// Portions copyright (c) 2010 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Entry point for CEF helper processes on macOS
// Uses minimal approach - helpers don't need full SimpleApp

#import <Foundation/Foundation.h>

#include "include/cef_app.h"
#include "include/wrapper/cef_library_loader.h"
#include "include/cef_render_process_handler.h"
#include "../include/handlers/simple_render_process_handler.h"
#include "../include/core/Logger.h"
#include "../include/core/ChildProcessLogSink.h"

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
  // Give this helper a working log. Helpers never call Logger::Initialize, so until this
  // landed every LOG_*_RENDER call here was a silent no-op — the same blindness that let
  // a total farbling failure go unreported for the life of the feature. The sink forwards
  // to Chromium's logging (settings.log_file), which works from a sandboxed child where
  // opening the log file directly does not.
  hodos::InstallChildProcessLogSink();

  // Load the CEF framework library at runtime
  CefScopedLibraryLoader library_loader;
  if (!library_loader.LoadInHelper()) {
    return 1;
  }

  // Create main args
  CefMainArgs main_args(argc, argv);

  // ⛔ Helpers do NOT open the history database. history.* is serviced by the browser
  // process over IPC (simple_render_process_handler.cpp, HistoryV8Handler); a helper
  // rendering an arbitrary page must not hold a read/write handle on the profile's
  // history DB. This block survived here after Windows removed its equivalent, and it
  // also crashed every helper at exit: the HistoryManager static was constructed before
  // Logger's mutex, so it was destroyed after it, and ~HistoryManager's "database closed"
  // log line locked a destroyed mutex — "mutex lock failed: Invalid argument", SIGABRT in
  // HistoryManager::~HistoryManager (8 crash reports 2026-09-19..23, installed beta.2 too).

  // Use minimal app with only render process handler (for V8 injections)
  CefRefPtr<HelperApp> app(new HelperApp);

  // Execute the helper process
  return CefExecuteProcess(main_args, app.get(), nullptr);
}
