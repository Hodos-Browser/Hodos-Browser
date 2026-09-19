#!/usr/bin/env python3
"""Stand up the two-window subject: a second tab in window A, torn off to window B."""
import sys, time
import p35macprobe as P

pid = P.preflight()
hs = P.headers()
if len(hs) >= 2:
    print("two windows already present")
    sys.exit(0)

c = P.Conn(hs[0]["webSocketDebuggerUrl"])
c.send_ipc("tab_create", "http://127.0.0.1:5137/newtab")
time.sleep(3)

# The new tab's id is not exposed to JS; take the highest id CEF logged.
# Tear off by id 2 on a fresh profile — asserted by the window count below.
c.send_ipc("tab_tearoff", 2, 150, 20)   # -> B.x = 50, fully on screen
time.sleep(5)
c.close()

wins = P.headers()
print(f"windows now: {len(wins)}")
for r in P.windows(pid):
    print("   ", r)
sys.exit(0 if len(wins) >= 2 else 1)
