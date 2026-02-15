#!/bin/bash
# Kill all HodosBrowserShell processes
# This script terminates all running instances of the browser to prevent file locking during rebuilds

echo "🔍 Checking for running HodosBrowserShell processes..."

# Check if any processes are running
if tasklist | grep -q -i HodosBrowserShell; then
    echo "⚠️  Found HodosBrowserShell processes. Terminating..."
    powershell.exe -Command "Get-Process HodosBrowserShell -ErrorAction SilentlyContinue | Stop-Process -Force"
    sleep 1

    # Verify they were killed
    if tasklist | grep -q -i HodosBrowserShell; then
        echo "❌ Some processes may still be running. Try manually:"
        echo "   powershell.exe -Command \"Get-Process HodosBrowserShell | Stop-Process -Force\""
    else
        echo "✅ All HodosBrowserShell processes terminated successfully"
    fi
else
    echo "✓ No HodosBrowserShell processes running"
fi
