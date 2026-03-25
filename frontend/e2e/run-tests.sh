#!/bin/bash
# Run Hodos Browser Playwright tests
# Prerequisites: frontend dev server running on :5137
cd "$(dirname "$0")/.."
npx playwright test --reporter=list "$@"
